//! A place's structural shell: floor, ceiling, and walls for the two place shapes
//! (polygon room / box hallway), and the threshold gateways (doorframe + optional
//! sealed leaf) cut into them.

use bevy::prelude::*;
use observed_style::{self as style, MarkerRole};

use crate::GameState;
use crate::layout::{PLACE_TILE, WALL_HEIGHT};
use crate::view::assets::{DOOR_LEAF_D, DOOR_LINTEL_H, MatchAssets};
use crate::view::components::{DoorLeaf, PlaceGeometry};

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
    threshold_style: ThresholdStyle,
    y_offset: f32,
) {
    spawn_place_frame(commands, assets, gap, threshold_style.tethered, y_offset);
    if let Some(leaf_material) = threshold_style.leaf_material {
        spawn_leaf(
            commands,
            assets,
            gap,
            threshold_style.openable,
            leaf_material,
            y_offset,
        );
    }
}

fn gap_yaw(normal: Vec2) -> f32 {
    let along = Vec2::new(-normal.y, normal.x);
    (-along.y).atan2(along.x)
}

/// A neon doorway frame (two posts + a lintel) at a gap, built from primitives and
/// rotated to the gap's wall so it matches the opening at any angle.
fn spawn_place_frame(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &DoorGap,
    tethered: bool,
    y_offset: f32,
) {
    let material = assets.doorframe_material.clone();
    let along = Vec2::new(-gap.normal.y, gap.normal.x);
    let rot = Quat::from_rotation_y(gap_yaw(gap.normal));
    let half = gap.width * 0.5;
    let status = crate::diagnostics::threshold_status(gap, tethered);
    for offset in [half, -half] {
        let p = gap.center + along * offset;
        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            crate::diagnostics::DiagnosticThresholdVisual {
                threshold: gap.threshold,
                kind: crate::diagnostics::DiagnosticThresholdVisualKind::Frame,
                status,
            },
            Mesh3d(assets.placeholder_mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(p.x, y_offset + WALL_HEIGHT * 0.5, p.y).with_scale(Vec3::new(
                0.24,
                WALL_HEIGHT,
                0.24,
            )),
            Name::new("Doorframe post"),
        ));
    }
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        crate::diagnostics::DiagnosticThresholdVisual {
            threshold: gap.threshold,
            kind: crate::diagnostics::DiagnosticThresholdVisualKind::Frame,
            status,
        },
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_xyz(gap.center.x, y_offset + WALL_HEIGHT - 0.2, gap.center.y)
            .with_rotation(rot)
            .with_scale(Vec3::new(gap.width.max(0.3), 0.34, 0.24)),
        Name::new("Doorframe lintel"),
    ));

    let tether_color = if tethered {
        style::marker(MarkerRole::Control).base_color
    } else {
        Color::srgb(0.45, 0.62, 0.78)
    };
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        crate::diagnostics::DiagnosticThresholdVisual {
            threshold: gap.threshold,
            kind: crate::diagnostics::DiagnosticThresholdVisualKind::FrameLight,
            status,
        },
        PointLight {
            color: tether_color,
            intensity: 1_400.0,
            range: 5.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(gap.center.x, y_offset + WALL_HEIGHT - 0.35, gap.center.y),
        Name::new("Doorframe tether light"),
    ));

    if crate::diagnostics::visual_audit_enabled() {
        commands.spawn((
            PlaceGeometry,
            DespawnOnExit(GameState::Match),
            crate::diagnostics::DiagnosticThresholdVisual {
                threshold: gap.threshold,
                kind: crate::diagnostics::DiagnosticThresholdVisualKind::Label,
                status,
            },
            Text2d::new(crate::diagnostics::threshold_label(&gap.threshold)),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Transform::from_xyz(gap.center.x, y_offset + WALL_HEIGHT + 0.35, gap.center.y)
                .with_rotation(rot)
                .with_scale(Vec3::splat(0.035)),
            Name::new("Threshold debug label"),
        ));
    }
}

/// A door leaf filling a sealed doorway gap, flush with the wall.
fn spawn_leaf(
    commands: &mut Commands,
    assets: &MatchAssets,
    gap: &DoorGap,
    openable: bool,
    material: Handle<StandardMaterial>,
    y_offset: f32,
) {
    let rot = Quat::from_rotation_y(gap_yaw(gap.normal));
    let leaf_h = WALL_HEIGHT - DOOR_LINTEL_H;
    let closed_y = y_offset + leaf_h * 0.5;
    let open_y = closed_y + leaf_h;
    commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        crate::diagnostics::DiagnosticThresholdVisual {
            threshold: gap.threshold,
            kind: crate::diagnostics::DiagnosticThresholdVisualKind::Leaf,
            status: crate::diagnostics::threshold_status(gap, false),
        },
        DoorLeaf {
            center: gap.center,
            closed_y,
            open_y,
            openable,
        },
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_xyz(gap.center.x, closed_y, gap.center.y)
            .with_rotation(rot)
            .with_scale(Vec3::new(gap.width.max(0.1), leaf_h, DOOR_LEAF_D)),
        Name::new(if openable { "Door leaf" } else { "Closed door" }),
    ));
}
