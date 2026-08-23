//! Pin and coverage gates for `composition_studio`.
//!
//! A sibling of `tests.rs` so neither file outgrows the 600-line review
//! budget the rest of the WFC path lives under.

use bevy::prelude::*;
use observed_facility::hex_wfc::{HexArchetype, HexWfcConfig, HexWfcWorld};

use crate::{PRESET_SEEDS, StudioState};
// ------------------------------------------------------------------------ pins

/// The first-class line: paint a pin, and the facility actually changes to
/// honour it. If this passes, the tool is an editor rather than a settings panel.
#[test]
fn a_painted_pin_reaches_the_solved_facility() {
    use observed_hex::HexCoord;

    let mut state = StudioState::default();
    crate::solve::run_solve(&mut state);

    // Find a cell the baseline did not already build as a junction, so the
    // assertion cannot pass by coincidence — and that nothing decided before the
    // collapse has already claimed.
    //
    // Two things can claim one. A stamped room is the long-standing case and the
    // test has always allowed for it below. A **routed corridor** is the new
    // one: `route_corridors` is on by default, the skeleton fixes its cells to
    // an exact door mask, and that mask outranks a pin the same way a blueprint
    // does. Painting a Junction onto a corridor is not a pin that failed to
    // reach the facility, it is a pin the route overruled — reported as
    // `PinDiagnostic::CorridorCollision` and asserted on in `pins.rs`.
    //
    // Rather than trying to guess which cells the router owns, the assertion
    // below accepts the reported override the same way it already accepts the
    // reported blueprint one. An override that is *not* reported is still a
    // failure, which is the property worth keeping.
    let target = state
        .solved
        .as_ref()
        .expect("solves")
        .world
        .placements
        .iter()
        .find(|(coord, placement)| {
            placement.archetype != HexArchetype::Junction
                && placement.archetype != HexArchetype::Void
                && placement.space != observed_facility::hex_wfc::HexSpace::Room
                && coord.level == 0
        })
        .map(|(coord, _)| *coord)
        .expect("some cell is not already a junction");

    let config = state.config;
    assert!(crate::brush::paint(
        &mut state.profile,
        config,
        target,
        crate::brush::Brush::Junction
    ));
    assert_eq!(state.profile.validate(), Ok(()));
    crate::solve::run_solve(&mut state);

    state.refresh_pin_diagnostics();
    let overruled_by_route = state.pin_diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic,
            observed_facility::hex_wfc::PinDiagnostic::CorridorCollision { coord, .. }
                if *coord == target
        )
    });
    let world = &state.solved.as_ref().expect("re-solves").world;
    // A stamped room may claim the cell, and so may a routed corridor. Both are
    // decided before the collapse and both outrank a pin; what the tool owes the
    // author is to *say so*, which is why the route case is read off the
    // diagnostics rather than inferred from the geometry.
    if world.room_id_at(target).is_none() && !overruled_by_route {
        assert_eq!(
            world.placements[&target].archetype,
            HexArchetype::Junction,
            "the painted pin must reach the facility at ({},{},{})",
            target.q,
            target.r,
            target.level
        );
    }
    assert!(
        world.authored_pins.contains(&target),
        "the solved world must carry the pin"
    );
    let _ = HexCoord::default;
}

/// Painting is a profile edit, so it must move the hash and mark the profile
/// unsaved — an author has to see that they just changed shipped content.
#[test]
fn painting_a_pin_marks_the_profile_unsaved_and_moves_the_hash() {
    let mut state = StudioState {
        catalog_hash: crate::CatalogHash::Known("a".repeat(64)),
        ..StudioState::default()
    };
    state.saved = state.profile.clone();
    assert!(!state.is_unsaved());
    let before = crate::persist::simulation_hash(&state, &state.profile);

    let config = state.config;
    let coord = observed_hex::HexCoord {
        q: 4,
        r: 4,
        level: 0,
    };
    crate::brush::paint(
        &mut state.profile,
        config,
        coord,
        crate::brush::Brush::Shaft,
    );

    assert!(state.is_unsaved(), "a pin is an unsaved profile edit");
    assert_ne!(
        before,
        crate::persist::simulation_hash(&state, &state.profile),
        "a pin must move the simulation content hash"
    );
}

/// An impossible pin must be reported, not merely fail to solve.
#[test]
fn an_impossible_pin_is_diagnosed_rather_than_silently_dropped() {
    use observed_facility::hex_wfc::profile::{HexPin, PinIntent, PinSet};

    let mut state = StudioState::default();
    let config = state.config;
    state.profile.pin_sets.push(PinSet {
        id: String::from("bad"),
        note: String::new(),
        cols: config.cols,
        rows: config.rows,
        levels: config.levels,
        pins: vec![HexPin {
            q: 2,
            r: 2,
            level: 0,
            intent: PinIntent::Space(observed_facility::hex_wfc::HexSpace::Room),
        }],
    });
    state.refresh_pin_diagnostics();

    assert!(
        state.pin_diagnostics.iter().any(|d| matches!(
            d,
            observed_facility::hex_wfc::PinDiagnostic::Unsatisfiable { .. }
        )),
        "a room pin on fabric must be diagnosed: {:?}",
        state.pin_diagnostics
    );

    // And the solve must lead with the pin, in words. A status that dumped the
    // error enum, or named the retry budget, would tell the author nothing
    // about which pin to go and change.
    crate::solve::run_solve(&mut state);
    assert!(
        state.status.contains("PIN:"),
        "the status must lead with the pin failure: {}",
        state.status
    );
    assert!(
        state.status.contains("rooms are stamped"),
        "the status must explain the failure, not just name it: {}",
        state.status
    );
    assert!(
        !state.status.contains("RetryBudgetExhausted"),
        "the retry budget is not the cause and must not be blamed: {}",
        state.status
    );
}

/// Clearing every pin must return the profile to byte-identical baseline, or a
/// stray empty pin set would move the content hash for nothing.
#[test]
fn clearing_pins_restores_a_baseline_profile() {
    let mut state = StudioState::default();
    let config = state.config;
    for q in 3..6 {
        crate::brush::paint(
            &mut state.profile,
            config,
            observed_hex::HexCoord { q, r: 3, level: 0 },
            crate::brush::Brush::Junction,
        );
    }
    assert!(!state.profile.is_baseline());
    crate::brush::clear(&mut state.profile);
    assert!(state.profile.is_baseline());
}

// ---------------------------------------------------------------------- detail

/// The focus set is the *connection* set: the selected cell plus everything it
/// shares an open port with. That is what "which tiles are connecting" asks,
/// and it is what bounds the cost at eight cells regardless of facility size.
#[test]
fn the_focus_set_is_exactly_the_connection_set() {
    use observed_facility::hex_wfc::{HexFace, PortClass};

    let world = HexWfcWorld::generate(PRESET_SEEDS[0], HexWfcConfig::default()).expect("solves");
    let grid = world.config.grid();

    let centre = *world
        .placements
        .iter()
        .find(|(_, placement)| {
            HexFace::LATERAL
                .iter()
                .filter(|face| placement.is_open(**face))
                .count()
                >= 2
        })
        .expect("some cell has two open faces")
        .0;

    let set = crate::detail::focus_set(&world, Some(centre));
    assert!(set.contains(&centre), "the selected cell is always in");
    assert!(set.len() <= 8, "focus must stay bounded, got {}", set.len());

    let placement = world.placements[&centre];
    for face in HexFace::ALL {
        let open = if face.is_lateral() {
            placement.is_open(face)
        } else if face == HexFace::Up {
            placement.up != PortClass::Sealed
        } else {
            placement.down != PortClass::Sealed
        };
        let Some(neighbour) = grid.neighbor(centre, face) else {
            continue;
        };
        if !world.placements.contains_key(&neighbour) {
            continue;
        }
        assert_eq!(
            set.contains(&neighbour),
            open,
            "{face:?}: a neighbour is in the focus set exactly when the port is open"
        );
    }
}

#[test]
fn an_empty_selection_focuses_nothing() {
    let world = HexWfcWorld::generate(PRESET_SEEDS[0], HexWfcConfig::default()).expect("solves");
    assert!(crate::detail::focus_set(&world, None).is_empty());
}

/// Detent 0 must be the historical camera, or every capture taken before this
/// feature silently stops matching what the tool now produces.
#[test]
fn detent_zero_is_the_historical_camera() {
    let (a, ..) = crate::viewport::frame_camera(Vec3::ZERO, Vec3::splat(10.0));
    let (b, ..) = crate::viewport::frame_camera_at(Vec3::ZERO, Vec3::splat(10.0), 0);
    assert_eq!(a.rotation, b.rotation);
    assert!(
        (crate::viewport::detent_yaw(0) - std::f32::consts::FRAC_PI_4).abs() < 1e-6,
        "detent 0 must be the 45-degree default"
    );
}

/// Six detents, evenly spaced, wrapping. If two detents shared a bearing the
/// cutaway would show the same three walls twice and hide two of them forever.
#[test]
fn the_six_detents_are_distinct_and_wrap() {
    let bearings: Vec<Vec2> = (0..crate::viewport::AZIMUTH_DETENTS)
        .map(crate::viewport::detent_bearing)
        .collect();
    for (i, a) in bearings.iter().enumerate() {
        assert!(
            (a.length() - 1.0).abs() < 1e-5,
            "bearing {i} must be a unit vector"
        );
        for (j, b) in bearings.iter().enumerate() {
            if i != j {
                assert!(a.dot(*b) < 0.99, "detents {i} and {j} share a bearing");
            }
        }
    }
    assert!(
        (crate::viewport::detent_yaw(crate::viewport::AZIMUTH_DETENTS)
            - crate::viewport::detent_yaw(0))
        .abs()
            < 1e-6,
        "the detent cycle must wrap"
    );
}

/// Floor navigation: the focus floor plus one either side, and no further. A
/// ten-level facility drawn all at once stacks into an unreadable thicket, and
/// the top floor hides everything under it anyway.
#[test]
fn a_single_floor_view_draws_its_neighbours_as_context_only() {
    use crate::Layer;

    let layer = Layer::Single(4);
    assert!(layer.draws(3) && layer.draws(4) && layer.draws(5));
    assert!(
        !layer.draws(2),
        "two floors away is not context, it is noise"
    );
    assert!(!layer.draws(6));

    assert!(layer.is_focus(4), "only the focus floor gets solid detail");
    assert!(!layer.is_focus(3));
    assert!(!layer.is_focus(5));

    // The bottom floor has no floor below; the window just clips.
    let ground = Layer::Single(0);
    assert!(ground.draws(0) && ground.draws(1));

    // All-layers draws and focuses everything, so it stays the escape hatch.
    assert!(Layer::All.draws(9) && Layer::All.is_focus(9));
}

/// Hover must not dirty geometry. This is the whole reason the flags are split:
/// hover fires on every mouse move, and re-triangulating a facility for a ring
/// is affordable at a hundred cells and ruinous at five thousand.
#[test]
fn moving_the_hover_does_not_dirty_geometry() {
    let mut state = StudioState {
        geometry_dirty: false,
        overlay_dirty: false,
        ..StudioState::default()
    };
    crate::solve::run_solve(&mut state);
    state.geometry_dirty = false;
    state.overlay_dirty = false;

    state.hovered = Some(observed_hex::HexCoord {
        q: 4,
        r: 4,
        level: 0,
    });
    state.touch_overlay();

    assert!(state.overlay_dirty, "the ring must be redrawn");
    assert!(
        !state.geometry_dirty,
        "moving a ring must never re-triangulate the facility"
    );
}

/// Changing what is drawn *must* dirty geometry, or the split would silently
/// stop the viewport updating.
#[test]
fn changing_the_view_dirties_geometry() {
    let mut state = StudioState {
        geometry_dirty: false,
        overlay_dirty: false,
        ..StudioState::default()
    };
    state.touch_view();
    assert!(state.geometry_dirty && state.overlay_dirty);
}

/// The cache exists to stop the same tile being triangulated once per cell.
/// Building twice must not grow it, and it must hold far fewer entries than the
/// hulls it drew — that ratio *is* the saving.
#[test]
fn the_tile_mesh_cache_is_reused_across_cells_and_rebuilds() {
    use observed_facility::hex_wfc::HexWfcConfig;

    let Ok((cells, rooms)) = crate::corpus() else {
        panic!("the committed corpus must load");
    };
    let world = HexWfcWorld::generate(PRESET_SEEDS[0], HexWfcConfig::default()).expect("solves");
    let snapshot =
        observed_match::hex_wfc::HexWfcGeometrySnapshot::project_with_rooms(&world, cells, rooms)
            .expect("projects");

    let drawn: std::collections::BTreeSet<_> = world
        .placements
        .iter()
        .filter(|(_, placement)| {
            placement.archetype != observed_facility::hex_wfc::HexArchetype::Void
        })
        .map(|(coord, _)| *coord)
        .collect();

    let mut cache = crate::detail::TileMeshCache::default();
    let bearing = crate::viewport::detent_bearing(0);
    let (_, _, first) =
        crate::detail::build(&world, &snapshot, &drawn, None, bearing, true, &mut cache);
    let after_first = cache.len();

    let (_, _, second) =
        crate::detail::build(&world, &snapshot, &drawn, None, bearing, true, &mut cache);
    assert_eq!(
        after_first,
        cache.len(),
        "a second rebuild must triangulate nothing new"
    );
    assert_eq!(
        first.hulls_drawn, second.hulls_drawn,
        "and must draw exactly the same geometry"
    );

    let emitted = first.hulls_drawn + first.hulls_cut;
    assert!(after_first > 0, "the cache must actually be populated");
    assert!(
        after_first < emitted,
        "caching must save work: {after_first} distinct hulls for {emitted} placements"
    );
}

// -------------------------------------------------------------------- coverage

/// The coverage report now *calls* the projector's selector instead of
/// restating its rule, so the two must never disagree: if coverage says every
/// demand is met, the real projector has to accept the layout, and vice versa.
///
/// This is the test that kept the old hand-written mirror honest. It stays,
/// because it is still the end-to-end claim — that coverage predicts loading —
/// and because it is what would catch the panel being rewired to a stale index.
#[test]
fn coverage_agrees_with_the_projector() {
    use crate::coverage::ProjectorVerdict;

    let Ok((cells, rooms)) = crate::corpus() else {
        panic!("the committed corpus must load for this gate to mean anything");
    };

    for &seed in &PRESET_SEEDS {
        let world = HexWfcWorld::generate(seed, HexWfcConfig::default()).expect("solves");
        let projected = observed_match::hex_wfc::HexWfcGeometrySnapshot::project_with_rooms(
            &world, cells, rooms,
        );
        let (snapshot, error) = match projected {
            Ok(snapshot) => (Some(snapshot), None),
            Err(error) => (None, Some(error)),
        };
        let coverage = crate::coverage::CoverageReport::build(
            &world,
            cells,
            rooms,
            snapshot.as_ref(),
            error.as_ref(),
        );

        match coverage.verdict {
            Some(ProjectorVerdict::Projected) => assert!(
                coverage.unmet.is_empty(),
                "seed {seed:#x}: the projector accepted a layout coverage called uncovered: {:?}",
                coverage.unmet
            ),
            Some(ProjectorVerdict::Failed(ref detail)) if detail.contains("MissingTile") => {
                assert!(
                    !coverage.unmet.is_empty(),
                    "seed {seed:#x}: the projector reported MissingTile but coverage found none"
                );
            }
            _ => {}
        }
    }
}

/// A real regression gate for the whole repository, not just this tool: the
/// committed corpus must be able to build every preset layout.
#[test]
fn the_committed_corpus_covers_every_preset_layout() {
    let Ok((cells, rooms)) = crate::corpus() else {
        panic!("the committed corpus must load");
    };
    for &seed in &PRESET_SEEDS {
        let world = HexWfcWorld::generate(seed, HexWfcConfig::default()).expect("solves");
        let coverage = crate::coverage::CoverageReport::build(&world, cells, rooms, None, None);
        assert!(
            coverage.unmet.is_empty(),
            "seed {seed:#x} wants geometry the catalog does not have: {:?}",
            coverage
                .unmet
                .iter()
                .map(|row| (row.archetype, row.register.as_str(), row.cells))
                .collect::<Vec<_>>()
        );
    }
}

/// An empty catalog must be reported as uncovered rather than as "no demands".
/// Silence on a missing corpus is how a coverage panel becomes reassuring noise.
#[test]
fn an_empty_catalog_reports_every_demand_as_missing() {
    let world = HexWfcWorld::generate(PRESET_SEEDS[0], HexWfcConfig::default()).expect("solves");
    let coverage = crate::coverage::CoverageReport::build(&world, &[], &[], None, None);
    assert!(
        !coverage.demanded.is_empty(),
        "the layout must demand something"
    );
    assert_eq!(
        coverage.demanded.len(),
        coverage.unmet.len(),
        "with no catalog, every demand is unmet"
    );
    assert!(coverage.headline().contains("UNCOVERED"));
}

/// The committed corpus ships exactly one *authored module* per room role.
///
/// `runtime_catalog` expands that one module across all ten registers, so the
/// prototype count is ten and looks like variety. It is not: every Decision
/// room in every district is the same room. Distinct source ids is the honest
/// measure, and this pins both numbers so the difference stays visible.
#[test]
fn room_roles_are_currently_single_module() {
    let Ok((_, rooms)) = crate::corpus() else {
        panic!("the committed corpus must load");
    };
    let world = HexWfcWorld::generate(PRESET_SEEDS[0], HexWfcConfig::default()).expect("solves");
    let coverage = crate::coverage::CoverageReport::build(&world, &[], rooms, None, None);

    assert!(
        !coverage.room_variety.is_empty(),
        "every room role must be reported"
    );
    let thin = coverage
        .room_variety
        .iter()
        .filter(|row| row.is_thin())
        .count();
    assert_eq!(
        thin,
        coverage.room_variety.len(),
        "every role is still backed by one authored module; if this fails somebody \
         added real variety - update the panel copy and bug_backlog #25"
    );
    // And the trap this metric exists to avoid: prototypes look plural.
    assert!(
        coverage
            .room_variety
            .iter()
            .any(|row| row.prototypes > row.modules),
        "register expansion should inflate the prototype count above the module count"
    );
}

/// A weight-zero prototype can never be selected, which is almost always a bug
/// in the authored weight rather than an intention.
#[test]
fn a_zero_weight_prototype_is_flagged_as_never_placed() {
    let Ok((cells, _)) = crate::corpus() else {
        panic!("the committed corpus must load");
    };
    let mut doctored = cells.clone();
    doctored[0].weight = 0;
    let expected = format!(
        "{}/{}/{}",
        doctored[0].key.archetype, doctored[0].key.register, doctored[0].key.variant
    );

    let world = HexWfcWorld::generate(PRESET_SEEDS[0], HexWfcConfig::default()).expect("solves");
    let coverage = crate::coverage::CoverageReport::build(&world, &doctored, &[], None, None);
    assert!(
        coverage.never_placed.zero_weight.contains(&expected),
        "a weight-0 prototype must be named: {:?}",
        coverage.never_placed.zero_weight
    );
}
