//! Accumulating buffers that merge many primitives into one mesh each.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// One colour's worth of accumulated line work.
#[derive(Default)]
pub struct LineBatch {
    points: Vec<[f32; 3]>,
}

impl LineBatch {
    pub fn segment(&mut self, from: Vec3, to: Vec3) {
        self.points.push(from.to_array());
        self.points.push(to.to_array());
    }

    /// Append a closed loop through `points`.
    pub fn loop_through(&mut self, points: &[Vec3]) {
        for index in 0..points.len() {
            self.segment(points[index], points[(index + 1) % points.len()]);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    pub fn segments(&self) -> usize {
        self.points.len() / 2
    }

    /// Consume the batch into one line-list mesh.
    #[must_use]
    pub fn build(self) -> Option<Mesh> {
        if self.points.is_empty() {
            return None;
        }
        #[allow(clippy::cast_possible_truncation)]
        let indices = (0..self.points.len() as u32).collect::<Vec<_>>();
        let mut mesh = Mesh::new(
            PrimitiveTopology::LineList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );
        // Every vertex needs a normal for the standard material pipeline even
        // though a line never shades; a constant up-normal keeps the emissive
        // term flat, which is what a phosphor line should look like.
        let normals = vec![[0.0, 1.0, 0.0]; self.points.len()];
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.points);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_indices(Indices::U32(indices));
        Some(mesh)
    }
}

/// Accumulated solid surfaces for one colour.
///
/// Walls are drawn as low solid bands rather than as full-height wireframe
/// boxes: a floor plan is read from above, and a waist-high solid says "you
/// cannot pass here" far faster than a tall transparent cage does.
#[derive(Default)]
pub struct SurfaceBatch {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl SurfaceBatch {
    /// Append a quad wound `a b c d`.
    pub fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        let normal = (b - a).cross(d - a).normalize_or_zero().to_array();
        #[allow(clippy::cast_possible_truncation)]
        let base = self.positions.len() as u32;
        for corner in [a, b, c, d] {
            self.positions.push(corner.to_array());
            self.normals.push(normal);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub fn quads(&self) -> usize {
        self.indices.len() / 6
    }

    #[must_use]
    pub fn build(self) -> Option<Mesh> {
        if self.indices.is_empty() {
            return None;
        }
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_indices(Indices::U32(self.indices));
        Some(mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_surface_batch_becomes_one_mesh_of_two_triangles_per_quad() {
        let mut walls = SurfaceBatch::default();
        walls.quad(Vec3::ZERO, Vec3::X, Vec3::X + Vec3::Y, Vec3::Y);
        walls.quad(Vec3::Z, Vec3::X + Vec3::Z, Vec3::X + Vec3::Y, Vec3::Y);
        assert_eq!(walls.quads(), 2);
        let mesh = walls.build().expect("a non-empty batch builds");
        assert_eq!(mesh.primitive_topology(), PrimitiveTopology::TriangleList);
        assert!(SurfaceBatch::default().build().is_none());
    }

    #[test]
    fn a_batch_becomes_one_line_mesh_with_paired_vertices() {
        let mut batch = LineBatch::default();
        batch.segment(Vec3::ZERO, Vec3::X);
        batch.loop_through(&[Vec3::ZERO, Vec3::X, Vec3::Z]);
        assert_eq!(batch.segments(), 4, "one segment plus a three-edge loop");
        let mesh = batch.build().expect("a non-empty batch builds");
        assert_eq!(mesh.primitive_topology(), PrimitiveTopology::LineList);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .expect("positions")
            .len();
        assert_eq!(positions, 8, "two vertices per segment");
    }

    #[test]
    fn an_empty_batch_builds_nothing_rather_than_an_empty_mesh() {
        assert!(LineBatch::default().build().is_none());
    }
}
