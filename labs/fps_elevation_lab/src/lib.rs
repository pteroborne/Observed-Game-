pub mod climb;
mod lab;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use climb::{ElevArena, STEP_HEIGHT, step_body_elev};
pub use lab::ElevRuntime;

pub struct ElevationLabPlugin;

impl Plugin for ElevationLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(60.0))
            .add_systems(Startup, (lab::setup_lab, lab::grab_cursor))
            .add_systems(FixedUpdate, lab::simulate)
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::perform_reset,
                    lab::present_camera,
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
                title: "Observed 2 — Elevation Controller".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ElevationLabPlugin);

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, phase: 0 })
            .add_systems(Update, capture_progress.after(lab::present_camera));
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
    mut runtime: ResMut<ElevRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        runtime.auto_walk = true; // climb the stairs for the shot
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 1.3 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 2.1 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use lab::{ElevCam, ElevUiRoot};
    use player_input::PlayerIntent;

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
        .add_plugins(ElevationLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_camera_and_ui_over_the_course() {
        let mut app = test_app();
        assert_eq!(count::<ElevCam>(&mut app), 1);
        assert_eq!(count::<ElevUiRoot>(&mut app), 1);
        assert!(
            !app.world()
                .resource::<ElevRuntime>()
                .arena
                .solids
                .is_empty()
        );
    }

    #[test]
    fn walking_forward_climbs_onto_the_platform() {
        let mut app = test_app();
        app.world_mut().resource_mut::<ElevRuntime>().intent = PlayerIntent {
            movement: Vec2::new(0.0, 1.0),
            ..default()
        };
        for _ in 0..170 {
            app.world_mut()
                .run_system_cached(lab::simulate)
                .expect("simulate runs");
        }
        let runtime = app.world().resource::<ElevRuntime>();
        assert!(
            runtime.max_feet >= runtime.arena.platform_top() - 0.05,
            "climbed onto the platform (reached {})",
            runtime.max_feet
        );
    }

    #[test]
    fn repeated_reset_restores_spawn_without_leaks() {
        let mut app = test_app();
        for expected in 1..=8 {
            app.world_mut().resource_mut::<ElevRuntime>().intent = PlayerIntent {
                movement: Vec2::new(0.0, 1.0),
                ..default()
            };
            for _ in 0..30 {
                app.world_mut().run_system_cached(lab::simulate).ok();
            }
            app.world_mut()
                .resource_mut::<ElevRuntime>()
                .reset_requested = true;
            app.update();
            assert_eq!(count::<ElevCam>(&mut app), 1);
            assert_eq!(count::<ElevUiRoot>(&mut app), 1);
            let runtime = app.world().resource::<ElevRuntime>();
            assert_eq!(runtime.reset_count, expected);
            assert_eq!(runtime.body.position.x, 0.0);
            assert_eq!(runtime.max_feet, 0.0);
        }
    }
}
