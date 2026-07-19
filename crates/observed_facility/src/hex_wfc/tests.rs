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
/// across all faces, no adjacent open rooms, blueprint footprints disjoint and
/// spaced by `min_room_distance`, and a spawn→exit route.
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
                        assert!(
                            !(open
                                && placement.space == HexSpace::Room
                                && other.space == HexSpace::Room),
                            "seed {seed:#x}: adjacent open rooms at {:?}",
                            placement.coord
                        );
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

/// The pinned showcase seed: a solve that contains a full-height (4-level)
/// wellshaft column and a ramp chain climbing three levels, with a spawn→exit
/// route that traverses a vertical element. If the seed ever drifts, re-pin
/// from `search_for_pinnable_3d_seeds`.
const PINNED_3D_SEED: u64 = 0xA11C_E3D0_0000_0008;

#[test]
fn the_pinned_seed_shows_a_tall_shaft_and_a_ramp_chain() {
    let config = config_3d();
    let world = HexWfcWorld::generate(PINNED_3D_SEED, config).expect("pinned seed must solve");

    let shaft = tallest_shaft_column(&world);
    assert!(shaft >= 4, "pinned seed shaft column only {shaft} levels");

    let ramp = tallest_ramp_chain(&world);
    assert!(ramp >= 3, "pinned seed ramp chain only {ramp} levels");

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
            seed, generation, attempt, config, None, &no_pins, None,
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
