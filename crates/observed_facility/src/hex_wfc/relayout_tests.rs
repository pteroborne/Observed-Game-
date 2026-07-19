use std::collections::{BTreeMap, BTreeSet};

use observed_core::PlayerId;

use super::relayout::fallback_geometry_relayout;
use super::{
    DEFAULT_MUTATION_MAX_CELLS, DEFAULT_MUTATION_TARGET_CELLS, HexArchetype, HexFace,
    HexObservationFrame, HexRelayoutProgress, HexThresholdKey, HexWfcConfig, HexWfcError,
    HexWfcWorld,
};

fn config() -> HexWfcConfig {
    HexWfcConfig {
        levels: 4,
        ..HexWfcConfig::default()
    }
}

fn candidate(world: &HexWfcWorld, frame: &HexObservationFrame) -> super::HexRelayoutCandidate {
    let mut work = world.begin_relayout(frame);
    loop {
        match world.advance_relayout(work).expect("relayout") {
            HexRelayoutProgress::Pending(next) => work = next,
            HexRelayoutProgress::Ready(candidate) => return candidate,
        }
    }
}

#[test]
fn pocket_selection_and_attempts_are_deterministic() {
    let world = HexWfcWorld::generate(0x9300, config()).expect("world");
    let mut frame = HexObservationFrame::default();
    frame
        .occupied_cells
        .insert(PlayerId(0), world.config.spawn());
    let a_work = world.begin_relayout(&frame);
    let b_work = world.begin_relayout(&frame);
    assert_eq!(a_work, b_work);
    assert_eq!(a_work.region().cells.len(), DEFAULT_MUTATION_TARGET_CELLS);
    assert_eq!(candidate(&world, &frame), candidate(&world, &frame));
}

#[test]
fn target_pocket_changes_at_most_four_topology_cells() {
    let world = HexWfcWorld::generate(0xA11C_9500_0000_0000, config()).expect("world");
    let proposal = candidate(&world, &HexObservationFrame::default());
    assert!(proposal.region.cells.len() >= DEFAULT_MUTATION_TARGET_CELLS);
    let topology_changes = proposal
        .placements
        .iter()
        .filter(|(coord, placement)| world.placements.get(coord) != Some(*placement))
        .count();
    assert!(
        topology_changes <= 4,
        "bounded structural core changed {topology_changes} cells"
    );
}

#[test]
fn pocket_is_bounded_and_closes_indivisible_units() {
    let world = HexWfcWorld::generate(0xA11C_E3D0_0000_0008, config()).expect("world");
    let mut frame = HexObservationFrame::default();
    frame
        .occupied_cells
        .insert(PlayerId(0), world.config.spawn());
    let work = world.begin_relayout(&frame);
    let region = work.region();
    assert!(region.cells.len() >= DEFAULT_MUTATION_TARGET_CELLS);
    assert!(region.cells.len() <= DEFAULT_MUTATION_MAX_CELLS);
    assert!(region.cells.is_disjoint(&region.protected_cells));
    assert!(region.boundary_cells.is_disjoint(&region.cells));

    for blueprint in &world.blueprints {
        if blueprint
            .cells
            .iter()
            .any(|cell| region.cells.contains(cell))
        {
            assert!(
                blueprint
                    .cells
                    .iter()
                    .all(|cell| region.cells.contains(cell))
            );
        }
    }
    for &coord in &region.cells {
        match world.placements[&coord].archetype {
            HexArchetype::RampUp => {
                let mate = world.config.grid().neighbor(coord, HexFace::Up).unwrap();
                assert!(region.cells.contains(&mate));
            }
            HexArchetype::RampHead => {
                let mate = world.config.grid().neighbor(coord, HexFace::Down).unwrap();
                assert!(region.cells.contains(&mate));
            }
            HexArchetype::Shaft => {
                for face in [HexFace::Up, HexFace::Down] {
                    if let Some(mate) = world.config.grid().neighbor(coord, face)
                        && world.placements[&mate].archetype == HexArchetype::Shaft
                    {
                        assert!(region.cells.contains(&mate));
                    }
                }
            }
            _ => {}
        }
    }
}

#[test]
fn observation_pins_receive_a_one_cell_safety_halo() {
    let world = HexWfcWorld::generate(0x9301, config()).expect("world");
    let cell = world.config.spawn();
    let mut frame = HexObservationFrame::default();
    frame.visible_cells.insert(cell);
    let work = world.begin_relayout(&frame);
    assert!(work.pinned_cells().contains(&cell));
    for face in HexFace::ALL {
        if let Some(neighbor) = world.config.grid().neighbor(cell, face) {
            assert!(work.pinned_cells().contains(&neighbor));
        }
    }
}

#[test]
fn fallback_and_commit_touch_only_the_bounded_patch_and_revision() {
    let mut world = HexWfcWorld::generate(0x9302, config()).expect("world");
    let before = world.clone();
    let work = world.begin_relayout(&HexObservationFrame::default());
    let candidate =
        fallback_geometry_relayout(&world, work.generation(), work.region(), 1).expect("fallback");
    assert_eq!(candidate.changed_cells.len(), 1);
    assert_eq!(candidate.placements.len(), candidate.region.cells.len());
    let changed = *candidate.changed_cells.first().expect("changed cell");
    let delta = world
        .commit_relayout_delta(candidate, &HexObservationFrame::default())
        .expect("commit");
    assert_eq!(delta.changed_cells, BTreeSet::from([changed]));
    assert_eq!(world.cell_revision(changed), Some(1));
    for &coord in before.placements.keys() {
        if coord == changed {
            continue;
        }
        assert_eq!(world.placements[&coord], before.placements[&coord]);
        assert_eq!(world.architecture[&coord], before.architecture[&coord]);
        assert_eq!(world.cell_revision(coord), Some(0));
    }
}

#[test]
fn accepted_delta_can_restore_the_exact_previous_world() {
    let mut world = HexWfcWorld::generate(0x9303, config()).expect("world");
    let before = world.clone();
    let work = world.begin_relayout(&HexObservationFrame::default());
    let fallback =
        fallback_geometry_relayout(&world, work.generation(), work.region(), 1).expect("fallback");
    let delta = world
        .commit_relayout_delta(fallback, &HexObservationFrame::default())
        .expect("commit");
    world.revert_relayout_delta(delta).expect("rollback");
    assert_eq!(world, before);
}

#[test]
fn commit_rejects_a_region_that_enters_the_latest_safety_halo() {
    let mut world = HexWfcWorld::generate(0x9304, config()).expect("world");
    let proposal = candidate(&world, &HexObservationFrame::default());
    let changed = *proposal.changed_cells.first().expect("changed cell");
    let mut latest = HexObservationFrame::default();
    latest.visible_cells.insert(changed);
    assert!(matches!(
        world.commit_relayout(proposal, &latest),
        Err(HexWfcError::UnsafeChange(_))
    ));
}

#[test]
fn protected_whole_grid_reports_no_region() {
    let world = HexWfcWorld::generate(0x9305, config()).expect("world");
    let mut frame = HexObservationFrame::default();
    frame.visible_cells.extend(world.placements.keys().copied());
    let work = world.begin_relayout(&frame);
    assert!(work.region().cells.is_empty());
    assert_eq!(
        world.advance_relayout(work),
        Err(HexWfcError::NoMutationRegion)
    );
}

#[test]
fn work_and_candidate_reject_other_seed_or_config() {
    let world_a = HexWfcWorld::generate(0x9306, config()).expect("world A");
    let mut world_b = HexWfcWorld::generate(0x9307, config()).expect("world B");
    assert_eq!(
        world_b.advance_relayout(world_a.begin_relayout(&HexObservationFrame::default())),
        Err(HexWfcError::StaleCandidate)
    );
    let proposal = candidate(&world_a, &HexObservationFrame::default());
    assert_eq!(
        world_b.commit_relayout(proposal, &HexObservationFrame::default()),
        Err(HexWfcError::StaleCandidate)
    );
}

#[test]
fn named_threshold_pins_room_and_attachment() {
    let world = HexWfcWorld::generate(0x9312, config()).expect("world");
    let stamped = &world.blueprints[2];
    let definition = super::blueprint_for_role(stamped.role);
    let &(port, offset, face) = definition.named_ports.first().expect("port");
    let index = definition
        .cells
        .iter()
        .position(|cell| *cell == offset)
        .unwrap();
    let attachment = world
        .config
        .grid()
        .neighbor(stamped.cells[index], face)
        .expect("attachment");
    let mut frame = HexObservationFrame::default();
    frame.visible_thresholds.insert(HexThresholdKey {
        room_generation_key: stamped.generation_key(),
        port,
    });
    let work = world.begin_relayout(&frame);
    assert!(
        stamped
            .cells
            .iter()
            .all(|cell| work.pinned_cells().contains(cell))
    );
    assert!(work.pinned_cells().contains(&attachment));
}

#[test]
fn stale_out_of_grid_observations_are_ignored() {
    let world = HexWfcWorld::generate(0x9315, config()).expect("world");
    let outside = super::HexCoord {
        q: u16::MAX,
        r: u16::MAX,
        level: u8::MAX,
    };
    let mut stale = HexObservationFrame::default();
    stale.visible_cells.insert(outside);
    stale.landmark_cells.insert(outside);
    stale.occupied_cells.insert(PlayerId(99), outside);
    let work = world.begin_relayout(&stale);
    assert!(!work.region().cells.contains(&outside));
}

#[test]
fn committed_patch_preserves_player_and_objective_routes() {
    let mut world = HexWfcWorld::generate(0x9316, config()).expect("world");
    let mut frame = HexObservationFrame::default();
    frame
        .occupied_cells
        .insert(PlayerId(0), world.config.spawn());
    frame.objective_cells.insert(world.config.spawn());
    let proposal = candidate(&world, &frame);
    world.commit_relayout(proposal, &frame).expect("commit");
    assert!(
        world
            .route_between(world.config.spawn(), world.config.exit())
            .is_some()
    );
}

#[test]
fn delta_maps_never_exceed_the_region() {
    let mut world = HexWfcWorld::generate(0x9317, config()).expect("world");
    let proposal = candidate(&world, &HexObservationFrame::default());
    let region = proposal.region.cells.clone();
    let delta = world
        .commit_relayout_delta(proposal, &HexObservationFrame::default())
        .expect("commit");
    let placement_cells = delta.placements.keys().copied().collect::<BTreeSet<_>>();
    let register_cells = delta.architecture.keys().copied().collect::<BTreeSet<_>>();
    assert!(placement_cells.is_subset(&region));
    assert!(register_cells.is_subset(&region));
    assert!(delta.changed_cells.is_subset(&region));
    let _: BTreeMap<_, _> = delta.cell_revisions;
}

#[test]
fn local_pockets_commit_routes_across_a_seed_corpus() {
    let mut committed = 0;
    for seed in 0xA11C_0000..0xA11C_0020 {
        let mut world = HexWfcWorld::generate(seed, config()).expect("world");
        let mut frame = HexObservationFrame::default();
        frame
            .occupied_cells
            .insert(PlayerId(0), world.config.spawn());
        frame.objective_cells.insert(world.config.spawn());
        let proposal = candidate(&world, &frame);
        if world.commit_relayout(proposal, &frame).is_ok() {
            committed += 1;
        }
    }
    assert!(
        committed >= 24,
        "at least three quarters of deterministic pockets should preserve live routes; got {committed}/32"
    );
}
