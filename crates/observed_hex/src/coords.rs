//! Axial hex coordinates on a rhombic-bounded, level-stacked lattice.

use crate::faces::HexFace;

/// One cell of the hex-prism lattice: axial `(q, r)` in plan view plus a
/// vertical `level`. Bounds are rhombic — every `(q, r)` in
/// `0..cols x 0..rows` is a cell — so indexing needs no offset-parity
/// branches and the world footprint is a parallelogram.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HexCoord {
    pub q: u16,
    pub r: u16,
    pub level: u8,
}

/// Lattice dimensions plus the coordinate/index bookkeeping shared by every
/// consumer (mirrors the `full_wfc` config API shape).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexGridSize {
    pub cols: u16,
    pub rows: u16,
    pub levels: u8,
}

impl HexGridSize {
    #[must_use]
    pub const fn cell_count(self) -> usize {
        self.cols as usize * self.rows as usize * self.levels as usize
    }

    #[must_use]
    pub const fn contains(self, coord: HexCoord) -> bool {
        coord.q < self.cols && coord.r < self.rows && coord.level < self.levels
    }

    /// Row-major flatten: `level * cols * rows + r * cols + q`.
    #[must_use]
    pub const fn index(self, coord: HexCoord) -> usize {
        debug_assert!(self.contains(coord));
        coord.level as usize * self.cols as usize * self.rows as usize
            + coord.r as usize * self.cols as usize
            + coord.q as usize
    }

    /// Inverse of [`HexGridSize::index`].
    #[must_use]
    pub const fn coord(self, index: usize) -> HexCoord {
        let per_level = self.cols as usize * self.rows as usize;
        HexCoord {
            q: (index % self.cols as usize) as u16,
            r: (index % per_level / self.cols as usize) as u16,
            level: (index / per_level) as u8,
        }
    }

    /// The neighbor behind `face`, or `None` at the lattice boundary.
    #[must_use]
    pub fn neighbor(self, coord: HexCoord, face: HexFace) -> Option<HexCoord> {
        let (dq, dr, dl) = face.delta();
        let q = i32::from(coord.q) + dq;
        let r = i32::from(coord.r) + dr;
        let level = i32::from(coord.level) + dl;
        let next = HexCoord {
            q: u16::try_from(q).ok()?,
            r: u16::try_from(r).ok()?,
            level: u8::try_from(level).ok()?,
        };
        self.contains(next).then_some(next)
    }
}

/// Lateral (in-plane) hex distance between two cells:
/// `(|dq| + |dr| + |dq + dr|) / 2` in axial coordinates.
#[must_use]
pub fn lateral_distance(a: HexCoord, b: HexCoord) -> u32 {
    let dq = i32::from(b.q) - i32::from(a.q);
    let dr = i32::from(b.r) - i32::from(a.r);
    ((dq.abs() + dr.abs() + (dq + dr).abs()) / 2) as u32
}

/// Travel-shaped distance: lateral hex distance plus a 2x-weighted level term,
/// mirroring the square lattice's routing heuristic where vertical moves cost
/// twice a lateral step.
#[must_use]
pub fn travel_distance(a: HexCoord, b: HexCoord) -> u32 {
    let dl = (i32::from(b.level) - i32::from(a.level)).unsigned_abs();
    lateral_distance(a, b) + 2 * dl
}

#[cfg(test)]
mod tests {
    use super::{HexCoord, HexGridSize, lateral_distance, travel_distance};
    use crate::faces::HexFace;

    const GRID: HexGridSize = HexGridSize {
        cols: 7,
        rows: 5,
        levels: 3,
    };

    fn all_coords() -> impl Iterator<Item = HexCoord> {
        (0..GRID.cell_count()).map(|i| GRID.coord(i))
    }

    #[test]
    fn index_and_coord_round_trip_over_the_whole_grid() {
        for index in 0..GRID.cell_count() {
            let coord = GRID.coord(index);
            assert!(GRID.contains(coord));
            assert_eq!(GRID.index(coord), index);
        }
    }

    #[test]
    fn stepping_a_face_and_its_opposite_returns_home_and_boundaries_are_none() {
        for coord in all_coords() {
            for face in HexFace::ALL {
                match GRID.neighbor(coord, face) {
                    Some(next) => {
                        assert_eq!(GRID.neighbor(next, face.opposite()), Some(coord));
                    }
                    None => {
                        let (dq, dr, dl) = face.delta();
                        let q = i32::from(coord.q) + dq;
                        let r = i32::from(coord.r) + dr;
                        let level = i32::from(coord.level) + dl;
                        assert!(
                            q < 0
                                || r < 0
                                || level < 0
                                || q >= i32::from(GRID.cols)
                                || r >= i32::from(GRID.rows)
                                || level >= i32::from(GRID.levels),
                            "in-bounds step reported as boundary: {coord:?} {face:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn lateral_distance_is_one_to_every_lateral_neighbor_and_zero_to_self() {
        let center = HexCoord {
            q: 3,
            r: 2,
            level: 1,
        };
        assert_eq!(lateral_distance(center, center), 0);
        for face in HexFace::LATERAL {
            let next = GRID.neighbor(center, face).expect("interior cell");
            assert_eq!(lateral_distance(center, next), 1, "{face:?}");
        }
    }

    #[test]
    fn travel_distance_weights_levels_double() {
        let a = HexCoord {
            q: 1,
            r: 1,
            level: 0,
        };
        let b = HexCoord {
            q: 1,
            r: 1,
            level: 2,
        };
        assert_eq!(travel_distance(a, b), 4);
        let c = HexCoord {
            q: 4,
            r: 1,
            level: 1,
        };
        assert_eq!(travel_distance(a, c), 3 + 2);
    }
}
