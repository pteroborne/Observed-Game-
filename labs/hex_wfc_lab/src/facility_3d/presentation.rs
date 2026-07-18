//! Exact collider-list rendering and semantic Phase 92 materials.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use observed_match::hex_wfc::{HexStructurePiece, HexStructureRole};
use observed_style::SurfaceRole;
use observed_traversal::ColliderShape;
use rapier3d::prelude::{SharedShape, Vector as RapierVector};

use super::{FacilityState, FacilityVisual};

pub(super) fn rebuild_geometry(
    mut commands: Commands,
    mut state: ResMut<FacilityState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    visuals: Query<Entity, (With<FacilityVisual>, Without<DirectionalLight>)>,
) {
    if !state.dirty {
        return;
    }
    for entity in &visuals {
        commands.entity(entity).despawn();
    }
    let pieces = state.snapshot.pieces.clone();
    for piece in &pieces {
        let Some(mesh) = piece_mesh(piece) else {
            continue;
        };
        let treatment = observed_style::surface(surface_role(piece.role));
        let semantic_color = treatment.edge.unwrap_or(treatment.base_color);
        let material = if state.collider_view {
            StandardMaterial {
                // Both tones are style-owned. Alternating by stable ID makes
                // individual authored hull boundaries explicit without
                // inventing another gameplay colour.
                base_color: if piece.id.0 % 2 == 0 {
                    semantic_color
                } else {
                    treatment.base_color
                },
                // Debug draws the exact collider mesh, but keeps lighting so
                // adjacent coplanar hulls remain visually separable.
                emissive: treatment.emissive * 0.02,
                perceptual_roughness: 1.0,
                ..default()
            }
        } else {
            StandardMaterial {
                base_color: semantic_color,
                // The authored hulls have no baked lightmaps. Keep their
                // semantic treatment legible at first-person scale.
                emissive: treatment.emissive * 0.12,
                perceptual_roughness: 0.91,
                ..default()
            }
        };
        commands.spawn((
            FacilityVisual,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(material)),
            Transform::from_translation(piece.center)
                .with_rotation(Quat::from_array(piece.rotation)),
            Name::new(format!("Hex {:?} collider {}", piece.role, piece.id.0)),
        ));
    }
    state.dirty = false;
}

fn surface_role(role: HexStructureRole) -> SurfaceRole {
    match role {
        HexStructureRole::Room => SurfaceRole::Spine,
        HexStructureRole::Hall => SurfaceRole::GantryDeck,
        HexStructureRole::Ramp => SurfaceRole::SafeBypass,
        HexStructureRole::Shaft => SurfaceRole::WellshaftStone,
        HexStructureRole::Boundary => SurfaceRole::Plain,
    }
}

fn piece_mesh(piece: &HexStructurePiece) -> Option<Mesh> {
    match &piece.shape {
        ColliderShape::Cuboid { half } => Some(Cuboid::from_size(*half * 2.0).mesh().build()),
        ColliderShape::ConvexHull { points } => hull_mesh(points),
    }
}

fn hull_mesh(hull: &[Vec3]) -> Option<Mesh> {
    let points: Vec<_> = hull
        .iter()
        .map(|point| RapierVector::new(point.x, point.y, point.z))
        .collect();
    let shape = SharedShape::convex_hull(&points)?;
    let (vertices, indices) = shape.as_convex_polyhedron()?.to_trimesh();
    let positions: Vec<[f32; 3]> = vertices
        .iter()
        .map(|point| [point.x, point.y, point.z])
        .collect();
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices.into_iter().flatten().collect()))
        .with_duplicated_vertices()
        .with_computed_flat_normals(),
    )
}
