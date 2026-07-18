use std::collections::{BTreeMap, BTreeSet};

use observed_core::PlayerId;

use super::relayout::fallback_geometry_relayout;
use super::{
    HexArchetype, HexCoord, HexFace, HexObservationFrame, HexPlacement, HexRelayoutCandidate,
    HexRelayoutProgress, HexSpace, HexThresholdKey, HexWfcConfig, HexWfcError, HexWfcWorld,
    PortClass,
};
use crate::map_spec::RoomRole;

fn config() -> HexWfcConfig {
    HexWfcConfig {
        levels: 4,
        ..HexWfcConfig::default()
    }
}

fn observed_room(world: &HexWfcWorld) -> (HexObservationFrame, HexCoord) {
    let room = world
        .blueprints
        .iter()
        .find(|room| !matches!(room.role, RoomRole::Start | RoomRole::Exit))
        .unwrap_or(&world.blueprints[0]);
    let mut frame = HexObservationFrame::default();
    frame.visible_cells.insert(room.cells[0]);
    frame.objective_cells.insert(world.config.spawn());
    (frame, room.cells[0])
}

#[test]
fn replay_is_deterministic_across_one_attempt_ticks() {
    let world = HexWfcWorld::generate(0x9300, config()).expect("world");
    let (frame, _) = observed_room(&world);
    let run = || {
        let mut work = world.begin_relayout(&frame);
        loop {
            match world.advance_relayout(work).expect("advance") {
                HexRelayoutProgress::Pending(next) => work = next,
                HexRelayoutProgress::Ready(candidate) => break candidate,
            }
        }
    };
    assert_eq!(run(), run());
}

#[test]
fn pinned_showcase_schedule_accepts_a_real_topology_delta_without_fallback() {
    let mut world = HexWfcWorld::generate(0xA11C_E3D0_0000_0008, config()).expect("world");
    world.config.retry_budget = 12;
    let mut frame = HexObservationFrame::default();
    frame.visible_cells.insert(world.config.spawn());
    let candidate = world.propose_relayout(&frame).expect("proposal");
    assert!(
        !candidate.used_fallback,
        "showcase must exercise WFC topology"
    );
    let placement_delta = candidate
        .placements
        .iter()
        .filter(|(coord, placement)| world.placements.get(coord) != Some(placement))
        .count();
    assert!(placement_delta > 0, "showcase topology did not change");
    world.commit_relayout(candidate, &frame).expect("commit");
}

#[test]
fn ramp_pin_expands_to_both_halves() {
    let world = (0..100u64)
        .find_map(|seed| {
            let world = HexWfcWorld::generate(seed, config()).ok()?;
            world
                .placements
                .values()
                .any(|p| p.archetype == HexArchetype::RampUp)
                .then_some(world)
        })
        .expect("ramp corpus");
    let ramp = world
        .placements
        .values()
        .find(|p| p.archetype == HexArchetype::RampUp)
        .expect("ramp")
        .coord;
    let mut frame = HexObservationFrame::default();
    frame.visible_cells.insert(ramp);
    let work = world.begin_relayout(&frame);
    let head = world.config.grid().neighbor(ramp, HexFace::Up).unwrap();
    assert!(work.pinned_cells().contains(&head));
}

#[test]
fn shaft_pin_stays_on_one_cell_of_a_real_column() {
    let world = HexWfcWorld::generate(0xA11C_E3D0_0000_0008, config()).expect("shaft world");
    let (shaft, neighbor) = world
        .placements
        .values()
        .filter(|p| p.archetype == HexArchetype::Shaft)
        .find_map(|placement| {
            [HexFace::Up, HexFace::Down].into_iter().find_map(|face| {
                let next = world.config.grid().neighbor(placement.coord, face)?;
                (world.placements[&next].archetype == HexArchetype::Shaft)
                    .then_some((placement.coord, next))
            })
        })
        .expect("pinned seed has adjacent shaft cells");
    let mut frame = HexObservationFrame::default();
    frame.visible_cells.insert(shaft);
    let pins = world.begin_relayout(&frame);
    assert!(pins.pinned_cells().contains(&shaft));
    assert!(!pins.pinned_cells().contains(&neighbor));
}

#[test]
fn fallback_changes_one_unpinned_register_and_no_ports() {
    let world = HexWfcWorld::generate(0x9301, config()).expect("world");
    let work = world.begin_relayout(&HexObservationFrame::default());
    let candidate = fallback_geometry_relayout(
        &world,
        work.generation(),
        work.pinned_cells(),
        world.config.retry_budget,
    )
    .expect("unseen fabric");
    assert!(candidate.used_fallback);
    assert_eq!(candidate.changed_cells.len(), 1);
    assert_eq!(candidate.placements, world.placements);
}

#[test]
fn a_failed_attempt_resumes_on_the_next_tick() {
    let found = (0..100u64).find_map(|seed| {
        let mut world = HexWfcWorld::generate(seed, config()).ok()?;
        world.config.retry_budget = 8;
        let (frame, _) = observed_room(&world);
        let work = world.begin_relayout(&frame);
        match world.advance_relayout(work).ok()? {
            HexRelayoutProgress::Pending(work) => Some((world, work)),
            HexRelayoutProgress::Ready(_) => None,
        }
    });
    let (world, work) = found.expect("seed corpus contains a multi-tick relayout");
    assert_eq!(work.next_attempt(), 1);
    let second = world.advance_relayout(work).expect("second tick");
    match second {
        HexRelayoutProgress::Pending(work) => assert_eq!(work.next_attempt(), 2),
        HexRelayoutProgress::Ready(candidate) => assert_eq!(candidate.attempts, 2),
    }
}

#[test]
fn pinned_blueprints_ramps_routes_and_room_ids_survive_corpus() {
    let generation_config = HexWfcConfig {
        cols: 7,
        rows: 5,
        levels: 2,
        min_rooms: 2,
        max_rooms: 4,
        retry_budget: 100,
        min_room_distance: 2,
    };
    let mut non_fallback_count = 0usize;
    let mut placement_delta_count = 0usize;
    let mut topology_delta_count = 0usize;
    let mut topology_delta_seeds = BTreeSet::new();
    let mut topology_delta_pulse_indices = BTreeSet::new();
    for seed_index in 0..100u64 {
        let seed = 0xA11C_9300_0000_0000 | seed_index;
        let mut world = HexWfcWorld::generate(seed, generation_config)
            .unwrap_or_else(|error| panic!("seed {seed:#x}: {error:?}"));
        world.config.retry_budget = 1;
        let (mut frame, observed_cell) = observed_room(&world);
        frame.occupied_cells.insert(PlayerId(0), observed_cell);
        for placement in world.placements.values() {
            if placement.archetype == HexArchetype::RampUp {
                frame.visible_cells.insert(placement.coord);
            }
        }
        let stable_room = world.room_id_at(observed_cell).expect("observed room ID");

        for pulse in 0..20 {
            let work = world.begin_relayout(&frame);
            let pins = work.pinned_cells().clone();
            let pinned_geometry = pins
                .iter()
                .map(|&coord| {
                    (
                        coord,
                        (
                            world.placements[&coord],
                            world.architecture[&coord],
                            world.tile_variation_key(coord),
                        ),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let candidate = match world.advance_relayout(work).expect("attempt") {
                HexRelayoutProgress::Ready(candidate) => candidate,
                HexRelayoutProgress::Pending(_) => {
                    panic!("retry budget one must terminate on pulse {pulse}")
                }
            };
            if !candidate.used_fallback {
                non_fallback_count += 1;
            }
            let placement_delta = candidate
                .placements
                .iter()
                .any(|(coord, placement)| world.placements.get(coord) != Some(placement));
            if placement_delta {
                placement_delta_count += 1;
            }
            if !candidate.used_fallback && placement_delta {
                topology_delta_count += 1;
                topology_delta_seeds.insert(seed_index);
                topology_delta_pulse_indices.insert(pulse);
            }
            for (&coord, &(before, register, _)) in &pinned_geometry {
                assert_eq!(candidate.placements[&coord], before, "seed {seed:#x}");
                assert_eq!(candidate.architecture[&coord], register, "seed {seed:#x}");
            }
            assert!(
                candidate
                    .blueprints
                    .iter()
                    .any(|blueprint| blueprint.cells.contains(&observed_cell)),
                "seed {seed:#x}: observed blueprint vanished"
            );
            assert!(
                super::topology::route_between(
                    world.config,
                    &candidate.placements,
                    observed_cell,
                    world.config.exit(),
                )
                .is_some(),
                "seed {seed:#x}: player route lost"
            );
            world.commit_relayout(candidate, &frame).expect("commit");
            for (&coord, &(_, _, variation_key)) in &pinned_geometry {
                assert_eq!(world.tile_variation_key(coord), variation_key);
            }
            assert_eq!(
                world.room_id_at(observed_cell),
                Some(stable_room),
                "seed {seed:#x}: room ID changed on pulse {pulse}"
            );
        }
    }
    println!(
        "100x20 corpus: {non_fallback_count} non-fallback candidates, \
         {placement_delta_count} placement deltas, {topology_delta_count} genuine topology \
         deltas across {} seeds and {} pulse indices",
        topology_delta_seeds.len(),
        topology_delta_pulse_indices.len()
    );
    assert!(
        non_fallback_count >= 200,
        "expected at least 200 genuine WFC candidates, got {non_fallback_count}"
    );
    assert!(
        placement_delta_count >= 200,
        "expected at least 200 placement deltas, got {placement_delta_count}"
    );
    assert!(
        topology_delta_count >= 200,
        "expected at least 200 genuine topology deltas, got {topology_delta_count}"
    );
    assert!(
        topology_delta_seeds.len() >= 50,
        "topology changed across only {} seeds",
        topology_delta_seeds.len()
    );
    assert!(
        topology_delta_pulse_indices.len() >= 10,
        "topology changed at only {} pulse indices",
        topology_delta_pulse_indices.len()
    );
}

#[test]
fn work_and_candidates_reject_stale_seed_or_config_worlds() {
    let world_a = HexWfcWorld::generate(0x9310, config()).expect("world A");
    let mut world_b = HexWfcWorld::generate(0x9311, config()).expect("world B");
    assert_eq!(world_a.generation, world_b.generation);

    let work = world_a.begin_relayout(&HexObservationFrame::default());
    assert_eq!(
        world_b.advance_relayout(work),
        Err(super::HexWfcError::StaleCandidate)
    );

    let source_work = world_a.begin_relayout(&HexObservationFrame::default());
    let candidate = fallback_geometry_relayout(
        &world_a,
        source_work.generation(),
        source_work.pinned_cells(),
        1,
    )
    .expect("candidate");
    assert_eq!(
        world_b.commit_relayout(candidate, &HexObservationFrame::default()),
        Err(super::HexWfcError::StaleCandidate)
    );

    let mut resized_world = world_a.clone();
    resized_world.config.cols -= 1;
    let work = world_a.begin_relayout(&HexObservationFrame::default());
    assert_eq!(
        resized_world.advance_relayout(work),
        Err(super::HexWfcError::StaleCandidate)
    );

    let source_work = world_a.begin_relayout(&HexObservationFrame::default());
    let candidate = fallback_geometry_relayout(
        &world_a,
        source_work.generation(),
        source_work.pinned_cells(),
        1,
    )
    .expect("candidate");
    assert_eq!(
        resized_world.commit_relayout(candidate, &HexObservationFrame::default()),
        Err(super::HexWfcError::StaleCandidate)
    );
}

#[test]
fn threshold_keys_require_stable_room_identity_and_a_valid_named_port() {
    let world = HexWfcWorld::generate(0x9312, config()).expect("world");
    let blueprint = &world.blueprints[3];
    let definition = super::blueprint_for_role(blueprint.role);
    let &(port, offset, face) = definition.named_ports.first().expect("named port");
    let index = definition
        .cells
        .iter()
        .position(|&cell| cell == offset)
        .expect("port cell");
    let attachment = world
        .config
        .grid()
        .neighbor(blueprint.cells[index], face)
        .expect("attachment in grid");
    let baseline = world
        .begin_relayout(&HexObservationFrame::default())
        .pinned_cells()
        .clone();

    let mut valid = HexObservationFrame::default();
    valid.visible_thresholds.insert(HexThresholdKey {
        room_generation_key: blueprint.generation_key(),
        port,
    });
    let valid_pins = world.begin_relayout(&valid);
    assert!(
        blueprint
            .cells
            .iter()
            .all(|cell| valid_pins.pinned_cells().contains(cell))
    );
    assert!(valid_pins.pinned_cells().contains(&attachment));

    let mut stale = HexObservationFrame::default();
    stale.visible_thresholds.insert(HexThresholdKey {
        room_generation_key: blueprint.generation_key() ^ 0xFFFF,
        port,
    });
    assert_eq!(world.begin_relayout(&stale).pinned_cells(), &baseline);

    let mut invalid = HexObservationFrame::default();
    invalid.visible_thresholds.insert(HexThresholdKey {
        room_generation_key: blueprint.generation_key(),
        port: "not-a-port",
    });
    assert_eq!(world.begin_relayout(&invalid).pinned_cells(), &baseline);
}

#[test]
fn stale_out_of_grid_observation_coordinates_are_ignored_without_panic() {
    let world = HexWfcWorld::generate(0x9315, config()).expect("world");
    let baseline = world
        .begin_relayout(&HexObservationFrame::default())
        .pinned_cells()
        .clone();
    let outside = HexCoord {
        q: u16::MAX,
        r: u16::MAX,
        level: u8::MAX,
    };
    let mut stale = HexObservationFrame::default();
    stale.visible_cells.insert(outside);
    stale.landmark_cells.insert(outside);
    stale.occupied_cells.insert(PlayerId(99), outside);
    assert_eq!(world.begin_relayout(&stale).pinned_cells(), &baseline);
    assert_eq!(world.decoherence_yield(&stale).0, baseline.len());
}

#[test]
fn stable_corridor_cells_keep_their_id_and_attachments_rederive() {
    let mut world = HexWfcWorld::generate(0x9302, config()).expect("world");
    let before_corridors = world.corridors();
    let before_attachments = world.threshold_attachments();
    assert!(!before_corridors.is_empty(), "fixture has corridors");
    assert!(!before_attachments.is_empty(), "fixture has attachments");
    let work = world.begin_relayout(&HexObservationFrame::default());
    let candidate = fallback_geometry_relayout(&world, work.generation(), work.pinned_cells(), 1)
        .expect("fallback");
    world
        .commit_relayout(candidate, &HexObservationFrame::default())
        .expect("commit");
    assert_eq!(world.corridors(), before_corridors);
    assert_eq!(world.threshold_attachments(), before_attachments);
}

fn disconnected_candidate(
    world: &HexWfcWorld,
    latest: &HexObservationFrame,
) -> HexRelayoutCandidate {
    let work = world.begin_relayout(latest);
    let pins = work.pinned_cells().clone();
    let mut candidate =
        fallback_geometry_relayout(world, work.generation(), work.pinned_cells(), 1)
            .expect("fallback candidate");
    for (&coord, placement) in &mut candidate.placements {
        if !pins.contains(&coord) {
            *placement = HexPlacement {
                coord,
                space: HexSpace::Void,
                archetype: HexArchetype::Void,
                doors: 0,
                up: PortClass::Sealed,
                down: PortClass::Sealed,
            };
            candidate.changed_cells.insert(coord);
        }
    }
    candidate
}

fn unpinned_hall(world: &HexWfcWorld) -> HexCoord {
    let baseline = world
        .begin_relayout(&HexObservationFrame::default())
        .pinned_cells()
        .clone();
    world
        .placements
        .values()
        .find(|placement| placement.space == HexSpace::Hall && !baseline.contains(&placement.coord))
        .expect("unpinned hall")
        .coord
}

#[test]
fn commit_rejects_missing_player_and_distinct_objective_routes() {
    let world = HexWfcWorld::generate(0x9313, config()).expect("world");
    let player_cell = unpinned_hall(&world);
    let mut player_frame = HexObservationFrame::default();
    player_frame.occupied_cells.insert(PlayerId(7), player_cell);
    let player_candidate = disconnected_candidate(&world, &player_frame);
    let mut player_world = world.clone();
    assert_eq!(
        player_world.commit_relayout(player_candidate, &player_frame),
        Err(HexWfcError::MissingPlayerRoute(PlayerId(7)))
    );

    let objective_cell = unpinned_hall(&world);
    assert_ne!(objective_cell, world.config.spawn());
    let mut objective_frame = HexObservationFrame::default();
    objective_frame.objective_cells.insert(objective_cell);
    let objective_candidate = disconnected_candidate(&world, &objective_frame);
    let mut objective_world = world;
    assert_eq!(
        objective_world.commit_relayout(objective_candidate, &objective_frame),
        Err(HexWfcError::MissingObjectiveRoute(objective_cell))
    );
}

#[test]
fn commit_rechecks_cells_that_became_visible_or_occupied_after_proposal() {
    let mut world = HexWfcWorld::generate(0x9314, config()).expect("world");
    world.config.retry_budget = 1;
    let candidate = world
        .propose_relayout(&HexObservationFrame::default())
        .expect("proposal");
    let changed = *candidate.changed_cells.iter().next().expect("changed cell");

    let mut visible = HexObservationFrame::default();
    visible.visible_cells.insert(changed);
    let mut visible_world = world.clone();
    assert_eq!(
        visible_world.commit_relayout(candidate.clone(), &visible),
        Err(HexWfcError::UnsafeChange(changed))
    );

    let mut occupied = HexObservationFrame::default();
    occupied.occupied_cells.insert(PlayerId(8), changed);
    assert_eq!(
        world.commit_relayout(candidate, &occupied),
        Err(HexWfcError::UnsafeChange(changed))
    );
}

/// Manual Arc L risk audit. It deliberately uses the 5,600-cell production
/// dimensions and prints the free-cell curve as whole blueprints are observed.
#[test]
#[ignore = "production-scale decoherence-yield measurement"]
fn print_arc_default_decoherence_yield_curve() {
    let world = HexWfcWorld::generate(0xA11C_9300_0000_0001, HexWfcConfig::arc_default())
        .expect("arc-default world");
    let mut frame = HexObservationFrame::default();
    println!("observed_rooms,pinned_cells,free_cells,free_percent");
    for observed_rooms in 0..=world.blueprints.len() {
        let (pinned, free) = world.decoherence_yield(&frame);
        let percent = free as f64 * 100.0 / world.placements.len() as f64;
        println!("{observed_rooms},{pinned},{free},{percent:.2}");
        if let Some(room) = world.blueprints.get(observed_rooms) {
            frame.visible_cells.insert(room.cells[0]);
        }
    }
}
