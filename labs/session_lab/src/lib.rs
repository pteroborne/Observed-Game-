mod lab;

// The session/matchmaking model was promoted into `crates/observed_progression`
// (refactor R9). Re-export it under the familiar `model` path; this lab is the projection.
pub mod model {
    pub use observed_progression::session::*;
}

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::SessionRuntime;
pub use model::{SessionLabWorld, SessionPhase};

pub struct SessionLabPlugin;

impl Plugin for SessionLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SessionLabWorld::authored())
            .init_resource::<SessionRuntime>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::perform_reset,
                    lab::simulate,
                    lab::present_accounts,
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
            scale: 1.15,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 20.0, 1000.0),
        Name::new("Session Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.008, 0.016, 0.028)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Matchmaking / Session Formation Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SessionLabPlugin);

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
    mut request: ResMut<CaptureRequest>,
    mut runtime: ResMut<SessionRuntime>,
    mut world: ResMut<SessionLabWorld>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        runtime.auto_run = false;
        for _ in 0..9 {
            world.advance_demo();
        }
        let session = world.session.as_ref().expect("captured session");
        assert!(matches!(session.phase, SessionPhase::InMatch { frame: 8 }));
        assert_eq!(session.host_migrations, 1);
        assert_eq!(session.reconnects, 1);
        assert!(session.launch.as_ref().is_some_and(|launch| launch.valid()));
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.7 {
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
    use crate::lab::{AccountDot, SessionUiRoot};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin, input::InputSystems};

    #[derive(Resource, Default)]
    struct InjectedKey {
        next: Option<KeyCode>,
        held: Option<KeyCode>,
    }

    fn inject_key(mut keyboard: ResMut<ButtonInput<KeyCode>>, mut injected: ResMut<InjectedKey>) {
        if let Some(previous) = injected.held.take() {
            keyboard.release(previous);
        }
        if let Some(next) = injected.next.take() {
            keyboard.press(next);
            injected.held = Some(next);
        }
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .init_resource::<InjectedKey>()
        .add_plugins(SessionLabPlugin)
        .add_systems(
            Update,
            inject_key
                .after(InputSystems)
                .before(crate::lab::handle_input),
        );
        app.update();
        app.world_mut().resource_mut::<SessionRuntime>().auto_run = false;
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    fn tap(app: &mut App, key: KeyCode) {
        app.world_mut().resource_mut::<InjectedKey>().next = Some(key);
        app.update();
        app.update();
    }

    #[test]
    fn boots_with_six_account_dots_and_one_ui() {
        let mut app = test_app();
        assert_eq!(count::<AccountDot>(&mut app), 6);
        assert_eq!(count::<SessionUiRoot>(&mut app), 1);
        assert_eq!(
            app.world()
                .resource::<SessionLabWorld>()
                .matchmaker
                .queue
                .len(),
            6
        );
    }

    #[test]
    fn schedule_advances_the_matchmaking_and_lobby_lifecycle() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<SessionRuntime>()
            .step_requested = true;
        app.update();
        assert!(app.world().resource::<SessionLabWorld>().session.is_some());
        assert_eq!(
            app.world()
                .resource::<SessionLabWorld>()
                .matchmaker
                .queue
                .len(),
            2
        );

        for _ in 0..4 {
            app.world_mut()
                .resource_mut::<SessionRuntime>()
                .step_requested = true;
            app.update();
        }
        let session = app
            .world()
            .resource::<SessionLabWorld>()
            .session
            .as_ref()
            .unwrap();
        assert!(matches!(session.phase, SessionPhase::InMatch { frame: 0 }));
        assert!(session.launch.as_ref().unwrap().valid());
    }

    #[test]
    fn repeated_reset_restores_queue_without_entity_leaks() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            for _ in 0..8 {
                app.world_mut()
                    .resource_mut::<SessionRuntime>()
                    .step_requested = true;
                app.update();
            }
            app.world_mut()
                .resource_mut::<SessionRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<AccountDot>(&mut app), 6);
            assert_eq!(count::<SessionUiRoot>(&mut app), 1);
            let world = app.world().resource::<SessionLabWorld>();
            assert_eq!(world.reset_count, reset_count);
            assert!(world.session.is_none());
            assert_eq!(world.matchmaker.queue.len(), 6);
            assert_eq!(world.demo_step, 0);
        }
    }

    #[test]
    fn keyboard_controls_form_launch_disconnect_reconnect_finish_and_reset() {
        let mut app = test_app();
        for _ in 0..5 {
            tap(&mut app, KeyCode::Space);
        }
        {
            let world = app.world().resource::<SessionLabWorld>();
            let session = world.session.as_ref().unwrap();
            assert!(matches!(session.phase, SessionPhase::InMatch { frame: 0 }));
            assert!(session.launch.as_ref().unwrap().valid());
        }

        tap(&mut app, KeyCode::KeyD);
        {
            let world = app.world().resource::<SessionLabWorld>();
            let session = world.session.as_ref().unwrap();
            assert!(matches!(session.phase, SessionPhase::ReconnectGrace { .. }));
            assert_eq!(session.host_migrations, 1);
        }
        tap(&mut app, KeyCode::KeyC);
        {
            let world = app.world().resource::<SessionLabWorld>();
            let session = world.session.as_ref().unwrap();
            assert!(matches!(session.phase, SessionPhase::InMatch { frame: 0 }));
            assert_eq!(session.reconnects, 1);
        }

        tap(&mut app, KeyCode::KeyF);
        assert!(matches!(
            app.world()
                .resource::<SessionLabWorld>()
                .session
                .as_ref()
                .unwrap()
                .phase,
            SessionPhase::PostMatch { .. }
        ));

        tap(&mut app, KeyCode::KeyR);
        let world = app.world().resource::<SessionLabWorld>();
        assert!(world.session.is_none());
        assert_eq!(world.matchmaker.queue.len(), 6);
    }
}
