//! Gates for [`super::neighborhood`].
//!
//! Split out for the same reason `relayout_tests` is: the query and its
//! evidence are each easier to read at half the length, and the review budget
//! the rest of this path lives under is a real one.

use super::collapse::placement_variant;
use super::neighborhood::{Neighborhood, NeighborhoodError, neighborhood};
use super::profile::HexCompositionProfile;
use super::variants::variants_compatible;
use super::{HexArchetype, HexFace, HexWfcConfig, HexWfcWorld};

fn solved() -> HexWfcWorld {
    HexWfcWorld::generate(0x0000_0000_000c_0ffe, HexWfcConfig::default())
        .expect("the fixture config solves")
}

/// The gate this whole module lives or dies by. Whatever the solver
/// actually placed must be one of the things this says could be placed. A
/// domain that excludes reality is not a preview of the solver, it is a
/// different program.
#[test]
fn the_solvers_own_answer_is_always_in_the_domain() {
    let world = solved();
    let profile = HexCompositionProfile::baseline();
    let mut checked = 0;
    for &centre in world.placements.keys() {
        let neighborhood =
            neighborhood(&world, &profile, None, centre).expect("every solved cell describes");
        for domain in &neighborhood.faces {
            assert!(
                domain
                    .candidates
                    .iter()
                    .any(|candidate| candidate.is_actual),
                "{:?} across {:?} from {centre:?}: the placed variant is not in its own domain",
                domain.coord,
                domain.face
            );
            assert_eq!(
                domain
                    .candidates
                    .iter()
                    .filter(|candidate| candidate.is_actual)
                    .count(),
                1,
                "exactly one candidate is the actual placement"
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "the fixture has neighbours to check");
}

/// The centre alone can only ever be a weaker constraint than the centre
/// plus the rest of the facility. If these were ever equal everywhere, the
/// surroundings would not be doing anything and the extra work would be
/// pointless.
#[test]
fn the_surrounding_facility_narrows_what_the_centre_alone_permits() {
    let world = solved();
    let profile = HexCompositionProfile::baseline();
    let mut narrowed = 0;
    for &centre in world.placements.keys() {
        let neighborhood = neighborhood(&world, &profile, None, centre).expect("describes");
        for domain in &neighborhood.faces {
            assert!(
                domain.candidates.len() <= domain.from_centre,
                "narrowing can only remove candidates"
            );
            if domain.candidates.len() < domain.from_centre {
                narrowed += 1;
            }
        }
    }
    assert!(
        narrowed > 0,
        "somewhere in the facility the surroundings must matter, \
         or this query is just an alphabet lookup"
    );
}

/// A wrong attempt number replays a different, entirely plausible
/// constraint set. The blueprint check is what stops that being reported
/// as an answer.
#[test]
fn a_replay_that_disagrees_with_the_world_is_refused() {
    let world = solved();
    let mut lying = world.clone();
    lying.last_attempts = world.last_attempts + 7;
    let centre = *world.placements.keys().next().expect("a cell");
    let result = neighborhood(&lying, &HexCompositionProfile::baseline(), None, centre);
    assert_eq!(result, Err(NeighborhoodError::ConstraintReplayFailed));
}

/// A sampled ring has to be a ring the solver could have produced: every
/// cell drawn from its own domain, and consistent with its ring siblings.
#[test]
fn a_sampled_ring_is_internally_consistent() {
    let world = solved();
    let profile = HexCompositionProfile::baseline();
    let grid = world.config.grid();
    // A cell with room to move, so the sample is actually choosing.
    let centre = *world
        .placements
        .keys()
        .find(|&&coord| {
            neighborhood(&world, &profile, None, coord)
                .is_ok_and(|hood| hood.faces.iter().any(|face| face.candidates.len() > 1))
        })
        .expect("some cell has an open neighbour");
    let hood = neighborhood(&world, &profile, None, centre).expect("describes");

    for roll in 0..16u64 {
        let ring = hood
            .sample_ring(&world, &profile, roll)
            .expect("a consistent ring exists");
        for (&coord, placement) in &ring {
            let domain = hood
                .faces
                .iter()
                .find(|face| face.coord == coord)
                .expect("sampled cell is a ring cell");
            assert!(
                domain
                    .candidates
                    .iter()
                    .any(|candidate| candidate.placement == *placement),
                "sampled a placement outside its own domain"
            );
            for face in HexFace::ALL {
                let Some(neighbor) = grid.neighbor(coord, face) else {
                    continue;
                };
                let other = if neighbor == centre {
                    hood.centre_placement
                } else if let Some(sibling) = ring.get(&neighbor) {
                    *sibling
                } else {
                    continue;
                };
                assert!(
                    variants_compatible(
                        placement_variant(*placement),
                        placement_variant(other),
                        face
                    ),
                    "{coord:?} and {neighbor:?} disagree across {face:?}"
                );
            }
        }
    }
}

/// The default fixture is one level, so it exercises the six lateral faces and
/// nothing else. Verticality is where this facility's grammar is fussiest —
/// paired ramps, shafts that must agree with the cell above — so the gate above
/// is re-run on a 3D lattice, and the up/down faces are asserted to actually
/// appear rather than being quietly absent.
#[test]
fn a_vertical_lattice_describes_its_up_and_down_faces_too() {
    let config = HexWfcConfig {
        levels: 4,
        ..HexWfcConfig::default()
    };
    let world =
        HexWfcWorld::generate(0xa11c_e3d0_0000_0008, config).expect("the 3D fixture solves");
    let profile = HexCompositionProfile::baseline();

    let mut vertical_faces = 0;
    for &centre in world.placements.keys() {
        let hood =
            neighborhood(&world, &profile, None, centre).expect("every solved cell describes");
        for domain in &hood.faces {
            assert!(
                domain
                    .candidates
                    .iter()
                    .any(|candidate| candidate.is_actual),
                "{:?} across {:?}: the placed variant is not in its own domain",
                domain.coord,
                domain.face
            );
            if !domain.face.is_lateral() {
                vertical_faces += 1;
            }
        }
    }
    assert!(
        vertical_faces > 0,
        "a four-level lattice must report up and down faces"
    );
}

/// Weights are what an author tunes against, so the profile has to reach
/// them. If a bias moved the solve but not this readout, the tool would be
/// showing the wrong distribution.
#[test]
fn the_profile_moves_the_reported_weights() {
    let world = solved();
    let baseline = HexCompositionProfile::baseline();
    let mut biased = HexCompositionProfile::baseline();
    biased.archetype_bias = biased.archetype_bias.with(HexArchetype::Junction, 4.0);

    let centre = *world
        .placements
        .keys()
        .find(|&&coord| {
            neighborhood(&world, &baseline, None, coord).is_ok_and(|hood| {
                hood.faces.iter().any(|face| {
                    face.candidates
                        .iter()
                        .any(|c| c.placement.archetype == HexArchetype::Junction)
                })
            })
        })
        .expect("a junction is reachable somewhere");

    let before = neighborhood(&world, &baseline, None, centre).expect("describes");
    let after = neighborhood(&world, &biased, None, centre).expect("describes");
    let junction_weight = |hood: &Neighborhood| -> u64 {
        hood.faces
            .iter()
            .flat_map(|face| face.candidates.iter())
            .filter(|c| c.placement.archetype == HexArchetype::Junction)
            .map(|c| c.weight)
            .sum()
    };
    assert!(
        junction_weight(&after) > junction_weight(&before),
        "raising the junction bias must raise the junction weight in the readout"
    );
}
