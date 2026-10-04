//! Decorative conveyor electricity, owned by streamed decks, with no simulation writes.
use super::ConveyorDeck;
use crate::{
    GameState,
    hex_wfc::{sim::HexWfcRuntime, view::spectate::Cutaway},
};
use bevy::{
    asset::RenderAssetUsages,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};
use observed_hex::hex_origin;
use observed_style::chargeworks as style;
use std::collections::BTreeMap;

#[derive(Clone, ShaderType)]
struct FieldSettings {
    tint: Vec4,
    edge: Vec4,
    /// x: floor power fraction, y: visual drift speed, zw: reserved.
    params: Vec4,
}
#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct FieldMaterial {
    #[uniform(0)]
    settings: FieldSettings,
}
impl Material for FieldMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/chargeworks_field.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}
#[derive(Resource, Default)]
struct FieldAssets {
    floors: BTreeMap<u8, Handle<FieldMaterial>>,
}

pub(super) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<FieldMaterial>::default())
        .add_systems(OnEnter(GameState::HexWfc), setup)
        .add_systems(
            PostUpdate,
            (spawn, sync_power)
                .chain()
                .before(bevy::transform::TransformSystems::Propagate)
                .run_if(in_state(GameState::HexWfc)),
        )
        .add_systems(OnExit(GameState::HexWfc), cleanup);
}
fn setup(mut commands: Commands) {
    commands.insert_resource(FieldAssets::default());
}
fn cleanup(mut commands: Commands) {
    commands.remove_resource::<FieldAssets>();
}
fn linear(color: Color) -> Vec4 {
    let c = LinearRgba::from(color);
    Vec4::new(c.red, c.green, c.blue, 1.0)
}
fn material() -> FieldMaterial {
    FieldMaterial {
        settings: FieldSettings {
            tint: linear(style::field_color()),
            edge: linear(style::field_edge_color()),
            params: Vec4::new(1.0, style::FIELD_SPEED, 0.0, 0.0),
        },
    }
}
fn mesh(length: f32) -> Mesh {
    let half = length * 0.5;
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [-half, 0.0, -0.75],
            [-half, 0.0, 0.75],
            [half, 0.0, 0.75],
            [half, 0.0, -0.75],
        ],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 4])
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 0.0], [0.0, 1.0], [length, 1.0], [length, 0.0]],
    )
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
}
fn spawn(
    mut commands: Commands,
    decks: Query<(Entity, &ConveyorDeck, &Transform), Added<ConveyorDeck>>,
    mut assets: ResMut<FieldAssets>,
    mut materials: ResMut<Assets<FieldMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (parent, deck, pose) in &decks {
        let material = assets
            .floors
            .entry(deck.coord.level)
            .or_insert_with(|| materials.add(material()))
            .clone();
        let origin = Vec3::from_array(hex_origin(deck.coord));
        let local = pose.translation - origin + Vec3::Y * 0.035;
        commands.spawn((
            Mesh3d(meshes.add(mesh(deck.length))),
            MeshMaterial3d(material),
            Transform::from_xyz(0.0, 0.035, 0.0),
            ChildOf(parent),
            NotShadowCaster,
            NotShadowReceiver,
            Cutaway {
                local,
                min_y: local.y,
                max_y: local.y,
                origin_y: origin.y,
                cell_level: deck.coord.level,
                climb_wall: false,
            },
            Name::new("Decorative conveyor electric field"),
        ));
    }
}
fn sync_power(
    runtime: Res<HexWfcRuntime>,
    assets: Res<FieldAssets>,
    mut materials: ResMut<Assets<FieldMaterial>>,
) {
    for (&level, handle) in &assets.floors {
        let powered = runtime
            .ascent
            .as_ref()
            .is_none_or(|a| a.rules().economy.is_powered(level));
        if let Some(mut material) = materials.get_mut(handle) {
            let fraction = if powered { 1.0 } else { style::FIELD_EMERGENCY };
            if material.settings.params.x != fraction {
                material.settings.params.x = fraction;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn electric_fields_spawn_once_and_despawn_with_their_streamed_decks() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<FieldMaterial>>()
            .init_resource::<FieldAssets>()
            .add_systems(Update, spawn);
        let deck = app
            .world_mut()
            .spawn((
                ConveyorDeck {
                    coord: observed_hex::HexCoord {
                        q: 1,
                        r: 1,
                        level: 2,
                    },
                    length: 7.0,
                },
                Transform::from_xyz(14.0, 16.76, 0.0),
            ))
            .id();
        app.update();
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query::<&MeshMaterial3d<FieldMaterial>>()
                .iter(world)
                .count(),
            1
        );
        assert_eq!(world.resource::<FieldAssets>().floors.len(), 1);
        let cutaway = world.query::<&Cutaway>().single(world).unwrap();
        assert_eq!(cutaway.cell_level, 2);
        world.entity_mut(deck).despawn();
        assert_eq!(
            world
                .query::<&MeshMaterial3d<FieldMaterial>>()
                .iter(world)
                .count(),
            0
        );
    }

    #[test]
    fn electric_fields_leave_no_cached_handles_after_exit_and_reentry() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<GameState>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<FieldMaterial>>()
            .add_systems(OnEnter(GameState::HexWfc), setup)
            .add_systems(OnExit(GameState::HexWfc), cleanup)
            .add_systems(Update, spawn.run_if(in_state(GameState::HexWfc)));
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::HexWfc);
        app.update();
        app.world_mut().spawn((
            ConveyorDeck {
                coord: observed_hex::HexCoord {
                    q: 1,
                    r: 1,
                    level: 0,
                },
                length: 7.0,
            },
            Transform::default(),
            DespawnOnExit(GameState::HexWfc),
        ));
        app.update();
        assert_eq!(app.world().resource::<FieldAssets>().floors.len(), 1);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::MainMenu);
        app.update();
        assert!(!app.world().contains_resource::<FieldAssets>());
        let world = app.world_mut();
        assert_eq!(
            world
                .query::<&MeshMaterial3d<FieldMaterial>>()
                .iter(world)
                .count(),
            0
        );
        world
            .resource_mut::<NextState<GameState>>()
            .set(GameState::HexWfc);
        app.update();
        assert!(app.world().resource::<FieldAssets>().floors.is_empty());
    }
}
