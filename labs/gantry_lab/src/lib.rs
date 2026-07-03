//! Gantry lab: the Phase 40 jump-map hallway proof.
//!
//! The pure rules live in `observed_traversal::gantry`; this lab renders that course
//! and runs the deterministic bot through clean-jump, fall-recover, and safe-bypass
//! routes so the timing spread and lower-floor recovery are reviewable.

mod lab;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::{GantryRunner, GantryRuntime, GantryUiRoot};
use observed_traversal::gantry::GantryRoute;

pub struct GantryLabPlugin;

impl Plugin for GantryLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(60.0))
            .add_systems(Startup, lab::setup_lab)
            .add_systems(FixedUpdate, lab::simulate)
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::present_runner,
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
    app.insert_resource(ClearColor(Color::srgb(0.006, 0.010, 0.018)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Gantry Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(GantryLabPlugin);

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest {
            path,
            phase: 0,
            route: GantryRoute::FallRecover,
        })
        .add_systems(Update, capture_progress.after(lab::present_camera));
    }

    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
    route: GantryRoute,
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut runtime: ResMut<GantryRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        runtime.set_route(request.route);
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 4.2 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 5.0 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use observed_traversal::gantry::{GantryExit, simulate_route};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Image>()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(GantryLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_camera_ui_runner_and_course() {
        let mut app = test_app();
        assert_eq!(count::<lab::GantryCam>(&mut app), 1);
        assert_eq!(count::<GantryUiRoot>(&mut app), 1);
        assert_eq!(count::<GantryRunner>(&mut app), 1);
        assert!(
            !app.world()
                .resource::<GantryRuntime>()
                .course
                .platforms
                .is_empty()
        );
    }

    #[test]
    fn route_keys_reset_runner_without_leaking_entities() {
        let mut app = test_app();
        let spawned = count::<lab::GantrySpawned>(&mut app);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Digit2);
        app.world_mut().run_schedule(Update);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Digit2);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.world_mut().run_schedule(Update);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyR);

        assert_eq!(count::<lab::GantrySpawned>(&mut app), spawned);
        let runtime = app.world().resource::<GantryRuntime>();
        assert_eq!(runtime.route, GantryRoute::FallRecover);
        assert_eq!(runtime.reset_count, 2);
    }

    #[test]
    fn pure_routes_remain_available_to_the_lab() {
        let runtime = GantryRuntime::default();
        let result = simulate_route(
            GantryRoute::FallRecover,
            &runtime.course,
            &runtime.config,
            900,
        )
        .expect("fall route completes");
        assert_eq!(result.exit, GantryExit::UnderstorySideExit);
    }
}
