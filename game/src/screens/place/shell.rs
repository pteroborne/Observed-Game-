//! A place's structural shell: floor, ceiling, and walls for the two place shapes
//! (polygon room / box hallway), and the threshold gateways (doorframe + optional
//! sealed leaf) cut into them.

use bevy::prelude::*;

use crate::GameState;
use crate::layout::{PLACE_TILE, WALL_HEIGHT};
use crate::view::assets::MatchAssets;
use crate::view::components::PlaceGeometry;

use super::mesh;
use crate::teleport::{DoorGap, PlaceGeom};

/// A polygon room's shell: the extruded floor/ceiling shell plus per-edge walls with
/// passage openings left open.
pub(crate) fn spawn_room_shell(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    geom: &PlaceGeom,
    floor_material: Handle<StandardMaterial>,
    y_offset: f32,
) {
    if let Some(poly) = geom.poly.as_ref() {
        let root_xform = Transform::from_xyz(0.0, y_offset, 0.0);
        mesh::spawn_polygon_shell(
            commands,
            assets,
            meshes,
            poly,
            floor_material,
            root_xform,
            false,
        );
        mesh::spawn_polygon_walls(commands, assets, poly, &geom.gaps, root_xform, false, |g| {
            g.kind.is_passage()
        });
    }
}

/// A box hallway's shell: scaled floor/ceiling planes plus the collision arena's
/// interior maze walls rendered as solid blocks.
pub(crate) fn spawn_hallway_shell(
    commands: &mut Commands,
    assets: &MatchAssets,
    geom: &PlaceGeom,
    floor_material: Handle<StandardMaterial>,
    solids: &[observed_traversal::Aabb3],
    y_offset: f32,
) {
    let (hx, hz) = (geom.half.x, geom.half.y);
    let plane_scale = Vec3::new(hx * 2.0 / PLACE_TILE, 1.0, hz * 2.0 / PLACE_TILE);
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.floor_mesh.clone()),
        MeshMaterial3d(floor_material),
        Transform::from_xyz(0.0, y_offset, 0.0).with_scale(plane_scale),
        Name::new("Place floor"),
    ));
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.ceiling_mesh.clone()),
        MeshMaterial3d(assets.ceiling_material.clone()),
        Transform::from_xyz(0.0, y_offset + WALL_HEIGHT, 0.0)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::PI))
            .with_scale(plane_scale),
        Name::new("Place ceiling"),
    ));
    for solid in solids {
        let center = (solid.min + solid.max) * 0.5;
        let size = solid.max - solid.min;
        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(assets.wall_material.clone()),
            Transform::from_translation(center).with_scale(size),
            Name::new("Place wall"),
        ));
    }
}

/// How a threshold gateway is dressed: an optional sealed leaf (and whether it can
/// slide open), plus the tether light on the frame. Three kinds of threshold exist —
/// open passages, the locked exit, and sealed side doors — each a constructor here.
pub(crate) struct ThresholdStyle {
    leaf_material: Option<Handle<StandardMaterial>>,
    openable: bool,
    tethered: bool,
}

impl ThresholdStyle {
    /// An always-open passage: frame only, no hiding leaf.
    pub(crate) fn passage(tethered: bool) -> Self {
        Self {
            leaf_material: None,
            openable: false,
            tethered,
        }
    }

    /// The keystone-locked exit: sealed with the armed-trap red leaf.
    pub(crate) fn locked_exit(assets: &MatchAssets, tethered: bool) -> Self {
        Self {
            leaf_material: Some(assets.trap_active_material.clone()),
            openable: false,
            tethered,
        }
    }

    /// A sealed side door (decor / future route): a plain closed leaf, never tethered.
    pub(crate) fn side_door(assets: &MatchAssets) -> Self {
        Self {
            leaf_material: Some(assets.door_leaf_material.clone()),
            openable: false,
            tethered: false,
        }
    }
}

/// Spawn one threshold gateway: the frame + lintel + tether light, and — for sealed
/// styles — the closed leaf filling the opening.
pub(crate) fn spawn_threshold_gateway(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &DoorGap,
    style: ThresholdStyle,
    y_offset: f32,
) {
    super::spawn_place_frame(commands, assets, gap, style.tethered, y_offset);
    if let Some(leaf_material) = style.leaf_material {
        super::spawn_leaf(
            commands,
            assets,
            gap,
            style.openable,
            leaf_material,
            y_offset,
        );
    }
}
