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
    let solved = HexWfcWorld::generate(rules.facility_seed.unwrap_or(FACILITY_SEED), config)?;
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
    world.mark_unrailed_edges();

    // Stand the Observer somewhere with room around it, and put the generator
    // and a station on real floor rather than at the authored coordinates,
    // which almost certainly landed in void.
    let standable = world.standable_cells();
    let spawn = fighting_post(&world, &standable).unwrap_or_default();
    world.observers[0].cell = spawn;

    world.generator = farthest_from(&standable, spawn).unwrap_or(spawn);
    // The station sits next door, not halfway across the building. A siege is a
    // stand, and a refill you cannot reach without abandoning the position is
    // the same as no refill at all — the first cut put it at the midpoint and
    // the run simply went dry after three shoves and never recovered.
    world.stations = HexFace::LATERAL
        .into_iter()
        .filter_map(|face| world.passable_neighbor(spawn, face))
        .find(|cell| world.cell(*cell).is_standable())
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

/// Where to make a stand.
///
/// Not simply the roomiest cell. The tool kills by putting things into the
/// architecture, so a position with no void within a push of it cannot kill
/// anything — the first cut spawned in the middle of the structure and every
/// single shove landed `Rest` on more floor. A fighting post is a cell with
/// open ground to retreat through *and* a hole beside it to shove things into.
fn fighting_post(world: &KineticWorld, cells: &[HexCoord]) -> Option<HexCoord> {
    cells.iter().copied().max_by_key(|coord| {
        let open = HexFace::LATERAL
            .iter()
            .filter(|face| world.passable_neighbor(*coord, **face).is_some())
            .count();
        // A face that opens onto nothing is an edge to shove things over.
        let edges = HexFace::LATERAL
            .iter()
            .filter(|face| {
                world
                    .grid
                    .neighbor(*coord, **face)
                    .is_none_or(|next| !world.cell(next).is_standable())
            })
            .count();
        // Ties break on coordinate so a re-solve of the same seed picks the
        // same post.
        (edges.min(3), open, std::cmp::Reverse((coord.r, coord.q)))
    })
}

fn farthest_from(cells: &[HexCoord], from: HexCoord) -> Option<HexCoord> {
    cells
        .iter()
        .copied()
        .filter(|coord| *coord != from)
        .max_by_key(|coord| (lateral_distance(*coord, from), coord.r, coord.q))
}

#[cfg(test)]
mod tests {
    use super::*;
    const fn coord(q: u16, r: u16) -> HexCoord {
        HexCoord { q, r, level: 0 }
    }

    #[test]
    #[ignore = "diagnostic: cargo test -p kinetic_lab -- --ignored --nocapture facility_shape"]
    fn facility_shape() {
        let world = facility(KineticRules::default(), SiegeRules::default()).expect("solved");
        let standable = world.standable_cells();
        let mut histogram = [0usize; 7];
        for coord in &standable {
            let open = HexFace::LATERAL
                .iter()
                .filter(|face| world.connected(*coord, **face))
                .count();
            histogram[open] += 1;
        }
        println!("standable plates: {}", standable.len());
        println!("of {} cells total", world.grid.cell_count());
        for (open, count) in histogram.iter().enumerate() {
            println!("  {open} open faces: {count} plates");
        }
        let longest = standable
            .iter()
            .flat_map(|coord| HexFace::LATERAL.map(move |face| (*coord, face)))
            .map(|(coord, face)| {
                let mut run = 0;
                let mut cursor = coord;
                while let Some(next) = world.passable_neighbor(cursor, face) {
                    run += 1;
                    cursor = next;
                    if run > 20 {
                        break;
                    }
                }
                run
            })
            .max()
            .unwrap_or(0);
        println!("longest unobstructed run: {longest} plates");
    }

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
    ///
    /// This is the test that catches a Guardian which cannot navigate. Pursuit
    /// was greedy — take the neighbouring plate that reduces the distance most —
    /// which works on an open board and strands a Guardian in the first corridor
    /// that runs the wrong way before it runs the right way. On this facility
    /// that meant the waves milled about near their spawns and an Observer who
    /// never moved *won*, which looks exactly like a working siege until you
    /// watch one.
    #[test]
    fn standing_still_loses_the_siege() {
        let world = besiege(3.0, true);
        assert_eq!(world.outcome, crate::model::MatchOutcome::Lost);
        assert!(world.observers[0].jailed);
    }

    /// Every plate must be able to route to every other, or some spawn is a
    /// pocket nothing can leave.
    #[test]
    fn the_facility_is_navigable_from_everywhere() {
        let world = facility(KineticRules::default(), SiegeRules::default()).expect("solved");
        let standable = world.standable_cells();
        let home = world.observers[0].cell;
        let stranded: Vec<_> = standable
            .iter()
            .copied()
            .filter(|coord| *coord != home)
            .filter(|coord| world.route_step(*coord, home, &|_| false).is_none())
            .collect();
        assert!(
            stranded.is_empty(),
            "{} plates cannot reach the Observer at all: {stranded:?}",
            stranded.len()
        );
    }

    /// Routing must go *around* a wall, not give up at it.
    #[test]
    fn a_route_goes_around_a_sealed_face() {
        let mut world = KineticWorld::authored();
        let from = coord(3, 3);
        let to = coord(5, 3);
        assert_eq!(
            world.route_step(from, to, &|_| false),
            Some(coord(4, 3)),
            "with no wall the route is the straight one"
        );

        // Seal the direct corridor.
        world.seal_face(coord(3, 3), HexFace::East);
        world.seal_face(coord(4, 3), HexFace::East);

        let step = world
            .route_step(from, to, &|_| false)
            .expect("a walled corridor has a way around, and routing must find it");
        assert_ne!(step, coord(4, 3), "that step is through the wall");
        assert!(
            world.passable_neighbor(from, world.face_toward(from, step).unwrap()) == Some(step)
        );
    }

    /// Surviving the clock must be reachable, and must be reported as a win
    /// rather than simply as "not lost".
    #[test]
    fn outlasting_the_clock_survives() {
        let mut world = besiege(1.0, false);
        assert_eq!(world.outcome, crate::model::MatchOutcome::Survived);
        assert!(!world.observers[0].jailed);

        // The outcome is terminal: more ticks must not un-win it, no further
        // waves may arrive, and — the part this missed — the Guardians already
        // on the board must stop. A recorded run ended with SURVIVED on the
        // overlay and JAILED in the HUD in the same frame, eleven seconds after
        // the clock ran out, because they kept walking.
        let waves = world.waves_released;
        let before: Vec<_> = world.minors.iter().map(|minor| minor.cell).collect();
        for _ in 0..600 {
            world.step(&[]);
        }
        let after: Vec<_> = world.minors.iter().map(|minor| minor.cell).collect();
        assert_eq!(world.outcome, crate::model::MatchOutcome::Survived);
        assert_eq!(world.waves_released, waves, "a wave arrived after the end");
        assert_eq!(before, after, "a decided siege must freeze its Guardians");
    }

    /// And with jail *on*, the same freeze is what stops a win turning into a
    /// capture. `besiege(_, false)` cannot see this, because nothing can jail
    /// anyone there in the first place.
    #[test]
    fn winning_cannot_be_taken_back_by_a_guardian() {
        // A short clock, so it runs out before a wave can cross the facility:
        // `besiege(1.0, true)` is a *loss*, which is its own test.
        let mut world = besiege(0.1, true);
        assert_eq!(world.outcome, crate::model::MatchOutcome::Survived);
        assert!(!world.observers[0].jailed, "the bot survived the clock");

        // Put a Guardian directly on top of the Observer. Before the freeze
        // this was an immediate capture.
        let cell = world.observers[0].cell;
        match world.minors.iter_mut().find(|minor| minor.alive) {
            Some(minor) => minor.cell = cell,
            None => world.minors.push(crate::model::MinorGuardian {
                id: crate::model::MinorGuardianId(0),
                cell,
                stagger: 0,
                step_progress: 0,
                alive: true,
            }),
        }

        for _ in 0..120 {
            world.step(&[]);
        }
        assert!(
            !world.observers[0].jailed,
            "a won siege must not be able to jail you afterwards"
        );
        assert_eq!(world.outcome, crate::model::MatchOutcome::Survived);
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
