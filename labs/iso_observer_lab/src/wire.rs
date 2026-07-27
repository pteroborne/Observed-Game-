//! Line work for the schematic view.
//!
//! A schematic is drawn as batched line lists, not as one entity per edge. At
//! production scale a single layer resolves to roughly twelve thousand convex
//! hulls; as entities that is hopeless, and as three or four merged line meshes
//! it is nothing. Everything here accumulates segments into per-colour buffers
//! that the caller turns into one mesh each.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use observed_hex::CORNERS;
use observed_traversal::ConvexRenderMesh;
use std::collections::BTreeMap;

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
    #[allow(dead_code)]
    pub fn loop_through(&mut self, points: &[Vec3]) {
        for index in 0..points.len() {
            self.segment(points[index], points[(index + 1) % points.len()]);
        }
    }

    #[allow(dead_code)]
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

/// The cell-local outline of a hex prism `height` tall: both rings and the six
/// uprights. This is the schematic's structural shorthand — it says "a cell is
/// here and this is how tall it is" without claiming anything about its
/// interior.
#[must_use]
pub fn hex_shell(height: f32, inset: f32) -> Vec<(Vec3, Vec3)> {
    let ring = |y: f32| {
        CORNERS
            .iter()
            .map(|&(x, z)| {
                #[allow(clippy::cast_precision_loss)]
                Vec3::new(x as f32 * inset, y, z as f32 * inset)
            })
            .collect::<Vec<_>>()
    };
    let low = ring(0.0);
    let high = ring(height);
    let mut out = Vec::with_capacity(18);
    for index in 0..6 {
        let next = (index + 1) % 6;
        out.push((low[index], low[next]));
        out.push((high[index], high[next]));
        out.push((low[index], high[index]));
    }
    out
}

/// The *structural* edges of a set of convex hulls, in the hulls' own local
/// frame.
///
/// The hulls arrive as unordered point clouds, so the geometry comes from
/// triangulating each one through [`ConvexRenderMesh`] — the same path the solid
/// renderer uses. Two filters then turn a triangle soup into a schematic:
///
/// 1. **De-duplicate by position, not by vertex index.** `ConvexRenderMesh`
///    duplicates vertices per face so flat shading works, so neighbouring
///    triangles never share an index even where they share an edge.
/// 2. **Drop coplanar diagonals.** An edge whose two adjacent triangles face the
///    same way is an artifact of triangulating a flat face, not an edge of the
///    solid. Keeping them draws every rectangular face with a diagonal through
///    it, which is what turns a clean prism into a web.
///
/// Degenerate hulls (a coplanar sliver, say) contribute nothing rather than
/// failing the draw.
#[must_use]
pub fn hull_edges(hulls: &[Vec<Vec3>]) -> Vec<(Vec3, Vec3)> {
    /// Millimetre quantization: fine enough to keep genuinely distinct corners
    /// apart, coarse enough that the same corner reached from two faces agrees.
    fn key(point: [f32; 3]) -> [i32; 3] {
        #[allow(clippy::cast_possible_truncation)]
        [
            (point[0] * 1000.0).round() as i32,
            (point[1] * 1000.0).round() as i32,
            (point[2] * 1000.0).round() as i32,
        ]
    }

    /// Above this, two adjacent faces are the same plane and their shared edge
    /// is a triangulation diagonal.
    const COPLANAR_DOT: f32 = 0.999;

    /// Quantized endpoints, so an edge reached from two faces hashes the same.
    type EdgeKey = ([i32; 3], [i32; 3]);
    /// The edge's real endpoints, plus the normal of every face sharing it.
    type EdgeFaces = ((Vec3, Vec3), Vec<Vec3>);

    let mut edges = Vec::new();
    for hull in hulls {
        let Some(mesh) = ConvexRenderMesh::from_convex_hull(hull) else {
            continue;
        };
        // edge -> (endpoints, the normals of the faces that share it)
        let mut found: BTreeMap<EdgeKey, EdgeFaces> = BTreeMap::new();
        for triangle in mesh.indices.chunks_exact(3) {
            let corner = |index: u32| Vec3::from_array(mesh.positions[index as usize]);
            let (a, b, c) = (
                corner(triangle[0]),
                corner(triangle[1]),
                corner(triangle[2]),
            );
            let normal = (b - a).cross(c - a).normalize_or_zero();
            for (from, to) in [(a, b), (b, c), (c, a)] {
                let (low, high) = (key(from.to_array()), key(to.to_array()));
                if low == high {
                    continue;
                }
                let ordered = if low <= high {
                    (low, high)
                } else {
                    (high, low)
                };
                found
                    .entry(ordered)
                    .or_insert_with(|| ((from, to), Vec::new()))
                    .1
                    .push(normal);
            }
        }
        for ((from, to), normals) in found.into_values() {
            let structural = normals.len() < 2
                || normals
                    .windows(2)
                    .any(|pair| pair[0].dot(pair[1]).abs() < COPLANAR_DOT);
            if structural {
                edges.push((from, to));
            }
        }
    }
    edges
}

/// Local-frame line work per authored tile, computed once and instanced by
/// offset at every cell that resolves to it.
///
/// Without this the schematic re-triangulates the same few dozen prototypes
/// thousands of times per rebuild. With it, a layer costs one hull pass per
/// distinct tile.
#[derive(Default)]
pub struct EdgeCache {
    by_tile: BTreeMap<String, Vec<(Vec3, Vec3)>>,
}

impl EdgeCache {
    pub fn edges_for(&mut self, key: &str, hulls: &[Vec<Vec3>]) -> &[(Vec3, Vec3)] {
        self.by_tile
            .entry(key.to_string())
            .or_insert_with(|| hull_edges(hulls))
    }

    pub fn len(&self) -> usize {
        self.by_tile.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.by_tile.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_closes_both_rings_and_stands_them_on_uprights() {
        let shell = hex_shell(2.0, 1.0);
        assert_eq!(shell.len(), 18, "six each of low ring, high ring, upright");
        let uprights = shell
            .iter()
            .filter(|(a, b)| (a.y - b.y).abs() > f32::EPSILON)
            .count();
        assert_eq!(uprights, 6);
        let top = shell
            .iter()
            .flat_map(|(a, b)| [a.y, b.y])
            .fold(0.0, f32::max);
        assert!((top - 2.0).abs() < 1e-6);
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

    #[test]
    fn hull_edges_wire_a_box_without_repeating_an_edge() {
        let box_hull = vec![
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, -1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, 1.0, 1.0),
        ];
        let edges = hull_edges(std::slice::from_ref(&box_hull));
        // Exactly the twelve edges a box has. The six face diagonals the
        // triangulation introduced must be filtered as coplanar, and the
        // per-face vertex duplication must not defeat de-duplication.
        assert_eq!(edges.len(), 12, "a box wires as a box, not as a web");
        for (a, b) in &edges {
            assert!((*a - *b).length() > f32::EPSILON, "no zero-length edges");
        }
    }

    #[test]
    fn a_degenerate_hull_contributes_nothing_instead_of_failing() {
        let sliver = vec![Vec3::ZERO, Vec3::X, Vec3::X * 2.0];
        assert!(hull_edges(std::slice::from_ref(&sliver)).is_empty());
    }

    #[test]
    fn the_cache_triangulates_each_tile_once() {
        let hull = vec![
            Vec3::ZERO,
            Vec3::X,
            Vec3::Y,
            Vec3::Z,
            Vec3::new(1.0, 1.0, 1.0),
        ];
        let mut cache = EdgeCache::default();
        assert!(cache.is_empty());
        let first = cache
            .edges_for("hall_straight/generic/0", std::slice::from_ref(&hull))
            .len();
        // A second ask for the same key must not re-triangulate, and must not
        // grow the cache even when handed different geometry.
        let second = cache.edges_for("hall_straight/generic/0", &[]).len();
        assert_eq!(first, second);
        assert_eq!(cache.len(), 1);
    }
}
