//! The exact quantized-hexagon prism, meshed for the isometric diagram.
//!
//! The footprint is [`observed_hex::CORNERS`] verbatim rather than a regular
//! hexagon. The quantized hexagon is deliberately *not* regular — its east and
//! west faces sit 7 m out while its north and south vertices reach 8 m — so a
//! `RegularPolygon` approximation would leave sub-metre gaps between neighbours
//! and misreport exactly the thing this lab exists to show: whether cells
//! compose into continuous space.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use observed_hex::CORNERS;

/// Build a flat-shaded prism over the canonical footprint, `height` metres tall,
/// with its base at y = 0 and its footprint scaled by `inset`.
///
/// `inset` slightly shrinks each cell so neighbours read as countable tiles
/// instead of one undifferentiated mass. Keep it close to 1.0: the seam has to
/// stay hairline or genuinely open regions stop reading as continuous.
#[must_use]
pub fn hex_prism(height: f32, inset: f32) -> Mesh {
    #[allow(clippy::cast_precision_loss)]
    let ring: Vec<Vec2> = CORNERS
        .iter()
        .map(|&(x, z)| Vec2::new(x as f32 * inset, z as f32 * inset))
        .collect();

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(6 * 4 + 12);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(6 * 4 + 12);
    let mut indices: Vec<u32> = Vec::with_capacity(6 * 6 + 12 * 3);

    // Top and bottom faces, each a fan of six triangles around a centre vertex.
    for (y, normal, wind_forward) in [
        (height, [0.0, 1.0, 0.0], true),
        (0.0, [0.0, -1.0, 0.0], false),
    ] {
        let centre = positions.len() as u32;
        positions.push([0.0, y, 0.0]);
        normals.push(normal);
        for corner in &ring {
            positions.push([corner.x, y, corner.y]);
            normals.push(normal);
        }
        for i in 0..6u32 {
            let a = centre + 1 + i;
            let b = centre + 1 + (i + 1) % 6;
            if wind_forward {
                indices.extend_from_slice(&[centre, b, a]);
            } else {
                indices.extend_from_slice(&[centre, a, b]);
            }
        }
    }

    // Six side quads, each with its own duplicated vertices so the facets stay
    // hard-edged under flat shading.
    for i in 0..6usize {
        let a = ring[i];
        let b = ring[(i + 1) % 6];
        let edge = b - a;
        // Outward normal in plan: the edge rotated -90 degrees.
        let normal = Vec3::new(edge.y, 0.0, -edge.x)
            .normalize_or_zero()
            .to_array();
        let base = positions.len() as u32;
        positions.push([a.x, 0.0, a.y]);
        positions.push([b.x, 0.0, b.y]);
        positions.push([b.x, height, b.y]);
        positions.push([a.x, height, a.y]);
        for _ in 0..4 {
            normals.push(normal);
        }
        indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prism_covers_the_full_quantized_footprint() {
        let mesh = hex_prism(1.0, 1.0);
        let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
            panic!("prism must carry positions");
        };
        let bounds = positions.as_float3().expect("positions are 3-component");
        let max_x = bounds.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        let max_z = bounds.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
        // The quantized hexagon is wider across its corners (z) than its flats (x).
        assert!((max_x - 7.0).abs() < 1e-5, "east face sits at x = 7");
        assert!((max_z - 8.0).abs() < 1e-5, "north vertex sits at z = 8");
    }

    #[test]
    fn every_face_is_wound_and_shaded_flat() {
        let mesh = hex_prism(2.0, 1.0);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .expect("positions")
            .as_float3()
            .expect("3-component");
        let normals = mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .expect("normals")
            .as_float3()
            .expect("3-component");
        assert_eq!(positions.len(), normals.len());
        // Two fans of seven vertices, plus four per side quad.
        assert_eq!(positions.len(), 7 * 2 + 4 * 6);
        for normal in normals {
            let length = Vec3::from_array(*normal).length();
            assert!((length - 1.0).abs() < 1e-4, "normals are unit length");
        }
    }

    #[test]
    fn the_inset_shrinks_the_footprint_without_moving_the_base() {
        let mesh = hex_prism(1.0, 0.5);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .expect("positions")
            .as_float3()
            .expect("3-component");
        let max_x = positions.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        let min_y = positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        assert!((max_x - 3.5).abs() < 1e-5, "inset halves the plan extent");
        assert!(min_y.abs() < 1e-5, "the base stays on y = 0");
    }
}
