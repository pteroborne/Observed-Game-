use crate::GameState;
use crate::layout::WALL_HEIGHT;
use crate::teleport;
use crate::view::assets::MatchAssets;
use crate::view::components::{PassagePreview, PlaceGeometry};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

pub(crate) const SURFACE_UV_REPEAT_PER_METRE: f32 = 0.25;

fn uv_xz(v: Vec2) -> [f32; 2] {
    [
        v.x * SURFACE_UV_REPEAT_PER_METRE,
        v.y * SURFACE_UV_REPEAT_PER_METRE,
    ]
}

/// Build the floor (or ceiling) mesh for a polygon room: a triangle fan from the centre
/// to each vertex, emitted with both windings so it's visible regardless of facing.
pub(crate) fn polygon_mesh(verts: &[Vec2], y: f32, normal_up: bool) -> Mesh {
    let ny = if normal_up { 1.0 } else { -1.0 };
    let mut positions: Vec<[f32; 3]> = vec![[0.0, y, 0.0]];
    let mut normals: Vec<[f32; 3]> = vec![[0.0, ny, 0.0]];
    let mut uvs: Vec<[f32; 2]> = vec![[0.0, 0.0]];
    for v in verts {
        positions.push([v.x, y, v.y]);
        normals.push([0.0, ny, 0.0]);
        uvs.push(uv_xz(*v));
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

pub(crate) fn polygon_ring_mesh(verts: &[Vec2], inner_scale: f32, y: f32, normal_up: bool) -> Mesh {
    let ny = if normal_up { 1.0 } else { -1.0 };
    let mut positions = Vec::with_capacity(verts.len() * 4);
    let mut normals = Vec::with_capacity(verts.len() * 4);
    let mut uvs = Vec::with_capacity(verts.len() * 4);
    let mut indices = Vec::with_capacity(verts.len() * 6);
    for index in 0..verts.len() {
        let a = verts[index];
        let b = verts[(index + 1) % verts.len()];
        let inner_a = a * inner_scale;
        let inner_b = b * inner_scale;
        let start = positions.len() as u32;
        for point in [a, b, inner_b, inner_a] {
            positions.push([point.x, y, point.y]);
            normals.push([0.0, ny, 0.0]);
            uvs.push(uv_xz(point));
        }
        if normal_up {
            indices.extend_from_slice(&[start, start + 2, start + 1, start, start + 3, start + 2]);
        } else {
            indices.extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
        }
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

pub(crate) fn rect_mesh(half: Vec2, y: f32, normal_up: bool) -> Mesh {
    let ny = if normal_up { 1.0 } else { -1.0 };
    let positions = vec![
        [-half.x, y, -half.y],
        [half.x, y, -half.y],
        [half.x, y, half.y],
        [-half.x, y, half.y],
    ];
    let normals = vec![[0.0, ny, 0.0]; 4];
    let uvs = vec![
        uv_xz(Vec2::new(-half.x, -half.y)),
        uv_xz(Vec2::new(half.x, -half.y)),
        uv_xz(Vec2::new(half.x, half.y)),
        uv_xz(Vec2::new(-half.x, half.y)),
    ];
    let indices = if normal_up {
        vec![0, 2, 1, 0, 3, 2]
    } else {
        vec![0, 1, 2, 0, 2, 3]
    };
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

fn push_cuboid_face(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    verts: [[f32; 3]; 4],
    normal: [f32; 3],
    uv_span: Vec2,
) {
    let start = positions.len() as u32;
    positions.extend_from_slice(&verts);
    normals.extend_from_slice(&[normal; 4]);
    uvs.extend_from_slice(&[
        [0.0, 0.0],
        [uv_span.x, 0.0],
        [uv_span.x, uv_span.y],
        [0.0, uv_span.y],
    ]);
    indices.extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
}

pub(crate) fn cuboid_mesh(size: Vec3) -> Mesh {
    let h = size * 0.5;
    let mut positions = Vec::with_capacity(24);
    let mut normals = Vec::with_capacity(24);
    let mut uvs = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    let xy = Vec2::new(size.x, size.y) * SURFACE_UV_REPEAT_PER_METRE;
    let zy = Vec2::new(size.z, size.y) * SURFACE_UV_REPEAT_PER_METRE;
    let xz = Vec2::new(size.x, size.z) * SURFACE_UV_REPEAT_PER_METRE;

    push_cuboid_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [-h.x, -h.y, h.z],
            [h.x, -h.y, h.z],
            [h.x, h.y, h.z],
            [-h.x, h.y, h.z],
        ],
        [0.0, 0.0, 1.0],
        xy,
    );
    push_cuboid_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [h.x, -h.y, -h.z],
            [-h.x, -h.y, -h.z],
            [-h.x, h.y, -h.z],
            [h.x, h.y, -h.z],
        ],
        [0.0, 0.0, -1.0],
        xy,
    );
    push_cuboid_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [h.x, -h.y, h.z],
            [h.x, -h.y, -h.z],
            [h.x, h.y, -h.z],
            [h.x, h.y, h.z],
        ],
        [1.0, 0.0, 0.0],
        zy,
    );
    push_cuboid_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [-h.x, -h.y, -h.z],
            [-h.x, -h.y, h.z],
            [-h.x, h.y, h.z],
            [-h.x, h.y, -h.z],
        ],
        [-1.0, 0.0, 0.0],
        zy,
    );
    push_cuboid_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [-h.x, h.y, h.z],
            [h.x, h.y, h.z],
            [h.x, h.y, -h.z],
            [-h.x, h.y, -h.z],
        ],
        [0.0, 1.0, 0.0],
        xz,
    );
    push_cuboid_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [-h.x, -h.y, -h.z],
            [h.x, -h.y, -h.z],
            [h.x, -h.y, h.z],
            [-h.x, -h.y, h.z],
        ],
        [0.0, -1.0, 0.0],
        xz,
    );

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
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_polygon_shell(
    commands: &mut Commands,
    _assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    poly: &[Vec2],
    floor_material: Handle<StandardMaterial>,
    ceiling_material: Handle<StandardMaterial>,
    xform: Transform,
    preview: bool,
    height: f32,
) {
    let floor = meshes.add(polygon_mesh(poly, 0.0, true));
    let ceiling = meshes.add(polygon_mesh(poly, height, false));
    let floor_name = if preview {
        "Preview floor"
    } else {
        "Place floor"
    };
    let ceiling_name = if preview {
        "Preview ceiling"
    } else {
        "Place ceiling"
    };
    spawn_geo(commands, floor, floor_material, xform, preview, floor_name);
    spawn_geo(
        commands,
        ceiling,
        ceiling_material,
        xform,
        preview,
        ceiling_name,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_polygon_shell_with_aperture(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    poly: &[Vec2],
    floor_material: Handle<StandardMaterial>,
    ceiling_material: Handle<StandardMaterial>,
    xform: Transform,
    preview: bool,
    height: f32,
    aperture_scale: f32,
) {
    let floor = meshes.add(polygon_mesh(poly, 0.0, true));
    let ceiling = meshes.add(polygon_ring_mesh(poly, aperture_scale, height, false));
    spawn_geo(
        commands,
        floor,
        floor_material,
        xform,
        preview,
        if preview {
            "Preview floor"
        } else {
            "Place floor"
        },
    );
    spawn_geo(
        commands,
        ceiling,
        ceiling_material,
        xform,
        preview,
        if preview {
            "Preview ceiling"
        } else {
            "Place ceiling"
        },
    );
}

/// Render the shared threshold-aware boundary plan under `xform`.
///
/// Every threshold cuts the architectural shell. A sealed threshold gets its blocking
/// leaf from the gateway renderer, rather than silently becoming an unbroken wall.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_polygon_walls(
    commands: &mut Commands,
    assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    poly: &[Vec2],
    gaps: &[teleport::DoorGap],
    wall_material: Handle<StandardMaterial>,
    xform: Transform,
    preview: bool,
) {
    let plan = teleport::plan_boundary(poly, gaps, WALL_HEIGHT, WALL_HEIGHT)
        .expect("visible room geometry must produce a valid threshold aperture plan");
    for panel in plan.wall_panels {
        spawn_wall_panel(
            commands,
            assets,
            meshes,
            panel.start,
            panel.end,
            panel.y_min,
            panel.y_max,
            wall_material.clone(),
            xform,
            preview,
        );
    }
}

/// Build a closed convex prism from the same place-local footprint used by collision.
/// Keeping this mesh projection beside the cuboid projection makes faceted columns and
/// platforms visually agree with their authored Rapier hulls.
pub(crate) fn prism_mesh(footprint: &[Vec2], bottom_y: f32, top_y: f32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    let centre = footprint.iter().copied().sum::<Vec2>() / footprint.len() as f32;

    let mut push_triangle = |a: Vec3, b: Vec3, c: Vec3, normal: Vec3| {
        let start = positions.len() as u32;
        for point in [a, b, c] {
            positions.push(point.to_array());
            normals.push(normal.to_array());
            uvs.push(uv_xz(Vec2::new(point.x, point.z)));
        }
        indices.extend_from_slice(&[start, start + 1, start + 2]);
    };

    for index in 0..footprint.len() {
        let a = footprint[index];
        let b = footprint[(index + 1) % footprint.len()];
        let bottom_a = Vec3::new(a.x, bottom_y, a.y);
        let bottom_b = Vec3::new(b.x, bottom_y, b.y);
        let top_a = Vec3::new(a.x, top_y, a.y);
        let top_b = Vec3::new(b.x, top_y, b.y);
        let edge = b - a;
        let side_normal = Vec3::new(edge.y, 0.0, -edge.x).normalize_or_zero();
        push_triangle(bottom_a, bottom_b, top_b, side_normal);
        push_triangle(bottom_a, top_b, top_a, side_normal);

        let top_centre = Vec3::new(centre.x, top_y, centre.y);
        let bottom_centre = Vec3::new(centre.x, bottom_y, centre.y);
        push_triangle(top_centre, top_b, top_a, Vec3::Y);
        push_triangle(bottom_centre, bottom_a, bottom_b, Vec3::NEG_Y);
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

/// A vertical, single-span portal surface. Unlike structural cuboids, its UVs cover the
/// render target exactly once so a camera image can never repeat or crop at the aperture.
pub(crate) fn portal_quad_mesh(size: Vec2) -> Mesh {
    let half = size * 0.5;
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [-half.x, -half.y, 0.0],
            [half.x, -half.y, 0.0],
            [half.x, half.y, 0.0],
            [-half.x, half.y, 0.0],
        ],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4])
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
    )
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_wall_panel(
    commands: &mut Commands,
    _assets: &MatchAssets,
    meshes: &mut Assets<Mesh>,
    p1: Vec2,
    p2: Vec2,
    y_min: f32,
    y_max: f32,
    wall_material: Handle<StandardMaterial>,
    xform: Transform,
    preview: bool,
) {
    const T: f32 = 0.4;
    let d = p2 - p1;
    let len = d.length();
    let height = y_max - y_min;
    if len < 0.05 || height < 0.05 {
        return;
    }
    let mid = (p1 + p2) * 0.5;
    let yaw = (-d.y).atan2(d.x);
    let local = Transform::from_xyz(mid.x, y_min + height * 0.5, mid.y)
        .with_rotation(Quat::from_rotation_y(yaw));
    spawn_geo(
        commands,
        meshes.add(cuboid_mesh(Vec3::new(len + T, height, T))),
        wall_material,
        xform.mul_transform(local),
        preview,
        if preview { "Preview wall" } else { "Room wall" },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    fn uv_span(mesh: &Mesh) -> Vec2 {
        let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("mesh should have Float32x2 UVs");
        };
        let (mut min, mut max) = (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY));
        for uv in uvs {
            let uv = Vec2::new(uv[0], uv[1]);
            min = min.min(uv);
            max = max.max(uv);
        }
        max - min
    }

    #[test]
    fn rectangular_surface_uvs_scale_in_world_units() {
        let mesh = rect_mesh(Vec2::new(20.0, 4.0), 0.0, true);
        let span = uv_span(&mesh);

        assert!((span.x - 10.0).abs() < 0.001, "uv span was {span:?}");
        assert!((span.y - 2.0).abs() < 0.001, "uv span was {span:?}");
    }

    #[test]
    fn polygon_surface_uvs_scale_in_world_units() {
        let mesh = polygon_mesh(
            &[
                Vec2::new(-16.0, -4.0),
                Vec2::new(16.0, -4.0),
                Vec2::new(16.0, 4.0),
                Vec2::new(-16.0, 4.0),
            ],
            0.0,
            true,
        );
        let span = uv_span(&mesh);

        assert!((span.x - 8.0).abs() < 0.001, "uv span was {span:?}");
        assert!((span.y - 2.0).abs() < 0.001, "uv span was {span:?}");
    }
}
