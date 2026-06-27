// The model was promoted into `crates/observed_match` (refactor R9); re-export
// it under the familiar `hybrid` path. This lab is the debug projection.
pub mod hybrid {
    pub use observed_match::hybrid::*;
}
mod lab;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use hybrid::{HybridMatch, HybridTape};
pub use lab::{HybridRuntime, Mode};

const SEED: u64 = 1;

pub struct FpsHybridMatchPlugin;

impl Plugin for FpsHybridMatchPlugin {
    fn build(&self, app: &mut App) {
        let live = HybridMatch::authored(SEED);
        let tape = HybridTape::record_demo(SEED);
        app.insert_resource(lab::ActiveView(live.clone()))
            .insert_resource(lab::LiveMatch(live))
            .insert_resource(lab::DemoTape(tape))
            .init_resource::<HybridRuntime>()
            .init_resource::<lab::InputIntent>()
            .init_resource::<lab::ResolutionTimer>()
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .add_systems(Startup, (lab::setup_lab, lab::grab_cursor))
            .add_systems(FixedUpdate, lab::simulate_live)
            .add_systems(
                Update,
                (
                    lab::map_input.after(InputSystems),
                    lab::toggle_grab,
                    lab::handle_controls,
                    lab::advance_replay,
                    lab::resolve_after_local_finish,
                    lab::perform_reset,
                    lab::update_view,
                    lab::present_camera,
                    lab::draw_world,
                    lab::update_debug_text,
                )
                    .chain(),
            );
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.01, 0.014, 0.022)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — First-Person Hybrid Match".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FpsHybridMatchPlugin);

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

fn capture_progress(
    time: Res<Time>,
    tape: Res<lab::DemoTape>,
    mut request: ResMut<CaptureRequest>,
    mut runtime: ResMut<HybridRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        lab::configure_capture(&mut runtime, &tape.0);
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.7 {
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
    use lab::{ActiveView, DemoTape, HybridCam, HybridUiRoot, LiveMatch};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(FpsHybridMatchPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_camera_ui_and_a_recorded_match() {
        let mut app = test_app();
        assert_eq!(count::<HybridCam>(&mut app), 1);
        assert_eq!(count::<HybridUiRoot>(&mut app), 1);
        assert!(!app.world().resource::<DemoTape>().0.is_empty());
        assert!(app.world().resource::<LiveMatch>().0.navigable());
        assert!(app.world().resource::<HybridRuntime>().replay_verified);
    }

    #[test]
    fn the_replay_view_reproduces_the_recorded_state_exactly() {
        let mut app = test_app();
        let len = app.world().resource::<DemoTape>().0.len();
        for round in [0usize, 1, len / 2, len] {
            {
                let mut runtime = app.world_mut().resource_mut::<HybridRuntime>();
                runtime.mode = Mode::Replay;
                runtime.replay_playing = false;
                runtime.replay_cursor = round as f32;
            }
            app.update();
            let expected = app
                .world()
                .resource::<DemoTape>()
                .0
                .replay_to(round)
                .snapshot();
            assert_eq!(app.world().resource::<ActiveView>().0.snapshot(), expected);
        }
    }

    #[test]
    fn the_live_view_mirrors_the_live_match() {
        let mut app = test_app();
        {
            let mut live = app.world_mut().resource_mut::<LiveMatch>();
            live.0.apply_action(hybrid::LocalAction::Advance);
            live.0.apply_action(hybrid::LocalAction::Advance);
        }
        // Stay in Live mode (default); update_view mirrors the live match.
        app.update();
        let live_round = app.world().resource::<LiveMatch>().0.competitive.round;
        let view_round = app.world().resource::<ActiveView>().0.competitive.round;
        assert_eq!(view_round, live_round);
        assert!(live_round >= 2);
        assert_eq!(count::<HybridCam>(&mut app), 1);
    }

    #[test]
    fn repeated_reset_restores_the_live_match_without_entity_leaks() {
        let mut app = test_app();
        for expected in 1..=10 {
            app.world_mut()
                .resource_mut::<LiveMatch>()
                .0
                .apply_action(hybrid::LocalAction::Advance);
            app.world_mut()
                .resource_mut::<HybridRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<HybridCam>(&mut app), 1);
            assert_eq!(count::<HybridUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<HybridRuntime>().reset_count,
                expected
            );
            let live = &app.world().resource::<LiveMatch>().0;
            assert_eq!(live.competitive.round, 0);
            assert!(live.navigable());
            assert!(live.player_on_floor());
        }
    }
}
