use super::*;

fn corpus_seeds() -> impl Iterator<Item = u64> {
    (0..100u64).map(|n| 0xA11C_E000_0000_0000 | n)
}

#[test]
fn the_same_seed_yields_identical_placements_and_trace() {
    let config = HexWfcConfig::default();
    let (first, first_trace) =
        HexWfcWorld::generate_traced(42, config).expect("seed 42 must solve");
    let (second, second_trace) =
        HexWfcWorld::generate_traced(42, config).expect("seed 42 must solve");
    assert_eq!(first.placements, second.placements);
    assert_eq!(first_trace, second_trace);
    assert_eq!(first.last_attempts, second.last_attempts);
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

#[test]
fn a_hundred_seed_corpus_solves_and_validates() {
    let config = HexWfcConfig::default();
    for seed in corpus_seeds() {
        let world = HexWfcWorld::generate(seed, config)
            .unwrap_or_else(|error| panic!("seed {seed:#x} failed: {error:?}"));

        // Spawn and exit are rooms with a live route between them.
        assert_eq!(world.placements[&config.spawn()].space, HexSpace::Room);
        assert_eq!(world.placements[&config.exit()].space, HexSpace::Room);
        let route = world
            .route_between(config.spawn(), config.exit())
            .unwrap_or_else(|| panic!("seed {seed:#x} has no spawn->exit route"));
        assert!(route.len() >= 2);

        // Room quota inside the configured range.
        let rooms = world.room_count();
        assert!(
            (config.min_rooms..=config.max_rooms).contains(&rooms),
            "seed {seed:#x} room count {rooms} outside range"
        );

        // Edge symmetry, room separation, and hall grammar.
        let grid = config.grid();
        for placement in world.placements.values() {
            for face in HexFace::LATERAL {
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
            }
        }
    }
}

#[test]
fn ports_view_matches_the_door_mask() {
    let config = HexWfcConfig::default();
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
        assert_eq!(ports.port(HexFace::Up), PortClass::Sealed);
        assert_eq!(ports.port(HexFace::Down), PortClass::Sealed);
    }
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
