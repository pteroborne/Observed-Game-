//! The prison dimension's maze: a braided hex maze built only from authored halls.
//!
//! A caught Observer wakes in a sealed maze and walks out to the prison lobby
//! (`docs/architect_ascent_design.md`, section 3). The facility's own solver will not
//! make one: a single-level solve is one corridor from spawn to exit, with nothing to
//! get lost in. So the maze is carved directly.
//!
//! A classic maze is a spanning tree, and a tree has dead ends. The authored corpus has
//! no one-door hall to end one with, so this maze is *braided*: every dead end is joined
//! into a neighbour, which turns each into a loop. Every cell is then a straight, a
//! turn or a junction of two to four doors, all of which the corpus is required to
//! build. A braided maze is still a maze - with no dead end to back out of, a wrong turn
//! is a loop that brings you round again.

use std::collections::{BTreeMap, BTreeSet};

use observed_content::ArchitectureRegister;
use observed_hex::{HexCoord, HexFace, PortClass};

use super::profile::SpaceMix;
use super::{
    HexArchetype, HexPlacement, HexSpace, HexWfcConfig, HexWfcWorld, authored_hall, lateral_bit,
};

/// A cell's doors in the carved tree, leaving room for braiding to add one more.
const TREE_DEGREE: u32 = 3;
/// The most doors an authored flat hall has.
const HALL_DEGREE: u32 = 4;

/// A braided maze of `cols` by `rows` cells on one level, entered at the lattice's spawn
/// corner and left at its exit corner, every hall in `register`.
///
/// Deterministic in `seed`. The two corners are always joined.
#[must_use]
pub fn braided_maze(
    seed: u64,
    cols: u16,
    rows: u16,
    register: ArchitectureRegister,
) -> HexWfcWorld {
    let config = HexWfcConfig {
        cols,
        rows,
        levels: 1,
        min_rooms: 0,
        max_rooms: 0,
        retry_budget: 1,
        min_room_distance: 0,
    };
    // A depth-first tree sometimes winds its one true path through most of the lattice,
    // and pruning can, rarely, reach a corner. Either way another draw of the same
    // seed's stream carves a different tree, so every maze is about as hard as the next.
    let doors = (0u64..)
        .map(|attempt| carve(config, seed ^ attempt.wrapping_mul(0x9E37_79B9_7F4A_7C15)))
        .find(|doors| fair(config, doors))
        .expect("some tree is fair");
    let grid = config.grid();
    let placements = (0..grid.cell_count())
        .map(|index| grid.coord(index))
        .map(|coord| {
            let placement = doors
                .get(&coord)
                .and_then(|&mask| authored_hall(coord, mask))
                .unwrap_or(HexPlacement {
                    coord,
                    space: HexSpace::Void,
                    archetype: HexArchetype::Void,
                    doors: 0,
                    up: PortClass::Sealed,
                    down: PortClass::Sealed,
                });
            (coord, placement)
        })
        .collect::<BTreeMap<_, _>>();
    let architecture = placements.keys().map(|&coord| (coord, register)).collect();
    HexWfcWorld {
        seed,
        generation: 0,
        config,
        placements,
        blueprints: Vec::new(),
        architecture,
        cell_revisions: BTreeMap::new(),
        last_attempts: 0,
        authored_pins: BTreeSet::new(),
        space_mix: SpaceMix::baseline(),
        route_corridors: false,
        carve_unrouted: false,
        open_air: false,
    }
}

/// Whether a carved maze is one worth escaping: both corners joined, the shortest way out
/// between one and one and a half times the lattice's width plus height, and a junction
/// for every six cells, so there are choices to get wrong.
fn fair(config: HexWfcConfig, doors: &BTreeMap<HexCoord, u8>) -> bool {
    let span = usize::from(config.cols) + usize::from(config.rows);
    let junctions = doors.values().filter(|mask| mask.count_ones() >= 3).count();
    shortest_way_out(config, doors).is_some_and(|cells| (span..=span * 3 / 2).contains(&cells))
        && junctions * 6 >= config.grid().cell_count()
}

/// Cells on the shortest walk from the spawn corner to the exit corner, both counted.
fn shortest_way_out(config: HexWfcConfig, doors: &BTreeMap<HexCoord, u8>) -> Option<usize> {
    let grid = config.grid();
    let mut seen = BTreeSet::from([config.spawn()]);
    let mut frontier = std::collections::VecDeque::from([(config.spawn(), 1)]);
    while let Some((cell, cells)) = frontier.pop_front() {
        if cell == config.exit() {
            return Some(cells);
        }
        let mask = doors.get(&cell).copied().unwrap_or(0);
        for face in HexFace::LATERAL {
            if mask & lateral_bit(face) != 0
                && let Some(next) = grid.neighbor(cell, face)
                && seen.insert(next)
            {
                frontier.push_back((next, cells + 1));
            }
        }
    }
    None
}

/// Each maze cell's lateral door mask. Cells absent from the map are rock.
fn carve(config: HexWfcConfig, seed: u64) -> BTreeMap<HexCoord, u8> {
    let grid = config.grid();
    let mut rng = SplitMix(seed);
    let mut doors: BTreeMap<HexCoord, u8> = BTreeMap::new();
    let degree = |doors: &BTreeMap<HexCoord, u8>, cell: HexCoord| {
        doors.get(&cell).map_or(0, |mask| mask.count_ones())
    };
    let join = |doors: &mut BTreeMap<HexCoord, u8>, cell: HexCoord, face: HexFace| {
        let next = grid
            .neighbor(cell, face)
            .expect("joined within the lattice");
        *doors.entry(cell).or_default() |= lateral_bit(face);
        *doors.entry(next).or_default() |= lateral_bit(face.opposite());
    };
    let open_faces = |cell: HexCoord| {
        HexFace::LATERAL
            .into_iter()
            .filter(move |&face| grid.neighbor(cell, face).is_some())
    };

    // 1. A random depth-first spanning tree, no cell past three doors.
    let mut visited = BTreeSet::from([config.spawn()]);
    let mut stack = vec![config.spawn()];
    while let Some(&cell) = stack.last() {
        let mut options: Vec<HexFace> = if degree(&doors, cell) < TREE_DEGREE {
            open_faces(cell)
                .filter(|&face| !visited.contains(&grid.neighbor(cell, face).expect("open face")))
                .collect()
        } else {
            Vec::new()
        };
        if options.is_empty() {
            stack.pop();
            continue;
        }
        let face = options.swap_remove(rng.below(options.len()));
        let next = grid.neighbor(cell, face).expect("open face");
        join(&mut doors, cell, face);
        visited.insert(next);
        stack.push(next);
    }

    // 2. Braid: join every dead end into a neighbour with a door to spare, preferring
    //    another dead end so one join mends two. What cannot be joined is pruned back to
    //    rock, which may leave a new dead end behind it; repeat until none is left.
    while let Some(leaf) = doors
        .iter()
        .find(|&(_, mask)| mask.count_ones() == 1)
        .map(|(&cell, _)| cell)
    {
        let mask = doors[&leaf];
        let spare: Vec<HexFace> = open_faces(leaf)
            .filter(|&face| mask & lateral_bit(face) == 0)
            .filter(|&face| {
                let next = grid.neighbor(leaf, face).expect("open face");
                doors.contains_key(&next) && degree(&doors, next) < HALL_DEGREE
            })
            .collect();
        let leaves: Vec<HexFace> = spare
            .iter()
            .copied()
            .filter(|&face| degree(&doors, grid.neighbor(leaf, face).expect("open face")) == 1)
            .collect();
        let pool = if leaves.is_empty() { spare } else { leaves };
        if pool.is_empty() {
            let face = HexFace::LATERAL
                .into_iter()
                .find(|&face| mask & lateral_bit(face) != 0)
                .expect("a dead end has one door");
            let next = grid.neighbor(leaf, face).expect("its door leads somewhere");
            doors.remove(&leaf);
            *doors.get_mut(&next).expect("a joined cell") &= !lateral_bit(face.opposite());
            continue;
        }
        join(&mut doors, leaf, pool[rng.below(pool.len())]);
    }
    doors
}

/// A small, fixed, dependency-free stream: the maze must be identical on every peer.
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn maze(seed: u64) -> HexWfcWorld {
        braided_maze(seed, 9, 7, ArchitectureRegister::FacetMonument)
    }

    #[test]
    fn every_cell_is_an_authored_hall_or_rock() {
        for seed in 0..64 {
            let world = maze(seed);
            for placement in world.placements.values() {
                if placement.space.built() {
                    assert!(
                        (2..=4).contains(&placement.doors.count_ones()),
                        "seed {seed}: {placement:?}"
                    );
                    assert_eq!(
                        Some(*placement),
                        authored_hall(placement.coord, placement.doors)
                    );
                } else {
                    assert_eq!(placement.doors, 0);
                }
            }
        }
    }

    #[test]
    fn doors_always_meet_a_door() {
        for seed in 0..64 {
            let world = maze(seed);
            let grid = world.config.grid();
            for placement in world.placements.values() {
                for face in HexFace::LATERAL {
                    if placement.is_open(face) {
                        let next = grid
                            .neighbor(placement.coord, face)
                            .expect("no door faces off the lattice");
                        assert!(
                            world.placements[&next].is_open(face.opposite()),
                            "seed {seed}: {placement:?} opens onto a wall"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_built_cell_reaches_the_way_out() {
        for seed in 0..64 {
            let world = maze(seed);
            let exit = world.config.exit();
            for (&cell, placement) in &world.placements {
                if placement.space.built() {
                    assert!(
                        world.route_between(cell, exit).is_some(),
                        "seed {seed}: {cell:?} is cut off"
                    );
                }
            }
            assert!(world.placements[&world.config.spawn()].space.built());
        }
    }

    #[test]
    fn it_is_a_maze_and_not_a_corridor() {
        for seed in 0..64 {
            let world = maze(seed);
            let built = world
                .placements
                .values()
                .filter(|p| p.space.built())
                .count();
            let junctions = world
                .placements
                .values()
                .filter(|p| p.doors.count_ones() >= 3)
                .count();
            let route = world
                .route_between(world.config.spawn(), world.config.exit())
                .expect("the corners are joined")
                .len();
            assert!(
                built >= 9 * 7 * 9 / 10,
                "seed {seed}: only {built} cells built"
            );
            assert!(
                junctions * 6 >= 9 * 7,
                "seed {seed}: only {junctions} choices to make"
            );
            assert!(
                (16..=24).contains(&route),
                "seed {seed}: a way out of {route} cells"
            );
        }
    }

    #[test]
    fn the_same_seed_carves_the_same_maze_and_another_does_not() {
        assert_eq!(maze(5).placements, maze(5).placements);
        assert_ne!(maze(5).placements, maze(6).placements);
    }
}
