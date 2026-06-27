mod lab;
pub mod model;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::FacilityRuntime;
pub use model::FacilityStage;

pub struct FpsFacilityPlugin;

impl Plugin for FpsFacilityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FacilityStage>()
            .init_resource::<FacilityRuntime>()
            .init_resource::<lab::InputIntent>()
            .init_resource::<lab::DecohereTimer>()
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .add_systems(Startup, (lab::setup_lab, lab::grab_cursor))
            .add_systems(FixedUpdate, lab::simulate)
            .add_systems(
                Update,
                (
                    lab::map_input.after(InputSystems),
                    lab::toggle_grab,
                    lab::handle_actions,
                    lab::auto_decohere,
                    lab::perform_reset,
                    lab::sync_door_panels,
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
                title: "Observed 2 - FPS Facility Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FpsFacilityPlugin);

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
    mut stage: ResMut<FacilityStage>,
    mut runtime: ResMut<FacilityRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        runtime.auto_decohere = false;
        lab::stage_capture_showcase(&mut stage);
        runtime.camera_override = Some(
            Transform::from_xyz(38.0, 45.0, 40.0).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
        );
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
    use lab::{DoorPanelRoot, FacilityUiRoot, ModuleVisualRoot, PlayerCam};
    use observation_lab::model::{DOOR_COUNT, DoorId, ROOM_COUNT};

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
        .add_plugins(FpsFacilityPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_nine_modules_one_camera_ui_and_exact_sealed_panels() {
        let mut app = test_app();
        assert_eq!(count::<PlayerCam>(&mut app), 1);
        assert_eq!(count::<FacilityUiRoot>(&mut app), 1);
        assert_eq!(count::<ModuleVisualRoot>(&mut app), ROOM_COUNT);
        let sealed = (0..DOOR_COUNT)
            .filter(|index| {
                app.world()
                    .resource::<FacilityStage>()
                    .graph
                    .is_sealed(DoorId(*index as u16))
            })
            .count();
        assert_eq!(count::<DoorPanelRoot>(&mut app), sealed);
        assert!(app.world().resource::<FacilityStage>().projection_exact());
    }

    #[test]
    fn decoherence_updates_rendered_door_panels_to_match_the_same_graph() {
        let mut app = test_app();
        app.world_mut().resource_mut::<FacilityStage>().decohere();
        app.update();
        let (sealed, exact) = {
            let stage = app.world().resource::<FacilityStage>();
            (
                (0..DOOR_COUNT)
                    .filter(|index| stage.graph.is_sealed(DoorId(*index as u16)))
                    .count(),
                stage.projection_exact(),
            )
        };
        assert_eq!(count::<DoorPanelRoot>(&mut app), sealed);
        assert!(exact);
    }

    #[test]
    fn repeated_reset_restores_geometry_graph_and_entity_counts_without_leaks() {
        let mut app = test_app();
        let baseline_meshes = count::<Mesh3d>(&mut app);
        for expected in 1..=10 {
            {
                let mut stage = app.world_mut().resource_mut::<FacilityStage>();
                stage.decohere();
                stage.traverse(observation_lab::model::Side::East);
            }
            app.world_mut()
                .resource_mut::<FacilityRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<PlayerCam>(&mut app), 1);
            assert_eq!(count::<FacilityUiRoot>(&mut app), 1);
            assert_eq!(count::<ModuleVisualRoot>(&mut app), ROOM_COUNT);
            assert_eq!(count::<Mesh3d>(&mut app), baseline_meshes);
            let stage = app.world().resource::<FacilityStage>();
            assert_eq!(stage.reset_count, expected);
            assert_eq!(stage.player_room, observed_core::RoomId(4));
            assert!(stage.projection_exact());
        }
    }
}
