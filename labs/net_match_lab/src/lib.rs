// The networked-match model was promoted into `crates/observed_net` (refactor R9).
// Re-export it under the familiar `netmatch` path; this lab is the projection.
pub mod netmatch {
    pub use observed_net::netmatch::*;
}

mod lab;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::NetRuntime;
pub use netmatch::{LiveNetMatch, NetMatch, NetPeer};

pub struct NetMatchLabPlugin;

impl Plugin for NetMatchLabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetRuntime>()
            .add_systems(Startup, lab::setup_lab)
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::advance,
                    lab::perform_reset,
                    lab::draw_world,
                    lab::update_debug_text,
                )
                    .chain(),
            );
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.015, 0.018, 0.026)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Networked Hybrid Match".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(NetMatchLabPlugin);

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
    mut runtime: ResMut<NetRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Drive the hostile network to convergence so the shot shows the resolved
        // match and a green [PASS].
        runtime.net.run_until_synchronized(50_000);
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
    use lab::{DebugText, NetCam, NetUiRoot};
    use observed_net::network::NetworkProfile;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(NetMatchLabPlugin);
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
        assert_eq!(count::<NetCam>(&mut app), 1);
        assert_eq!(count::<NetUiRoot>(&mut app), 1);
        assert_eq!(count::<DebugText>(&mut app), 1);
        assert!(app.world().resource::<NetRuntime>().net.total > 0);
    }

    #[test]
    fn the_lab_drives_the_match_to_convergence_over_frames() {
        let mut app = test_app();
        for _ in 0..6_000 {
            if app.world().resource::<NetRuntime>().net.synchronized() {
                break;
            }
            app.update();
        }
        let runtime = app.world().resource::<NetRuntime>();
        assert!(runtime.net.synchronized());
        assert!(runtime.net.peers_agree());
        assert!(runtime.net.matches_reference());
    }

    #[test]
    fn repeated_reset_restores_a_fresh_session_without_leaks() {
        let mut app = test_app();
        for expected in 1..=8 {
            // Advance a little, then request a reset.
            for _ in 0..20 {
                app.update();
            }
            app.world_mut().resource_mut::<NetRuntime>().reset_requested = true;
            app.update();

            assert_eq!(count::<NetCam>(&mut app), 1);
            assert_eq!(count::<NetUiRoot>(&mut app), 1);
            let runtime = app.world().resource::<NetRuntime>();
            assert_eq!(runtime.reset_count, expected);
            assert_eq!(runtime.net.transport_ticks, 0);
            assert!(
                runtime
                    .net
                    .peers
                    .iter()
                    .all(|peer| peer.committed_round == 0)
            );
        }
    }

    #[test]
    fn toggling_the_profile_resets_into_a_clean_network() {
        let mut app = test_app();
        {
            let mut runtime = app.world_mut().resource_mut::<NetRuntime>();
            runtime.profile = NetworkProfile::Clean;
            runtime.reset_requested = true;
        }
        app.update();
        assert_eq!(
            app.world().resource::<NetRuntime>().net.network.profile,
            NetworkProfile::Clean
        );
    }
}
