mod lab;
pub mod model;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::HazardRuntime;
pub use model::{
    DirectorHazardAction, HazardPlayer, HazardTeam, HazardWorld, HazardZone, HazardZoneId,
    PlayerHazardIntent,
};

pub struct HazardLabPlugin;

impl Plugin for HazardLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(HazardWorld::authored())
            .init_resource::<HazardRuntime>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::perform_reset,
                    lab::simulate,
                    lab::present,
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
            scale: 1.05,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Name::new("Hazard Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.010, 0.018, 0.025)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Cooperative Hazard Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(HazardLabPlugin);

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
    mut runtime: ResMut<HazardRuntime>,
    mut world: ResMut<HazardWorld>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        for team in &mut world.teams {
            team.progress = 4;
        }
        runtime.staged_intents = [
            PlayerHazardIntent::VentA,
            PlayerHazardIntent::VentB,
            PlayerHazardIntent::Advance,
            PlayerHazardIntent::Advance,
        ];
        runtime.director_action = DirectorHazardAction::Steer(HazardZoneId(1));
        let intents = world
            .players
            .iter()
            .map(|player| (player.id, runtime.staged_intents[player.id.0 as usize]))
            .collect::<Vec<_>>();
        world.resolve_round(&intents, runtime.director_action);
        runtime.director_action = DirectorHazardAction::Hold;
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.6 {
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
    use crate::lab::{HazardField, HazardUiRoot, PlayerDot, TeamMarker};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use model::{PLAYER_COUNT, TEAM_COUNT};
    use observed_core::TeamId;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(HazardLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_players_teams_one_hazard_and_one_ui() {
        let mut app = test_app();
        assert_eq!(count::<PlayerDot>(&mut app), PLAYER_COUNT);
        assert_eq!(count::<TeamMarker>(&mut app), TEAM_COUNT);
        assert_eq!(count::<HazardField>(&mut app), 1);
        assert_eq!(count::<HazardUiRoot>(&mut app), 1);
        assert_eq!(app.world().resource::<HazardWorld>().round, 0);
    }

    #[test]
    fn staged_coordination_resolves_through_the_bevy_schedule() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<HazardRuntime>()
            .step_requested = true;
        app.update();

        let world = app.world().resource::<HazardWorld>();
        assert_eq!(world.round, 1);
        assert!(world.hazard.contained_last_round);
        assert_eq!(world.team(TeamId(1)).unwrap().progress, 2);
    }

    #[test]
    fn repeated_reset_restores_state_without_entity_leaks() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            {
                let mut runtime = app.world_mut().resource_mut::<HazardRuntime>();
                runtime.step_requested = true;
            }
            app.update();
            app.world_mut()
                .resource_mut::<HazardRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<PlayerDot>(&mut app), PLAYER_COUNT);
            assert_eq!(count::<TeamMarker>(&mut app), TEAM_COUNT);
            assert_eq!(count::<HazardField>(&mut app), 1);
            assert_eq!(count::<HazardUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<HazardRuntime>().reset_count,
                reset_count
            );
            assert_eq!(
                app.world().resource::<HazardWorld>(),
                &HazardWorld::authored()
            );
        }
    }
}
