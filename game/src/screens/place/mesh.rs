use crate::GameState;
use crate::layout::WALL_HEIGHT;
use crate::teleport;
use crate::view::assets::MatchAssets;
use crate::view::components::{PassagePreview, PlaceGeometry};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// Build the floor (or ceiling) mesh for a polygon room: a triangle fan from the centre
/// to each vertex, emitted with both windings so it's visible regardless of facing.
pub(crate) fn polygon_mesh(verts: &[Vec2], y: f32, normal_up: bool) -> Mesh {
    let ny = if normal_up { 1.0 } else { -1.0 };
    let mut positions: Vec<[f32; 3]> = vec![[0.0, y, 0.0]];
    let mut normals: Vec<[f32; 3]> = vec![[0.0, ny, 0.0]];
    let mut uvs: Vec<[f32; 2]> = vec![[0.5, 0.5]];
    for v in verts {
        positions.push([v.x, y, v.y]);
        normals.push([0.0, ny, 0.0]);
        uvs.push([0.5 + v.x * 0.04, 0.5 + v.y * 0.04]);
    }
    let n = verts.len() as u32;
    let mut indices: Vec<u32> = Vec::new();
    for i in 0..n {
        let a = 1 + i;
        let b = 1 + (i + 1) % n;
        indices.extend_from_slice(&[0, a, b, 0, b, a]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// Spawn one piece of place geometry at `transform`, marking it a [`PassagePreview`]
/// when `preview` (so previews can be queried/tested); either way it is despawned with
/// the rest of the place geometry on the next teleport.
pub(crate) fn spawn_geo(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    transform: Transform,
    preview: bool,
    name: &'static str,
) {
    let mut entity = commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        Mesh3d(mesh),
        MeshMaterial3d(material),
        transform,
        Name::new(name),
    ));
    if preview {
        entity.insert(PassagePreview);
    }
}

/// The floor + ceiling of a polygon room (custom fan meshes matching the footprint),
/// placed under `xform` (identity for the current place, the alignment transform for a
/// preview).
pub(crate) fn spawn_polygon_shell(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    poly: &[Vec2],
    floor_material: Handle<StandardMaterial>,
    xform: Transform,
    preview: bool,
) {
    let floor = meshes.add(polygon_mesh(poly, 0.0, true));
    let ceiling = meshes.add(polygon_mesh(poly, WALL_HEIGHT, false));
    spawn_geo(
        commands,
        floor,
        floor_material,
        xform,
        preview,
        "Place floor",
    );
    spawn_geo(
        commands,
        ceiling,
        assets.ceiling_material.clone(),
        xform,
        preview,
        "Place ceiling",
    );
}

/// One angled wall panel per polygon edge, split around any doorway `open` returns true
/// for (so the body can walk through it / you can see in), placed under `xform`. Edges
/// with no open doorway are a solid wall.
pub(crate) fn spawn_polygon_walls(
    commands: &mut Commands,
    assets: &MatchAssets,
    poly: &[Vec2],
    gaps: &[teleport::DoorGap],
    xform: Transform,
    preview: bool,
    open: impl Fn(&teleport::DoorGap) -> bool,
) {
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let mid = (a + b) * 0.5;
        let gap = gaps.iter().find(|g| (g.center - mid).length() < 0.05);
        match gap {
            Some(g) if open(g) => {
                let dir = (b - a).normalize_or_zero();
                spawn_wall_segment(
                    commands,
                    assets,
                    a,
                    g.center - dir * (g.width * 0.5),
                    xform,
                    preview,
                );
                spawn_wall_segment(
                    commands,
                    assets,
                    g.center + dir * (g.width * 0.5),
                    b,
                    xform,
                    preview,
                );
            }
            _ => spawn_wall_segment(commands, assets, a, b, xform, preview),
        }
    }
}

/// A single rotated wall panel spanning `p1`→`p2` (extended slightly so corners seal),
/// placed under `xform`.
pub(crate) fn spawn_wall_segment(
    commands: &mut Commands,
    assets: &MatchAssets,
    p1: Vec2,
    p2: Vec2,
    xform: Transform,
    preview: bool,
) {
    const T: f32 = 0.4; // full thickness
    let d = p2 - p1;
    let len = d.length();
    if len < 0.05 {
        return;
    }
    let mid = (p1 + p2) * 0.5;
    let yaw = (-d.y).atan2(d.x); // align local +X with the edge direction
    let local = Transform::from_xyz(mid.x, WALL_HEIGHT * 0.5, mid.y)
        .with_rotation(Quat::from_rotation_y(yaw))
        .with_scale(Vec3::new(len + T, WALL_HEIGHT, T));
    spawn_geo(
        commands,
        assets.placeholder_mesh.clone(),
        assets.wall_material.clone(),
        xform.mul_transform(local),
        preview,
        "Room wall",
    );
}
