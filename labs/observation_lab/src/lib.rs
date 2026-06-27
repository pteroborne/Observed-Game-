mod lab;
pub mod model;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::ObservationRuntime;
pub use model::ObservationWorld;

pub struct ObservationLabPlugin;

impl Plugin for ObservationLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ObservationWorld::authored())
            .init_resource::<ObservationRuntime>()
            .init_resource::<lab::DecohereTimer>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::auto_decohere,
                    lab::perform_reset,
                    lab::present_players,
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
            scale: 1.55,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 40.0, 1000.0),
        Name::new("Observation Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.010, 0.018, 0.030)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Observation Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ObservationLabPlugin);

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
    mut world: ResMut<ObservationWorld>,
    mut runtime: ResMut<ObservationRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Freeze auto-decoherence and rewire the unobserved structure a few times
        // so the watched corners stay frozen while the interior is scrambled.
        runtime.auto_decohere = false;
        for _ in 0..6 {
            world.decohere();
        }
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
    use crate::lab::{ObsUiRoot, PlayerDot};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use model::PLAYER_COUNT;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(ObservationLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_an_observer_dot_each_and_one_ui_root() {
        let mut app = test_app();
        assert_eq!(count::<PlayerDot>(&mut app), PLAYER_COUNT);
        assert_eq!(count::<ObsUiRoot>(&mut app), 1);
    }

    #[test]
    fn repeated_reset_restores_counts_and_structure() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            {
                let mut world = app.world_mut().resource_mut::<ObservationWorld>();
                world.decohere();
                world.decohere();
            }
            app.world_mut()
                .resource_mut::<ObservationRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<PlayerDot>(&mut app), PLAYER_COUNT);
            assert_eq!(count::<ObsUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<ObservationRuntime>().reset_count,
                reset_count
            );
            assert_eq!(
                app.world().resource::<ObservationWorld>().decoherence_count,
                0
            );
        }
    }
}
