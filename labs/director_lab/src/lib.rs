mod lab;
// The model was promoted into `crates/observed_match` (refactor R9); re-export
// it under the familiar `model` path. This lab is the debug projection.
pub mod model {
    pub use observed_match::director::*;
}

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::DirectorRuntime;
pub use model::DirectorWorld;

pub struct DirectorLabPlugin;

impl Plugin for DirectorLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DirectorWorld::authored())
            .init_resource::<DirectorRuntime>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::simulate,
                    lab::perform_reset,
                    lab::present_racers,
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
            scale: 0.95,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 20.0, 1000.0),
        Name::new("Director Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.020, 0.012, 0.016)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Facility Director Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(DirectorLabPlugin);

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
    mut world: ResMut<DirectorWorld>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Arrange a finished run: the two leaders escaped; the laggards were
        // absorbed into the facility director.
        for team in &mut world.teams {
            match team.id.0 {
                0 => {
                    team.progress = 1.0;
                    team.placement = Some(1);
                }
                1 => {
                    team.progress = 1.0;
                    team.placement = Some(2);
                }
                2 => {
                    team.progress = 0.55;
                    team.role = model::Role::Director;
                }
                _ => {
                    team.progress = 0.34;
                    team.role = model::Role::Director;
                }
            }
        }
        world.slots_remaining = 0;
        world.next_placement = 3;
        world.purge_line = 0.62;
        world.director_actions = 1;
        world.finished = true;
        world.last_event = "Run over — 2 escaped, 2 absorbed by the facility.".to_string();
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
    use crate::lab::{DirUiRoot, Racer};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use model::RUNNER_COUNT;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(DirectorLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_a_racer_per_team() {
        let mut app = test_app();
        assert_eq!(count::<Racer>(&mut app), RUNNER_COUNT);
        assert_eq!(count::<DirUiRoot>(&mut app), 1);
        let world = app.world().resource::<DirectorWorld>();
        assert!(!world.finished);
        assert_eq!(world.director_strength(), 0);
    }

    #[test]
    fn repeated_reset_restores_a_fresh_run() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            {
                let mut world = app.world_mut().resource_mut::<DirectorWorld>();
                world.finished = true;
                world.purge_line = 0.9;
            }
            app.world_mut()
                .resource_mut::<DirectorRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<Racer>(&mut app), RUNNER_COUNT);
            assert_eq!(count::<DirUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<DirectorRuntime>().reset_count,
                reset_count
            );
            let world = app.world().resource::<DirectorWorld>();
            assert!(!world.finished);
            assert_eq!(world.director_strength(), 0);
        }
    }
}
