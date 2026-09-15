//! The siege: a real solved facility, and a clock to outlast inside it.
//!
//! The authored rectangle exists to make one rule visible at a time, and it is
//! deliberately an open plain — which is exactly why it plays like a car park.
//! This builds the board from the production solver instead, so the floor has
//! rooms, corridors, doorways and holes that somebody's constraint system
//! produced rather than a hand-placed hazard or two.
//!
//! ## What is real here, and what is not
//!
//! **Real:** the layout. Cell occupancy, the void between structures, and the
//! per-face door mask all come from [`HexWfcWorld`] at a pinned seed. Walls are
//! where the solver put walls, so a shove stops at one, sight stops at one, and
//! a Guardian has to come through a doorway like everything else.
//!
//! **Not real:** the geometry. Plates are still flat rectangles and walls are
//! face slabs. Projecting the authored tile hulls needs a mesh collider and is a
//! separate piece of work. The distinction matters enough to state here rather
//! than let a reader infer it from a screenshot.

use observed_facility::hex_wfc::{HexSpace, HexWfcConfig, HexWfcError, HexWfcWorld};
use observed_hex::{
    coords::{HexCoord, HexGridSize, lateral_distance},
    faces::HexFace,
};

use crate::model::{KineticRules, KineticWorld, SiegeRules, Station, StationId};

/// The pinned composition. One level, because the kinetic model is single-floor;
/// wide enough to have somewhere to retreat to.
#[must_use]
pub fn facility_config() -> HexWfcConfig {
    HexWfcConfig {
        cols: 11,
        rows: 9,
        levels: 1,
        min_rooms: 3,
        max_rooms: 6,
        retry_budget: 200,
        min_room_distance: 2,
    }
}

/// The seed this preset composition is pinned to.
pub const FACILITY_SEED: u64 = 0x0B5E_51E6;

/// Solve the preset facility and turn it into a kinetic board.
///
/// # Errors
/// Propagates a solver failure rather than papering over it: a lab that silently
/// fell back to the authored rectangle would be claiming to test WFC geometry
/// while testing a hand-drawn plain.
pub fn facility(rules: KineticRules, siege: SiegeRules) -> Result<KineticWorld, HexWfcError> {
    let config = facility_config();
    let solved = HexWfcWorld::generate(FACILITY_SEED, config)?;
    Ok(from_solved(&solved, rules, siege))
}

/// Convert a solved facility into a kinetic board.
#[must_use]
pub fn from_solved(solved: &HexWfcWorld, rules: KineticRules, siege: SiegeRules) -> KineticWorld {
    let config = solved.config;
    let grid = HexGridSize {
        cols: config.cols,
        rows: config.rows,
        levels: 1,
    };

    let placement_at = |coord: HexCoord| -> Option<(bool, u8)> {
        let placement = solved.placements.get(&coord)?;
        let solid = placement.space != HexSpace::Void;
        let mut doors = 0u8;
        for face in HexFace::LATERAL {
            if placement.is_open(face) {
                doors |= 1 << face.index();
            }
        }
        Some((solid, doors))
    };

    let mut world = KineticWorld::from_placements(grid, 0, &placement_at, rules);
    world.siege = siege;

    // Stand the Observer somewhere with room around it, and put the generator
    // and a station on real floor rather than at the authored coordinates,
    // which almost certainly landed in void.
    let standable = world.standable_cells();
    let spawn = most_connected(&world, &standable).unwrap_or_default();
    world.observers[0].cell = spawn;

    world.generator = farthest_from(&standable, spawn).unwrap_or(spawn);
    world.stations = midpoint_from(&standable, spawn, world.generator)
        .map(|cell| {
            vec![Station {
                id: StationId(0),
                cell,
            }]
        })
        .unwrap_or_default();

    // Guardians start as far away as the floor allows; the siege supplies the
    // rest on its own schedule.
    let far = farthest_from(&standable, spawn).unwrap_or(spawn);
    for minor in &mut world.minors {
        minor.cell = far;
        minor.alive = false;
    }
    world.major.cell = far;

    world
}

/// The standable cell with the most open faces — the least cramped place to
/// start, and the one most likely to be a room rather than a dead-end corridor.
fn most_connected(world: &KineticWorld, cells: &[HexCoord]) -> Option<HexCoord> {
    cells.iter().copied().max_by_key(|coord| {
        let open = HexFace::LATERAL
            .iter()
            .filter(|face| world.passable_neighbor(*coord, **face).is_some())
            .count();
        // Ties break on coordinate so a re-solve of the same seed picks the
        // same spawn.
        (open, std::cmp::Reverse((coord.r, coord.q)))
    })
}

fn farthest_from(cells: &[HexCoord], from: HexCoord) -> Option<HexCoord> {
    cells
        .iter()
        .copied()
        .filter(|coord| *coord != from)
        .max_by_key(|coord| (lateral_distance(*coord, from), coord.r, coord.q))
}

/// A cell roughly between two others, for the recharge station: far enough from
/// spawn to be a trip, not so far that it is never worth making.
fn midpoint_from(cells: &[HexCoord], from: HexCoord, to: HexCoord) -> Option<HexCoord> {
    let span = lateral_distance(from, to).max(1);
    cells
        .iter()
        .copied()
        .filter(|coord| *coord != from && *coord != to)
        .min_by_key(|coord| {
            let a = lateral_distance(*coord, from) as i64;
            let b = lateral_distance(*coord, to) as i64;
            // Closest to equidistant, then stable by coordinate.
            (
                (a - b).abs(),
                (a + b - span as i64).abs(),
                coord.r as i64,
                coord.q as i64,
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preset_facility_solves() {
        let world = facility(KineticRules::default(), SiegeRules::default())
            .expect("the pinned seed solves");
        assert!(
            world.standable_cells().len() > 20,
            "the facility should have somewhere to stand"
        );
    }

    /// The point of using the solver at all: this board must have walls, and
    /// the authored rectangle must not.
    #[test]
    fn the_facility_has_walls_where_the_authored_board_has_none() {
        let facility = facility(KineticRules::default(), SiegeRules::default()).expect("solved");
        let sealed = facility
            .standable_cells()
            .into_iter()
            .flat_map(|coord| HexFace::LATERAL.map(move |face| (coord, face)))
            .filter(|(coord, face)| !facility.connected(*coord, *face))
            .count();
        assert!(sealed > 0, "a solved facility with no walls is a plain");

        let authored = KineticWorld::authored();
        let authored_sealed = authored
            .standable_cells()
            .into_iter()
            .flat_map(|coord| HexFace::LATERAL.map(move |face| (coord, face)))
            .filter(|(coord, face)| !authored.connected(*coord, *face))
            .count();
        assert_eq!(
            authored_sealed, 0,
            "the authored board is deliberately open; walls there would change every other test"
        );
    }

    /// Doors must agree from both sides, or a Guardian could walk through a wall
    /// one way and be stopped the other.
    #[test]
    fn every_door_is_mutual() {
        let world = facility(KineticRules::default(), SiegeRules::default()).expect("solved");
        for index in 0..world.grid.cell_count() {
            let coord = world.grid.coord(index);
            for face in HexFace::LATERAL {
                if let Some(next) = world.passable_neighbor(coord, face) {
                    assert!(
                        world.connected(next, face.opposite()),
                        "{coord:?} opens onto {next:?} but not back again"
                    );
                }
            }
        }
    }

    #[test]
    fn the_spawn_and_its_fixtures_are_on_real_floor() {
        let world = facility(KineticRules::default(), SiegeRules::default()).expect("solved");
        let spawn = world.observers[0].cell;
        assert!(world.cell(spawn).is_standable(), "spawned inside void");
        assert!(
            world.cell(world.generator).is_standable(),
            "the generator is in void"
        );
        for station in &world.stations {
            assert!(
                world.cell(station.cell).is_standable(),
                "a station is in void"
            );
        }
        assert_ne!(spawn, world.generator, "the generator is under your feet");
    }

    /// Solving the pinned seed twice must give the same board, or the "preset
    /// composition" is not one.
    /// Run the siege headlessly with an Observer who never moves.
    fn besiege(minutes: f32, jail: bool) -> KineticWorld {
        let rules = KineticRules {
            jail,
            siege: true,
            siege_minutes: minutes,
            ..Default::default()
        };
        let mut world = facility(rules, rules.siege_rules()).expect("solved");
        let ticks = world.siege.duration_ticks + 10;
        for _ in 0..ticks {
            world.step(&[]);
        }
        world
    }

    /// Waves arrive on schedule, grow, and respect the population ceiling.
    #[test]
    fn the_siege_releases_growing_waves() {
        let world = besiege(3.0, false);
        assert!(
            world.waves_released >= 12,
            "three minutes should be a dozen waves, got {}",
            world.waves_released
        );

        let siege = world.siege;
        assert_eq!(siege.wave_size(0), 1, "the first wave is a single Guardian");
        assert!(siege.wave_size(10) > siege.wave_size(0), "waves must grow");
        assert_eq!(
            siege.wave_size(1_000),
            siege.max_wave_size,
            "growth is capped"
        );
        assert!(
            world.minors.len() <= crate::model::MAX_LIVE_MINORS,
            "the population outgrew the pool presentation reserves for it"
        );
    }

    /// Standing still with jail on must lose, and the outcome must say so.
    #[test]
    fn standing_still_loses_the_siege() {
        let world = besiege(3.0, true);
        assert_eq!(world.outcome, crate::model::MatchOutcome::Lost);
        assert!(world.observers[0].jailed);
    }

    /// Surviving the clock must be reachable, and must be reported as a win
    /// rather than simply as "not lost".
    #[test]
    fn outlasting_the_clock_survives() {
        let mut world = besiege(1.0, false);
        assert_eq!(world.outcome, crate::model::MatchOutcome::Survived);
        assert!(!world.observers[0].jailed);

        // The outcome is terminal: more ticks must not un-win it, and no
        // further waves may arrive after the clock has stopped.
        let waves = world.waves_released;
        for _ in 0..600 {
            world.step(&[]);
        }
        assert_eq!(world.outcome, crate::model::MatchOutcome::Survived);
        assert_eq!(world.waves_released, waves, "a wave arrived after the end");
    }

    /// A siege is only reproducible if its spawns are.
    #[test]
    fn the_siege_is_reproducible() {
        let a = besiege(1.0, false);
        let b = besiege(1.0, false);
        assert_eq!(a.digest(), b.digest());
        assert_eq!(a.waves_released, b.waves_released);
        let cells_a: Vec<_> = a
            .minors
            .iter()
            .map(|minor| (minor.cell, minor.alive))
            .collect();
        let cells_b: Vec<_> = b
            .minors
            .iter()
            .map(|minor| (minor.cell, minor.alive))
            .collect();
        assert_eq!(cells_a, cells_b, "wave placement diverged between runs");
    }

    /// Nothing may *arrive* on top of the Observer.
    ///
    /// Note what this does not assert: that no Guardian ever shares the
    /// Observer's plate. With jail off one walks onto you eventually, and that
    /// is the rule working. The spawn is the part that has to be fair.
    #[test]
    fn waves_never_spawn_near_the_observer() {
        let rules = KineticRules {
            jail: false,
            siege: true,
            siege_minutes: 2.0,
            ..Default::default()
        };
        let mut world = facility(rules, rules.siege_rules()).expect("solved");
        let min = world.siege.min_spawn_distance;
        let mut releases = 0;

        for _ in 0..world.siege.duration_ticks {
            let here = world.observers[0].cell;
            world.step(&[]);
            for event in &world.events {
                if let crate::model::KineticEvent::MinorReleased { cell, .. } = event {
                    releases += 1;
                    assert!(
                        lateral_distance(*cell, here) >= min,
                        "a wave spawned {} plates from the Observer, minimum is {min}",
                        lateral_distance(*cell, here)
                    );
                    assert!(
                        world.cell(*cell).is_standable(),
                        "a wave spawned inside void"
                    );
                }
            }
        }
        assert!(releases > 0, "no wave ever arrived, so this proved nothing");
    }

    #[test]
    fn the_preset_is_reproducible() {
        let a = facility(KineticRules::default(), SiegeRules::default()).expect("solved");
        let b = facility(KineticRules::default(), SiegeRules::default()).expect("solved");
        assert_eq!(a.digest(), b.digest());
        assert_eq!(a.observers[0].cell, b.observers[0].cell);
    }
}
