use super::*;
use crate::map_spec::TraversalArchetype;
use crate::room_def::RoomTemplate;

fn small_config() -> FullWfcConfig {
    FullWfcConfig {
        cols: 6,
        rows: 4,
        levels: 3,
        min_rooms: 10,
        max_rooms: 24,
        retry_budget: 128,
        pulse_ticks: 60,
        min_room_distance: 1,
    }
}

#[test]
fn same_seed_is_byte_identical_and_uses_all_levels() {
    let a = FullWfcWorld::new(7, small_config()).expect("world");
    let b = FullWfcWorld::new(7, small_config()).expect("world");
    assert_eq!(a.placements, b.placements);
    let route = a.spawn_route().expect("spawn reaches exit");
    assert_eq!(route.cells.first(), Some(&a.spawn()));
    assert_eq!(route.cells.last(), Some(&a.exit()));
    assert!(route.cells.iter().any(|cell| cell.level == 1));
    assert!(route.cells.iter().any(|cell| cell.level == 2));
    assert_eq!(a.connected_active_cells(), a.active_cell_count());
}

#[test]
fn halls_have_two_to_four_faces_and_rooms_never_connect_directly() {
    let world = FullWfcWorld::new(19, small_config()).expect("world");
    for placement in world.placements.values() {
        if placement.space == ModuleSpace::Hall {
            assert!((2..=4).contains(&placement.openings.count_ones()));
        }
        for face in ModuleFace::ALL {
            if !placement.is_open(face) {
                continue;
            }
            let next = world.config.neighbor(placement.coord, face).unwrap();
            let other = &world.placements[&next];
            assert_ne!(
                (placement.space, other.space),
                (ModuleSpace::Room, ModuleSpace::Room)
            );
        }
    }
    assert!(
        world
            .corridors
            .values()
            .all(|corridor| (2..=4).contains(&corridor.endpoints.len()))
    );
}

#[test]
fn observation_holds_room_identity_and_exact_hall_but_not_other_room_faces() {
    let mut world = FullWfcWorld::new(33, small_config()).expect("world");
    let route = world.spawn_route().unwrap();
    let room = route
        .cells
        .iter()
        .copied()
        .find(|coord| world.placements[coord].space == ModuleSpace::Room)
        .unwrap();
    let hall = route
        .cells
        .iter()
        .copied()
        .find(|coord| world.placements[coord].space == ModuleSpace::Hall)
        .unwrap();
    let before_room = world.placements[&room].clone();
    let before_hall = world.placements[&hall].clone();
    let frame = ObservationFrame {
        visible_cells: BTreeSet::from([room, hall]),
        occupied_cells: BTreeMap::from([(PlayerId(0), room)]),
        ..Default::default()
    };
    world.update_observation(frame.clone());
    let candidate = world.propose_relayout(&frame).expect("candidate");
    assert_eq!(
        candidate.placements[&room].geometry_identity(),
        before_room.geometry_identity()
    );
    assert_eq!(candidate.placements[&hall], before_hall);
}

#[test]
fn a_visible_threshold_locks_its_hall_chain_and_destination_room() {
    let world = FullWfcWorld::new(41, small_config()).expect("world");
    let route = world.spawn_route().unwrap();
    let room_index = route
        .cells
        .iter()
        .position(|coord| world.placements[coord].space == ModuleSpace::Room)
        .unwrap_or(0);
    let room = route.cells[room_index];
    let next = route.cells[room_index + 1];
    let face = face_between(world.config, room, next).unwrap();
    let frame = ObservationFrame {
        visible_thresholds: BTreeSet::from([ThresholdKey { room, face }]),
        ..Default::default()
    };
    let pins = pinned_cells(world.config, &world.placements, &frame);
    assert!(pins.contains(&room));
    assert!(pins.contains(&next));
    assert!(
        pins.len() >= 3,
        "the far room is included with the hall chain"
    );
}

#[test]
fn relayouts_keep_spawn_and_every_player_connected() {
    for seed in 0..20u64 {
        let mut world = FullWfcWorld::new(seed, small_config()).expect("world");
        let mut player = world.spawn();
        for _ in 0..10 {
            let frame = ObservationFrame {
                occupied_cells: BTreeMap::from([(PlayerId(0), player)]),
                visible_cells: BTreeSet::from([player]),
                ..Default::default()
            };
            world.update_observation(frame.clone());
            let candidate = world.propose_relayout(&frame).expect("candidate");
            world
                .commit_relayout(candidate, frame)
                .expect("safe commit");
            let route = world.route(player).expect("player route");
            player = *route.cells.get(1).unwrap_or(&player);
            assert!(world.spawn_route().is_some());
            assert_eq!(world.connected_active_cells(), world.active_cell_count());
        }
    }
}

#[test]
fn candle_brightens_monotonically_along_the_selected_route() {
    let world = FullWfcWorld::new(57, small_config()).expect("world");
    let route = world.spawn_route().unwrap();
    let values: Vec<f32> = route
        .cells
        .iter()
        .map(|&coord| world.candle_proximity(coord))
        .collect();
    assert_eq!(values[0], 0.0);
    assert_eq!(*values.last().unwrap(), 1.0);
    for pair in values.windows(2) {
        assert!(pair[1] + 1.0e-5 >= pair[0], "{values:?}");
    }
}

#[test]
fn an_exit_claim_is_rejected_when_it_would_strand_landmarks() {
    let mut world = FullWfcWorld::new(73, small_config()).expect("world");
    let route = world.spawn_route().unwrap();
    let room_index = route.cells[..route.cells.len() - 1]
        .iter()
        .rposition(|coord| world.placements[coord].space == ModuleSpace::Room)
        .expect("terminal chain starts in a room");
    let room = route.cells[room_index];
    let face = face_between(world.config, room, route.cells[room_index + 1]).unwrap();
    let frame = ObservationFrame {
        visible_cells: BTreeSet::from([room]),
        visible_thresholds: BTreeSet::from([ThresholdKey { room, face }]),
        occupied_cells: BTreeMap::from([(PlayerId(0), room)]),
    };
    world.update_observation(frame);
    assert!(world.exit_claim.is_none());
    assert!(world.reserved_exit_faces.is_empty());
    assert!(
        world
            .landmark_cells
            .iter()
            .all(|&coord| world.route(coord).is_some())
    );
}

#[test]
fn unsafe_visible_geometry_change_is_rejected() {
    let mut world = FullWfcWorld::new(89, small_config()).expect("world");
    let frame = ObservationFrame::default();
    let mut candidate = world.propose_relayout(&frame).expect("candidate");
    let coord = candidate
        .changed_cells
        .iter()
        .copied()
        .find(|coord| {
            world.placements[coord].geometry_identity()
                != candidate.placements[coord].geometry_identity()
        })
        .expect("some geometry changes");
    let latest = ObservationFrame {
        visible_cells: BTreeSet::from([coord]),
        ..Default::default()
    };
    assert_eq!(
        world.commit_relayout(candidate.clone(), latest),
        Err(FullWfcError::UnsafeChange(coord))
    );
    candidate.changed_cells.clear();
}

/// Release-gate corpus matching the experiment plan. It is ignored in ordinary
/// workspace runs because it performs 5,000 complete constrained collapses; CI or
/// an agent can opt in with `cargo test ... extended_default_corpus -- --ignored`.
#[test]
#[ignore = "extended 100-seed x 50-pulse WFC corpus"]
fn extended_default_corpus_keeps_every_runner_route() {
    for seed in 0..100u64 {
        let mut world = FullWfcWorld::new(seed, FullWfcConfig::default())
            .unwrap_or_else(|error| panic!("seed {seed} initial solve failed: {error:?}"));
        let mut player = world.spawn();
        let mut committed = 0;
        for pulse_index in 0..50 {
            let route = world.route(player).unwrap_or_else(|| {
                panic!("seed {seed} pulse {pulse_index} lost its pre-pulse route")
            });
            let next = route.cells.get(1).copied();
            let mut visible_thresholds = BTreeSet::new();
            if world.placements[&player].space == ModuleSpace::Room
                && let Some(next) = next
            {
                visible_thresholds.insert(ThresholdKey {
                    room: player,
                    face: face_between(world.config, player, next).unwrap(),
                });
            }
            let frame = ObservationFrame {
                visible_cells: BTreeSet::from([player]),
                visible_thresholds,
                occupied_cells: BTreeMap::from([(PlayerId(0), player)]),
            };
            match world.propose_relayout(&frame) {
                Ok(candidate) => {
                    world
                        .commit_relayout(candidate, frame)
                        .unwrap_or_else(|error| {
                            panic!("seed {seed} pulse {pulse_index} failed commit: {error:?}")
                        });
                    committed += 1;
                }
                Err(FullWfcError::RetryBudgetExhausted { attempts }) => {
                    world.reject_relayout(attempts);
                }
                Err(error) => {
                    panic!("seed {seed} pulse {pulse_index} failed unexpectedly: {error:?}")
                }
            }
            assert!(
                world.route(player).is_some(),
                "seed {seed} pulse {pulse_index}"
            );
            assert_eq!(world.connected_active_cells(), world.active_cell_count());
            player = next.unwrap_or(player);
        }
        assert!(committed > 0, "seed {seed} never committed a mutation");
    }
}
#[test]
fn complete_catalog_fixture_contains_every_room_and_hall_archetype() {
    let world = FullWfcWorld::catalog_fixture(0x0CA7_A10A).expect("complete fixture");
    let templates = world
        .rooms
        .values()
        .map(|room| room.template)
        .collect::<BTreeSet<_>>();
    let traversals = world
        .corridors
        .values()
        .map(|corridor| corridor.traversal)
        .collect::<BTreeSet<_>>();
    assert_eq!(templates, BTreeSet::from(RoomTemplate::ALL));
    assert_eq!(traversals, BTreeSet::from(TraversalArchetype::ALL));
    assert!(world.corridors.values().all(|corridor| {
        corridor
            .traversal
            .is_compatible(corridor.register, corridor.role)
    }));
}
