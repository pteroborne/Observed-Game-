mod lab;
pub mod model;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::{MatchMode, MatchRuntime};
pub use model::{FirstPersonMatch, MatchTape};

pub struct FpsMatchPlugin;

impl Plugin for FpsMatchPlugin {
    fn build(&self, app: &mut App) {
        let live = FirstPersonMatch::default();
        let demo = MatchTape::record_demo();
        app.insert_resource(lab::ActiveView(live.clone()))
            .insert_resource(lab::LiveMatch(live))
            .insert_resource(lab::LiveTape::default())
            .insert_resource(lab::DemoTape(demo))
            .init_resource::<MatchRuntime>()
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
                    lab::sync_door_panels,
                    lab::present_teams,
                    lab::present_camera,
                    lab::draw_debug,
                    lab::update_debug_text,
                )
                    .chain(),
            );
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.008, 0.012, 0.022)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - First-Person Competitive Match".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FpsMatchPlugin);

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
    demo: Res<lab::DemoTape>,
    mut request: ResMut<CaptureRequest>,
    mut runtime: ResMut<MatchRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        lab::configure_capture(&mut runtime, &demo.0);
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.8 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 1.6 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use competitive_facility::model::TEAM_COUNT;
    use lab::{
        ActiveView, DemoTape, LiveMatch, MatchCamera, MatchModuleRoot, MatchUiRoot, TeamMarker,
    };
    use observation_lab::model::ROOM_COUNT;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(FpsMatchPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_full_3d_match_spectator_and_recorded_replay() {
        let mut app = test_app();
        assert_eq!(count::<MatchCamera>(&mut app), 1);
        assert_eq!(count::<MatchUiRoot>(&mut app), 1);
        assert_eq!(count::<MatchModuleRoot>(&mut app), ROOM_COUNT);
        assert_eq!(count::<TeamMarker>(&mut app), TEAM_COUNT);
        assert!(!app.world().resource::<DemoTape>().0.is_empty());
        assert!(app.world().resource::<MatchRuntime>().replay_verified);
    }

    #[test]
    fn replay_view_equals_the_exact_recorded_first_person_match_state() {
        let mut app = test_app();
        let len = app.world().resource::<DemoTape>().0.len();
        for round in [0usize, 1, len / 2, len] {
            {
                let mut runtime = app.world_mut().resource_mut::<MatchRuntime>();
                runtime.mode = MatchMode::Replay;
                runtime.replay_cursor = round as f32;
                runtime.replay_playing = false;
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
    fn live_first_person_round_records_and_replays_exactly() {
        let mut app = test_app();
        {
            let mut live = app.world_mut().resource_mut::<LiveMatch>();
            assert!(live.0.apply_action(model::LocalAction::Advance));
        }
        let after = app.world().resource::<LiveMatch>().0.snapshot();
        let after_session = app.world().resource::<LiveMatch>().0.clone();
        let before = FirstPersonMatch::default().snapshot();
        {
            let mut tape = app.world_mut().resource_mut::<lab::LiveTape>();
            tape.0
                .push_live(model::LocalAction::Advance, &before, &after_session);
        }
        let tape = &app.world().resource::<lab::LiveTape>().0;
        assert_eq!(tape.replay_to(1).snapshot(), after);
    }

    #[test]
    fn repeated_reset_restores_live_match_without_entity_leaks() {
        let mut app = test_app();
        let meshes = count::<Mesh3d>(&mut app);
        for expected in 1..=10 {
            app.world_mut()
                .resource_mut::<LiveMatch>()
                .0
                .apply_action(model::LocalAction::Advance);
            app.world_mut()
                .resource_mut::<MatchRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<MatchCamera>(&mut app), 1);
            assert_eq!(count::<MatchUiRoot>(&mut app), 1);
            assert_eq!(count::<MatchModuleRoot>(&mut app), ROOM_COUNT);
            assert_eq!(count::<TeamMarker>(&mut app), TEAM_COUNT);
            assert_eq!(count::<Mesh3d>(&mut app), meshes);
            assert_eq!(app.world().resource::<MatchRuntime>().reset_count, expected);
            assert_eq!(app.world().resource::<LiveMatch>().0.competitive.round, 0);
        }
    }
}
