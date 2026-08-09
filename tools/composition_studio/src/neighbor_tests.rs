//! Gates for the neighbourhood explorer.
//!
//! A sibling of `tests.rs` so neither file outgrows the 600-line review budget
//! the rest of the WFC path lives under.

use bevy::prelude::Vec3;
use observed_facility::hex_wfc::HexArchetype;
use observed_traversal::ColliderShape;

use crate::StudioState;

/// A solved studio with a cell selected that actually has options, so a test
/// asserting about alternatives is not quietly asserting about a forced face.
fn studio_with_an_open_selection() -> StudioState {
    let mut state = StudioState::default();
    crate::solve::run_solve(&mut state);
    let coords: Vec<observed_hex::HexCoord> = state
        .solved
        .as_ref()
        .expect("the fixture solves")
        .world
        .placements
        .keys()
        .copied()
        .collect();
    for coord in coords {
        state.selected = Some(coord);
        crate::neighbors::refresh(&mut state);
        let open = state
            .neighbors
            .hood
            .as_ref()
            .is_some_and(|hood| hood.faces.iter().any(|face| face.candidates.len() > 1));
        if open {
            return state;
        }
    }
    panic!("no cell in the fixture facility has an unforced neighbour");
}

/// The load-bearing gate for the whole view.
///
/// The wireframe is drawn from `project_hypothetical_cell` and the solid from
/// the projected snapshot. They are two entry points into the same projector,
/// and if they ever drew different hulls for the same placement, the mode would
/// be quietly lying about the shape of the thing it is previewing — an author
/// would tune against geometry the game does not have.
#[test]
fn the_previewed_wireframe_is_the_same_geometry_the_solid_pass_draws() {
    let state = studio_with_an_open_selection();
    let solved = state.solved.as_ref().expect("solved");
    let snapshot = solved.geometry.as_ref().expect("the corpus projects");
    let (prototypes, _) = crate::corpus().as_ref().expect("the corpus loads");
    let hood = state.neighbors.hood.as_ref().expect("a neighbourhood");

    let mut compared = 0;
    for domain in &hood.faces {
        // The *actual* placement, so the snapshot has something to compare to.
        let projected: Vec<Vec<Vec3>> = snapshot
            .pieces
            .iter()
            .filter(|piece| piece.source_cell == domain.coord)
            .filter_map(|piece| match &piece.shape {
                ColliderShape::ConvexHull { points } => Some(points.clone()),
                _ => None,
            })
            .collect();
        let previewed =
            crate::neighbors::preview_hulls(&solved.world, prototypes, domain.coord, domain.actual);
        match previewed {
            Ok(previewed) => {
                assert_eq!(
                    previewed, projected,
                    "the preview and the snapshot disagree about ({},{},{})",
                    domain.coord.q, domain.coord.r, domain.coord.level
                );
                compared += 1;
            }
            // A cell whose geometry a stamped room supplies is projected once
            // from its anchor, not per cell, so the per-cell path has nothing
            // to say about it. Not a disagreement.
            Err(_) => assert!(
                solved.world.room_id_at(domain.coord).is_some()
                    || solved.world.placements[&domain.coord].archetype == HexArchetype::Void,
                "a per-cell placement failed to project for the preview"
            ),
        }
    }
    assert!(compared > 0, "the fixture compared no cells");
}

/// Cycling must stay inside the domain the solver would have had. A preview
/// that could walk off the end of its own candidate list would be showing an
/// arrangement the solver cannot produce, which is the one thing this view
/// exists not to do.
#[test]
fn cycling_a_face_never_previews_something_outside_its_domain() {
    let mut state = studio_with_an_open_selection();
    let faces = state.neighbors.hood.as_ref().expect("a hood").faces.len();

    for face_index in 0..faces {
        state.neighbors.cursor = face_index;
        // Well past a full cycle in both directions, so wrap-around is covered.
        for delta in [1isize, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1, -1, -1, -1] {
            state.neighbors.step_candidate(delta);
            let hood = state.neighbors.hood.as_ref().expect("a hood");
            let domain = &hood.faces[face_index];
            let previewed = state
                .neighbors
                .previewed(domain.coord)
                .expect("a previewed placement");
            assert!(
                domain
                    .candidates
                    .iter()
                    .any(|candidate| candidate.placement == previewed),
                "cycling left {:?} previewing something the solver could not place",
                domain.face
            );
        }
    }
}

/// Resetting has to land back on exactly what the seed produced, or "green
/// means what actually happened" stops being true after one keypress.
#[test]
fn resetting_returns_every_face_to_what_the_seed_solved() {
    let mut state = studio_with_an_open_selection();
    for index in 0..state.neighbors.hood.as_ref().expect("a hood").faces.len() {
        state.neighbors.cursor = index;
        state.neighbors.step_candidate(1);
    }
    state.neighbors.reset_to_solved();

    let hood = state.neighbors.hood.as_ref().expect("a hood");
    for domain in &hood.faces {
        assert_eq!(
            state.neighbors.previewed(domain.coord),
            Some(domain.actual),
            "{:?} did not return to what the seed solved",
            domain.face
        );
    }
}

/// A rolled ring is drawn from the real sampler, so every cell of it must be a
/// candidate of its own face. This is the studio-side half of the solver's own
/// consistency gate: it checks the *wiring*, not the sampler.
#[test]
fn a_rolled_ring_only_ever_previews_real_candidates() {
    let mut state = studio_with_an_open_selection();
    let profile = state.profile.clone();
    let world = state.solved.as_ref().expect("solved").world.clone();

    for _ in 0..8 {
        state.neighbors.roll_ring(&world, &profile);
        let hood = state.neighbors.hood.as_ref().expect("a hood");
        for domain in &hood.faces {
            let previewed = state
                .neighbors
                .previewed(domain.coord)
                .expect("a previewed placement");
            assert!(
                domain
                    .candidates
                    .iter()
                    .any(|candidate| candidate.placement == previewed),
                "a roll previewed something outside {:?}'s domain",
                domain.face
            );
        }
    }
}

/// The studio could not reach a vertical face at all before the working scale
/// became settable, so the mode's two most rule-heavy faces were unreachable in
/// the tool that exists to ask about them. This is that gap, closed.
#[test]
fn a_taller_working_scale_puts_up_and_down_faces_in_reach() {
    let mut state = StudioState::default();
    assert_eq!(
        state.config.levels, 1,
        "the historical default is one floor"
    );
    state.set_levels(4, 0.0);
    assert_eq!(state.config.levels, 4);
    // Changing the facility must clear a selection that described the old one.
    assert_eq!(state.selected, None);
    crate::solve::run_solve(&mut state);

    let coords: Vec<observed_hex::HexCoord> = state
        .solved
        .as_ref()
        .expect("the taller fixture solves")
        .world
        .placements
        .keys()
        .copied()
        .collect();
    let mut vertical = 0;
    for coord in coords {
        state.selected = Some(coord);
        crate::neighbors::refresh(&mut state);
        if let Some(hood) = state.neighbors.hood.as_ref() {
            vertical += hood
                .faces
                .iter()
                .filter(|face| !face.face.is_lateral())
                .count();
        }
    }
    assert!(vertical > 0, "a four-floor studio must reach up and down");
}

/// The cap is production scale, not an invented ceiling, and the floor is one.
#[test]
fn the_working_scale_is_clamped_at_both_ends() {
    let mut state = StudioState::default();
    state.set_levels(0, 0.0);
    assert_eq!(state.config.levels, 1);
    state.set_levels(200, 0.0);
    assert_eq!(state.config.levels, crate::MAX_WORKING_LEVELS);
}

/// An empty selection must say so. A blank panel reads as a broken tool, and
/// this view has a real refusal path worth keeping visible.
#[test]
fn no_selection_reports_a_refusal_rather_than_an_empty_panel() {
    let mut state = StudioState::default();
    crate::solve::run_solve(&mut state);
    state.selected = None;
    crate::neighbors::refresh(&mut state);

    assert!(state.neighbors.hood.is_none());
    let panel = crate::panels::neighbors::format_neighbors_panel(&state.neighbors);
    assert!(
        panel.contains("select a cell"),
        "the panel must say what to do: {panel}"
    );
}
