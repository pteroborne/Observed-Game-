mod lab;
pub mod vision;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

use std::f32::consts::FRAC_PI_2;

pub use lab::Observatory;

pub struct FpsObservationPlugin;

impl Plugin for FpsObservationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Observatory>()
            .init_resource::<lab::DecohereTimer>()
            .add_systems(Startup, (lab::setup_lab, lab::grab_cursor))
            .add_systems(
                Update,
                (
                    lab::control.after(InputSystems),
                    lab::toggle_grab,
                    lab::perform_reset,
                    lab::update_observation,
                    lab::decohere_step,
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
    app.insert_resource(ClearColor(Color::srgb(0.015, 0.02, 0.03)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — FPS Observation Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FpsObservationPlugin);

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
    mut obs: ResMut<Observatory>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Stand in the corner room looking east down the open corridor, then let
        // the unobserved structure rewire a few times: the seen corridor stays
        // green/frozen while everything off-camera (cyan) has shifted.
        obs.auto = false;
        obs.player = observed_core::RoomId(0);
        obs.yaw = FRAC_PI_2;
        for _ in 0..4 {
            obs.recompute_visibility();
            obs.decohere();
        }
        obs.recompute_visibility();
        // Pull the camera up to an angled overview so the whole facility — the seen
        // (green) corridor versus the rewired (cyan) rest — is legible in one shot.
        obs.camera_override = Some(
            Transform::from_xyz(26.0, 27.0, 26.0).looking_at(Vec3::new(0.0, 1.5, 0.0), Vec3::Y),
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
    use crate::lab::{FpsUiRoot, PlayerCam};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use observed_core::RoomId;
    use vision::{forward_from_yaw, side_direction};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(FpsObservationPlugin);
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
        // The opening observer set is non-empty (the player room at least).
        assert!(!app.world().resource::<Observatory>().visible.is_empty());
    }

    #[test]
    fn looking_changes_the_observed_set_live() {
        let mut app = test_app();
        // Face east (down the corridor): more than just the player room is seen.
        app.world_mut().resource_mut::<Observatory>().player = RoomId(0);
        app.world_mut().resource_mut::<Observatory>().yaw =
            yaw_facing(side_direction(observation_lab::model::Side::East));
        app.update();
        let east_seen = app.world().resource::<Observatory>().visible.len();
        assert!(
            east_seen > 1,
            "looking down an open corridor sees beyond the room"
        );

        // Face west (into a wall): only the player room.
        app.world_mut().resource_mut::<Observatory>().yaw =
            yaw_facing(side_direction(observation_lab::model::Side::West));
        app.update();
        let west_seen = app.world().resource::<Observatory>().visible.clone();
        assert_eq!(
            west_seen,
            vec![RoomId(0)],
            "looking at a wall sees only here"
        );
    }

    #[test]
    fn repeated_reset_is_stable_and_leak_free() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            {
                let mut obs = app.world_mut().resource_mut::<Observatory>();
                obs.decohere();
                obs.player = RoomId(4);
                obs.reset_requested = true;
            }
            app.update();

            assert_eq!(count::<PlayerCam>(&mut app), 1);
            assert_eq!(count::<FpsUiRoot>(&mut app), 1);
            let obs = app.world().resource::<Observatory>();
            assert_eq!(obs.reset_count, reset_count);
            assert_eq!(obs.decohere_count, 0);
            assert_eq!(obs.player, RoomId(0));
        }
    }

    /// Yaw whose forward best matches a target horizontal direction.
    fn yaw_facing(dir: Vec3) -> f32 {
        // forward_from_yaw(yaw) = (sin yaw, 0, -cos yaw); solve for yaw.
        let yaw = dir.x.atan2(-dir.z);
        // sanity: the produced forward should align with dir
        debug_assert!(forward_from_yaw(yaw).dot(dir) > 0.9);
        yaw
    }
}
