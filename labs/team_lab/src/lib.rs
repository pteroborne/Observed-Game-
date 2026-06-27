mod input;
mod lab;
pub mod model;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::TeamLabRuntime;
pub use model::{
    Item, ItemId, Station, StationId, StationKind, TeamEvent, TeamIntent, TeamPlayer, TeamWorld,
    Zone, ZoneId,
};

pub struct TeamLabPlugin;

impl Plugin for TeamLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TeamWorld::authored_lab())
            .init_resource::<TeamLabRuntime>()
            .init_resource::<input::HumanInput>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(PreUpdate, input::sample_hardware.after(InputSystems))
            .add_systems(
                Update,
                (
                    lab::handle_shortcuts,
                    lab::simulate,
                    lab::perform_reset,
                    lab::present_players,
                    lab::present_items,
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
            scale: 1.45,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Name::new("Team Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.012, 0.022, 0.035)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Team Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(TeamLabPlugin);

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(lab::FreezeAgents)
            .insert_resource(CaptureRequest { path, taken: false })
            .add_systems(Update, capture_progress);
    }

    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    taken: bool,
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if !request.taken && elapsed >= 0.6 {
        let path = request.path.clone();
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        request.taken = true;
    } else if request.taken && elapsed >= 1.6 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::{ItemDot, PlayerDot, TeamLabUiRoot};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use observed_core::TeamId;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        // Freeze bots so test ticks are deterministic and don't depend on timing.
        .insert_resource(lab::FreezeAgents)
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(TeamLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_a_dot_per_player_and_item() {
        let mut app = test_app();
        assert_eq!(count::<PlayerDot>(&mut app), 4);
        assert_eq!(count::<ItemDot>(&mut app), 2);
        assert_eq!(count::<TeamLabUiRoot>(&mut app), 1);
        // Team B (the machine crew) starts together; team A is split.
        let world = app.world().resource::<TeamWorld>();
        assert!(world.cohesive(TeamId(1)));
        assert!(!world.cohesive(TeamId(0)));
    }

    #[test]
    fn repeated_reset_restores_counts_and_baseline() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            {
                let mut world = app.world_mut().resource_mut::<TeamWorld>();
                world.denials += 4;
                world.reunions += 2;
            }
            app.world_mut()
                .resource_mut::<TeamLabRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<PlayerDot>(&mut app), 4);
            assert_eq!(count::<ItemDot>(&mut app), 2);
            assert_eq!(count::<TeamLabUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<TeamLabRuntime>().reset_count,
                reset_count
            );
            let world = app.world().resource::<TeamWorld>();
            assert_eq!(world.denials, 0);
            assert_eq!(world.reunions, 0);
            assert_eq!(world.station(StationId(3)).unwrap().occupants.len(), 2);
        }
    }
}
