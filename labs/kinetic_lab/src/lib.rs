//! Kinetic tool lab — the first-person half of Architect Ascent's step B proof.
//!
//! One question: **is a shove that commits a minor Guardian to void
//! deterministic, readable, and fair?**
//!
//! Run it with `cargo dev-run -p kinetic_lab`. The board is a single floor with
//! a void rim to shove things over, an unrailed ledge run that carries momentum
//! past its own length, one retracting tile that kills on a delay, a wall that
//! stops a shove dead, a recharge station, and the generator that powers it.
//!
//! What the lab is *not*: it is not the economy proof. Disturbance waves and the
//! Architect's hand live at cell level in `architect_lab`. This lab holds one
//! Observer, a fixed pair of minors and one major, and asks only whether the
//! tool itself reads honestly.
//!
//! ## Two views, one model
//!
//! - `cargo dev-run -p kinetic_lab --bin kinetic_lab` — the top-down schematic.
//!   Discrete steps, the whole board visible at once. Best for reading the rules.
//! - `cargo dev-run -p kinetic_lab --bin kinetic_fps` — first person. Continuous
//!   movement and mouse aim over the identical [`model::KineticWorld`].
//!
//! Neither view owns a rule, and the second one was the point of building the
//! first: the schematic proved the rules are legible, and the first-person view
//! asks whether a shove is *satisfying*, which a top-down board cannot answer.

pub mod embodied;
mod fps;
mod lab;
pub mod model;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};
use observed_hex::{coords::HexCoord, faces::HexFace};
use player_input::PlayerIntent;

pub use embodied::{Embodiment, ToolRequest};
pub use fps::FpsRuntime;
pub use lab::KineticRuntime;
pub use model::{
    CellKind, KineticEvent, KineticIntent, KineticWorld, MajorGuardian, MinorGuardian,
    MinorGuardianId, Observer, ShoveFate, ShoveResolution, Station, StationId, ToolRefusal,
};

/// The first-person view. Simulation runs in `FixedUpdate` at exactly the
/// model's tick rate; everything in `Update` is presentation.
pub struct KineticFpsPlugin;

impl Plugin for KineticFpsPlugin {
    fn build(&self, app: &mut App) {
        let world = KineticWorld::authored();
        let embodiment = Embodiment::new(&world);
        app.insert_resource(world)
            .insert_resource(embodiment)
            .init_resource::<FpsRuntime>()
            .insert_resource(Time::<Fixed>::from_hz(f64::from(model::TICKS_PER_SECOND)))
            .add_systems(Startup, (fps::setup_scene, fps::grab_cursor).chain())
            .add_systems(
                FixedUpdate,
                (fps::simulate, fps::refresh_arena_after_retraction).chain(),
            )
            .add_systems(
                Update,
                (
                    fps::gather_requests.after(InputSystems),
                    fps::toggle_grab,
                    fps::perform_reset,
                    fps::sync_camera,
                    fps::present_plates,
                    fps::present_guardians,
                    fps::present_stations,
                    fps::present_preview,
                    fps::draw_lane,
                    fps::update_hud,
                )
                    .chain(),
            );
    }
}

pub fn run_fps() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.004, 0.006, 0.010)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Kinetic Tool (first person)".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(KineticFpsPlugin);

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(FpsCaptureRequest { path, phase: 0 })
            .add_systems(Update, fps_capture_progress);
    }

    // A frame sequence for video. Stills cannot show the two things this lab is
    // actually about: the clockwork snap, and a Guardian going over an edge.
    if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE_SEQUENCE") {
        std::fs::create_dir_all(&dir).expect("sequence capture directory");
        app.insert_resource(SequenceCapture {
            dir,
            tick: 0,
            captured_tick: None,
            frame: 0,
        })
        // Saving a PNG per frame is far slower than the simulation, so leaving
        // this on the fixed timestep lets catch-up run several ticks between
        // renders and the recording skips state. Park `FixedUpdate` and advance
        // exactly one tick per rendered frame instead: the video is then a
        // faithful 60 Hz record rather than a stutter of sampled moments.
        .insert_resource(Time::<Fixed>::from_seconds(3600.0))
        .add_systems(
            Update,
            (
                drive_demo,
                fps::simulate,
                fps::refresh_arena_after_retraction,
            )
                .chain()
                .before(fps::gather_requests),
        )
        .add_systems(Update, capture_sequence_frame.after(fps::update_hud))
        .add_systems(Startup, stage_demo.after(fps::setup_scene));
    }

    app.run();
}

/// One frame per simulated tick, so the sequence encodes at 60 fps as a
/// true-rate record of the run.
const SEQUENCE_STRIDE: u32 = 1;
/// Total ticks the scripted demo runs.
const DEMO_TICKS: u32 = 420;

#[derive(Resource)]
struct SequenceCapture {
    dir: String,
    /// Advanced by the fixed-step demo, read by the per-frame capture, so the
    /// two schedules cannot drift apart and drop or duplicate a frame.
    tick: u32,
    captured_tick: Option<u32>,
    frame: u32,
}

/// The demo tape.
///
/// Deliberately expressed as intents rather than as poses: the recording drives
/// the lab through the same boundary a player's hands do, so what it shows is a
/// real run of the rules and not an animation of them. Because the model is
/// deterministic, this tape reproduces frame for frame.
/// Place the actors for the recording.
///
/// This stages a *scenario* — starting positions and a heading — exactly as the
/// still capture does. Everything after this point is driven by intents through
/// the ordinary command path, so what the video shows is the rules running, not
/// an animation of them.
fn stage_demo(mut world: ResMut<KineticWorld>, mut embodiment: ResMut<Embodiment>) {
    let cell = |q, r| HexCoord { q, r, level: 0 };
    let stand = embodied::plate_center(cell(3, 3));
    embodiment.body.position = Vec3::new(
        stand.x,
        embodied::FLOOR_TOP + embodiment.config.half_height,
        stand.z,
    );
    embodiment.body.velocity = Vec3::ZERO;
    // Already looking down the ledge run, so the recording opens on the shot
    // that matters instead of on three quarters of a second of turning.
    embodiment.body.yaw = std::f32::consts::FRAC_PI_2;
    embodiment.facing = HexFace::East;

    world.observers[0].cell = cell(3, 3);
    world.observers[0].facing = HexFace::East;
    world.minors[0].cell = cell(4, 3);
    // The second Guardian starts far enough south to arrive during the run
    // rather than on top of the Observer at the first step.
    world.minors[1].cell = cell(2, 5);
}

fn demo_intent(tick: u32) -> (PlayerIntent, ToolRequest) {
    let mut intent = PlayerIntent::default();
    let mut request = ToolRequest::None;

    match tick {
        // Hold. The preview beam is already up, stating the outcome before
        // anything has been committed.
        0..=89 => {}
        // Commit.
        90 => request = ToolRequest::Push,
        // Follow it down as it goes over the rim.
        91..=150 => intent.look = Vec2::new(0.0, 0.30),
        // Watch the empty ledge run it crossed.
        151..=240 => {}
        // Level back up, then walk, so the plates read in motion rather than as
        // a frozen diagram.
        241..=270 => intent.look = Vec2::new(0.0, -0.30),
        271..=DEMO_TICKS => intent.movement = Vec2::new(0.0, 1.0),
        _ => {}
    }

    (intent, request)
}

fn drive_demo(mut capture: ResMut<SequenceCapture>, mut runtime: ResMut<FpsRuntime>) {
    if capture.tick <= DEMO_TICKS {
        runtime.scripted = Some(demo_intent(capture.tick));
    }
    capture.tick += 1;
}

fn capture_sequence_frame(
    mut capture: ResMut<SequenceCapture>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let tick = capture.tick;
    if tick > DEMO_TICKS {
        exit.write(AppExit::Success);
        return;
    }
    // One frame per simulated tick at most, so a frame is never written twice
    // for the same state or skipped when two ticks land in one render frame.
    if capture.captured_tick == Some(tick) || !tick.is_multiple_of(SEQUENCE_STRIDE) {
        return;
    }
    capture.captured_tick = Some(tick);

    let path = format!("{}/frame_{:04}.png", capture.dir, capture.frame);
    capture.frame += 1;
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

#[derive(Resource)]
struct FpsCaptureRequest {
    path: String,
    phase: u8,
}

/// Stand the body where the ledge run and the void rim are both in frame with a
/// minor Guardian in the lane, so the capture shows a live preview.
fn fps_capture_progress(
    time: Res<Time>,
    mut request: ResMut<FpsCaptureRequest>,
    mut runtime: ResMut<FpsRuntime>,
    mut world: ResMut<KineticWorld>,
    mut embodiment: ResMut<Embodiment>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Freeze the board: a minor Guardian one plate away would otherwise
        // walk onto the Observer and jail them before the shutter opened.
        runtime.paused = true;
        let stand = embodied::plate_center(HexCoord {
            q: 3,
            r: 3,
            level: 0,
        });
        embodiment.body.position = Vec3::new(
            stand.x,
            embodied::FLOOR_TOP + embodiment.config.half_height,
            stand.z,
        );
        embodiment.body.velocity = Vec3::ZERO;
        // Look east, down the ledge run.
        embodiment.body.yaw = std::f32::consts::FRAC_PI_2;
        embodiment.body.pitch = -0.12;
        embodiment.facing = HexFace::East;
        world.observers[0].cell = HexCoord {
            q: 3,
            r: 3,
            level: 0,
        };
        world.observers[0].facing = HexFace::East;
        world.minors[0].cell = HexCoord {
            q: 4,
            r: 3,
            level: 0,
        };
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.9 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 1.8 {
        exit.write(AppExit::Success);
    }
}

pub struct KineticLabPlugin;

impl Plugin for KineticLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(KineticWorld::authored())
            .init_resource::<KineticRuntime>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::perform_reset,
                    lab::simulate,
                    lab::present,
                    lab::draw_debug,
                    lab::update_debug_text,
                )
                    .chain(),
            );
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: 1.0,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Name::new("Kinetic Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.008, 0.012, 0.018)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Kinetic Tool Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(KineticLabPlugin);

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, phase: 0 })
            .add_systems(Update, capture_progress);
    }

    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
}

/// Drive the board to the moment worth photographing: an Observer lined up on a
/// minor Guardian with the ledge run and the void rim behind it, so the capture
/// shows a live preview rather than an idle board.
fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut runtime: ResMut<KineticRuntime>,
    mut world: ResMut<KineticWorld>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        runtime.paused = true;
        // Stand west of the ledge run, facing along it, with a minor Guardian
        // in the lane: the preview then shows a live lethal outcome.
        world.observers[0].cell = HexCoord {
            q: 3,
            r: 3,
            level: 0,
        };
        world.observers[0].facing = HexFace::East;
        world.minors[0].cell = HexCoord {
            q: 4,
            r: 3,
            level: 0,
        };
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.6 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 1.5 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use lab::{CellTile, MajorBody, MinorBody, ObserverBody};
    use observed_core::PlayerId;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            bevy::mesh::MeshPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(KineticLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    /// Run exactly `ticks` simulation ticks through the real schedule.
    fn tick(app: &mut App, ticks: u32) {
        app.world_mut().resource_mut::<KineticRuntime>().paused = true;
        for _ in 0..ticks {
            app.world_mut()
                .resource_mut::<KineticRuntime>()
                .step_requested = true;
            app.update();
        }
    }

    #[test]
    fn boots_with_a_full_board_and_one_ui_panel() {
        let mut app = test_app();
        let cells = app.world().resource::<KineticWorld>().grid.cell_count();
        assert_eq!(count::<CellTile>(&mut app), cells);
        assert_eq!(count::<MinorBody>(&mut app), 2);
        assert_eq!(count::<MajorBody>(&mut app), 1);
        assert_eq!(count::<ObserverBody>(&mut app), 1);
        assert_eq!(count::<lab::KineticUiRoot>(&mut app), 1);
        assert_eq!(app.world().resource::<KineticWorld>().tick, 0);
    }

    #[test]
    fn a_staged_intent_resolves_through_the_bevy_schedule_exactly_once() {
        let mut app = test_app();
        {
            let mut world = app.world_mut().resource_mut::<KineticWorld>();
            world.observers[0].cell = HexCoord {
                q: 3,
                r: 4,
                level: 0,
            };
            world.observers[0].facing = HexFace::East;
            world.minors[0].cell = HexCoord {
                q: 4,
                r: 4,
                level: 0,
            };
        }
        app.world_mut().resource_mut::<KineticRuntime>().pending = KineticIntent::Push;

        let before = app.world().resource::<KineticWorld>().observers[0].charge;
        tick(&mut app, 4);
        let after = app.world().resource::<KineticWorld>().observers[0].charge;

        assert_eq!(
            before - after,
            model::PUSH_COST,
            "one keypress spends one shove, not one per tick"
        );
        assert_eq!(
            app.world().resource::<KineticRuntime>().pending,
            KineticIntent::Idle
        );
    }

    #[test]
    fn repeated_reset_restores_state_without_entity_leaks() {
        let mut app = test_app();
        let cells = app.world().resource::<KineticWorld>().grid.cell_count();

        for reset_count in 1..=10 {
            app.world_mut().resource_mut::<KineticRuntime>().pending = KineticIntent::Push;
            tick(&mut app, 30);
            app.world_mut()
                .resource_mut::<KineticRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<CellTile>(&mut app), cells);
            assert_eq!(count::<MinorBody>(&mut app), 2);
            assert_eq!(count::<MajorBody>(&mut app), 1);
            assert_eq!(count::<ObserverBody>(&mut app), 1);
            assert_eq!(count::<lab::KineticUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<KineticRuntime>().reset_count,
                reset_count
            );
            assert_eq!(
                app.world().resource::<KineticWorld>(),
                &KineticWorld::authored(),
                "reset restores the authored board exactly"
            );
        }
    }

    #[test]
    fn a_destroyed_guardian_is_hidden_rather_than_despawned() {
        let mut app = test_app();
        {
            let mut world = app.world_mut().resource_mut::<KineticWorld>();
            world.observers[0].cell = HexCoord {
                q: 4,
                r: 2,
                level: 0,
            };
            world.observers[0].facing = HexFace::NorthWest;
            world.minors[0].cell = HexCoord {
                q: 4,
                r: 1,
                level: 0,
            };
        }
        app.world_mut().resource_mut::<KineticRuntime>().pending = KineticIntent::Push;
        tick(&mut app, 2);

        assert_eq!(app.world().resource::<KineticWorld>().living_minors(), 1);
        // The entity count is stable across a kill, so reset has nothing to
        // rebuild and nothing can leak.
        assert_eq!(count::<MinorBody>(&mut app), 2);

        let world = app.world_mut();
        let mut query = world.query::<(&MinorBody, &Visibility)>();
        let hidden = query
            .iter(world)
            .filter(|(_, visibility)| matches!(visibility, Visibility::Hidden))
            .count();
        assert_eq!(hidden, 1);
    }

    #[test]
    fn the_observer_id_drives_the_simulation_rather_than_the_entity() {
        let mut app = test_app();
        let id = app.world().resource::<KineticWorld>().observers[0].id;
        assert_eq!(id, PlayerId(0));
        tick(&mut app, 5);
        assert_eq!(app.world().resource::<KineticWorld>().tick, 5);
    }
}
