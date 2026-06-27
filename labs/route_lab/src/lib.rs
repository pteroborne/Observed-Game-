mod lab;
pub mod model;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::RouteRuntime;
pub use model::RouteWorld;

pub struct RouteLabPlugin;

impl Plugin for RouteLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RouteWorld::authored())
            .init_resource::<RouteRuntime>()
            .init_resource::<lab::DecohereTimer>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::auto_decohere,
                    lab::perform_reset,
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
        Name::new("Route Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.010, 0.018, 0.028)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Persistent Route Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(RouteLabPlugin);

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
    mut world: ResMut<RouteWorld>,
    mut runtime: ResMut<RouteRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    use observation_lab::model::Side;
    use observed_core::{RoomId, TeamId};

    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        runtime.auto_decohere = false;
        // Both teams lay cables on a few interior routes, then the structure
        // decoheres around them — the cabled routes hold.
        for (team, room) in [
            (TeamId(0), 0u32),
            (TeamId(0), 6),
            (TeamId(1), 1),
            (TeamId(1), 4),
        ] {
            let door = world.graph.door_id(RoomId(room), Side::East);
            world.deploy_cable(team, door);
        }
        for _ in 0..8 {
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
    use crate::lab::RouteUiRoot;
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use observation_lab::model::{ROOM_COUNT, Side};
    use observed_core::{RoomId, TeamId};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(RouteLabPlugin);
        app.update();
        app
    }

    #[test]
    fn boots_with_rooms_and_one_ui_root() {
        let mut app = test_app();
        let rooms = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<Sprite>>();
            query.iter(world).count()
        };
        assert_eq!(rooms, ROOM_COUNT);
        let ui = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<RouteUiRoot>>();
            query.iter(world).count()
        };
        assert_eq!(ui, 1);
    }

    #[test]
    fn repeated_reset_restores_a_fresh_structure() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            {
                let mut world = app.world_mut().resource_mut::<RouteWorld>();
                let door = world.graph.door_id(RoomId(0), Side::East);
                world.deploy_cable(TeamId(0), door);
                world.decohere();
            }
            app.world_mut()
                .resource_mut::<RouteRuntime>()
                .reset_requested = true;
            app.update();

            let world = app.world().resource::<RouteWorld>();
            assert!(world.cables.is_empty());
            assert_eq!(world.decohere_count, 0);
            assert_eq!(world.budget_of(TeamId(0)), crate::model::CABLE_CAPACITY);
            assert_eq!(
                app.world().resource::<RouteRuntime>().reset_count,
                reset_count
            );
        }
    }
}
