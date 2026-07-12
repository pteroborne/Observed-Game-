//! **rapier_determinism_lab** — an isolated physics spike answering one question:
//! *can rapier3d step convex and smooth (curved) geometry in fixed-dt lockstep and
//! stay bit-for-bit reproducible?* The project deliberately avoids adding a physics
//! engine casually (see `docs/bevy_assets_research.md`); this lab is the "spike only
//! if the custom controller proves insufficient" escape hatch, kept self-contained.
//!
//! The verdict lives in [`physics`] (pure rapier3d, unit-tested). Bevy here is
//! presentation only: it renders **world A** and, every fixed tick, steps an
//! identical **world B** headlessly and confirms their state hashes still match —
//! a live lockstep-divergence monitor drawn on the HUD.
//!
//! Keys: R reset. `OBSERVED2_CAPTURE=<dir>` settles the scene and screenshots it.

pub mod physics;

use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};
use physics::{BodyKind, PhysicsWorld};

/// Steps confirmed bit-identical offline before the live monitor starts — matches
/// the unit test's horizon so the on-screen verdict and CI agree.
const OFFLINE_REPLAY_STEPS: u32 = 600;

#[derive(Component)]
pub struct ProbeCam;

#[derive(Component)]
pub struct ProbeUiRoot;

#[derive(Component)]
pub struct VerdictText;

/// One rendered dynamic body; the index maps back into the world snapshot.
#[derive(Component)]
pub struct ProbeBody(pub usize);

/// Holds both worlds. `world_a` is what you see; `world_b` is the headless shadow
/// stepped in lockstep beside it to catch any divergence live.
#[derive(Resource)]
pub struct RapierProbe {
    world_a: PhysicsWorld,
    world_b: PhysicsWorld,
    steps: u64,
    /// First tick at which the two worlds' hashes differed, if ever.
    diverged_at: Option<u64>,
    last_hash: u64,
    /// Result of the offline two-world bitwise replay (the unit-test check).
    offline_bit_identical: bool,
    reset_requested: bool,
    reset_count: u32,
}

impl Default for RapierProbe {
    fn default() -> Self {
        let world_a = PhysicsWorld::new_scene();
        let world_b = PhysicsWorld::new_scene();
        let last_hash = world_a.state_hash();
        Self {
            world_a,
            world_b,
            steps: 0,
            diverged_at: None,
            last_hash,
            offline_bit_identical: PhysicsWorld::replay_is_bit_identical(OFFLINE_REPLAY_STEPS),
            reset_requested: false,
            reset_count: 0,
        }
    }
}

pub struct RapierDeterminismLabPlugin;

impl Plugin for RapierDeterminismLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(60.0))
            .init_resource::<RapierProbe>()
            .add_systems(Startup, setup)
            .add_systems(FixedUpdate, step_physics)
            .add_systems(
                Update,
                (handle_input, perform_reset, sync_bodies, update_verdict).chain(),
            );
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.02, 0.025, 0.035)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Rapier3D Lockstep Determinism Spike".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(RapierDeterminismLabPlugin);

    if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE") {
        let _ = std::fs::create_dir_all(&dir);
        app.insert_resource(CaptureRun {
            dir,
            frames_since_shot: None,
        })
        .add_systems(Update, capture_progress.after(update_verdict));
    }

    app.run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    probe: Res<RapierProbe>,
) {
    commands.spawn((
        ProbeCam,
        Camera3d::default(),
        Transform::from_xyz(0.0, 3.6, 11.5).looking_at(Vec3::new(0.0, 1.2, 0.0), Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.5, 0.55, 0.65),
        brightness: 80.0,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            illuminance: 6000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Ground (top face at y=0, matching the collider in physics::new_scene).
    let ground_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.13, 0.15),
        perceptual_roughness: 0.95,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(40.0, 2.0, 40.0))),
        MeshMaterial3d(ground_mat),
        Transform::from_xyz(0.0, -1.0, 0.0),
    ));

    // One rendered entity per dynamic body, keyed by snapshot index.
    let palette = [
        Color::srgb(0.15, 0.85, 0.95), // ball — cyan
        Color::srgb(1.0, 0.7, 0.2),    // capsule — amber
        Color::srgb(0.95, 0.3, 0.8),   // convex — magenta
    ];
    for (i, kind) in probe.world_a.kinds().iter().enumerate() {
        let mesh = match *kind {
            BodyKind::Ball { radius } => meshes.add(Sphere::new(radius)),
            BodyKind::Capsule {
                half_height,
                radius,
            } => meshes.add(Capsule3d::new(radius, half_height * 2.0)),
            // The collider is the true convex hull; this box is only illustrative.
            BodyKind::Convex => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        };
        let material = materials.add(StandardMaterial {
            base_color: palette[i % palette.len()],
            emissive: LinearRgba::from(palette[i % palette.len()]) * 0.25,
            perceptual_roughness: 0.5,
            ..default()
        });
        commands.spawn((
            ProbeBody(i),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::default(),
        ));
    }

    commands.spawn((
        ProbeUiRoot,
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.04, 0.07, 0.86)),
        children![(
            VerdictText,
            Text::new("Rapier3D lockstep spike"),
            TextFont {
                font_size: 15.0,
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.95, 1.0)),
        )],
    ));
}

fn step_physics(mut probe: ResMut<RapierProbe>) {
    if probe.reset_requested {
        return;
    }
    probe.world_a.step();
    probe.world_b.step();
    probe.steps += 1;
    let (ha, hb) = (probe.world_a.state_hash(), probe.world_b.state_hash());
    probe.last_hash = ha;
    if probe.diverged_at.is_none() && ha != hb {
        probe.diverged_at = Some(probe.steps);
    }
}

fn handle_input(keyboard: Res<ButtonInput<KeyCode>>, mut probe: ResMut<RapierProbe>) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        probe.reset_requested = true;
    }
}

pub fn perform_reset(mut probe: ResMut<RapierProbe>) {
    if !probe.reset_requested {
        return;
    }
    let reset_count = probe.reset_count + 1;
    *probe = RapierProbe {
        reset_count,
        ..RapierProbe::default()
    };
}

fn sync_bodies(probe: Res<RapierProbe>, mut query: Query<(&ProbeBody, &mut Transform)>) {
    let snapshot = probe.world_a.snapshot();
    for (body, mut transform) in &mut query {
        let Some(state) = snapshot.get(body.0) else {
            continue;
        };
        transform.translation = Vec3::from_array(state.pos);
        transform.rotation =
            Quat::from_xyzw(state.quat[0], state.quat[1], state.quat[2], state.quat[3]);
    }
}

fn update_verdict(probe: Res<RapierProbe>, mut query: Query<&mut Text, With<VerdictText>>) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    let live = match probe.diverged_at {
        None => "MATCH  (worlds A/B bit-identical)".to_string(),
        Some(step) => format!("DIVERGED at step {step}  <-- NOT lockstep-safe"),
    };
    let settled = if probe.world_a.settled() {
        "at rest"
    } else {
        "settling..."
    };
    **text = format!(
        "RAPIER3D LOCKSTEP DETERMINISM SPIKE   [reset {}]\n\
         shapes        ball (smooth) | capsule (smooth) | convex hull\n\
         steps         {}   ({} @ 60 Hz lockstep)\n\
         live monitor  {}\n\
         offline replay {}   ({} steps, two worlds)\n\
         state hash    0x{:016x}\n\
         motion        {}   (max speed {:.4} m/s)\n\
         finite        {}\n\n\
         enhanced-determinism = ON | render mesh for convex is illustrative;\n\
         the collider under test is the real convex hull.\n\
         R reset",
        probe.reset_count,
        probe.steps,
        settled,
        live,
        if probe.offline_bit_identical {
            "bit-identical PASS"
        } else {
            "MISMATCH FAIL"
        },
        OFFLINE_REPLAY_STEPS,
        probe.last_hash,
        settled,
        probe.world_a.max_speed(),
        probe.world_a.all_finite(),
    );
}

/// Scripted evidence: let the scene settle, take one screenshot, exit.
#[derive(Resource)]
struct CaptureRun {
    dir: String,
    /// `None` until the shot is spawned, then counts frames so the async GPU
    /// readback and file write finish before we exit.
    frames_since_shot: Option<u32>,
}

fn capture_progress(
    probe: Res<RapierProbe>,
    mut run: ResMut<CaptureRun>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    match run.frames_since_shot {
        // ~2.5 s of lockstep is enough for the drop to come to rest, then shoot.
        None => {
            if probe.steps < 150 {
                return;
            }
            let path = format!("{}/rapier_determinism.png", run.dir);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
            run.frames_since_shot = Some(0);
        }
        Some(frames) if frames >= 60 => {
            exit.write(AppExit::Success);
        }
        Some(frames) => run.frames_since_shot = Some(frames + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, input::InputPlugin};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), InputPlugin))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .add_plugins(RapierDeterminismLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_camera_ui_and_one_entity_per_body() {
        let mut app = test_app();
        let bodies = app.world().resource::<RapierProbe>().world_a.body_count();
        assert_eq!(count::<ProbeCam>(&mut app), 1);
        assert_eq!(count::<ProbeUiRoot>(&mut app), 1);
        assert_eq!(count::<ProbeBody>(&mut app), bodies);
    }

    #[test]
    fn reset_rebuilds_scene_without_leaking_entities() {
        let mut app = test_app();
        let body_entities = count::<ProbeBody>(&mut app);
        for expected in 1..=5 {
            {
                let mut probe = app.world_mut().resource_mut::<RapierProbe>();
                probe.reset_requested = true;
            }
            app.world_mut()
                .run_system_cached(perform_reset)
                .expect("reset system runs");
            let probe = app.world().resource::<RapierProbe>();
            assert_eq!(probe.reset_count, expected);
            assert_eq!(probe.steps, 0);
            assert!(probe.diverged_at.is_none());
            assert_eq!(count::<ProbeBody>(&mut app), body_entities);
            assert_eq!(count::<ProbeCam>(&mut app), 1);
        }
    }
}
