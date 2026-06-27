mod lab;
pub mod model;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::RewireRuntime;
pub use model::RewireStage;

pub struct FpsRewirePlugin;

impl Plugin for FpsRewirePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RewireStage>()
            .init_resource::<RewireRuntime>()
            .init_resource::<lab::CameraIntent>()
            .init_resource::<lab::RewireTimer>()
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .add_systems(Startup, (lab::setup_lab, lab::grab_cursor))
            .add_systems(FixedUpdate, lab::simulate)
            .add_systems(
                Update,
                (
                    lab::map_input.after(InputSystems),
                    lab::toggle_grab,
                    lab::handle_actions,
                    lab::perform_reset,
                    lab::auto_request,
                    lab::commit_safe_swaps,
                    lab::sync_module_visuals,
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
                title: "Observed 2 - FPS Rewire Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FpsRewirePlugin);

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
    mut stage: ResMut<RewireStage>,
    mut runtime: ResMut<RewireRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        runtime.auto = false;
        lab::stage_capture_showcase(&mut stage);
        runtime.camera_override = Some(
            Transform::from_xyz(19.0, 23.0, 21.0).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
        );
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.6 {
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
    use lab::{ModuleVisualRoot, PlayerCam, RewireUiRoot};
    use model::{GATEWAY_COUNT, GatewayId};

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
        .add_plugins(FpsRewirePlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    fn root_entity(app: &mut App, gateway: GatewayId) -> Entity {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &ModuleVisualRoot)>();
        query
            .iter(world)
            .find(|(_, root)| root.gateway == gateway)
            .map(|(entity, _)| entity)
            .expect("gateway visual root")
    }

    #[test]
    fn boots_with_one_camera_ui_and_four_rendered_modules() {
        let mut app = test_app();
        assert_eq!(count::<PlayerCam>(&mut app), 1);
        assert_eq!(count::<RewireUiRoot>(&mut app), 1);
        assert_eq!(count::<ModuleVisualRoot>(&mut app), GATEWAY_COUNT);
    }

    #[test]
    fn actual_module_entities_swap_only_after_their_portals_are_hidden() {
        let mut app = test_app();
        let east_before = root_entity(&mut app, GatewayId::EAST);

        // Looking east excludes the east portal from the first batch.
        {
            let mut stage = app.world_mut().resource_mut::<RewireStage>();
            stage.set_facing(GatewayId::EAST);
            stage.request_rewire();
            assert!(stage.commit_pending());
        }
        app.update();
        assert_eq!(
            root_entity(&mut app, GatewayId::EAST),
            east_before,
            "visible geometry entity must not be replaced"
        );

        // Turn north: east is hidden, so a later atomic batch may replace it.
        {
            let mut stage = app.world_mut().resource_mut::<RewireStage>();
            stage.set_facing(GatewayId::NORTH);
            stage.request_rewire();
            assert!(stage.commit_pending());
        }
        app.update();
        assert_ne!(root_entity(&mut app, GatewayId::EAST), east_before);
        assert_eq!(app.world().resource::<RewireStage>().seam_violations, 0);
    }

    #[test]
    fn repeated_reset_restores_four_modules_without_entity_leaks() {
        let mut app = test_app();
        let baseline_meshes = count::<Mesh3d>(&mut app);
        for expected in 1..=10 {
            {
                let mut stage = app.world_mut().resource_mut::<RewireStage>();
                stage.set_facing(GatewayId::NORTH);
                stage.request_rewire();
                stage.commit_pending();
                stage.begin_transit(GatewayId::EAST);
            }
            app.world_mut()
                .resource_mut::<RewireRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<PlayerCam>(&mut app), 1);
            assert_eq!(count::<RewireUiRoot>(&mut app), 1);
            assert_eq!(count::<ModuleVisualRoot>(&mut app), GATEWAY_COUNT);
            assert_eq!(
                count::<Mesh3d>(&mut app),
                baseline_meshes,
                "module descendants must be recursively cleaned up"
            );
            let stage = app.world().resource::<RewireStage>();
            assert_eq!(stage.reset_count, expected);
            assert_eq!(stage.commit_count, 0);
            assert!(stage.pending.is_none());
            assert!(stage.transit.is_none());
        }
    }
}
