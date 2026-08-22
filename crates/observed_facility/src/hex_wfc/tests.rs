use std::collections::BTreeSet;

use observed_hex::lateral_distance;

use super::*;

fn corpus_seeds() -> impl Iterator<Item = u64> {
    (0..100u64).map(|n| 0xA11C_E000_0000_0000 | n)
}

fn config_3d() -> HexWfcConfig {
    HexWfcConfig {
        levels: 4,
        ..HexWfcConfig::default()
    }
}

fn corpus_seeds_3d() -> impl Iterator<Item = u64> {
    (0..100u64).map(|n| 0xA11C_E3D0_0000_0000 | n)
}

#[test]
fn the_same_seed_yields_identical_placements_and_trace() {
    let config = HexWfcConfig::default();
    let (first, first_trace) =
        HexWfcWorld::generate_traced(42, config).expect("seed 42 must solve");
    let (second, second_trace) =
        HexWfcWorld::generate_traced(42, config).expect("seed 42 must solve");
    assert_eq!(first.placements, second.placements);
    assert_eq!(first.blueprints, second.blueprints);
    assert_eq!(first_trace, second_trace);
    assert_eq!(first.last_attempts, second.last_attempts);
}

#[test]
fn determinism_holds_for_the_3d_config() {
    let config = config_3d();
    let (first, first_trace) =
        HexWfcWorld::generate_traced(0xA11C_E3D0_0000_0007, config).expect("must solve");
    let (second, second_trace) =
        HexWfcWorld::generate_traced(0xA11C_E3D0_0000_0007, config).expect("must solve");
    assert_eq!(first.placements, second.placements);
    assert_eq!(first.blueprints, second.blueprints);
    assert_eq!(first_trace, second_trace);
}

#[test]
fn production_generation_guarantees_every_arc_m_gameplay_room() {
    use crate::map_spec::RoomRole;

    let world = HexWfcWorld::generate(0xA11C_9600_0000_0001, HexWfcConfig::arc_default())
        .expect("production Arc M seed solves");
    let roles = world
        .blueprints
        .iter()
        .map(|blueprint| blueprint.role)
        .collect::<Vec<_>>();
    for role in [
        RoomRole::Start,
        RoomRole::Exit,
        RoomRole::GuardianControl,
        RoomRole::Decision,
        RoomRole::Keystone,
        RoomRole::DualStation,
        RoomRole::Monitor,
        RoomRole::AnchorCheckpoint,
        RoomRole::Recovery,
    ] {
        assert!(roles.contains(&role), "production omitted {role:?}");
    }
}

#[test]
fn traced_and_untraced_solves_agree() {
    let config = HexWfcConfig::default();
    let plain = HexWfcWorld::generate(7, config).expect("seed 7 must solve");
    let (traced, steps) = HexWfcWorld::generate_traced(7, config).expect("seed 7 must solve");
    assert_eq!(plain.placements, traced.placements);
    assert!(
        steps
            .iter()
            .any(|step| matches!(step, SolveStep::Completed { .. })),
        "a successful traced solve ends with Completed"
    );
}

/// Every stamped world must satisfy the structural invariants: edge symmetry
/// across all faces, room↔room openings only inside one blueprint footprint,
/// blueprint footprints disjoint and spaced by `min_room_distance`, and a
/// spawn→exit route.
fn assert_world_valid(world: &HexWfcWorld, seed: u64) {
    let config = world.config;
    let grid = config.grid();

    assert_eq!(world.placements[&config.spawn()].space, HexSpace::Room);
    assert_eq!(world.placements[&config.exit()].space, HexSpace::Room);
    let route = world
        .route_between(config.spawn(), config.exit())
        .unwrap_or_else(|| panic!("seed {seed:#x} has no spawn->exit route"));
    assert!(route.len() >= 2);

    // Blueprint count in range, footprints disjoint, anchors spaced apart.
    assert!(
        (config.min_rooms..=config.max_rooms).contains(&world.blueprints.len()),
        "seed {seed:#x}: blueprint count {} outside range",
        world.blueprints.len()
    );
    let mut occupied = BTreeSet::new();
    for stamped in &world.blueprints {
        for &cell in &stamped.cells {
            assert!(
                occupied.insert(cell),
                "seed {seed:#x}: blueprint footprints overlap at {cell:?}"
            );
        }
    }
    for i in 0..world.blueprints.len() {
        for j in (i + 1)..world.blueprints.len() {
            assert!(
                lateral_distance(world.blueprints[i].anchor, world.blueprints[j].anchor)
                    >= config.min_room_distance,
                "seed {seed:#x}: anchors closer than min_room_distance"
            );
        }
    }

    for placement in world.placements.values() {
        for face in HexFace::ALL {
            if face.is_lateral() {
                let open = placement.is_open(face);
                match grid.neighbor(placement.coord, face) {
                    Some(neighbor) => {
                        let other = &world.placements[&neighbor];
                        assert_eq!(
                            open,
                            other.is_open(face.opposite()),
                            "seed {seed:#x}: asymmetric edge at {:?} {face:?}",
                            placement.coord
                        );
                        if open
                            && placement.space == HexSpace::Room
                            && other.space == HexSpace::Room
                        {
                            assert!(
                                world.blueprints.iter().any(|blueprint| {
                                    blueprint.cells.contains(&placement.coord)
                                        && blueprint.cells.contains(&neighbor)
                                }),
                                "seed {seed:#x}: room-room opening outside one footprint at {:?}",
                                placement.coord
                            );
                        }
                    }
                    None => assert!(
                        !open,
                        "seed {seed:#x}: boundary door at {:?} {face:?}",
                        placement.coord
                    ),
                }
            } else {
                let a_port = if face == HexFace::Up {
                    placement.up
                } else {
                    placement.down
                };
                match grid.neighbor(placement.coord, face) {
                    Some(neighbor) => {
                        let other = &world.placements[&neighbor];
                        let b_port = if face == HexFace::Up {
                            other.down
                        } else {
                            other.up
                        };
                        assert_eq!(
                            a_port as u8, b_port as u8,
                            "seed {seed:#x}: vertical port mismatch at {:?} {face:?}",
                            placement.coord
                        );
                    }
                    None => assert_eq!(
                        a_port,
                        PortClass::Sealed,
                        "seed {seed:#x}: boundary vertical port at {:?} {face:?}",
                        placement.coord
                    ),
                }
            }
        }
    }
}

#[test]
fn a_hundred_seed_corpus_solves_and_validates() {
    let config = HexWfcConfig::default();
    for seed in corpus_seeds() {
        let world = HexWfcWorld::generate(seed, config)
            .unwrap_or_else(|error| panic!("seed {seed:#x} failed: {error:?}"));
        assert_world_valid(&world, seed);
    }
}

#[test]
fn a_hundred_seed_3d_corpus_solves_and_validates() {
    let config = config_3d();
    for seed in corpus_seeds_3d() {
        let world = HexWfcWorld::generate(seed, config)
            .unwrap_or_else(|error| panic!("3D seed {seed:#x} failed: {error:?}"));
        assert_world_valid(&world, seed);
    }
}

/// No `RampHead` sits without its matching `RampUp` below, and no `RampUp`
/// without its `RampHead` above — over the whole 3D corpus.
#[test]
fn ramp_pairs_never_orphan_over_the_3d_corpus() {
    let config = config_3d();
    for seed in corpus_seeds_3d() {
        let world = HexWfcWorld::generate(seed, config).expect("must solve");
        let grid = config.grid();
        for placement in world.placements.values() {
            match placement.archetype {
                HexArchetype::RampUp => {
                    let above = grid
                        .neighbor(placement.coord, HexFace::Up)
                        .map(|c| world.placements[&c].archetype);
                    assert_eq!(
                        above,
                        Some(HexArchetype::RampHead),
                        "seed {seed:#x}: RampUp at {:?} not capped by RampHead",
                        placement.coord
                    );
                }
                HexArchetype::RampHead => {
                    let below = grid
                        .neighbor(placement.coord, HexFace::Down)
                        .map(|c| world.placements[&c].archetype);
                    assert_eq!(
                        below,
                        Some(HexArchetype::RampUp),
                        "seed {seed:#x}: RampHead at {:?} not seated on RampUp",
                        placement.coord
                    );
                }
                _ => {}
            }
        }
    }
}

/// A vertical `GuardianControl` atrium blueprint stamps somewhere on the 3D
/// corpus, and it carries an internal shaft between its two levels.
#[test]
fn a_two_level_atrium_blueprint_stamps_on_the_3d_corpus() {
    let config = config_3d();
    let mut found = false;
    for seed in corpus_seeds_3d() {
        let world = HexWfcWorld::generate(seed, config).expect("must solve");
        if let Some(atrium) = world
            .blueprints
            .iter()
            .find(|b| b.role == crate::map_spec::RoomRole::GuardianControl)
        {
            assert_eq!(atrium.cells.len(), 2, "atrium spans two cells");
            let levels: BTreeSet<u8> = atrium.cells.iter().map(|c| c.level).collect();
            assert_eq!(levels.len(), 2, "atrium spans two levels");
            let lower = atrium.cells.iter().min_by_key(|c| c.level).unwrap();
            assert_eq!(
                world.placements[lower].up,
                PortClass::ShaftOpen,
                "atrium lower cell opens a shaft upward"
            );
            found = true;
            break;
        }
    }
    assert!(found, "some 3D seed stamps a GuardianControl atrium");
}

fn tallest_shaft_column(world: &HexWfcWorld) -> u8 {
    let grid = world.config.grid();
    let mut best = 0;
    for q in 0..grid.cols {
        for r in 0..grid.rows {
            let mut run = 0u8;
            for level in 0..grid.levels {
                let coord = HexCoord { q, r, level };
                let vertical = world.placements.get(&coord).is_some_and(|p| {
                    p.up == PortClass::ShaftOpen || p.down == PortClass::ShaftOpen
                });
                if vertical {
                    run += 1;
                    best = best.max(run);
                } else {
                    run = 0;
                }
            }
        }
    }
    best
}

fn tallest_ramp_chain(world: &HexWfcWorld) -> u8 {
    // Longest ladder of stacked ramp pairs: each `RampUp` climbs one level,
    // exiting laterally through its `RampHead` into the next base.
    let grid = world.config.grid();
    let mut best = 0;
    for (&coord, placement) in &world.placements {
        if placement.archetype != HexArchetype::RampUp {
            continue;
        }
        let below_is_head = grid
            .neighbor(coord, HexFace::Down)
            .is_some_and(|c| world.placements[&c].archetype == HexArchetype::RampHead);
        if below_is_head {
            continue; // count from the base of a chain only
        }
        let mut climbed = 0u8;
        let mut current = coord;
        loop {
            climbed += 1;
            let Some(head) = grid.neighbor(current, HexFace::Up) else {
                break;
            };
            let mut advanced = false;
            for face in HexFace::LATERAL {
                if world.placements[&head].is_open(face)
                    && let Some(next) = grid.neighbor(head, face)
                    && world.placements[&next].archetype == HexArchetype::RampUp
                {
                    current = next;
                    advanced = true;
                    break;
                }
            }
            if !advanced {
                break;
            }
        }
        best = best.max(climbed);
    }
    best
}

/// The showcase seed that opened Arc L. Kept as the *starting* point of the
/// corpus below rather than as the sole subject: composition changes with every
/// arc that touches weighting, so a single pinned seed re-breaks this test each
/// time while the property it guards — that the solver still builds tall
/// verticals — is untouched.
const PINNED_3D_SEED: u64 = 0xA11C_E3D0_0000_0008;

#[test]
fn the_solver_still_builds_full_height_shafts_and_multi_level_ramp_chains() {
    // Verticality is asserted at **production** scale, not on the compact
    // fixture. A three-level ramp chain needs three of four levels on a 12x9
    // grid, which composition changes can legitimately price out without the
    // solver having lost the capability — measured, the compact config tops out
    // at two chained ramps while `arc_default` still reaches three and stacks a
    // full ten-level shaft. The capability is the invariant; the fixture is not.
    let config = HexWfcConfig::arc_default();
    let mut best_shaft = 0;
    let mut best_ramp = 0;
    let mut solved = 0;
    for step in 0u64..3 {
        let seed = PINNED_3D_SEED ^ step.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let Ok(world) = HexWfcWorld::generate(seed, config) else {
            continue;
        };
        solved += 1;
        best_shaft = best_shaft.max(tallest_shaft_column(&world));
        best_ramp = best_ramp.max(tallest_ramp_chain(&world));
    }
    assert!(solved >= 2, "only {solved} of 3 production seeds solved");
    assert!(
        best_shaft >= 4,
        "no production seed built a tall shaft column (best {best_shaft})"
    );
    assert!(
        best_ramp >= 3,
        "no production seed built a three-level ramp chain (best {best_ramp})"
    );

    // The pinned compact seed still has to solve and route, even if its own
    // composition has moved on.
    let config = config_3d();
    let world = HexWfcWorld::generate(PINNED_3D_SEED, config).expect("pinned seed must solve");

    let route = world
        .route_between(config.spawn(), config.exit())
        .expect("pinned seed route");
    let crosses_vertical = route.iter().any(|coord| {
        let p = &world.placements[coord];
        p.up != PortClass::Sealed || p.down != PortClass::Sealed
    });
    assert!(crosses_vertical, "pinned route stays flat");
}

/// Diagnostic search used to (re)pin the showcase seeds; ignored in normal
/// runs. Run with `--ignored` to print candidate seeds.
#[test]
#[ignore]
fn search_for_pinnable_3d_seeds() {
    let config = config_3d();
    let mut best_shaft = (0u8, 0u64);
    let mut best_ramp = (0u8, 0u64);
    let mut hits = Vec::new();
    for n in 0..160u64 {
        let seed = 0xA11C_E3D0_0000_0000 | n;
        if let Ok(world) = HexWfcWorld::generate(seed, config) {
            let shaft = tallest_shaft_column(&world);
            let ramp = tallest_ramp_chain(&world);
            let route_vertical = world
                .route_between(config.spawn(), config.exit())
                .is_some_and(|route| {
                    route.iter().any(|c| {
                        let p = &world.placements[c];
                        p.up != PortClass::Sealed || p.down != PortClass::Sealed
                    })
                });
            if shaft > best_shaft.0 {
                best_shaft = (shaft, seed);
            }
            if ramp > best_ramp.0 {
                best_ramp = (ramp, seed);
            }
            if shaft >= 4 && ramp >= 3 && route_vertical {
                hits.push((format!("{seed:#x}"), shaft, ramp));
            }
        }
    }
    println!("best_shaft={best_shaft:x?} best_ramp={best_ramp:x?}");
    println!("shaft>=4 & ramp>=3 & vertical route: {hits:?}");
    assert!(!hits.is_empty(), "no pinnable seed found in search range");
}

fn fnv1a(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// Refactor audit: stable digests of materialized placements across explicit
/// `(seed, generation, attempt)` triples and whole solves. Run with
/// `--ignored --nocapture` before and after solver-internal changes; the
/// printed digests must not move.
#[test]
#[ignore = "solver-refactor placement-digest audit"]
fn print_placement_digests() {
    let small = HexWfcConfig::default();
    let showcase = config_3d();
    for (label, config, seed) in [
        ("small", small, 42u64),
        ("small", small, 7),
        ("small", small, 0xA11C_E000_0000_0005),
        ("showcase", showcase, 0xA11C_E3D0_0000_0007),
        ("showcase", showcase, PINNED_3D_SEED),
    ] {
        let world = HexWfcWorld::generate(seed, config).expect("digest seed must solve");
        let digest = fnv1a(&format!("{:?}|{:?}", world.placements, world.blueprints));
        println!(
            "{label} seed={seed:#x} attempts={} digest={digest:#018x}",
            world.last_attempts
        );
    }
    let no_pins = BTreeSet::new();
    for (label, config, seed, generation, attempt) in [
        ("small", small, 42u64, 0u32, 0u32),
        ("small", small, 42, 3, 1),
        ("small", small, 7, 0, 3),
        ("small", small, 0xA11C_E000_0000_0005, 0, 3),
        ("showcase", showcase, 0xA11C_E3D0_0000_0007, 0, 2),
        ("showcase", showcase, PINNED_3D_SEED, 0, 1),
    ] {
        let digest = match collapse::collapse_attempt(
            seed,
            generation,
            attempt,
            config,
            None,
            &no_pins,
            &super::profile::HexCompositionProfile::baseline(),
            None,
        ) {
            Ok(solved) => fnv1a(&format!("{:?}|{:?}", solved.placements, solved.blueprints)),
            Err(reason) => fnv1a(reason),
        };
        println!("{label} seed={seed:#x} gen={generation} attempt={attempt} digest={digest:#018x}");
    }
}

/// Manual Arc L performance audit at the 5,600-cell production dimensions.
/// Run with `--ignored --nocapture` to print the wall-clock solve time.
#[test]
#[ignore = "production-scale solve timing audit"]
fn time_arc_default_production_solve() {
    let config = HexWfcConfig::arc_default();
    let start = std::time::Instant::now();
    let world =
        HexWfcWorld::generate(0xA11C_9300_0000_0001, config).expect("arc-default seed must solve");
    let elapsed = start.elapsed();
    println!(
        "arc_default 28x20x10 solved in {elapsed:?} (attempts={}, rooms={})",
        world.last_attempts,
        world.room_count()
    );
    assert!(
        elapsed.as_secs() < 10,
        "production solve took {elapsed:?}; budget is 10s"
    );
}

#[test]
fn ports_view_matches_the_placement() {
    let config = config_3d();
    let world = HexWfcWorld::generate(3, config).expect("seed 3 must solve");
    for placement in world.placements.values() {
        let ports = placement.ports();
        for face in HexFace::LATERAL {
            let expected = if placement.is_open(face) {
                PortClass::Door
            } else {
                PortClass::Sealed
            };
            assert_eq!(ports.port(face), expected);
        }
        assert_eq!(ports.port(HexFace::Up), placement.up);
        assert_eq!(ports.port(HexFace::Down), placement.down);
    }
}

#[test]
fn demandable_signatures_are_distinct_and_include_vertical_classes() {
    let signatures = demandable_signatures();
    let unique: BTreeSet<_> = signatures.iter().copied().collect();
    assert_eq!(unique.len(), signatures.len(), "list has duplicates");
    assert!(
        signatures
            .iter()
            .any(|s| s.port(HexFace::Up) == PortClass::ShaftOpen),
        "coverage feed must demand ShaftOpen up faces"
    );
    assert!(
        signatures
            .iter()
            .any(|s| s.port(HexFace::Up) == PortClass::RampOpen),
        "coverage feed must demand RampOpen up faces"
    );
    assert!(
        signatures
            .iter()
            .any(|s| s.port(HexFace::Down) == PortClass::RampOpen),
        "coverage feed must demand RampOpen down faces"
    );
}

#[test]
fn different_seeds_produce_different_layouts() {
    let config = HexWfcConfig::default();
    let a = HexWfcWorld::generate(100, config).expect("seed 100 must solve");
    let b = HexWfcWorld::generate(101, config).expect("seed 101 must solve");
    assert_ne!(a.placements, b.placements);
}

#[test]
fn invalid_configs_are_rejected() {
    let config = HexWfcConfig {
        cols: 2,
        ..HexWfcConfig::default()
    };
    assert_eq!(
        HexWfcWorld::generate(1, config),
        Err(HexWfcError::InvalidConfig)
    );
}

/// The bound in [`HexWfcWorld::route_within_cost`] must be exact, not heuristic: inside it
/// the answer has to be byte-for-byte the unbounded one, and outside it `None`. This is
/// what lets `pressure_for` and the Guardian visibility test bound their searches at the
/// cost where their answers saturate without changing any behaviour.
#[test]
fn bounded_routing_agrees_with_unbounded_inside_the_bound() {
    let mut checked = 0usize;
    let mut saw_truncation = false;
    for seed in corpus_seeds().take(12) {
        let world = HexWfcWorld::generate(seed, HexWfcConfig::default()).expect("seed generates");
        let live: Vec<HexCoord> = world
            .placements
            .iter()
            .filter(|(_, placement)| placement.space != HexSpace::Void)
            .map(|(coord, _)| *coord)
            .collect();
        let Some(&from) = live.first() else { continue };
        for &to in live.iter().step_by(7) {
            let unbounded = world.route_between_cells(from, to);
            for bound in [0, 1_000, 4_000, 12_000, u32::MAX] {
                let bounded = world.route_within_cost(from, to, bound);
                match &unbounded {
                    Some(route) if route.cost_millis <= bound => {
                        let bounded = bounded.expect("a route inside the bound must be found");
                        assert_eq!(
                            bounded.cells, route.cells,
                            "seed {seed:x} bound {bound}: bounded route diverged"
                        );
                        assert_eq!(bounded.cost_millis, route.cost_millis);
                    }
                    _ => {
                        saw_truncation |= bounded.is_none() && unbounded.is_some();
                        assert!(
                            bounded.is_none(),
                            "seed {seed:x} bound {bound}: found a route the bound excludes"
                        );
                    }
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "corpus must actually exercise the bound");
    assert!(
        saw_truncation,
        "corpus must include pairs the bound genuinely excludes, or this proves nothing"
    );
}

/// Rooms belong to districts, and the binding actually holds on real seeds.
///
/// This is the legibility payoff the arc is for: recognising a district should
/// tell a player what it holds. The binding is a preference rather than a
/// constraint — a seed can put a role's districts somewhere a room will not fit
/// — so this asserts it dominates rather than that it never yields. Losing a
/// room to an unplaceable role would be a far worse failure than a Monitor
/// turning up somewhere odd.
#[test]
fn stamped_rooms_land_in_the_districts_their_role_belongs_to() {
    let mut bound = 0usize;
    let mut fell_back = 0usize;
    let mut forks = 0usize;
    for raw in 0u64..12 {
        let seed = 0xa11c_e3d0_0000_0000 ^ raw.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let Ok(world) = HexWfcWorld::generate(seed, super::HexWfcConfig::arc_default()) else {
            continue;
        };
        for stamped in &world.blueprints {
            if stamped.role == crate::map_spec::RoomRole::DecoherenceFork {
                forks += 1;
            }
            let Some(register) = world.architecture.get(&stamped.anchor) else {
                continue;
            };
            let wanted = super::constraints::role_districts_for_probe(stamped.role);
            if wanted.is_empty() {
                continue;
            }
            if wanted.contains(register) {
                bound += 1;
            } else {
                fell_back += 1;
            }
        }
    }
    assert!(bound + fell_back > 60, "unexpectedly small sample");
    let ratio = bound as f64 / (bound + fell_back) as f64;
    assert!(
        ratio >= 0.9,
        "only {bound} of {} rooms landed in their own district",
        bound + fell_back
    );

    // The largest authored room in the corpus had a blueprint, a `.map` module
    // and no way into a match: it was absent from the stamping pool, and the
    // room-count target could never reach the pool's last slot anyway. Bug
    // backlog #16. It should be rare, not impossible.
    assert!(forks > 0, "DecoherenceFork still never reaches a facility");
}

/// Does a shaft column place a tower at every level, or every other one?
///
/// This decides what `levels: 2` means on a stair tower, and with it where its
/// `up` port belongs. Counting the solver's own placements rather than checking
/// an assumption: if consecutive levels in one column are both `Shaft`, then
/// every cell gets its own tower and each tower climbs exactly one level, so
/// `levels: 2` is a reservation for the flight to poke into the cell above.
#[test]
#[ignore = "diagnostic"]
fn survey_how_shaft_columns_stack() {
    use std::collections::BTreeMap;

    let config = HexWfcConfig::arc_default();
    let world = HexWfcWorld::generate(0x5EED_C0DE, config).expect("must solve");

    // Group shaft cells by plan column.
    let mut columns: BTreeMap<(u16, u16), Vec<u8>> = BTreeMap::new();
    for (coord, placement) in &world.placements {
        if placement.archetype == HexArchetype::Shaft {
            columns
                .entry((coord.q, coord.r))
                .or_default()
                .push(coord.level);
        }
    }

    let mut adjacent = 0;
    let mut gapped = 0;
    let mut tallest = 0;
    for levels in columns.values() {
        let mut levels = levels.clone();
        levels.sort_unstable();
        tallest = tallest.max(levels.len());
        for pair in levels.windows(2) {
            if pair[1] - pair[0] == 1 {
                adjacent += 1;
            } else {
                gapped += 1;
            }
        }
    }
    println!(
        "shaft columns={} tallest={tallest} adjacent_pairs={adjacent} gapped_pairs={gapped}",
        columns.len()
    );

    // And what the ports say: a Through shaft claims open above and below.
    let mut through = 0;
    for placement in world.placements.values() {
        if placement.archetype == HexArchetype::Shaft
            && placement.up == PortClass::ShaftOpen
            && placement.down == PortClass::ShaftOpen
        {
            through += 1;
        }
    }
    println!("through shafts (open above and below)={through}");
}

/// How often does a stamped room end up unreachable, and what does the room
/// graph actually look like at production scale?
///
/// `prune_disconnected` runs before `layout_failure`, so "disconnected
/// component survived" can never fire: an island of rooms and halls is voided
/// first and the check then finds nothing wrong. But `room_quota_failure` and
/// `HexWfcWorld::rooms()` both read `blueprints`, never `placements`, so a
/// voided room still satisfies its quota and still hands out a `RoomId` — and
/// `project_blueprint` will project its objective sockets into cells that are
/// now `Void`. A `Void` cell is never in the exit component, so
/// `validate_patch_routes_and_boundary` then refuses every relayout commit for
/// the rest of the match.
///
/// That is the chain this measures. `phantom` is the number it exists for:
/// anything above zero means the chain is reachable in practice.
///
/// The rest is baseline. `attempts` and wall-clock are what any later routing
/// work has to be measured against — the desync and late-join paths run this
/// solve synchronously against a two-second LAN client timeout, so a change
/// that multiplies attempts is not affordable however good the layouts get.
/// `halls` tests a second claim: `score::connectivity_score` adds an uncapped
/// `sqrt(hall components)`, so if facilities routinely carry one large hall
/// network then that term is noise, and if they carry many then the score is
/// actively selecting for shattered ones.
///
/// Run with `--ignored --nocapture`.
#[test]
#[ignore = "production-scale room-graph survey"]
fn survey_room_graph_at_production_scale() {
    use std::collections::{BTreeMap, BTreeSet};

    let config = HexWfcConfig::arc_default();
    let quotas = HexRoomQuotas::for_team_count(2);

    let mut phantom_total = 0usize;
    let mut solved = 0usize;
    let mut worst_attempts = 0u32;
    let mut slowest = std::time::Duration::ZERO;

    println!("seed             attempts   solve   rooms  phantom  halls  deg1  mean_deg  far");
    for index in 0..24u64 {
        let seed = 0xA11C_E3D0_0000_0000 ^ (index.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let start = std::time::Instant::now();
        let Ok(world) = HexWfcWorld::generate_with_room_quotas(seed, config, quotas) else {
            println!("{seed:016x}  UNSOLVED");
            continue;
        };
        let elapsed = start.elapsed();
        solved += 1;
        worst_attempts = worst_attempts.max(world.last_attempts);
        slowest = slowest.max(elapsed);

        // A room is real only if at least one of its cells survived the prune.
        let live = super::topology::active_component(config, &world.placements, config.spawn());
        let phantom = world
            .blueprints
            .iter()
            .filter(|blueprint| !blueprint.cells.iter().any(|cell| live.contains(cell)))
            .count();
        phantom_total += phantom;

        // Degree over the derived room graph: distinct rooms each room shares a
        // corridor with, which is what `threshold_attachments` already answers.
        let mut rooms_on_corridor: BTreeMap<
            observed_core::CorridorId,
            BTreeSet<observed_core::RoomId>,
        > = BTreeMap::new();
        for attachment in world.threshold_attachments() {
            rooms_on_corridor
                .entry(attachment.corridor)
                .or_default()
                .insert(attachment.room);
        }
        let mut degree: BTreeMap<observed_core::RoomId, BTreeSet<observed_core::RoomId>> =
            BTreeMap::new();
        for rooms in rooms_on_corridor.values() {
            for room in rooms {
                let peers = rooms.iter().filter(|other| *other != room).copied();
                degree.entry(*room).or_default().extend(peers);
            }
        }
        let room_count = world.blueprints.len();
        let degree_one = degree.values().filter(|peers| peers.len() == 1).count()
            + room_count.saturating_sub(degree.len());
        #[allow(clippy::cast_precision_loss)]
        let mean_degree = if degree.is_empty() {
            0.0
        } else {
            degree.values().map(BTreeSet::len).sum::<usize>() as f64 / room_count as f64
        };

        // Furthest room from spawn, in the same weighted metric the solver's own
        // A* heuristic uses.
        let far = world
            .blueprints
            .iter()
            .map(|blueprint| observed_hex::travel_distance(config.spawn(), blueprint.anchor))
            .max()
            .unwrap_or(0);

        println!(
            "{seed:016x}  {:>8}  {:>6.2}s  {:>5}  {:>7}  {:>5}  {:>4}  {:>8.2}  {:>3}",
            world.last_attempts,
            elapsed.as_secs_f64(),
            room_count,
            phantom,
            world.corridors().len(),
            degree_one,
            mean_degree,
            far
        );
    }

    println!(
        "\nsolved {solved}/24 | phantom rooms total {phantom_total} | \
         worst attempts {worst_attempts} | slowest {slowest:?}"
    );
    println!(
        "phantom > 0 means an unreachable room keeps its quota slot and its RoomId, \
         projects objective sockets into Void, and freezes relayout for the match."
    );
}

/// The seed that produced a phantom room must not produce one any more.
///
/// A hand-built fixture was tried first and abandoned, which is worth recording
/// because the reason is the defect itself. Voiding a room by hand leaves the
/// lattice inconsistent somewhere - a hall below its two-door minimum, a door
/// facing a hole - and `all_edges_match` reports that instead, so the fixture
/// "failed without the guard" for the wrong reason and proved nothing. The real
/// article is silent precisely because `prune_disconnected` leaves everything
/// self-consistent on the way out.
///
/// So this uses the seed a 24-seed production survey actually caught:
/// `survey_room_graph_at_production_scale` found exactly one, and before the
/// guard it solved with a stamped room whose every cell had been voided - still
/// counted by `room_quota_failure`, still holding a `RoomId`, still projecting
/// its objective sockets into `Void`.
///
/// Production-scale, so it costs a solve. That is the price of testing the
/// thing that actually happened rather than a model of it.
#[test]
fn the_seed_that_stranded_a_room_no_longer_does() {
    let config = HexWfcConfig::arc_default();
    let quotas = HexRoomQuotas::for_team_count(2);
    let world = HexWfcWorld::generate_with_room_quotas(0x8F36_22EE_F8E8_D8D2, config, quotas)
        .expect("the seed must still solve; the guard costs a retry, not the facility");

    let live = super::topology::active_component(config, &world.placements, config.spawn());
    let stranded: Vec<_> = world
        .blueprints
        .iter()
        .filter(|blueprint| !blueprint.cells.iter().any(|cell| live.contains(cell)))
        .map(|blueprint| (blueprint.role, blueprint.anchor))
        .collect();

    assert!(
        stranded.is_empty(),
        "every stamped room must be reachable; stranded: {stranded:?}"
    );
}

/// Does raising `void_bias` break the one hall network into several?
///
/// `survey_room_graph_at_production_scale` found 22 of 24 seeds carrying a
/// single hall component. Every room hangs off it, so the derived room graph is
/// very nearly a clique and "which rooms connect" carries no information. That
/// is the structural form of the thing agents.md warns against — rooms and
/// corridors are supposed to have distinct jobs and produce a tension/release
/// rhythm "instead of uniform mush", and one undifferentiated network touching
/// thirty rooms is mush however good the tiles are.
///
/// Void is the only thing that separates hall components: a component is a
/// flood through open connections, so two runs of hall are distinct only when
/// something non-walkable lies between them. `Void` is a catalogue variant of
/// weight 4 and `ArchetypeBias::void` scales it, bounded to [0.25, 4.0].
///
/// So this sweeps the one knob that could produce separation and reports what
/// it actually buys: how much of the lattice goes dark, how many hall networks
/// result, and whether rooms start to sit on different ones.
///
/// Run with `--ignored --nocapture`.
#[test]
#[ignore = "composition sweep"]
fn survey_whether_void_bias_separates_hall_networks() {
    use std::collections::{BTreeMap, BTreeSet};

    let config = HexWfcConfig::arc_default();
    let quotas = HexRoomQuotas::for_team_count(2);

    println!("void_bias  seeds  void%   halls  biggest%  rooms_on_biggest  mean_deg  attempts");
    for &bias in &[1.0_f64, 1.5, 2.0, 3.0, 4.0] {
        let mut profile = HexCompositionProfile::baseline();
        profile.archetype_bias.void = bias;
        assert!(profile.validate().is_ok(), "{bias} must be a legal bias");

        let (mut solved, mut void_pct, mut halls, mut biggest_pct) = (0u32, 0.0, 0.0, 0.0);
        let (mut on_biggest, mut mean_deg, mut attempts) = (0.0, 0.0, 0u32);

        for index in 0..6u64 {
            let seed = 0xA11C_E3D0_0000_0000 ^ (index.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let Ok(world) =
                HexWfcWorld::generate_with_profile(seed, config, Some(quotas), &profile)
            else {
                continue;
            };
            solved += 1;
            attempts = attempts.max(world.last_attempts);

            let total = world.placements.len();
            let dark = world
                .placements
                .values()
                .filter(|placement| placement.space == HexSpace::Void)
                .count();
            #[allow(clippy::cast_precision_loss)]
            {
                void_pct += dark as f64 * 100.0 / total as f64;
            }

            let corridors = world.corridors();
            #[allow(clippy::cast_precision_loss)]
            {
                halls += corridors.len() as f64;
            }
            let largest = corridors
                .iter()
                .map(|corridor| corridor.cells.len())
                .max()
                .unwrap_or(0);
            let hall_cells: usize = corridors.iter().map(|c| c.cells.len()).sum();
            #[allow(clippy::cast_precision_loss)]
            {
                biggest_pct += if hall_cells == 0 {
                    0.0
                } else {
                    largest as f64 * 100.0 / hall_cells as f64
                };
            }

            // How many rooms sit on the single largest network? If that is every
            // room, the graph is a clique whatever the component count says.
            let mut rooms_per_corridor: BTreeMap<
                observed_core::CorridorId,
                BTreeSet<observed_core::RoomId>,
            > = BTreeMap::new();
            for attachment in world.threshold_attachments() {
                rooms_per_corridor
                    .entry(attachment.corridor)
                    .or_default()
                    .insert(attachment.room);
            }
            #[allow(clippy::cast_precision_loss)]
            {
                on_biggest += rooms_per_corridor
                    .values()
                    .map(BTreeSet::len)
                    .max()
                    .unwrap_or(0) as f64;
                let peers: usize = rooms_per_corridor
                    .values()
                    .map(|rooms| rooms.len().saturating_sub(1) * rooms.len())
                    .sum();
                mean_deg += peers as f64 / world.blueprints.len().max(1) as f64;
            }
        }

        if solved == 0 {
            println!("{bias:>9.2}      0   (no seed solved)");
            continue;
        }
        let n = f64::from(solved);
        println!(
            "{bias:>9.2}  {solved:>5}  {:>5.1}  {:>6.1}  {:>7.1}  {:>16.1}  {:>8.1}  {:>8}",
            void_pct / n,
            halls / n,
            biggest_pct / n,
            on_biggest / n,
            mean_deg / n,
            attempts
        );
    }
    println!(
        "\nhalls is the component count; biggest% is how much hall area the largest one holds.\n\
         rooms_on_biggest near the room count means the graph is still a clique."
    );
}

/// Can a pinned wall of Void split the facility into separate hall networks?
///
/// The sweep next door shows composition cannot do it: the profile can only
/// bend weights, and `profile.rs`'s first invariant guarantees it can never
/// remove a variant, so it can make connective fabric rarer and never absent.
/// The same module names the alternative — "a genuine prohibition is a pin" —
/// so this asks whether the sanctioned structural mechanism can express what
/// the tuning one cannot.
///
/// The wall is a column of `Space(Void)` pins across the middle of the lattice,
/// with a deliberate gap so spawn and exit are not severed. If separation is
/// achievable at all, this is the cheapest possible demonstration of it, and it
/// needs no change to the solver: pins already filter initial domains.
///
/// Three things are worth reading off the result. Whether it still solves at
/// all, because `hall_components_valid` requires every hall component to touch
/// two rooms and a bisected facility may simply be rejected. Whether the
/// component count actually rises. And whether the rooms end up distributed
/// across the components or all still hang off one, which is the question that
/// matters — a second component holding two cells and no rooms is not
/// structure.
///
/// Run with `--ignored --nocapture`.
#[test]
#[ignore = "structural pin experiment"]
fn survey_whether_a_pinned_void_wall_splits_the_facility() {
    use std::collections::{BTreeMap, BTreeSet};

    let config = HexWfcConfig::arc_default();
    let quotas = HexRoomQuotas::for_team_count(2);

    // A wall down the middle, with a gap left open at the top few rows so the
    // two halves still have a legal route between them. A full bisection would
    // sever spawn from exit and fail for a reason that says nothing about
    // whether separation is expressible.
    for &gap_rows in &[0usize, 3, 6] {
        let wall_q = config.cols / 2;
        let mut pins = Vec::new();
        for r in 0..config.rows {
            if usize::from(r) < gap_rows {
                continue;
            }
            for level in 0..config.levels {
                pins.push(HexPin {
                    q: wall_q,
                    r,
                    level,
                    intent: PinIntent::Space(HexSpace::Void),
                });
            }
        }
        let wall_cells = pins.len();

        let mut profile = HexCompositionProfile::baseline();
        profile.pin_sets = vec![PinSet {
            id: "void_wall_experiment".to_string(),
            note: "Bisect the lattice and see whether hall networks separate.".to_string(),
            cols: config.cols,
            rows: config.rows,
            levels: config.levels,
            pins,
        }];
        assert!(profile.validate().is_ok(), "the pin set must be legal");

        let seed = 0xA11C_E3D0_0000_0000u64;
        match HexWfcWorld::generate_with_profile(seed, config, Some(quotas), &profile) {
            Err(error) => {
                println!("gap_rows={gap_rows:>2}  wall={wall_cells:>4}  UNSOLVED: {error:?}");
            }
            Ok(world) => {
                let corridors = world.corridors();
                let mut rooms_per_corridor: BTreeMap<
                    observed_core::CorridorId,
                    BTreeSet<observed_core::RoomId>,
                > = BTreeMap::new();
                for attachment in world.threshold_attachments() {
                    rooms_per_corridor
                        .entry(attachment.corridor)
                        .or_default()
                        .insert(attachment.room);
                }
                let mut sizes: Vec<_> = rooms_per_corridor
                    .values()
                    .map(BTreeSet::len)
                    .collect::<Vec<_>>();
                sizes.sort_unstable_by(|a, b| b.cmp(a));
                sizes.truncate(5);
                println!(
                    "gap_rows={gap_rows:>2}  wall={wall_cells:>4}  attempts={:>2}  \
                     halls={:>3}  rooms={:>3}  rooms_per_network={sizes:?}",
                    world.last_attempts,
                    corridors.len(),
                    world.blueprints.len(),
                );
            }
        }
    }
    println!(
        "\nrooms_per_network is the top five hall networks by room count. Several \
         entries of comparable size is the outcome that would mean structure;\n\
         one large entry and a tail of small ones means the wall made pockets, not regions."
    );
    println!(
        "Observed: every wall exhausts the retry budget on \"pinned cell contradicts \
         blueprint, forced route, or an authored pin\".\nThat is the pipeline order, \
         not the wall. `stamp_blueprints_with_pins` takes no pins - its name means \
         locked blueprints -\nand `forced_route_edges` runs before `resolved_pins` \
         too, so both place geometry that the pins then contradict."
    );
}

/// How permeable are the regions the facility already has?
///
/// `region::region_plan` names the districts as territories and the cells where
/// two of them touch. This asks what a solved facility actually does at those
/// frontiers: of all the places two regions could be joined, how many are
/// joined?
///
/// The number is the concrete form of "uniform mush". A frontier that is open
/// almost everywhere is not a boundary between two areas, it is a seam in one
/// area, and no amount of tile authoring on either side will make crossing it
/// read as an event. It is also the number any future region-aware stage would
/// have to move, so it is worth having before such a stage exists rather than
/// after.
///
/// Run with `--ignored --nocapture`.
#[test]
#[ignore = "region permeability survey"]
fn survey_how_permeable_the_regions_are() {
    let config = HexWfcConfig::arc_default();
    let quotas = HexRoomQuotas::for_team_count(2);

    println!("seed              regions  gateways  frontier  open   open%  widest");
    let (mut total_open, mut total_frontier) = (0usize, 0usize);
    for index in 0..6u64 {
        let seed = 0xA11C_E3D0_0000_0000 ^ (index.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let Ok(world) = HexWfcWorld::generate_with_room_quotas(seed, config, quotas) else {
            continue;
        };
        let plan = super::region::region_plan(seed, config);

        // A frontier pair is "open" when the solved lattice actually lets you
        // walk across it.
        let mut frontier = 0usize;
        let mut open = 0usize;
        let mut widest = 0usize;
        for gateway in &plan.gateways {
            frontier += gateway.frontier.len();
            let mut crossings = 0usize;
            for &(a, b) in &gateway.frontier {
                let (Some(here), Some(there)) =
                    (world.placements.get(&a), world.placements.get(&b))
                else {
                    continue;
                };
                let Some(face) = HexFace::LATERAL
                    .into_iter()
                    .find(|face| config.grid().neighbor(a, *face) == Some(b))
                else {
                    continue;
                };
                if here.is_open(face) && there.is_open(face.opposite()) {
                    crossings += 1;
                }
            }
            open += crossings;
            widest = widest.max(crossings);
        }
        total_open += open;
        total_frontier += frontier;

        #[allow(clippy::cast_precision_loss)]
        let pct = if frontier == 0 {
            0.0
        } else {
            open as f64 * 100.0 / frontier as f64
        };
        println!(
            "{seed:016x}  {:>7}  {:>8}  {:>8}  {:>4}  {:>5.1}  {:>6}",
            plan.regions.len(),
            plan.gateways.len(),
            frontier,
            open,
            pct,
            widest
        );
    }

    #[allow(clippy::cast_precision_loss)]
    let overall = if total_frontier == 0 {
        0.0
    } else {
        total_open as f64 * 100.0 / total_frontier as f64
    };
    println!(
        "\n{total_open} of {total_frontier} frontier pairs are walkable ({overall:.1}%).\n\
         widest is the single most-crossed gateway: a boundary crossed in dozens of \
         places is a seam, not a threshold."
    );
}

/// The six seeds the region and room-graph measurements were taken on.
///
/// The same formula as the surveys above, truncated to the first six. Sharing
/// the seeds is the point: the bands below are read off numbers those surveys
/// printed, so drift here is comparable to drift there.
fn baseline_seeds() -> impl Iterator<Item = u64> {
    (0..6u64).map(|index| 0xA11C_E3D0_0000_0000 ^ (index.wrapping_mul(0x9E37_79B9_7F4A_7C15)))
}

/// One seed's worth of the numbers this area is judged on.
///
/// Two different degrees live here and they are easy to confuse, so they are
/// named apart. `port_ends` is what the *blueprints declare* - a ceiling no
/// solver change can raise. `derived_peers` is what the *solved lattice
/// delivers* - who actually ends up connected to whom.
struct FacilityBaseline {
    attempts: u32,
    elapsed: std::time::Duration,
    phantom: usize,
    rooms: usize,
    port_ends: usize,
    /// Rooms whose blueprint declares exactly one door. A hard leaf: no solver
    /// can route through it, because there is nothing to route through.
    leaves: usize,
    derived_peers: usize,
    frontier: usize,
    open: usize,
}

/// Solve one facility and measure it. `None` when the seed does not solve,
/// which the caller treats as a failure rather than skipping.
fn measure_baseline(
    seed: u64,
    config: HexWfcConfig,
    quotas: HexRoomQuotas,
) -> Option<FacilityBaseline> {
    use std::collections::BTreeMap;

    let start = std::time::Instant::now();
    let world = HexWfcWorld::generate_with_room_quotas(seed, config, quotas).ok()?;
    let elapsed = start.elapsed();

    // A room is real only if a cell of it survived the prune.
    let live = super::topology::active_component(config, &world.placements, config.spawn());
    let phantom = world
        .blueprints
        .iter()
        .filter(|blueprint| !blueprint.cells.iter().any(|cell| live.contains(cell)))
        .count();

    // The port budget, read off the rooms actually placed rather than off a
    // second copy of the quota table.
    let doors = world
        .blueprints
        .iter()
        .map(|blueprint| {
            super::blueprint::blueprint_for_role(blueprint.role)
                .named_ports
                .len()
        })
        .collect::<Vec<_>>();
    let port_ends = doors.iter().sum();
    let leaves = doors.iter().filter(|count| **count == 1).count();

    // The derived room graph: how many distinct rooms each room shares a
    // corridor with, summed.
    let mut rooms_on_corridor: BTreeMap<
        observed_core::CorridorId,
        BTreeSet<observed_core::RoomId>,
    > = BTreeMap::new();
    for attachment in world.threshold_attachments() {
        rooms_on_corridor
            .entry(attachment.corridor)
            .or_default()
            .insert(attachment.room);
    }
    let mut peers: BTreeMap<observed_core::RoomId, BTreeSet<observed_core::RoomId>> =
        BTreeMap::new();
    for rooms in rooms_on_corridor.values() {
        for room in rooms {
            peers
                .entry(*room)
                .or_default()
                .extend(rooms.iter().filter(|other| *other != room).copied());
        }
    }

    // How much of every region boundary you can actually walk across.
    let plan = super::region::region_plan(seed, config);
    let (mut frontier, mut open) = (0usize, 0usize);
    for gateway in &plan.gateways {
        frontier += gateway.frontier.len();
        for &(a, b) in &gateway.frontier {
            let (Some(here), Some(there)) = (world.placements.get(&a), world.placements.get(&b))
            else {
                continue;
            };
            let Some(face) = HexFace::LATERAL
                .into_iter()
                .find(|face| config.grid().neighbor(a, *face) == Some(b))
            else {
                continue;
            };
            if here.is_open(face) && there.is_open(face.opposite()) {
                open += 1;
            }
        }
    }

    Some(FacilityBaseline {
        attempts: world.last_attempts,
        elapsed,
        phantom,
        rooms: world.blueprints.len(),
        port_ends,
        leaves,
        derived_peers: peers.values().map(BTreeSet::len).sum(),
        frontier,
        open,
    })
}

/// The measured baseline of the production facility, as a guard rather than a
/// paragraph.
///
/// Every number this area's design argument rests on was, until now, printed by
/// an `#[ignore]`d survey - so it ran when someone remembered to run it, and
/// never in CI. A change that halved the room graph or doubled the solve time
/// would have stayed invisible until the next manual sweep.
///
/// These are bands, not equalities. The intent is to catch drift while leaving
/// the solver free to move: a change that improves a number should move the band
/// deliberately and say why, and a change that wrecks one should fail here rather
/// than in a playtest. Six seeds rather than the surveys' twenty-four keeps it
/// affordable on every run; the surveys stay for deep dives.
#[test]
fn the_production_facility_holds_its_measured_baseline() {
    let config = HexWfcConfig::arc_default();
    let quotas = HexRoomQuotas::for_team_count(2);

    let mut solved = 0usize;
    let mut worst_attempts = 0u32;
    let mut slowest = std::time::Duration::ZERO;
    let (mut phantom_total, mut rooms_total) = (0usize, 0usize);
    let (mut port_ends_total, mut peers_total) = (0usize, 0usize);
    let mut leaves_total = 0usize;
    let (mut total_open, mut total_frontier) = (0usize, 0usize);

    for seed in baseline_seeds() {
        let Some(measured) = measure_baseline(seed, config, quotas) else {
            panic!("{seed:016x} must solve at production scale");
        };
        solved += 1;
        worst_attempts = worst_attempts.max(measured.attempts);
        slowest = slowest.max(measured.elapsed);
        phantom_total += measured.phantom;
        rooms_total += measured.rooms;
        port_ends_total += measured.port_ends;
        leaves_total += measured.leaves;
        peers_total += measured.derived_peers;
        total_open += measured.open;
        total_frontier += measured.frontier;
    }

    #[allow(clippy::cast_precision_loss)]
    let port_budget = port_ends_total as f64 / rooms_total as f64;
    #[allow(clippy::cast_precision_loss)]
    let derived_degree = peers_total as f64 / rooms_total as f64;
    #[allow(clippy::cast_precision_loss)]
    let permeability = total_open as f64 * 100.0 / total_frontier as f64;
    println!(
        "baseline: {solved} solved | phantom {phantom_total} | \
         ports/room {port_budget:.2} | leaves {leaves_total} | \
         peers/room {derived_degree:.2} | \
         permeability {permeability:.1}% | attempts {worst_attempts} | slowest {slowest:?}"
    );

    let expected_rooms = solved * quotas.total_with_start_and_exit();
    assert_eq!(
        rooms_total, expected_rooms,
        "the quota total moved; every number below is per-room and reads against it"
    );

    // Established by `4caba31`, which made a stranded room fail the solve. An
    // equality rather than a band: a phantom room holds a quota slot and a
    // `RoomId`, projects its objective sockets into `Void`, and then refuses
    // every relayout commit for the rest of the match. No number but zero is
    // tolerable.
    assert_eq!(
        phantom_total, 0,
        "a stamped room was unreachable from spawn - see the guard in validate.rs"
    );

    // The ceiling. It was 62 port-ends across 30 rooms - 2.07 each - until
    // Keystone, Monitor and Recovery each gained a second door, which took it to
    // 73 and 2.43. A floor rather than an equality, so authoring another port
    // needs no test edit; *lowering* it means a room quietly lost a door, which
    // is the regression this catches.
    assert!(
        port_ends_total >= 73 * solved,
        "port budget fell to {port_budget:.2} per room, below the authored 2.43"
    );

    // Hard leaves: rooms declaring a single door, which no solver can route
    // through. Twelve of thirty before the second doors landed; now only the
    // Exit, whose one entrance is a deliberate contention chokepoint rather than
    // an oversight. This is the number that actually gates a richer room graph.
    assert!(
        leaves_total <= solved,
        "{leaves_total} single-door rooms across {solved} facilities - only the Exit should be one"
    );

    // The room graph is a clique: ~29 peers per room across 30 rooms means every
    // room reaches every other, so "which rooms connect" carries no information.
    //
    // This band records a *failing grade on purpose*. Breaking it is the goal of
    // a region-aware generation stage, so if this assertion fires because the
    // number dropped, that is the win and not the bug - move the band down and
    // record what did it. It exists so that such a change cannot happen by
    // accident and go unnoticed.
    assert!(
        derived_degree >= 20.0,
        "the room graph is no longer a clique ({derived_degree:.2} peers per room) - \
         if a region stage did this, move this band down deliberately"
    );

    // Was 49.9% while a region was one register on one floor. Regions became
    // whole-height volumes when a survey found that per-level ones had no
    // vertical boundary at all, and the figure fell to 23.7%.
    //
    // That fall is not an improvement and should not be read as one. The
    // frontier now blends lateral pairs, still about half open, with vertical
    // pairs, which are mostly solid floor - so the composite is lower for a
    // reason that has nothing to do with boundaries tightening. It is a
    // different measurement of a different object, re-baselined rather than
    // beaten.
    assert!(
        (18.7..=28.7).contains(&permeability),
        "region permeability moved to {permeability:.1}%, away from the measured 23.7%"
    );

    // The solve runs synchronously on the desync and late-join paths against a
    // 2 s LAN client timeout (bug #39), so this is a real ceiling and not a
    // vanity metric. The surveyed baseline was 3 attempts and 2.57 s.
    assert!(
        worst_attempts <= 5,
        "solve needed {worst_attempts} attempts, above the surveyed 3"
    );
    assert!(
        slowest < std::time::Duration::from_secs(4),
        "slowest solve was {slowest:?}, above the surveyed 2.57s"
    );
}

/// Would narrowing every region boundary to a few crossings break the clique?
///
/// This is the cheapest possible test of the whole region idea, and it is
/// deliberately run *before* building anything. The proposition is that the room
/// graph is a clique because connectivity is total, and that if a boundary
/// between two regions were crossed in two or three places instead of twenty,
/// "which rooms connect" would start carrying information. If sealing all but a
/// handful of crossings leaves the graph a clique anyway, the proposition is
/// wrong and no amount of solver work will rescue it.
///
/// **This is a lower bound, not a simulation.** It seals crossings on a finished
/// world, so the solver never gets to compensate - it cannot route around a
/// sealed gateway, re-place a room, or widen a corridor elsewhere. A real
/// region-aware stage would do all three. So the connectivity numbers here are
/// the worst case and the component counts are the best case. What the survey
/// can settle is the *direction*: whether constraining gateways moves the graph
/// at all, and how much reachability a naive constraint destroys - which is the
/// bill a real stage would have to pay to repair.
///
/// Crossings are kept in frontier order, which is arbitrary on purpose. A real
/// stage would choose which crossing to keep; choosing well is the difference
/// between these reachability numbers and better ones.
#[test]
#[ignore = "region gateway-narrowing probe"]
fn survey_whether_narrow_gateways_break_the_clique() {
    use std::collections::BTreeMap;

    let config = HexWfcConfig::arc_default();
    let quotas = HexRoomQuotas::for_team_count(2);
    let grid = config.grid();

    println!(
        "budget   halls  peers/room  biggest  linking  regions_spanned  exit_ok  open/frontier"
    );
    for budget in [usize::MAX, 3, 2, 1] {
        let mut halls_total = 0usize;
        let mut peers_total = 0usize;
        let mut rooms_total = 0usize;
        let mut exit_ok = 0usize;
        let (mut open_total, mut frontier_total) = (0usize, 0usize);
        let (mut biggest_total, mut linking_total) = (0usize, 0usize);
        let mut spanned_total = 0usize;
        let mut seeds = 0usize;

        for seed in baseline_seeds() {
            let Ok(world) = HexWfcWorld::generate_with_room_quotas(seed, config, quotas) else {
                continue;
            };
            let plan = super::region::region_plan(seed, config);
            let mut trial = world.clone();

            for gateway in &plan.gateways {
                let mut kept = 0usize;
                for &(a, b) in &gateway.frontier {
                    let Some(face) = HexFace::LATERAL
                        .into_iter()
                        .find(|face| grid.neighbor(a, *face) == Some(b))
                    else {
                        continue;
                    };
                    let walkable = trial
                        .placements
                        .get(&a)
                        .is_some_and(|here| here.is_open(face))
                        && trial
                            .placements
                            .get(&b)
                            .is_some_and(|there| there.is_open(face.opposite()));
                    if !walkable {
                        continue;
                    }
                    frontier_total += 1;
                    if kept < budget {
                        kept += 1;
                        open_total += 1;
                        continue;
                    }
                    // Seal from both sides, or the lattice contradicts itself.
                    if let Some(here) = trial.placements.get_mut(&a) {
                        here.doors &= !lateral_bit(face);
                    }
                    if let Some(there) = trial.placements.get_mut(&b) {
                        there.doors &= !lateral_bit(face.opposite());
                    }
                }
            }

            let live = super::topology::active_component(config, &trial.placements, config.spawn());

            let mut rooms_on_corridor: BTreeMap<
                observed_core::CorridorId,
                BTreeSet<observed_core::RoomId>,
            > = BTreeMap::new();
            for attachment in trial.threshold_attachments() {
                rooms_on_corridor
                    .entry(attachment.corridor)
                    .or_default()
                    .insert(attachment.room);
            }
            let mut peers: BTreeMap<observed_core::RoomId, BTreeSet<observed_core::RoomId>> =
                BTreeMap::new();
            for rooms in rooms_on_corridor.values() {
                for room in rooms {
                    peers
                        .entry(*room)
                        .or_default()
                        .extend(rooms.iter().filter(|other| *other != room).copied());
                }
            }

            let biggest = rooms_on_corridor
                .values()
                .map(BTreeSet::len)
                .max()
                .unwrap_or(0);
            biggest_total += biggest;
            let touching: usize = rooms_on_corridor
                .values()
                .filter(|rooms| rooms.len() > 1)
                .count();
            linking_total += touching;

            // How many regions does the room-linking corridor run through? If it
            // spans most of them, the seal never touched the artery.
            let owner: BTreeMap<HexCoord, _> = plan
                .regions
                .iter()
                .flat_map(|region| region.cells.iter().map(move |&cell| (cell, region.key())))
                .collect();
            let corridors = trial.corridors();
            let widest_regions = corridors
                .iter()
                .max_by_key(|corridor| corridor.cells.len())
                .map(|corridor| {
                    corridor
                        .cells
                        .iter()
                        .filter_map(|cell| owner.get(cell))
                        .collect::<BTreeSet<_>>()
                        .len()
                })
                .unwrap_or(0);
            spanned_total += widest_regions;

            halls_total += corridors.len();
            peers_total += peers.values().map(BTreeSet::len).sum::<usize>();
            rooms_total += trial.blueprints.len();
            exit_ok += usize::from(live.contains(&config.exit()));
            seeds += 1;
        }

        let label = if budget == usize::MAX {
            String::from("none  ")
        } else {
            format!("{budget:<6}")
        };
        println!(
            "{label}  {:>5.1}  {:>10.2}  {:>7.1}  {:>7.1}  {:>15.1}  {:>3}/{seeds}  \
             {open_total}/{frontier_total}",
            halls_total as f64 / seeds as f64,
            peers_total as f64 / rooms_total as f64,
            biggest_total as f64 / seeds as f64,
            linking_total as f64 / seeds as f64,
            spanned_total as f64 / seeds as f64,
            exit_ok,
        );
    }
    println!(
        "\npeers/room is the clique: 29 means every room reaches every other.\n\
         biggest is how many rooms the largest corridor touches; linking is how \
         many corridors touch more than one room at all.\n\
         regions_spanned is how many of the 100 regions that one corridor runs \
         through."
    );
}

/// Does thinning the *gateway graph* break the clique, and what does it cost?
///
/// The narrowing probe above found that width is not the variable: at one
/// crossing per gateway the surviving corridor still ran through 99.7 of the
/// hundred regions. The reason is arithmetic rather than geometry - a hundred
/// regions joined by about a hundred and seventy-six gateways is a graph of
/// average degree three and a half, so a single crossing on every edge still
/// leaves everything reachable from everything. Narrowing a boundary cannot
/// disconnect a graph that is connected many times over.
///
/// So this seals whole gateways instead. Keep a spanning tree of the region
/// graph - the minimum that leaves every region reachable - plus `extras`
/// further edges, and seal every crossing of every gateway not kept. Kept
/// gateways are held to one crossing, since the probe above showed width does
/// not matter once the graph is thin.
///
/// Two questions at once, and the second is the one that decides whether any of
/// this is buildable. Does the room graph finally stop being complete, and what
/// does confining the facility to a thin region graph cost in reachability? A
/// tree over a hundred ten-cell regions is a very long, very thin maze, and it
/// may simply not admit thirty rooms and a spawn-to-exit route.
///
/// Same caveat as the probe above: sealing is a lower bound, because the solver
/// never gets to compensate. The reachability column is the bill a real stage
/// would have to pay back by routing.
#[test]
#[ignore = "region gateway-graph probe"]
fn survey_whether_a_thin_gateway_graph_breaks_the_clique() {
    use std::collections::{BTreeMap, VecDeque};

    let config = HexWfcConfig::arc_default();
    let quotas = HexRoomQuotas::for_team_count(2);
    let grid = config.grid();

    println!(
        "extras  kept  halls  peers/room  biggest  rooms_live  exit_ok  spanned  artery  cycles"
    );
    for extras in [usize::MAX, 40, 20, 10, 0] {
        let mut kept_total = 0usize;
        let mut halls_total = 0usize;
        let mut peers_total = 0usize;
        let mut rooms_total = 0usize;
        let mut biggest_total = 0usize;
        let mut live_total = 0usize;
        let mut spanned_total = 0usize;
        let (mut artery_cells, mut artery_cycles) = (0usize, 0usize);
        let mut exit_ok = 0usize;
        let mut seeds = 0usize;

        for seed in baseline_seeds() {
            let Ok(world) = HexWfcWorld::generate_with_room_quotas(seed, config, quotas) else {
                continue;
            };
            let plan = super::region::region_plan(seed, config);

            // A spanning tree of the region graph, breadth-first from the
            // region the spawn sits in, so the kept set is deterministic and
            // every region stays nominally reachable.
            let mut adjacency: BTreeMap<_, Vec<(usize, _)>> = BTreeMap::new();
            for (index, gateway) in plan.gateways.iter().enumerate() {
                adjacency
                    .entry(gateway.a)
                    .or_default()
                    .push((index, gateway.b));
                adjacency
                    .entry(gateway.b)
                    .or_default()
                    .push((index, gateway.a));
            }
            let root = plan
                .regions
                .iter()
                .find(|region| region.cells.contains(&config.spawn()))
                .map(super::region::Region::key);
            let mut kept: BTreeSet<usize> = BTreeSet::new();
            let mut seen: BTreeSet<_> = root.into_iter().collect();
            let mut queue: VecDeque<_> = seen.iter().copied().collect();
            while let Some(here) = queue.pop_front() {
                for &(index, there) in adjacency.get(&here).into_iter().flatten() {
                    if seen.insert(there) {
                        kept.insert(index);
                        queue.push_back(there);
                    }
                }
            }
            // Then the requested number of extra edges, in gateway order.
            for index in 0..plan.gateways.len() {
                if kept.len() >= seen.len().saturating_sub(1).saturating_add(extras) {
                    break;
                }
                kept.insert(index);
            }
            kept_total += kept.len();

            let mut trial = world.clone();
            for (index, gateway) in plan.gateways.iter().enumerate() {
                let budget = usize::from(kept.contains(&index));
                let mut used = 0usize;
                for &(a, b) in &gateway.frontier {
                    let Some(face) = HexFace::LATERAL
                        .into_iter()
                        .find(|face| grid.neighbor(a, *face) == Some(b))
                    else {
                        continue;
                    };
                    let walkable = trial
                        .placements
                        .get(&a)
                        .is_some_and(|here| here.is_open(face))
                        && trial
                            .placements
                            .get(&b)
                            .is_some_and(|there| there.is_open(face.opposite()));
                    if !walkable {
                        continue;
                    }
                    if used < budget {
                        used += 1;
                        continue;
                    }
                    if let Some(here) = trial.placements.get_mut(&a) {
                        here.doors &= !lateral_bit(face);
                    }
                    if let Some(there) = trial.placements.get_mut(&b) {
                        there.doors &= !lateral_bit(face.opposite());
                    }
                }
            }

            let live = super::topology::active_component(config, &trial.placements, config.spawn());
            live_total += trial
                .blueprints
                .iter()
                .filter(|blueprint| blueprint.cells.iter().any(|cell| live.contains(cell)))
                .count();
            exit_ok += usize::from(live.contains(&config.exit()));

            let mut rooms_on_corridor: BTreeMap<
                observed_core::CorridorId,
                BTreeSet<observed_core::RoomId>,
            > = BTreeMap::new();
            for attachment in trial.threshold_attachments() {
                rooms_on_corridor
                    .entry(attachment.corridor)
                    .or_default()
                    .insert(attachment.room);
            }
            let mut peers: BTreeMap<observed_core::RoomId, BTreeSet<observed_core::RoomId>> =
                BTreeMap::new();
            for rooms in rooms_on_corridor.values() {
                for room in rooms {
                    peers
                        .entry(*room)
                        .or_default()
                        .extend(rooms.iter().filter(|other| *other != room).copied());
                }
            }
            biggest_total += rooms_on_corridor
                .values()
                .map(BTreeSet::len)
                .max()
                .unwrap_or(0);

            let owner: BTreeMap<HexCoord, _> = plan
                .regions
                .iter()
                .flat_map(|region| region.cells.iter().map(move |&cell| (cell, region.key())))
                .collect();
            let corridors = trial.corridors();
            spanned_total += corridors
                .iter()
                .max_by_key(|corridor| corridor.cells.len())
                .map(|corridor| {
                    corridor
                        .cells
                        .iter()
                        .filter_map(|cell| owner.get(cell))
                        .collect::<BTreeSet<_>>()
                        .len()
                })
                .unwrap_or(0);
            // The artery's own shape. `cells` is how big it is; `cycles` is
            // edges - nodes + 1, the number of independent loops in it. A snake
            // threading room to room has almost none and could be cut into legs
            // at the rooms it passes; a lattice with many loops cannot, because
            // every cut leaves another way round.
            if let Some(artery) = corridors.iter().max_by_key(|corridor| corridor.cells.len()) {
                let cells: BTreeSet<HexCoord> = artery.cells.iter().copied().collect();
                let mut edges = 0usize;
                for &cell in &cells {
                    for face in HexFace::LATERAL {
                        if grid
                            .neighbor(cell, face)
                            .is_some_and(|next| cells.contains(&next))
                        {
                            edges += 1;
                        }
                    }
                }
                artery_cells += cells.len();
                artery_cycles += (edges / 2 + 1).saturating_sub(cells.len());
            }

            halls_total += corridors.len();
            peers_total += peers.values().map(BTreeSet::len).sum::<usize>();
            rooms_total += trial.blueprints.len();
            seeds += 1;
        }

        let label = if extras == usize::MAX {
            String::from("all   ")
        } else {
            format!("{extras:<6}")
        };
        #[allow(clippy::cast_precision_loss)]
        {
            println!(
                "{label}  {:>4.0}  {:>5.1}  {:>10.2}  {:>7.1}  {:>10.1}  \
                 {:>3}/{seeds}  {:>7.1}  {:>6.0}  {:>6.0}",
                kept_total as f64 / seeds as f64,
                halls_total as f64 / seeds as f64,
                peers_total as f64 / rooms_total as f64,
                biggest_total as f64 / seeds as f64,
                live_total as f64 / seeds as f64,
                exit_ok,
                spanned_total as f64 / seeds as f64,
                artery_cells as f64 / seeds as f64,
                artery_cycles as f64 / seeds as f64,
            );
        }
    }
    println!(
        "\nextras is gateways kept beyond a spanning tree. rooms_live and exit_ok \
         are the feasibility bill.\n\
         artery is the largest corridor's cell count against a 5,600-cell \
         facility, and cycles is how many independent loops it holds. Those two \
         are the finding: the facility is 97.2% hall, 1.1% room and 1.7% void, \
         so the artery is not a corridor system with boundaries to police - it \
         is nearly the whole building, and the rooms are inclusions in it."
    );
}

/// What the collapse lottery actually offers, before any seed or profile.
///
/// A production facility measures 97.2% hall, 1.1% room and 1.7% void, and the
/// composition profile cannot change that - a survey found `void_bias` from 1.0
/// to 4.0 moves nothing, and the profile's first invariant is that it reweights
/// variants and never removes one.
///
/// This asks whether the answer was decided before any of that. The collapse
/// draws from the variant alphabet weighted per entry, so a space represented by
/// three hundred entries at weight 6 beats one represented by a single entry at
/// weight 4 by a factor of four hundred and fifty, whatever the weights say.
/// That arithmetic has bitten this solver before: the comment above the shaft
/// family in `variants.rs` records the facility coming out 47% stairs because
/// 190 shaft entries competed with a handful for a straight (backlog #13), and
/// the fix was to lower the shaft weight rather than to change the counting.
#[test]
#[ignore = "variant alphabet composition"]
fn survey_what_the_collapse_lottery_offers() {
    use std::collections::BTreeMap;

    let catalogue = super::variants::catalogue();
    let mut by_space: BTreeMap<String, (usize, u32)> = BTreeMap::new();
    for variant in &catalogue {
        let entry = by_space.entry(format!("{:?}", variant.space)).or_default();
        entry.0 += 1;
        entry.1 += variant.weight;
    }
    let total_weight: u32 = catalogue.iter().map(|variant| variant.weight).sum();

    println!("space    entries  total_weight  lottery_share");
    for (space, (entries, weight)) in &by_space {
        #[allow(clippy::cast_precision_loss)]
        let share = f64::from(*weight) * 100.0 / f64::from(total_weight);
        println!("{space:<8} {entries:>7}  {weight:>12}  {share:>12.2}%");
    }

    let mut by_archetype: BTreeMap<String, (usize, u32)> = BTreeMap::new();
    for variant in &catalogue {
        let entry = by_archetype
            .entry(format!("{:?}", variant.archetype))
            .or_default();
        entry.0 += 1;
        entry.1 += variant.weight;
    }
    println!("\narchetype     entries  total_weight  lottery_share");
    for (archetype, (entries, weight)) in &by_archetype {
        let share = f64::from(*weight) * 100.0 / f64::from(total_weight);
        println!("{archetype:<13} {entries:>7}  {weight:>12}  {share:>12.2}%");
    }
    // Room variants are 45% of the alphabet and 1.1% of the facility because
    // they are only ever legal inside a stamped blueprint - `geometry_demands`
    // says as much, calling them "generic Room variants that can never leave a
    // blueprint domain". So the lottery an *ordinary* cell actually runs is Hall
    // against Void, and nothing else.
    let hall: u32 = catalogue
        .iter()
        .filter(|variant| variant.space == HexSpace::Hall)
        .map(|variant| variant.weight)
        .sum();
    let void: u32 = catalogue
        .iter()
        .filter(|variant| variant.space == HexSpace::Void)
        .map(|variant| variant.weight)
        .sum();
    let share = |weight: f64| weight * 100.0 / (f64::from(hall) + weight);
    let ceiling = super::context::PROFILE_MAX;
    // What multiplier would a quarter-empty facility need?
    let wanted = f64::from(hall) / 3.0 / f64::from(void);
    println!(
        "\nthe lottery a non-blueprint cell runs: hall {hall} against void {void}.\n\
         void share at bias 1.0        {:.2}%\n\
         void share at the {ceiling:.1} ceiling  {:.2}%\n\
         bias needed for 25% void      {wanted:.0}x\n\
         \n\
         PROFILE_MAX is {ceiling:.1} and exists so that \"no single input can \
         dominate the lottery\". A quarter-empty facility needs {:.0} times more \
         than the profile is allowed to ask for, so no tuning reaches it - which \
         is why void_bias 1.0 to 4.0 measured no change at all. The alphabet \
         offers one way for a cell to be nothing and {} ways for it to be hall.",
        share(f64::from(void)),
        share(f64::from(void) * ceiling),
        wanted / ceiling,
        catalogue
            .iter()
            .filter(|variant| variant.space == HexSpace::Hall)
            .count(),
    );
}

/// Sweep the void share and watch the facility empty out.
///
/// The point of drawing the space before the variant is that "how much of this
/// building is nothing" becomes a number somebody sets. This is the check that
/// it really is one: the alphabet's own implied ratio is 4 against hall's 639,
/// so raising the void share should move the composition roughly as the ratio
/// predicts, and the columns after it are what that costs.
#[test]
#[ignore = "space-mix sweep"]
fn survey_what_the_space_mix_buys() {
    let config = HexWfcConfig::arc_default();
    let quotas = HexRoomQuotas::for_team_count(2);

    println!(
        "void_share  predicted  void%  hall%  peers/room  artery  halls  slowest  live  exit_ok"
    );
    for void_share in [4.0, 320.0, 639.0, 1918.0, 5000.0, 10_000.0] {
        let mut profile = super::profile::HexCompositionProfile::baseline();
        profile.space_mix = profile.space_mix.with(HexSpace::Void, void_share);

        let (mut void, mut room, mut hall, mut total) = (0usize, 0usize, 0usize, 0usize);
        let mut worst_attempts = 0u32;
        let mut slowest = std::time::Duration::ZERO;
        let (mut live_total, mut exit_ok, mut solved) = (0usize, 0usize, 0usize);
        let (mut peers_total, mut rooms_counted) = (0usize, 0usize);
        let (mut artery_total, mut halls_total) = (0usize, 0usize);

        for seed in baseline_seeds() {
            let start = std::time::Instant::now();
            let Ok(world) =
                HexWfcWorld::generate_with_profile(seed, config, Some(quotas), &profile)
            else {
                continue;
            };
            slowest = slowest.max(start.elapsed());
            worst_attempts = worst_attempts.max(world.last_attempts);
            solved += 1;
            for placement in world.placements.values() {
                total += 1;
                match placement.space {
                    HexSpace::Void => void += 1,
                    HexSpace::Room => room += 1,
                    HexSpace::Hall => hall += 1,
                }
            }
            // The payoff metric. A sparser facility is only worth anything if
            // the room graph gets sparser with it.
            {
                use std::collections::BTreeMap;
                let mut rooms_on_corridor: BTreeMap<
                    observed_core::CorridorId,
                    BTreeSet<observed_core::RoomId>,
                > = BTreeMap::new();
                for attachment in world.threshold_attachments() {
                    rooms_on_corridor
                        .entry(attachment.corridor)
                        .or_default()
                        .insert(attachment.room);
                }
                let mut peers: BTreeMap<observed_core::RoomId, BTreeSet<observed_core::RoomId>> =
                    BTreeMap::new();
                for rooms in rooms_on_corridor.values() {
                    for room in rooms {
                        peers
                            .entry(*room)
                            .or_default()
                            .extend(rooms.iter().filter(|other| *other != room).copied());
                    }
                }
                peers_total += peers.values().map(BTreeSet::len).sum::<usize>();
                rooms_counted += world.blueprints.len();
                let corridors = world.corridors();
                halls_total += corridors.len();
                artery_total += corridors
                    .iter()
                    .map(|corridor| corridor.cells.len())
                    .max()
                    .unwrap_or(0);
            }
            let live = super::topology::active_component(config, &world.placements, config.spawn());
            live_total += world
                .blueprints
                .iter()
                .filter(|blueprint| blueprint.cells.iter().any(|cell| live.contains(cell)))
                .count();
            exit_ok += usize::from(live.contains(&config.exit()));
        }

        if total == 0 {
            println!("{void_share:<10}  UNSOLVED at every seed");
            continue;
        }
        #[allow(clippy::cast_precision_loss)]
        {
            let pct = |n: usize| n as f64 * 100.0 / total as f64;
            let predicted = void_share * 100.0 / (void_share + 639.0);
            let _ = (room, worst_attempts);
            println!(
                "{void_share:<10}  {predicted:>8.1}%  {:>4.1}%  {:>4.1}%  {:>10.2}  {:>6.0}  \
                 {:>5.0}  {:>6.2}s  {:>4.1}  {exit_ok:>3}/{solved}",
                pct(void),
                pct(hall),
                peers_total as f64 / rooms_counted.max(1) as f64,
                artery_total as f64 / solved.max(1) as f64,
                halls_total as f64 / solved.max(1) as f64,
                slowest.as_secs_f64(),
                live_total as f64 / solved.max(1) as f64,
            );
        }
    }
    println!(
        "\npredicted is void/(void+hall) from the shares alone. Where the measured \
         column tracks it, the mix is a real control; where it falls short, \
         propagation is refusing the void a neighbour would contradict."
    );
}

#[test]
#[ignore = "diagnostic"]
fn diagnose_archetype_shift() {
    use std::collections::BTreeMap;
    let config = HexWfcConfig::arc_default();
    let quotas = HexRoomQuotas::for_team_count(2);
    for (label, mix) in [
        (
            "alphabet",
            super::profile::SpaceMix::implied_by_the_alphabet(),
        ),
        ("baseline", super::profile::SpaceMix::baseline()),
    ] {
        let mut profile = super::profile::HexCompositionProfile::baseline();
        profile.space_mix = mix;
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut total = 0usize;
        for seed in baseline_seeds() {
            let Ok(world) =
                HexWfcWorld::generate_with_profile(seed, config, Some(quotas), &profile)
            else {
                continue;
            };
            for placement in world.placements.values() {
                *counts
                    .entry(format!("{:?}", placement.archetype))
                    .or_default() += 1;
                total += 1;
            }
        }
        #[allow(clippy::cast_precision_loss)]
        let line = counts
            .iter()
            .map(|(k, v)| format!("{k} {:.1}%", *v as f64 * 100.0 / total as f64))
            .collect::<Vec<_>>()
            .join("  ");
        println!("{label:<9} {line}");
    }
}
