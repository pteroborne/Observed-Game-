//! **wellshaft_lab** — Arc I Phase 71b prototype. Proves the vertical "silo"
//! hub before it touches the game or sim: six threshold bridges radiate from
//! landings around a central hexagonal pillar, joined by real stair treads whose
//! visible heights are the production AABB controller's collision heights.
//!
//! Keys: WASD + mouse to walk, Shift sprint, Space jump, R reset.
//! `OBSERVED2_CAPTURE=<dir>` stages three diagnostic views and captures them.

pub mod lab;
pub mod shaft;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub struct WellshaftLabPlugin;

impl Plugin for WellshaftLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(60.0))
            .add_systems(Startup, (lab::setup_lab, lab::grab_cursor))
            .add_systems(FixedUpdate, lab::simulate)
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::perform_reset,
                    lab::present_camera,
                    lab::update_debug_text,
                )
                    .chain(),
            );
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.004, 0.005, 0.008)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Hex Wellshaft Prototype".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(WellshaftLabPlugin);

    if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE") {
        let _ = std::fs::create_dir_all(&dir);
        app.insert_resource(CaptureRun {
            dir,
            phase: 0,
            next_at: 0.0,
        })
        .add_systems(Update, capture_progress.after(lab::present_camera));
    }

    app.run();
}

/// Scripted evidence: staged camera vantages of the shaft (top looking down,
/// bottom looking up, and a plan view) so the prototype can be
/// judged from captures without piloting it.
#[derive(Resource)]
pub(crate) struct CaptureRun {
    dir: String,
    phase: u8,
    next_at: f32,
}

fn capture_progress(
    time: Res<Time>,
    mut run: ResMut<CaptureRun>,
    mut runtime: ResMut<lab::ShaftRuntime>,
    mut cam: Query<&mut Transform, With<lab::ShaftCam>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    // Freeze player input during capture; drive the camera directly.
    runtime.intent = player_input::PlayerIntent::default();
    let top = shaft::level_top(shaft::LEVELS - 1);
    let first_upper = shaft::landing_center(1);
    let shots: [(Transform, &str); 3] = [
        (
            Transform::from_xyz(0.0, top + 7.0, shaft::HALF + 5.0)
                .looking_at(Vec3::new(0.0, top * 0.4, 0.0), Vec3::Y),
            "descent",
        ),
        (
            Transform::from_xyz(shaft::BRIDGE_END_RADIUS - 0.8, 1.6, 0.0).looking_at(
                Vec3::new(first_upper.x, shaft::level_top(1) + 1.2, first_upper.y),
                Vec3::Y,
            ),
            "ascent",
        ),
        (
            Transform::from_xyz(0.05, top + 36.0, 0.05)
                .looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::NEG_Z),
            "plan",
        ),
    ];
    let idx = (run.phase / 2) as usize;
    if run.phase.is_multiple_of(2) {
        if idx >= shots.len() {
            if elapsed >= run.next_at {
                exit.write(AppExit::Success);
            }
            return;
        }
        if let Ok(mut t) = cam.single_mut() {
            *t = shots[idx].0;
        }
        run.next_at = elapsed + 0.6;
        run.phase += 1;
    } else if elapsed >= run.next_at {
        let path = format!("{}/wellshaft_{}.png", run.dir, shots[idx].1);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        // Keep the application alive long enough for the render-world readback
        // and file observer to complete, especially after the final shot.
        run.next_at = elapsed + 1.0;
        run.phase += 1;
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
            .insert_resource(ClearColor(Color::BLACK))
            .add_plugins(WellshaftLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_camera_geometry_ui_and_controller_arena() {
        let mut app = test_app();
        assert_eq!(count::<lab::ShaftCam>(&mut app), 1);
        assert_eq!(count::<lab::ShaftUiRoot>(&mut app), 1);
        assert_eq!(count::<lab::ShaftGeometry>(&mut app), shaft::build().len());
        assert_eq!(
            app.world().resource::<lab::ShaftRuntime>().arena.solids,
            shaft::solids(&shaft::build())
        );
    }

    #[test]
    fn repeated_reset_restores_spawn_without_leaking_entities() {
        let mut app = test_app();
        let geometry_count = count::<lab::ShaftGeometry>(&mut app);
        for expected in 1..=8 {
            {
                let mut runtime = app.world_mut().resource_mut::<lab::ShaftRuntime>();
                runtime.body.position = Vec3::ZERO;
                runtime.min_feet = 0.0;
                runtime.reset_requested = true;
            }
            app.world_mut()
                .run_system_cached(lab::perform_reset)
                .expect("reset system runs");
            assert_eq!(count::<lab::ShaftGeometry>(&mut app), geometry_count);
            assert_eq!(count::<lab::ShaftCam>(&mut app), 1);
            assert_eq!(count::<lab::ShaftUiRoot>(&mut app), 1);
            let runtime = app.world().resource::<lab::ShaftRuntime>();
            assert_eq!(runtime.reset_count, expected);
            assert_eq!(
                runtime.body.position.y,
                shaft::spawn_height() + runtime.config.half_height
            );
            assert_eq!(runtime.min_feet, shaft::spawn_height());
        }
    }
}
