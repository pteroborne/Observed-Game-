//! The prison's gate: where a jailed Observer comes back into the facility, and the way
//! out of the maze that leads there.
//!
//! One model in the equipment's hexagonal language, twice. On the facility's lobby
//! tile it marks where a jailbreak is held, and a column of the prison's light rises in
//! it as the local team's hold fills. In the maze it stands in the hall that leads out.
//! It is a cage: six bronze bars in the Guardian's own trim between two rings, which is
//! how the shape says "prison" before the colour does. The light is
//! [`MarkerRole::Prison`], and nothing else in the facility wears it.

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use observed_hex::{FLOOR_SLAB_TOP, HexCoord, hex_origin};
use observed_match::hex_wfc::LOBBY_HOLD_TICKS;
use observed_style::guardian::{Part, finish};
use observed_style::{MarkerRole, marker};

use super::equipment::{hex_prism, hex_ring, light_tube};
use super::sim::HexWfcRuntime;
use crate::GameState;

/// The cage's radius and height, metres: a body stands inside it.
const RADIUS: f32 = 2.3;
const HEIGHT: f32 = 3.2;
const BAR: f32 = 0.09;

#[derive(Resource)]
pub(in crate::hex_wfc) struct PrisonGateAssets {
    ring: Handle<Mesh>,
    bar: Handle<Mesh>,
    cap: Handle<Mesh>,
    column: Handle<Mesh>,
    bronze: Handle<StandardMaterial>,
    light: Handle<StandardMaterial>,
    haze: Handle<StandardMaterial>,
}

impl FromWorld for PrisonGateAssets {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let ring = meshes.add(hex_ring(RADIUS + 0.12, RADIUS - 0.12, 0.0, 0.06));
        let bar = meshes.add(hex_prism(BAR, BAR, 0.0, HEIGHT));
        let cap = meshes.add(hex_ring(
            RADIUS + 0.12,
            RADIUS - 0.08,
            HEIGHT,
            HEIGHT + 0.14,
        ));
        let column = meshes.add(light_tube(RADIUS - 0.3, 1.0));
        let trim = finish(Part::Trim);
        let prison = marker(MarkerRole::Prison);
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        Self {
            ring,
            bar,
            cap,
            column,
            bronze: materials.add(StandardMaterial {
                base_color: trim.base_color,
                metallic: trim.metallic,
                perceptual_roughness: trim.roughness,
                ..default()
            }),
            light: materials.add(StandardMaterial {
                base_color: prison.base_color,
                emissive: prison.emissive,
                ..default()
            }),
            haze: materials.add(StandardMaterial {
                base_color: Color::LinearRgba(prison.emissive * 0.05),
                alpha_mode: AlphaMode::Add,
                unlit: true,
                cull_mode: None,
                ..default()
            }),
        }
    }
}

/// The lobby's gate in the facility.
#[derive(Component)]
pub(super) struct LobbyGate;

/// The column that fills while the local team holds the lobby.
#[derive(Component)]
pub(super) struct HoldColumn;

/// Spawn a gate standing on the floor of `cell`, as a child of `parent` when given.
/// `hold` gives it a hold column.
pub(in crate::hex_wfc) fn spawn_gate(
    commands: &mut Commands,
    assets: &PrisonGateAssets,
    cell: HexCoord,
    parent: Option<Entity>,
    hold: bool,
) -> Entity {
    let floor = Vec3::from_array(hex_origin(cell)) + Vec3::Y * FLOOR_SLAB_TOP;
    let gate = commands
        .spawn((
            DespawnOnExit(GameState::HexWfc),
            Transform::from_translation(floor),
            Visibility::default(),
            Name::new("Prison gate"),
        ))
        .with_children(|gate| {
            gate.spawn((
                Mesh3d(assets.ring.clone()),
                MeshMaterial3d(assets.light.clone()),
            ));
            gate.spawn((
                Mesh3d(assets.cap.clone()),
                MeshMaterial3d(assets.bronze.clone()),
            ));
            // Bars at the hex's corners, where the equipment's plates put theirs.
            for corner in 0..6u8 {
                let angle = (f32::from(corner) * 60.0 - 30.0).to_radians();
                gate.spawn((
                    Mesh3d(assets.bar.clone()),
                    MeshMaterial3d(assets.bronze.clone()),
                    Transform::from_xyz(RADIUS * angle.cos(), 0.0, RADIUS * angle.sin()),
                ));
            }
            if hold {
                gate.spawn((
                    HoldColumn,
                    Mesh3d(assets.column.clone()),
                    MeshMaterial3d(assets.haze.clone()),
                    Transform::from_xyz(0.0, 0.06, 0.0).with_scale(Vec3::new(1.0, 0.01, 1.0)),
                    Visibility::Hidden,
                    NotShadowCaster,
                    NotShadowReceiver,
                ));
            }
        })
        .id();
    if let Some(parent) = parent {
        commands.entity(parent).add_child(gate);
    }
    gate
}

/// Stand the lobby's gate once the match has a prison, and fill its column with the
/// local team's hold.
pub(super) fn sync_lobby_gate(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    assets: Option<Res<PrisonGateAssets>>,
    gates: Query<(), With<LobbyGate>>,
    mut columns: Query<(&mut Transform, &mut Visibility), With<HoldColumn>>,
) {
    let Some(prison) = runtime.match_state.prison.as_ref() else {
        return;
    };
    // Built on first need, so a match without a prison never makes them.
    let Some(assets) = assets else {
        commands.init_resource::<PrisonGateAssets>();
        return;
    };
    if gates.is_empty() {
        let gate = spawn_gate(&mut commands, &assets, prison.lobby_anchor, None, true);
        commands.entity(gate).insert(LobbyGate);
    }
    let held = prison
        .lobby_hold
        .get(&runtime.local().team)
        .copied()
        .unwrap_or(0);
    let fill = hold_fill(held);
    for (mut transform, mut visibility) in &mut columns {
        transform.scale.y = (fill * HEIGHT).max(0.01);
        *visibility = if fill > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// How full the hold column stands, from the ticks the lobby has been held.
#[must_use]
pub(super) fn hold_fill(held: u16) -> f32 {
    (f32::from(held) / f32::from(LOBBY_HOLD_TICKS)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::hold_fill;
    use observed_match::hex_wfc::LOBBY_HOLD_TICKS;

    #[test]
    fn the_column_fills_with_the_hold_and_stops_full() {
        assert_eq!(hold_fill(0), 0.0);
        assert!((hold_fill(LOBBY_HOLD_TICKS / 2) - 0.5).abs() < 1e-3);
        assert_eq!(hold_fill(LOBBY_HOLD_TICKS), 1.0);
        assert_eq!(hold_fill(LOBBY_HOLD_TICKS * 2), 1.0);
    }
}
