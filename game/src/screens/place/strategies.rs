use crate::GameState;
use crate::layout::WALL_HEIGHT;
use crate::screens::place::mesh;
use crate::teleport::{DoorGap, PlaceGeom};
use crate::view::assets::MatchAssets;
use crate::view::components::PlaceGeometry;
use bevy::prelude::*;

// ==========================================
// 1. STRATEGY PATTERN (Place Geometry Spawning)
// ==========================================

pub(crate) trait SpawningStrategy {
    #[allow(clippy::too_many_arguments)]
    fn spawn_geometry(
        &self,
        commands: &mut Commands,
        assets: &MatchAssets,
        meshes: &mut Assets<Mesh>,
        geom: &PlaceGeom,
        floor_material: Handle<StandardMaterial>,
        solids: &[observed_traversal::Aabb3],
        y_offset: f32,
    );
}

pub(crate) struct RoomSpawningStrategy;
impl SpawningStrategy for RoomSpawningStrategy {
    fn spawn_geometry(
        &self,
        commands: &mut Commands,
        assets: &MatchAssets,
        meshes: &mut Assets<Mesh>,
        geom: &PlaceGeom,
        floor_material: Handle<StandardMaterial>,
        _solids: &[observed_traversal::Aabb3],
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
}

pub(crate) struct HallwaySpawningStrategy;
impl SpawningStrategy for HallwaySpawningStrategy {
    fn spawn_geometry(
        &self,
        commands: &mut Commands,
        assets: &MatchAssets,
        _meshes: &mut Assets<Mesh>,
        geom: &PlaceGeom,
        floor_material: Handle<StandardMaterial>,
        solids: &[observed_traversal::Aabb3],
        y_offset: f32,
    ) {
        let (hx, hz) = (geom.half.x, geom.half.y);
        let plane_scale = Vec3::new(
            hx * 2.0 / crate::layout::PLACE_TILE,
            1.0,
            hz * 2.0 / crate::layout::PLACE_TILE,
        );
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
}

// ==========================================
// 2. TEMPLATE METHOD PATTERN (Gateway Policies)
// ==========================================

pub(crate) trait GatewayPolicy {
    fn leaf_material(&self, assets: &MatchAssets) -> Option<Handle<StandardMaterial>>;
    fn openable(&self) -> bool;
    fn tethered(&self) -> bool;
}

pub(crate) struct PassagePolicy {
    pub tethered: bool,
}
impl GatewayPolicy for PassagePolicy {
    fn leaf_material(&self, _assets: &MatchAssets) -> Option<Handle<StandardMaterial>> {
        None
    }
    fn openable(&self) -> bool {
        false
    }
    fn tethered(&self) -> bool {
        self.tethered
    }
}

pub(crate) struct LockedExitPolicy {
    pub tethered: bool,
}
impl GatewayPolicy for LockedExitPolicy {
    fn leaf_material(&self, assets: &MatchAssets) -> Option<Handle<StandardMaterial>> {
        Some(assets.trap_active_material.clone())
    }
    fn openable(&self) -> bool {
        false
    }
    fn tethered(&self) -> bool {
        self.tethered
    }
}

pub(crate) struct SideDoorPolicy;
impl GatewayPolicy for SideDoorPolicy {
    fn leaf_material(&self, assets: &MatchAssets) -> Option<Handle<StandardMaterial>> {
        Some(assets.door_leaf_material.clone())
    }
    fn openable(&self) -> bool {
        false
    }
    fn tethered(&self) -> bool {
        false
    }
}

pub(crate) fn spawn_threshold_gateway<P: GatewayPolicy>(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &DoorGap,
    policy: P,
    y_offset: f32,
) {
    // 1. Frame, Lintel, and Tether light (common to all gateway templates)
    super::spawn_place_frame(commands, assets, gap, policy.tethered(), y_offset);

    // 2. Closed door leaf (templated based on leaf_material presence)
    if let Some(leaf_mat) = policy.leaf_material(assets) {
        super::spawn_leaf(commands, assets, gap, policy.openable(), leaf_mat, y_offset);
    }
}
