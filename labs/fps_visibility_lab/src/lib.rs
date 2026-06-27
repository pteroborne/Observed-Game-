pub mod field;
mod lab;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use field::VisionField;
pub use lab::VisibilityRuntime;

pub struct FpsVisibilityPlugin;

impl Plugin for FpsVisibilityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(VisionField::authored())
            .init_resource::<VisibilityRuntime>()
            .init_resource::<lab::CameraIntent>()
            .init_resource::<lab::DecohereTimer>()
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .add_systems(Startup, (lab::setup_lab, lab::grab_cursor))
            .add_systems(FixedUpdate, lab::advance_camera)
            .add_systems(
                Update,
                (
                    lab::map_input.after(InputSystems),
                    lab::toggle_grab,
                    lab::handle_toggles,
                    lab::perform_reset,
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
    app.insert_resource(ClearColor(Color::srgb(0.012, 0.018, 0.028)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - FPS Visibility Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FpsVisibilityPlugin);

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
    mut field: ResMut<VisionField>,
    mut runtime: ResMut<VisibilityRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        runtime.auto = false;
        field.eye = field::room_center(observed_core::RoomId(4)) + Vec2::new(-1.2, 0.8);
        field.yaw = std::f32::consts::FRAC_PI_2 - 0.14;
        field.recompute();
        for _ in 0..5 {
            field.decohere();
        }
        runtime.camera_override = Some(
            Transform::from_xyz(24.0, 28.0, 27.0).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
        );
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
    use lab::{FpsUiRoot, PlayerCam};
    use player_input::PlayerIntent;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(FpsVisibilityPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_one_camera_one_ui_and_partial_visibility() {
        let mut app = test_app();
        assert_eq!(count::<PlayerCam>(&mut app), 1);
        assert_eq!(count::<FpsUiRoot>(&mut app), 1);
        assert!(app.world().resource::<VisionField>().seen_cell_count() > 0);
        assert!(app.world().resource::<VisionField>().partially_seen_rooms() > 0);
    }

    #[test]
    fn camera_intent_updates_the_observed_set() {
        let mut app = test_app();
        let before = app.world().resource::<VisionField>().seen_cells.clone();
        {
            let mut field = app.world_mut().resource_mut::<VisionField>();
            for _ in 0..30 {
                field.advance_camera(
                    PlayerIntent {
                        look: Vec2::X,
                        ..default()
                    },
                    1.0 / 60.0,
                );
            }
        }
        let field = app.world().resource::<VisionField>();
        assert_ne!(field.seen_cells, before);
    }

    #[test]
    fn repeated_reset_is_stable_and_leak_free() {
        let mut app = test_app();
        for expected in 1..=10 {
            {
                let mut field = app.world_mut().resource_mut::<VisionField>();
                field.decohere();
                field.eye += Vec2::splat(1.0);
            }
            app.world_mut()
                .resource_mut::<VisibilityRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<PlayerCam>(&mut app), 1);
            assert_eq!(count::<FpsUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<VisibilityRuntime>().reset_count,
                expected
            );
            assert_eq!(app.world().resource::<VisionField>().decohere_count, 0);
            assert_eq!(
                app.world().resource::<VisionField>().eye,
                field::room_center(observed_core::RoomId(4))
            );
        }
    }
}
