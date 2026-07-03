mod lab;
pub mod model;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::DiscoveryRuntime;
pub use model::DiscoveryWorld;

pub struct DiscoveryLabPlugin;

impl Plugin for DiscoveryLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DiscoveryWorld::authored())
            .init_resource::<DiscoveryRuntime>()
            .init_resource::<lab::ShiftTimer>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::auto_explore,
                    lab::perform_reset,
                    lab::present_rooms,
                    lab::present_door_frames,
                    lab::present_door_glyphs,
                    lab::present_door_bleeds,
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
            scale: 1.25,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, -40.0, 1000.0),
        Name::new("Discovery Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.012, 0.018, 0.022)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Discovery Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(DiscoveryLabPlugin);

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
    mut world: ResMut<DiscoveryWorld>,
    mut runtime: ResMut<DiscoveryRuntime>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Stage the Phase 39 read layer: Sensor reveals room 8 into the team map as a
        // remote vault lie, while the live doorframe still advertises the false exit
        // read. A few direct visits add harvested/spent contrast without resolving it.
        runtime.auto_explore = false;
        world.visit(7); // Sensor reveals 4, 6, and the decoy at 8.
        world.visit(0); // Keystone.
        world.visit(5); // Reactor power.
        world.visit(1); // Dead-end/spent contrast.
        world.last_event =
            "Phase 39 capture - room 8 reads E at the door while the map says K.".to_string();
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
    use crate::lab::{DiscUiRoot, DoorReadFrame, RoomTile};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use model::ROOM_COUNT;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(DiscoveryLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_a_tile_per_room_and_a_locked_gate() {
        let mut app = test_app();
        assert_eq!(count::<RoomTile>(&mut app), ROOM_COUNT);
        assert_eq!(count::<DoorReadFrame>(&mut app), ROOM_COUNT);
        assert_eq!(count::<DiscUiRoot>(&mut app), 1);
        let world = app.world().resource::<DiscoveryWorld>();
        assert!(!world.gate_open());
        assert!(world.solvable);
    }

    #[test]
    fn repeated_reset_restores_counts_and_a_fresh_facility() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            {
                let mut world = app.world_mut().resource_mut::<DiscoveryWorld>();
                for _ in 0..4 {
                    if let Some(r) = world.next_unharvested() {
                        world.visit(r);
                    }
                    world.shift();
                }
            }
            app.world_mut()
                .resource_mut::<DiscoveryRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<RoomTile>(&mut app), ROOM_COUNT);
            assert_eq!(count::<DoorReadFrame>(&mut app), ROOM_COUNT);
            assert_eq!(count::<DiscUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<DiscoveryRuntime>().reset_count,
                reset_count
            );
            let world = app.world().resource::<DiscoveryWorld>();
            assert_eq!(world.shift_count, 0);
            assert_eq!(world.visit_count, 0);
            assert!(world.solvable);
            assert!(!world.escaped);
        }
    }

    #[test]
    fn the_systems_run_cleanly_through_a_solved_facility() {
        let mut app = test_app();
        // Drive a full sweep to the win in the model, then let the presentation systems
        // render the solved facility for a few frames and confirm nothing leaks/panics.
        {
            let mut world = app.world_mut().resource_mut::<DiscoveryWorld>();
            let mut guard = 0;
            while !world.gate_open() && guard < 100 {
                let room = world
                    .next_unharvested()
                    .expect("a sweep finds every keystone");
                world.visit(room);
                world.shift();
                guard += 1;
            }
            assert!(world.escape(), "a full sweep unlocks the gate");
        }
        app.update();
        app.update();
        assert!(app.world().resource::<DiscoveryWorld>().escaped);
        assert_eq!(count::<RoomTile>(&mut app), ROOM_COUNT);
        assert_eq!(count::<DoorReadFrame>(&mut app), ROOM_COUNT);
        assert_eq!(count::<DiscUiRoot>(&mut app), 1);
    }
}
