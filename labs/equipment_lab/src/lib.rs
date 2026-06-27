mod input;
mod lab;

// The equipment persistence model was promoted into `crates/observed_interaction`
// (refactor R8). Re-export it under the familiar `model` path so this lab's presentation
// is unchanged; the lab is the debug projection.
pub mod model {
    pub use observed_interaction::equipment::*;
}

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::EquipmentLabRuntime;
pub use model::{
    Equipment, EquipmentEvent, EquipmentKind, EquipmentLocation, EquipmentWorld, Room, Socket,
    SocketId,
};

pub struct EquipmentLabPlugin;

impl Plugin for EquipmentLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(EquipmentWorld::authored_lab())
            .init_resource::<EquipmentLabRuntime>()
            .init_resource::<input::HumanInput>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(PreUpdate, input::sample_hardware.after(InputSystems))
            .add_systems(
                Update,
                (
                    lab::handle_shortcuts,
                    lab::apply_input,
                    lab::tick_power,
                    lab::perform_reset,
                    lab::sync_equipment_visuals,
                    lab::sync_player_visuals,
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
            scale: 1.35,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Name::new("Equipment Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.012, 0.022, 0.035)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Equipment Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EquipmentLabPlugin);

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, taken: false })
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
    use crate::lab::{EquipLabUiRoot, EquipmentVisual, PlayerVisualE};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use observed_core::{EquipmentId, PlayerId};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(EquipmentLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_a_visual_per_equipment_and_present_player() {
        let mut app = test_app();
        assert_eq!(count::<EquipmentVisual>(&mut app), 5);
        assert_eq!(count::<PlayerVisualE>(&mut app), 4);
        assert_eq!(count::<EquipLabUiRoot>(&mut app), 1);
    }

    #[test]
    fn despawning_visuals_does_not_lose_logical_equipment() {
        let mut app = test_app();

        // Wipe every equipment visual — the logical world is untouched.
        let visuals: Vec<Entity> = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<EquipmentVisual>>();
            query.iter(world).collect()
        };
        for entity in visuals {
            app.world_mut().entity_mut(entity).despawn();
        }
        assert_eq!(count::<EquipmentVisual>(&mut app), 0);
        assert_eq!(app.world().resource::<EquipmentWorld>().equipment.len(), 5);

        // The sync system rebuilds the projection from logical state.
        app.update();
        assert_eq!(count::<EquipmentVisual>(&mut app), 5);
    }

    #[test]
    fn player_visual_disappears_when_the_carrier_leaves_but_equipment_remains() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<EquipmentWorld>()
            .set_player_present(PlayerId(0), false);
        app.update();

        assert_eq!(count::<PlayerVisualE>(&mut app), 3);
        // The grapple device P1 was carrying still exists, now on the ground.
        let world = app.world().resource::<EquipmentWorld>();
        assert_eq!(world.equipment.len(), 5);
        assert!(matches!(
            world.equipment(EquipmentId(4)).unwrap().location,
            EquipmentLocation::Ground { .. }
        ));
        assert_eq!(count::<EquipmentVisual>(&mut app), 5);
    }

    #[test]
    fn repeated_reset_restores_counts_and_baseline() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            {
                let mut world = app.world_mut().resource_mut::<EquipmentWorld>();
                world.replace_room(observed_core::RoomId(0));
                world.set_player_present(PlayerId(1), false);
            }
            app.world_mut()
                .resource_mut::<EquipmentLabRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(count::<EquipmentVisual>(&mut app), 5);
            assert_eq!(count::<PlayerVisualE>(&mut app), 4);
            assert_eq!(count::<EquipLabUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<EquipmentLabRuntime>().reset_count,
                reset_count
            );
            let world = app.world().resource::<EquipmentWorld>();
            assert_eq!(world.room_replacements, 0);
            assert!(world.room_powered(observed_core::RoomId(0)));
        }
    }
}
