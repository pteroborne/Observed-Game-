//! Local-frame wireframe for the *authored tile hulls* this lab draws.
//!
//! The batching and cell-shell half of this module moved to
//! `observed_schematic` when `tactics_lab` became a second consumer. What stays
//! is what only a view rendering real authored geometry wants: turning a hull
//! point cloud into structural edges, and caching that per tile prototype.

use bevy::prelude::*;
use std::collections::BTreeMap;

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
