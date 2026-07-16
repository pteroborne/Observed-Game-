//! Pre-collapse constraints: room-site seeding and the forced spawn→exit
//! route.

use std::collections::{BTreeMap, BTreeSet};

use observed_core::SplitMix;
use observed_hex::{HexCoord, HexFace, lateral_distance};

use super::{HexWfcConfig, lateral_bit};

/// Every coordinate of the (single-level) grid in index order.
pub(super) fn all_coords(config: HexWfcConfig) -> impl Iterator<Item = HexCoord> {
    let grid = config.grid();
    (0..grid.cell_count()).map(move |index| grid.coord(index))
}

/// Deterministically seed the cells allowed to become rooms: a greedy
/// blue-noise pick over an rng-salted shuffle, spawn and exit always
/// included, every pair at least `min_room_distance` apart. Restricting the
/// Room alphabet to these sites makes room separation structural instead of
/// a validation lottery — and is the single-cell precursor of Phase 90's
/// multi-hex blueprint stamping.
pub(super) fn room_sites(config: HexWfcConfig, rng: &mut SplitMix) -> BTreeSet<HexCoord> {
    let salt = rng.next_u64();
    let span = (config.max_rooms - config.min_rooms).max(1) as u64;
    let target = config.min_rooms + 1 + (rng.next_u64() % span) as usize;
    let mut coords: Vec<HexCoord> = all_coords(config).collect();
    coords.sort_by_key(|&coord| {
        let mut mix = SplitMix::new(coord_key(coord) ^ salt);
        mix.next_u64()
    });
    let mut sites = BTreeSet::from([config.spawn(), config.exit()]);
    for coord in coords {
        if sites.len() >= target {
            break;
        }
        if sites
            .iter()
            .all(|&site| lateral_distance(site, coord) >= config.min_room_distance)
        {
            sites.insert(coord);
        }
    }
    sites
}

fn coord_key(coord: HexCoord) -> u64 {
    u64::from(coord.q) | (u64::from(coord.r) << 16) | (u64::from(coord.level) << 32)
}

/// A randomized monotone staircase from spawn to exit on the rhombus: each
/// step goes `East` (q+1) or `SouthEast` (r+1), choice drawn from `rng`.
/// Returns required door bits per cell, guaranteeing spawn→exit solvability
/// before the collapse even starts.
pub(super) fn forced_route_edges(
    config: HexWfcConfig,
    rng: &mut SplitMix,
) -> BTreeMap<HexCoord, u8> {
    let grid = config.grid();
    let mut required: BTreeMap<HexCoord, u8> = BTreeMap::new();
    let mut current = config.spawn();
    let exit = config.exit();
    while current != exit {
        let need_east = current.q < exit.q;
        let need_south = current.r < exit.r;
        let face = match (need_east, need_south) {
            (true, true) => {
                if rng.next_u64() & 1 == 0 {
                    HexFace::East
                } else {
                    HexFace::SouthEast
                }
            }
            (true, false) => HexFace::East,
            (false, true) => HexFace::SouthEast,
            (false, false) => unreachable!("loop ends at exit"),
        };
        let next = grid
            .neighbor(current, face)
            .expect("monotone staircase stays inside the rhombus");
        *required.entry(current).or_default() |= lateral_bit(face);
        *required.entry(next).or_default() |= lateral_bit(face.opposite());
        current = next;
    }
    required
}
