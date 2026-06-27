mod lab;

// The lockstep protocol + model were promoted into `crates/observed_net` (refactor R9).
// Re-export them under the familiar `model`/`protocol` paths; this lab is the projection.
pub mod model {
    pub use observed_net::network::*;
}
pub mod protocol {
    pub use observed_net::protocol::*;
}

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::NetworkRuntime;
pub use model::{LockstepDemo, NetworkProfile};

pub struct NetworkLabPlugin;

impl Plugin for NetworkLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LockstepDemo::authored(NetworkProfile::Hostile))
            .init_resource::<NetworkRuntime>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::perform_reset,
                    lab::simulate,
                    lab::present_bodies,
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
            scale: 1.05,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 30.0, 1000.0),
        Name::new("Network Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.008, 0.016, 0.028)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Deterministic Lockstep Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(NetworkLabPlugin);

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
    mut runtime: ResMut<NetworkRuntime>,
    mut demo: ResMut<LockstepDemo>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        demo.run_until_synchronized(20_000);
        assert!(
            lab::capture_ready(&demo),
            "capture requires a hostile synchronized run"
        );
        runtime.running = false;
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
    use crate::lab::{NetworkUiRoot, PeerBodyDot};
    use crate::model::TARGET_FRAMES;
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin, input::InputSystems};
    use protocol::PEER_COUNT;

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
        .add_plugins(NetworkLabPlugin)
        .add_systems(
            Update,
            inject_key
                .after(InputSystems)
                .before(crate::lab::handle_input),
        );
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_two_peer_projections_and_one_ui() {
        let mut app = test_app();
        assert_eq!(count::<PeerBodyDot>(&mut app), PEER_COUNT * PEER_COUNT);
        assert_eq!(count::<NetworkUiRoot>(&mut app), 1);
        assert_eq!(app.world().resource::<LockstepDemo>().transport_ticks, 6);
    }

    #[test]
    fn bevy_schedule_reaches_a_synchronized_replayable_session() {
        let mut app = test_app();
        for _ in 0..2_000 {
            app.update();
            if app.world().resource::<LockstepDemo>().synchronized() {
                break;
            }
        }
        let demo = app.world().resource::<LockstepDemo>();
        assert!(demo.synchronized());
        assert_eq!(demo.peers[0].next_frame, TARGET_FRAMES);
        assert!(demo.replay_matches());
    }

    #[test]
    fn repeated_reset_restores_session_without_entity_leaks() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            for _ in 0..5 {
                app.update();
            }
            app.world_mut()
                .resource_mut::<NetworkRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<PeerBodyDot>(&mut app), PEER_COUNT * PEER_COUNT);
            assert_eq!(count::<NetworkUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<NetworkRuntime>().reset_count,
                reset_count
            );
            let demo = app.world().resource::<LockstepDemo>();
            assert_eq!(demo.transport_ticks, 0);
            assert!(demo.peers.iter().all(|peer| peer.next_frame == 0));
            assert!(demo.peers.iter().all(|peer| peer.tape.frames.is_empty()));
        }
    }

    fn tap(app: &mut App, key: KeyCode) {
        app.world_mut().resource_mut::<InjectedKey>().next = Some(key);
        app.update();
        app.update();
    }

    #[test]
    fn keyboard_controls_pause_step_switch_profile_and_detect_desync() {
        let mut app = test_app();

        tap(&mut app, KeyCode::Space);
        assert!(!app.world().resource::<NetworkRuntime>().running);
        let paused_tick = app.world().resource::<LockstepDemo>().transport_ticks;

        tap(&mut app, KeyCode::KeyN);
        assert_eq!(
            app.world().resource::<LockstepDemo>().transport_ticks,
            paused_tick + 1
        );
        assert!(!app.world().resource::<NetworkRuntime>().running);

        tap(&mut app, KeyCode::KeyL);
        assert_eq!(
            app.world().resource::<NetworkRuntime>().profile,
            NetworkProfile::Clean
        );
        assert_eq!(app.world().resource::<LockstepDemo>().transport_ticks, 0);

        app.world_mut()
            .resource_mut::<LockstepDemo>()
            .run_until_synchronized(2_000);
        assert!(app.world().resource::<LockstepDemo>().synchronized());
        tap(&mut app, KeyCode::KeyD);
        assert!(app.world().resource::<LockstepDemo>().has_desync());
    }
}
