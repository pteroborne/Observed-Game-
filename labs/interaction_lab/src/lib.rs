mod input;
mod lab;

// The interaction model + engine were promoted into `crates/observed_interaction`
// (refactor R8). Re-export them under the familiar `model`/`engine` paths so this lab's
// presentation is unchanged; the lab is the debug projection.
pub mod model {
    pub use observed_interaction::interaction::model::*;
}
pub mod engine {
    pub use observed_interaction::interaction::engine::*;
}

use bevy::{
    input::InputSystems,
    prelude::*,
    window::{PresentMode, WindowResolution},
};

pub use engine::{InteractionPrompt, prompt_for_player, tick_interactions};
pub use lab::InteractionLabRuntime;
pub use model::{
    EquipmentId, InteractionEvent, InteractionId, InteractionKind, InteractionObject,
    InteractionPlayer, InteractionPolicy, InteractionWorld, ItemLocation, SocketId,
};

pub struct InteractionLabPlugin;

impl Plugin for InteractionLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(InteractionWorld::authored_lab())
            .init_resource::<InteractionLabRuntime>()
            .init_resource::<input::InteractionInput>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(PreUpdate, input::sample_hardware.after(InputSystems))
            .add_systems(
                Update,
                (
                    input::distribute_intents,
                    lab::handle_shortcuts,
                    lab::perform_reset,
                    lab::move_players,
                    lab::simulate_interactions,
                    lab::update_last_event,
                    lab::present,
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
            scale: 1.18,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Name::new("Interaction Lab Camera"),
    ));
}

pub fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.012, 0.022, 0.035)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Interaction Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(InteractionLabPlugin)
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        input::InteractionInput,
        lab::{InteractionLabUiRoot, InteractionPlayerVisual},
        model::PLAYER_COUNT,
    };
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use observed_core::PlayerId;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(InteractionLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn repeated_reset_preserves_entities_and_clears_interaction_state() {
        let mut app = test_app();
        let players = count::<InteractionPlayerVisual>(&mut app);
        let roots = count::<InteractionLabUiRoot>(&mut app);
        assert_eq!(players, PLAYER_COUNT);
        assert_eq!(roots, 1);

        for reset_count in 1..=10 {
            {
                let world = app.world_mut();
                {
                    let mut simulation = world.resource_mut::<InteractionWorld>();
                    simulation.player_mut(PlayerId(0)).unwrap().carrying = Some(EquipmentId(0));
                    simulation.total_events = 9;
                }
                world
                    .resource_mut::<InteractionLabRuntime>()
                    .reset_requested = true;
            }
            app.update();

            assert_eq!(count::<InteractionPlayerVisual>(&mut app), players);
            assert_eq!(count::<InteractionLabUiRoot>(&mut app), roots);
            assert_eq!(
                app.world().resource::<InteractionLabRuntime>().reset_count,
                reset_count
            );
            assert_eq!(app.world().resource::<InteractionWorld>().total_events, 0);
            assert!(
                app.world()
                    .resource::<InteractionWorld>()
                    .players
                    .iter()
                    .all(|player| player.carrying.is_none() && player.active_target.is_none())
            );
            assert_eq!(
                *app.world().resource::<InteractionInput>(),
                InteractionInput::default()
            );
        }
    }
}
