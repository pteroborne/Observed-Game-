//! Authored geometry shared by the replay and the live survivor map.
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use observed_match::hex_wfc::HexStructurePiece;
use observed_traversal::{ColliderShape, ConvexRenderMesh};

/// Floor membership and camera-facing wall removal use the replay's exact policy.
pub(crate) fn surface(
    piece: &HexStructurePiece,
    level: u8,
    bearing: Vec2,
    eyes: bool,
) -> Option<bool> {
    if !piece.part.drawn() {
        return None;
    }
    let (min_y, max_y, local, floor) = match &piece.shape {
        ColliderShape::ConvexHull { points } => (
            points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min),
            points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max),
            points.iter().copied().sum::<Vec3>() / points.len().max(1) as f32,
            observed_traversal::render_mesh::is_horizontal_slab(points),
        ),
        ColliderShape::Cuboid { half } => (-half.y, half.y, Vec3::ZERO, half.y < 0.5),
    };
    if !eyes {
        if piece.surface == Some(observed_authoring::HullSurface::Ceiling) {
            return None;
        }
        let low = f32::from(level) * observed_hex::TILE_LEVEL_HEIGHT;
        let middle = piece.center.y + (min_y + max_y) * 0.5;
        if middle < low
            || middle >= low + observed_hex::TILE_LEVEL_HEIGHT
            || !observed_style::iso::survives(min_y, max_y, local, bearing, true)
        {
            return None;
        }
    }
    Some(floor)
}

pub(crate) fn mesh(shape: &ColliderShape) -> Option<Mesh> {
    Some(match shape {
        ColliderShape::Cuboid { half } => Mesh::from(Cuboid::from_size(*half * 2.0)),
        ColliderShape::ConvexHull { points } => {
            let data = ConvexRenderMesh::from_convex_hull(points)?;
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs)
            .with_inserted_indices(Indices::U32(data.indices))
        }
    })
}
