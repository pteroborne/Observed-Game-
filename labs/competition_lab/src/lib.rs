mod lab;
// The model was promoted into `crates/observed_match` (refactor R9); re-export
// it under the familiar `model` path. This lab is the debug projection.
pub mod model {
    pub use observed_match::competition::*;
}

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};
use observed_core::TeamId;

pub use lab::CompetitionRuntime;
pub use model::CompetitionWorld;

pub struct CompetitionLabPlugin;

impl Plugin for CompetitionLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CompetitionWorld::authored())
            .init_resource::<CompetitionRuntime>()
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
        Transform::from_xyz(0.0, 30.0, 1000.0),
        Name::new("Competition Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.012, 0.020, 0.028)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Competition Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(CompetitionLabPlugin);

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
    mut world: ResMut<CompetitionWorld>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Arrange a finished result: Team 3 seized the control and won; Team 1
        // took the last exit; Team 2 was locked out.
        for team in &mut world.teams {
            match team.id.0 {
                0 => {
                    team.progress = 1.0;
                    team.placement = Some(2);
                }
                1 => {
                    team.progress = 0.78;
                    team.eliminated = true;
                }
                _ => {
                    team.progress = 1.0;
                    team.placement = Some(1);
                }
            }
        }
        world.slots_remaining = 0;
        world.next_placement = 3;
        world.winner = Some(TeamId(2));
        world.control_holder = Some(TeamId(2));
        world.finished = true;
        world.last_event = "Match over — Team 3 wins; Team 2 locked out.".to_string();
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
    use crate::lab::{CompUiRoot, Racer};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use model::{EXIT_CAPACITY, TEAM_COUNT};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(CompetitionLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_a_racer_per_team_and_open_exits() {
        let mut app = test_app();
        assert_eq!(count::<Racer>(&mut app), TEAM_COUNT);
        assert_eq!(count::<CompUiRoot>(&mut app), 1);
        let world = app.world().resource::<CompetitionWorld>();
        assert!(!world.finished);
        assert_eq!(world.slots_remaining, EXIT_CAPACITY);
    }

    #[test]
    fn repeated_reset_restores_a_fresh_match() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            {
                let mut world = app.world_mut().resource_mut::<CompetitionWorld>();
                world.finished = true; // simulate a finished match
                world.winner = Some(TeamId(1));
            }
            app.world_mut()
                .resource_mut::<CompetitionRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<Racer>(&mut app), TEAM_COUNT);
            assert_eq!(count::<CompUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<CompetitionRuntime>().reset_count,
                reset_count
            );
            let world = app.world().resource::<CompetitionWorld>();
            assert!(!world.finished);
            assert!(world.winner.is_none());
            assert_eq!(world.slots_remaining, EXIT_CAPACITY);
        }
    }
}
