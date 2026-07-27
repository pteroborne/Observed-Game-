//! The quantized-hexagon physical metrics.
//!
//! A regular hexagon cannot sit on an integer grid (its diagonals involve
//! sqrt(3)), so the canonical tile footprint trades 0.8% of edge regularity for
//! **exact integer corners in meters** — every TrenchBroom brush plane snaps
//! precisely at any integer units-per-meter scale, and the importer validates
//! footprints with zero tolerance instead of epsilon games.
//!
//! Plan view is pointy-top: the East/West faces are flat, axis-aligned walls.
//! World axes: `x` east, `y` up, `z` south. Deltas here agree with
//! [`crate::faces::HexFace::delta`] through [`hex_origin`]: stepping a lateral
//! face moves the origin by exactly that face's pitch.

use crate::coords::HexCoord;
use crate::faces::HexFace;

/// Cell width across the flat East/West faces (meters).
pub const ACROSS_FLATS: f32 = 14.0;
/// Cell height across the north/south corners (meters).
pub const ACROSS_CORNERS: f32 = 16.0;
/// Height of one level (meters) — tall enough for a full-level ramp rise
/// inside one cell at a walkable slope (rise 8 over run 14, ~29.7 degrees).
pub const TILE_LEVEL_HEIGHT: f32 = 8.0;

/// The canonical footprint corners `(x, z)` in meters, counterclockwise in
/// plan view starting north-east. Edge `i` runs from corner `i` to corner
/// `i + 1` (mod 6) and is the boundary of lateral face `i`
/// ([`HexFace::East`] .. [`HexFace::NorthEast`]).
pub const CORNERS: [(i32, i32); 6] = [(7, -4), (7, 4), (0, 8), (-7, 4), (-7, -4), (0, -8)];

/// Integer plan-view origin of a cell in meters: `(x, z)` of its center.
/// Exact by construction — use this (not the float form) for tiling proofs
/// and importer validation.
#[must_use]
pub const fn hex_origin_plan(coord: HexCoord) -> (i32, i32) {
    (
        coord.q as i32 * 14 + coord.r as i32 * 7,
        coord.r as i32 * 12,
    )
}

/// World-space origin of a cell `(x, y, z)` in meters — the float bridge from
/// lattice to world used by geometry projection and rendering.
#[must_use]
pub fn hex_origin(coord: HexCoord) -> [f32; 3] {
    let (x, z) = hex_origin_plan(coord);
    #[allow(clippy::cast_precision_loss)]
    [
        x as f32,
        f32::from(coord.level) * TILE_LEVEL_HEIGHT,
        z as f32,
    ]
}

/// The twelve corners of a cell-local hexagonal prism `height` metres tall,
/// its footprint scaled by `inset`, base on `y = 0`. Bottom ring first, then
/// top, both in [`CORNERS`] order.
///
/// The quantized hexagon is not regular — its east and west faces sit 7 m out
/// while its north and south vertices reach 8 m — so a regular-polygon
/// approximation leaves sub-metre gaps between neighbours. Any renderer that
/// draws a cell as a solid takes its corners from here rather than
/// re-deriving them, and the result is a convex hull, so a mesh follows from
/// `observed_traversal::ConvexRenderMesh::from_convex_hull` without repeating
/// triangulation. `inset` below 1.0 shrinks a cell so neighbours read as
/// countable tiles in a diagram; it has no meaning for authored geometry.
#[must_use]
pub fn prism_hull(height: f32, inset: f32) -> [[f32; 3]; 12] {
    let mut hull = [[0.0_f32; 3]; 12];
    for (index, &(x, z)) in CORNERS.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let (x, z) = (x as f32 * inset, z as f32 * inset);
        hull[index] = [x, 0.0, z];
        hull[index + 6] = [x, height, z];
    }
    hull
}

/// The two footprint corners bounding a lateral face's edge, in the cell's
/// local plan frame (meters).
#[must_use]
pub const fn face_edge(face: HexFace) -> [(i32, i32); 2] {
    let i = face as usize;
    debug_assert!(i < 6);
    [CORNERS[i], CORNERS[(i + 1) % 6]]
}

#[cfg(test)]
mod tests {
    use super::{
        ACROSS_CORNERS, ACROSS_FLATS, CORNERS, face_edge, hex_origin, hex_origin_plan, prism_hull,
    };
    use crate::coords::{HexCoord, HexGridSize};
    use crate::faces::HexFace;

    #[test]
    fn corners_span_the_documented_extents() {
        let xs = CORNERS.map(|(x, _)| x);
        let zs = CORNERS.map(|(_, z)| z);
        let width = xs.iter().max().unwrap() - xs.iter().min().unwrap();
        let height = zs.iter().max().unwrap() - zs.iter().min().unwrap();
        assert_eq!(width as f32, ACROSS_FLATS);
        assert_eq!(height as f32, ACROSS_CORNERS);
    }

    #[test]
    fn edge_irregularity_is_the_documented_zero_point_eight_percent() {
        // Flat edges (E/W) are exactly 8 m; diagonal edges are sqrt(65).
        for face in HexFace::LATERAL {
            let [(ax, az), (bx, bz)] = face_edge(face);
            let len_sq = (ax - bx).pow(2) + (az - bz).pow(2);
            match face {
                HexFace::East | HexFace::West => assert_eq!(len_sq, 64, "{face:?}"),
                _ => assert_eq!(len_sq, 65, "{face:?}"),
            }
        }
    }

    #[test]
    fn shared_edges_of_lateral_neighbors_coincide_exactly() {
        // Tiling identity, in pure integer arithmetic: my face-i edge,
        // translated by my origin, is the neighbor's opposite-face edge
        // translated by the neighbor's origin (as a point set).
        let grid = HexGridSize {
            cols: 4,
            rows: 4,
            levels: 1,
        };
        let center = HexCoord {
            q: 1,
            r: 1,
            level: 0,
        };
        let (cx, cz) = hex_origin_plan(center);
        for face in HexFace::LATERAL {
            let next = grid.neighbor(center, face).expect("interior cell");
            let (nx, nz) = hex_origin_plan(next);
            let mine: Vec<(i32, i32)> = face_edge(face)
                .iter()
                .map(|(x, z)| (x + cx, z + cz))
                .collect();
            let mut theirs: Vec<(i32, i32)> = face_edge(face.opposite())
                .iter()
                .map(|(x, z)| (x + nx, z + nz))
                .collect();
            theirs.reverse(); // shared edge is walked in opposite directions
            assert_eq!(mine, theirs, "{face:?}");
        }
    }

    #[test]
    fn lateral_pitch_matches_face_deltas() {
        let origin = hex_origin_plan(HexCoord {
            q: 2,
            r: 2,
            level: 0,
        });
        let expected = [
            (HexFace::East, (14, 0)),
            (HexFace::SouthEast, (7, 12)),
            (HexFace::SouthWest, (-7, 12)),
            (HexFace::West, (-14, 0)),
            (HexFace::NorthWest, (-7, -12)),
            (HexFace::NorthEast, (7, -12)),
        ];
        for (face, pitch) in expected {
            let (dq, dr, _) = face.delta();
            let stepped = hex_origin_plan(HexCoord {
                q: (2 + dq) as u16,
                r: (2 + dr) as u16,
                level: 0,
            });
            assert_eq!(
                (stepped.0 - origin.0, stepped.1 - origin.1),
                pitch,
                "{face:?}"
            );
        }
    }

    #[test]
    fn origins_are_injective_over_a_full_grid() {
        let grid = HexGridSize {
            cols: 9,
            rows: 7,
            levels: 4,
        };
        let mut seen = std::collections::BTreeSet::new();
        for index in 0..grid.cell_count() {
            let coord = grid.coord(index);
            let (x, z) = hex_origin_plan(coord);
            assert!(
                seen.insert((x, z, coord.level)),
                "duplicate origin for {coord:?}"
            );
        }
    }

    #[test]
    fn world_origin_agrees_with_plan_origin_and_level_height() {
        let coord = HexCoord {
            q: 3,
            r: 2,
            level: 2,
        };
        let (x, z) = hex_origin_plan(coord);
        assert_eq!(hex_origin(coord), [x as f32, 16.0, z as f32]);
    }

    #[test]
    fn the_prism_hull_reproduces_the_quantized_footprint_at_both_heights() {
        let hull = prism_hull(2.5, 1.0);
        for (index, &(x, z)) in CORNERS.iter().enumerate() {
            assert_eq!(hull[index], [x as f32, 0.0, z as f32]);
            assert_eq!(hull[index + 6], [x as f32, 2.5, z as f32]);
        }
        // The footprint is deliberately irregular: flats at 7, corners at 8.
        let max_x = hull.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        let max_z = hull.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
        assert!((max_x - 7.0).abs() < 1e-6);
        assert!((max_z - 8.0).abs() < 1e-6);
    }

    #[test]
    fn the_prism_inset_scales_the_plan_but_never_the_height() {
        let hull = prism_hull(4.0, 0.5);
        let max_x = hull.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        let max_y = hull.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        let min_y = hull.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        assert!((max_x - 3.5).abs() < 1e-6, "plan halves");
        assert!((max_y - 4.0).abs() < 1e-6, "height is untouched");
        assert!(min_y.abs() < 1e-6, "the base stays on y = 0");
    }
}
