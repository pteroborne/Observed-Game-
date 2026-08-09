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

/// The cell-local corners at `inset`, in [`CORNERS`] order.
fn ring(y: f32, inset: f32) -> Vec<Vec3> {
    CORNERS
        .iter()
        .map(|&(x, z)| {
            #[allow(clippy::cast_precision_loss)]
            Vec3::new(x as f32 * inset, y, z as f32 * inset)
        })
        .collect()
}

/// The floor outline of a cell at full extent, so neighbouring cells meet and
/// the lattice reads as one connected surface rather than as scattered tiles.
#[must_use]
pub fn floor_ring(inset: f32) -> Vec<(Vec3, Vec3)> {
    let corners = ring(0.0, inset);
    (0..6)
        .map(|index| (corners[index], corners[(index + 1) % 6]))
        .collect()
}

/// Solid wall bands, one per face the cell does **not** open through.
///
/// A face that opens keeps a jamb at each end and leaves the middle clear, so a
/// doorway is a real hole in a real wall. Walls sit slightly inside the
/// footprint: neighbouring cells each own their own wall, which is both true of
/// the authored tiles and what keeps two coincident quads from fighting.
#[must_use]
pub fn wall_bands(height: f32, open: [bool; 6]) -> Vec<[Vec3; 4]> {
    /// How much of an opening's edge stays walled at each end.
    const JAMB: f32 = 0.3;
    /// Pulled off the shared edge so adjacent cells do not co-plane.
    const WALL_INSET: f32 = 0.965;

    let low = ring(0.0, WALL_INSET);
    let high = ring(height, WALL_INSET);
    let mut out = Vec::with_capacity(8);
    let band = |from: usize, to: usize, start: f32, end: f32, out: &mut Vec<[Vec3; 4]>| {
        let a = low[from].lerp(low[to], start);
        let b = low[from].lerp(low[to], end);
        let c = high[from].lerp(high[to], end);
        let d = high[from].lerp(high[to], start);
        out.push([a, b, c, d]);
    };
    for (index, opens) in open.iter().enumerate() {
        let next = (index + 1) % 6;
        if *opens {
            band(index, next, 0.0, JAMB, &mut out);
            band(index, next, 1.0 - JAMB, 1.0, &mut out);
        } else {
            band(index, next, 0.0, 1.0, &mut out);
        }
    }
    out
}

/// A staircase, drawn in profile at the centre of a cell that connects upward.
///
/// The schematic convention: you should be able to tell a floor change is
/// available here without selecting anything or counting hull edges.
#[must_use]
pub fn stair_glyph(height: f32) -> Vec<(Vec3, Vec3)> {
    const STEPS: usize = 4;
    const RUN: f32 = 1.5;
    #[allow(clippy::cast_precision_loss)]
    let span = RUN * STEPS as f32;
    #[allow(clippy::cast_precision_loss)]
    let rise = height / STEPS as f32;
    let mut out = Vec::with_capacity(STEPS * 2);
    let mut cursor = Vec3::new(-span * 0.5, 0.0, 0.0);
    for _ in 0..STEPS {
        let tread = cursor + Vec3::X * RUN;
        out.push((cursor, tread));
        let riser = tread + Vec3::Y * rise;
        out.push((tread, riser));
        cursor = riser;
    }
    out
}

/// A slope with a chevron at its high end, for a cell that ramps upward.
#[must_use]
pub fn ramp_glyph(height: f32) -> Vec<(Vec3, Vec3)> {
    let low = Vec3::new(-3.0, 0.0, 0.0);
    let high = Vec3::new(3.0, height, 0.0);
    vec![
        (low, high),
        (high, high + Vec3::new(-1.4, -0.5, 0.9)),
        (high, high + Vec3::new(-1.4, -0.5, -0.9)),
    ]
}

/// The *structural* edges of a set of convex hulls, in the hulls' own local
/// frame.
///
/// The rule moved to [`observed_traversal::structural_edges`] when the
/// composition studio started drawing uncollapsed cells as wireframe: two
/// surfaces filtering triangulation diagonals differently would disagree about
/// the shape of the same tile. This name stays because it is what the lab's
/// prose calls it.
#[must_use]
pub fn hull_edges(hulls: &[Vec<Vec3>]) -> Vec<(Vec3, Vec3)> {
    observed_traversal::structural_edges(hulls)
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
    fn the_floor_ring_closes_at_full_extent_so_neighbours_meet() {
        let ring = floor_ring(1.0);
        assert_eq!(ring.len(), 6);
        // Every edge must end where the next begins, or the lattice reads as
        // scattered tiles instead of one connected surface.
        for index in 0..6 {
            assert_eq!(ring[index].1, ring[(index + 1) % 6].0);
        }
        let reach = ring
            .iter()
            .flat_map(|(a, b)| [a.x.abs(), b.x.abs()])
            .fold(0.0, f32::max);
        assert!(
            (reach - 7.0).abs() < 1e-5,
            "full extent reaches the flat faces"
        );
    }

    #[test]
    fn a_sealed_face_walls_and_an_open_one_leaves_a_doorway() {
        let sealed = wall_bands(1.0, [false; 6]);
        assert_eq!(sealed.len(), 6, "six sealed faces, six solid bands");

        let mut open = [false; 6];
        open[0] = true;
        let doored = wall_bands(1.0, open);
        assert_eq!(
            doored.len(),
            7,
            "an opening splits its face into two jambs, so the count rises by one"
        );
        // The doorway is a real hole: the two jambs must not meet.
        let jamb_a = doored[0];
        let jamb_b = doored[1];
        assert!(
            (jamb_a[1] - jamb_b[0]).length() > 1.0,
            "the jambs leave a gap wide enough to read as a door"
        );
    }

    #[test]
    fn walls_sit_inside_the_footprint_so_neighbours_do_not_co_plane() {
        let reach = wall_bands(1.0, [false; 6])
            .iter()
            .flat_map(|quad| quad.iter().map(|corner| corner.x.abs()))
            .fold(0.0, f32::max);
        let floor = floor_ring(1.0)
            .iter()
            .flat_map(|(a, b)| [a.x.abs(), b.x.abs()])
            .fold(0.0, f32::max);
        assert!(
            reach < floor,
            "a cell owns its own wall, just inside its edge"
        );
    }

    #[test]
    fn a_wall_band_is_only_as_tall_as_it_is_asked_to_be() {
        let top = wall_bands(2.5, [false; 6])
            .iter()
            .flat_map(|quad| quad.iter().map(|corner| corner.y))
            .fold(f32::MIN, f32::max);
        assert!((top - 2.5).abs() < 1e-5);
    }

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
