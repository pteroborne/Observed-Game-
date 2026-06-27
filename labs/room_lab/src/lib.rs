mod lab;

// The facility topology was promoted into `crates/observed_facility` (refactor R6).
// Re-export it under the familiar `model`/`world` paths so this lab — and the labs that
// reuse it via `room_lab::*` — keep the same imports; this lab is the debug projection.
pub mod model {
    pub use observed_facility::room_def::*;
}
pub mod world {
    pub use observed_facility::room_world::*;
}

use bevy::{
    input::InputSystems,
    prelude::*,
    window::{PresentMode, WindowResolution},
};

pub use lab::RoomLabRuntime;
pub use model::*;
pub use world::*;

pub struct RoomLabPlugin;

impl Plugin for RoomLabPlugin {
    fn build(&self, app: &mut App) {
        let registry = RoomRegistry::default();
        let world = RoomWorld::authored_facility(&registry);
        app.insert_resource(registry)
            .insert_resource(world)
            .init_resource::<RoomLabRuntime>()
            .add_systems(Startup, (setup_camera, lab::setup_ui))
            .add_systems(
                Update,
                (
                    lab::handle_shortcuts.after(InputSystems),
                    lab::perform_reset,
                    lab::sync_room_visuals,
                    ApplyDeferred,
                    lab::update_ui,
                    lab::draw_debug,
                )
                    .chain(),
            );
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: 1.24,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, -30.0, 1000.0),
        Name::new("Room Lab Camera"),
    ));
}

pub fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.012, 0.022, 0.035)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Room Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(RoomLabPlugin)
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::{RoomLabUiRoot, RoomOwned, RoomVisualRoot};
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(RoomLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    fn roots_for(app: &mut App, room: RoomId) -> Vec<Entity> {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &RoomVisualRoot)>();
        query
            .iter(world)
            .filter(|(_, root)| root.0 == room)
            .map(|(entity, _)| entity)
            .collect()
    }

    fn owned_for(app: &mut App, room: RoomId) -> usize {
        let world = app.world_mut();
        let mut query = world.query::<&RoomOwned>();
        query.iter(world).filter(|owned| owned.0 == room).count()
    }

    #[test]
    fn visual_sync_cleans_despawned_and_replaced_room_entities() {
        let mut app = test_app();
        assert_eq!(count::<RoomVisualRoot>(&mut app), 8);
        let target = RoomId(4);
        let old_root = roots_for(&mut app, target)[0];
        let old_owned = owned_for(&mut app, target);
        assert!(old_owned > 1);

        {
            let registry = app.world().resource::<RoomRegistry>().clone();
            app.world_mut()
                .resource_mut::<RoomWorld>()
                .replace_room(&registry, target, RoomTemplate::StraightCorridor)
                .unwrap();
        }
        app.update();
        let new_roots = roots_for(&mut app, target);
        assert_eq!(new_roots.len(), 1);
        assert_ne!(new_roots[0], old_root);

        app.world_mut()
            .resource_mut::<RoomWorld>()
            .despawn_room(target);
        app.update();
        assert!(roots_for(&mut app, target).is_empty());
        assert_eq!(owned_for(&mut app, target), 0);
        assert_eq!(count::<RoomVisualRoot>(&mut app), 7);
    }

    #[test]
    fn repeated_reset_restores_exact_visual_and_logical_baseline() {
        let mut app = test_app();
        for reset_count in 1..=10 {
            {
                let mut world = app.world_mut().resource_mut::<RoomWorld>();
                world.despawn_room(RoomId(2));
            }
            app.world_mut()
                .resource_mut::<RoomLabRuntime>()
                .reset_requested = true;
            app.update();

            assert_eq!(app.world().resource::<RoomWorld>().rooms.len(), 8);
            assert_eq!(app.world().resource::<RoomWorld>().connections.len(), 7);
            assert_eq!(count::<RoomVisualRoot>(&mut app), 8);
            assert_eq!(count::<RoomLabUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<RoomLabRuntime>().reset_count,
                reset_count
            );
            for room in 0..8 {
                assert_eq!(roots_for(&mut app, RoomId(room)).len(), 1);
                assert!(owned_for(&mut app, RoomId(room)) > 1);
            }
        }
    }
}
