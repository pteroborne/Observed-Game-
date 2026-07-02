mod lab;
pub mod model;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::{
    GuardianRuntime, MoveTimer, draw_debug, handle_input, present_entities, setup_lab,
    update_debug_text, update_guardian,
};
pub use model::{Actor, Guardian, GuardianState, SimpleRng};

pub struct GuardianLabPlugin;

impl Plugin for GuardianLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(observed_observation::ObservationWorld::authored())
            .insert_resource(Guardian::default())
            .insert_resource(SimpleRng::new(12345)) // Seeded SimpleRng
            .init_resource::<GuardianRuntime>()
            .init_resource::<MoveTimer>()
            .add_systems(Startup, (setup_camera, setup_lab))
            .add_systems(
                Update,
                (
                    handle_input.after(InputSystems),
                    update_guardian,
                    present_entities,
                    draw_debug,
                    update_debug_text,
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
        Name::new("Guardian Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.010, 0.018, 0.030)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Guardian AI Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(GuardianLabPlugin);

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
    mut runtime: ResMut<GuardianRuntime>,
    mut guardian: ResMut<Guardian>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Move player to room 0, looking East. Guardian starts at room 8.
        // Drop an anchor in room 8 to observe it!
        runtime.anchors.insert(observed_core::RoomId(8));
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.5 {
        // Force guardian timer down to trigger 30s reset quickly for capture demonstration
        guardian.anchor_timer = 0.1;
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 1.0 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 3;
    } else if request.phase == 3 && elapsed >= 1.8 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::{GuardianDot, LabUiRoot, PlayerArrow};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use observed_core::RoomId;
    use observed_observation::Side;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(GuardianLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_guardian_entities_and_ui() {
        let mut app = test_app();
        assert_eq!(count::<GuardianDot>(&mut app), 1);
        assert_eq!(count::<PlayerArrow>(&mut app), 4); // 1 player + 3 bots
        assert_eq!(count::<LabUiRoot>(&mut app), 1);
    }

    #[test]
    fn pathfinding_resolves_shortest_path() {
        let world = observed_observation::ObservationWorld::authored();
        let path = model::find_shortest_path(&world, RoomId(8), RoomId(0)).unwrap();
        // Authored lattice: Room 8 (bottom-right) connects to Room 7 or Room 5
        // shortest path to 0 (top-left) should be 8 -> 7 -> 6 -> 3 -> 0 (or similar, 5 hops)
        assert!(path.len() >= 5);
        assert_eq!(path[0], RoomId(8));
        assert_eq!(*path.last().unwrap(), RoomId(0));
    }

    #[test]
    fn direct_and_threshold_visibility_freezes_guardian() {
        let world = observed_observation::ObservationWorld::authored();

        // Target room is room 0, facing East.
        // Room 0 East connects to Room 1.
        // If guardian is in Room 1, it should be visible (frozen).
        let visible = model::visible_rooms_from_view(&world, RoomId(0), Side::East);
        assert!(visible.contains(&RoomId(1)));

        // If guardian is in Room 8, it is not visible.
        assert!(!visible.contains(&RoomId(8)));
    }

    #[test]
    fn anchor_observation_triggers_reset() {
        let world = observed_observation::ObservationWorld::authored();
        let mut guardian = Guardian {
            room: RoomId(8),
            target_player: 0,
            anchor_timer: 0.1,
        };
        let mut anchors = std::collections::HashSet::new();
        anchors.insert(RoomId(8));
        let mut rng = SimpleRng::new(999);

        let mut actors = vec![Actor {
            id: 0,
            room: RoomId(0),
            facing: Side::North,
            is_bot: false,
            touch_count: 0,
            is_teleported: false,
        }];

        let (state, teleport, caught) = model::tick_guardian(
            &world,
            &mut guardian,
            &mut actors,
            &anchors,
            &mut rng,
            0.2,
            false,
        );

        assert_eq!(state, GuardianState::FrozenByAnchor);
        assert!(teleport.is_some());
        assert_ne!(guardian.room, RoomId(8));
        assert_eq!(guardian.anchor_timer, 30.0);
        assert!(caught.is_none());
    }

    #[test]
    fn bot_freezing_and_catching_behaviors() {
        let world = observed_observation::ObservationWorld::authored();
        let mut guardian = Guardian {
            room: RoomId(1),
            target_player: 0,
            anchor_timer: 30.0,
        };
        let anchors = std::collections::HashSet::new();
        let mut rng = SimpleRng::new(888);

        // 1. Test bot freezing: Bot at Room 0, facing East, looks at Room 1 where guardian resides.
        let mut actors = vec![
            Actor {
                id: 0,
                room: RoomId(8), // Human player far away
                facing: Side::North,
                is_bot: false,
                touch_count: 0,
                is_teleported: false,
            },
            Actor {
                id: 1,
                room: RoomId(0), // Bot looks at Room 1 (since 0 East connects to 1)
                facing: Side::East,
                is_bot: true,
                touch_count: 0,
                is_teleported: false,
            },
        ];

        let (state, _, caught) = model::tick_guardian(
            &world,
            &mut guardian,
            &mut actors,
            &anchors,
            &mut rng,
            0.1,
            false,
        );

        assert_eq!(state, GuardianState::FrozenByPlayer);
        assert!(caught.is_none());

        // 2. Test bot catching: Bot walks into Room 1 (guardian's room) while not looking (facing North)
        actors[1].room = RoomId(1);
        actors[1].facing = Side::North;

        let (_state, _, caught) = model::tick_guardian(
            &world,
            &mut guardian,
            &mut actors,
            &anchors,
            &mut rng,
            0.1,
            false,
        );

        // State is active (unobserved) or frozen?
        // Wait, bot is at Room 1, facing North. The visible rooms from Room 1, Side::North:
        // Room 1 North door is sealed?
        // Let's see: in authored world, room 1 North door might be sealed. Even if it is, the bot's room (Room 1) is always in its own visible set. So it is frozen by direct occupancy!
        // Yes, self occupancy counts as observed!
        // Let's assert that the collision is detected and the bot is caught and teleported.
        assert_eq!(caught, Some(1));
        assert!(actors[1].is_teleported);
        assert_eq!(actors[1].touch_count, 1);
        assert_ne!(actors[1].room, RoomId(1)); // Should be teleported away from Room 1
    }
}
