pub mod controller;
mod lab;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::{Mode, Stage};

pub struct FpsControllerPlugin;

impl Plugin for FpsControllerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stage>()
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .add_systems(Startup, (lab::setup_lab, lab::grab_cursor))
            // The controller is stepped only at the fixed timestep — deterministic.
            .add_systems(FixedUpdate, lab::simulate)
            .add_systems(
                Update,
                (
                    lab::handle_toggles.after(InputSystems),
                    lab::toggle_grab,
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
    app.insert_resource(ClearColor(Color::srgb(0.015, 0.02, 0.022)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — FPS Controller Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FpsControllerPlugin);

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
    mut stage: ResMut<Stage>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Precompute a scripted run and its exact replay, then view the whole arena
        // from an angle so the two coincident trails are legible.
        lab::precompute(&mut stage, lab::scripted_tape());
        stage.camera_override = Some(
            Transform::from_xyz(24.0, 26.0, 24.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        );
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.6 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 1.4 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use lab::{FpsUiRoot, PlayerCam};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(FpsControllerPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_one_camera_and_ui() {
        let mut app = test_app();
        assert_eq!(count::<PlayerCam>(&mut app), 1);
        assert_eq!(count::<FpsUiRoot>(&mut app), 1);
    }

    #[test]
    fn recording_then_replaying_reproduces_the_path_exactly() {
        // The exit criterion, exercised through the lab's own record/replay: walk a
        // scripted tape live, then replay it on a fresh body and confirm an exact
        // match.
        let mut app = test_app();
        let tape = lab::scripted_tape();
        {
            let mut stage = app.world_mut().resource_mut::<Stage>();
            for intent in &tape {
                stage.step_live(*intent);
            }
            assert_eq!(stage.tape.len(), tape.len());
            stage.begin_replay();
            assert_eq!(stage.mode, Mode::Replay);
            while stage.replay_match.is_none() {
                stage.step_replay();
            }
            assert_eq!(
                stage.replay_match,
                Some(true),
                "replay must match the recording"
            );
            assert_eq!(stage.replay_path, stage.live_path);
        }
    }

    #[test]
    fn reset_clears_the_recording_and_returns_to_spawn() {
        let mut app = test_app();
        for reset_count in 1..=5 {
            {
                let mut stage = app.world_mut().resource_mut::<Stage>();
                for intent in lab::scripted_tape().iter().take(40) {
                    stage.step_live(*intent);
                }
                stage.reset();
                assert!(stage.tape.is_empty());
                assert!(stage.live_path.is_empty());
                assert_eq!(stage.mode, Mode::Live);
                assert_eq!(stage.reset_count, reset_count);
            }
            assert_eq!(count::<PlayerCam>(&mut app), 1);
            assert_eq!(count::<FpsUiRoot>(&mut app), 1);
        }
    }
}
