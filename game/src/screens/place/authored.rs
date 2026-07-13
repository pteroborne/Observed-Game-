//! Presentation projection of the exact convex collision snapshot used by Rapier.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use observed_style::{DistrictPalette, SurfaceRole};
use observed_traversal::{ArenaSpec, ColliderShape};
use rapier3d::prelude::{ColliderBuilder, Vector};

use crate::view::components::PlaceGeometry;
use crate::{GameState, view::components::PassagePreview};

use super::factory::place_surface_material;

pub(super) fn spawn_collision_shell(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    arena: &ArenaSpec,
    palette: &DistrictPalette,
    floor_base: &Handle<StandardMaterial>,
    wall_base: &Handle<StandardMaterial>,
) {
    let floor_material = place_surface_material(SurfaceRole::Plain, palette, floor_base, materials);
    let wall_material = place_surface_material(SurfaceRole::Wall, palette, wall_base, materials);
    for collider in &arena.colliders {
        let (mesh, floor_like) = collider_mesh(collider, arena.floor_y);
        commands.spawn((
            PlaceGeometry,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(if floor_like {
                floor_material.clone()
            } else {
                wall_material.clone()
            }),
            Transform::from_translation(collider.center).with_rotation(Quat::from_xyzw(
                collider.rotation[0],
                collider.rotation[1],
                collider.rotation[2],
                collider.rotation[3],
            )),
            Name::new(format!("Authored convex collider {}", collider.id.0)),
        ));
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_preview_collision_shell(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    arena: &ArenaSpec,
    parent: Transform,
    palette: &DistrictPalette,
    floor_base: &Handle<StandardMaterial>,
    wall_base: &Handle<StandardMaterial>,
) {
    let floor_material = place_surface_material(SurfaceRole::Plain, palette, floor_base, materials);
    let wall_material = place_surface_material(SurfaceRole::Wall, palette, wall_base, materials);
    for collider in &arena.colliders {
        let (mesh, floor_like) = collider_mesh(collider, arena.floor_y);
        let local = Transform::from_translation(collider.center).with_rotation(Quat::from_xyzw(
            collider.rotation[0],
            collider.rotation[1],
            collider.rotation[2],
            collider.rotation[3],
        ));
        commands.spawn((
            PlaceGeometry,
            PassagePreview,
            DespawnOnExit(GameState::Match),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(if floor_like {
                floor_material.clone()
            } else {
                wall_material.clone()
            }),
            parent.mul_transform(local),
            Name::new(format!(
                "Frozen authored preview collider {}",
                collider.id.0
            )),
        ));
    }
}

fn collider_mesh(collider: &observed_traversal::ColliderSpec, floor_y: f32) -> (Mesh, bool) {
    match &collider.shape {
        ColliderShape::Cuboid { half } => (
            Mesh::from(Cuboid::new(half.x * 2.0, half.y * 2.0, half.z * 2.0)),
            collider.center.y + half.y <= floor_y + 0.08,
        ),
        ColliderShape::ConvexHull { points } => {
            let max_y = points
                .iter()
                .map(|point| point.y)
                .fold(f32::NEG_INFINITY, f32::max);
            (convex_mesh(points), max_y <= floor_y + 0.08)
        }
    }
}

fn convex_mesh(points: &[Vec3]) -> Mesh {
    let points: Vec<Vector> = points
        .iter()
        .map(|point| Vector::new(point.x, point.y, point.z))
        .collect();
    let collider = ColliderBuilder::convex_hull(&points)
        .expect("content manifest validated convex hull")
        .build();
    let poly = collider
        .shape()
        .as_convex_polyhedron()
        .expect("convex hull builder produces polyhedron topology");
    let mut positions = Vec::<[f32; 3]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut uvs = Vec::<[f32; 2]>::new();
    let mut indices = Vec::<u32>::new();
    for face in poly.faces() {
        let first = face.first_vertex_or_edge as usize;
        let count = face.num_vertices_or_edges as usize;
        let vertices = &poly.vertices_adj_to_face()[first..first + count];
        let base = positions.len() as u32;
        for vertex in vertices {
            positions.push(poly.points()[*vertex as usize].into());
            normals.push(face.normal.into());
            uvs.push([0.0, 0.0]);
        }
        for index in 1..count - 1 {
            indices.extend_from_slice(&[base, base + index as u32, base + index as u32 + 1]);
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
