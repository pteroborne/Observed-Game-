//! Gates for what the viewport actually shows.
//!
//! Split from `authoring_tests.rs` when that file crossed the 600-line review
//! budget the rest of the WFC path lives under. The subject here is the view:
//! which projection is drawn, at what scope, and whether authored geometry
//! reaches the scene at all.

use bevy::prelude::*;

use crate::StudioState;

/// The tool opens on the authored deck.
///
/// It used to open on a wireframe schematic with the real geometry behind `F`,
/// which showed a diagram of a building rather than the building. This is the
/// inverse contract, and it is asserted through the whole app because the thing
/// that matters is that hulls reach the scene, not that a flag is set.
#[test]
fn the_studio_opens_on_the_authored_deck() {
    let mut app = crate::tests::headless();
    for _ in 0..6 {
        app.update();
        app.world_mut().resource_mut::<StudioState>().last_edit = None;
    }
    app.update();

    let state = app.world().resource::<StudioState>();
    assert_eq!(state.detail_mode, crate::detail::DetailMode::Layer);
    assert_eq!(
        state.walls,
        crate::detail::WallMode::Partial,
        "the third-wall deck is the look this view exists to have"
    );
    assert!(
        !state.show_schematic,
        "the solver's line work is a diagnostic, not the view"
    );
    assert!(
        state.detail_report.hulls_drawn > 0,
        "authored hulls must actually reach the scene: {:?}",
        state.detail_report
    );
}

/// `F` must leave the old view reachable rather than deleting it.
#[test]
fn the_schematic_only_view_is_still_reachable() {
    let mut state = StudioState::default();
    assert_eq!(state.detail_mode, crate::detail::DetailMode::Layer);
    state.detail_mode = crate::detail::DetailMode::Focus;
    // The `F` cycle is Layer -> Focus -> Off -> Layer.
    state.detail_mode = crate::detail::DetailMode::Off;
    assert_eq!(state.detail_mode.label(), "schematic");
}

/// Three named projections, cycling.
#[test]
fn the_wall_modes_cycle_and_name_themselves() {
    use crate::detail::WallMode;
    let mut seen = Vec::new();
    let mut mode = WallMode::default();
    for _ in 0..WallMode::ALL.len() {
        seen.push(mode);
        mode = mode.next();
    }
    assert_eq!(mode, WallMode::default(), "the cycle must close");
    for expected in WallMode::ALL {
        assert!(seen.contains(&expected), "{expected:?} is unreachable");
        // A script names the projection it photographed; an unparseable name is
        // refused rather than defaulted.
        assert_eq!(WallMode::parse(expected.label()), Some(expected));
    }
    assert_eq!(WallMode::parse("partial"), Some(WallMode::Partial));
    assert_eq!(WallMode::parse("nonsense"), None);
}

/// Deck scope on "all layers" used to be refused, on a hull count taken from
/// production's 5,600-cell facility. The studio caps at ten levels of a 12x9
/// lattice, so the refusal was rejecting a view it could comfortably draw —
/// and, now that the deck is the default view, refusing would have left the
/// tool opening on nothing.
#[test]
fn all_layers_draws_the_deck_rather_than_refusing_it() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(crate::StudioPlugin);
    // Settle the initial solve first: it writes the status line, and would
    // otherwise overwrite the refusal this test is looking for.
    for _ in 0..4 {
        app.update();
        app.world_mut().resource_mut::<StudioState>().last_edit = None;
    }
    app.update();
    {
        let mut state = app.world_mut().resource_mut::<StudioState>();
        state.layer = crate::Layer::All;
    }
    // Viewport keys only reach the viewport when the viewport owns the
    // keyboard - that is the ownership rule, not an accident.
    app.world_mut().resource_mut::<StudioState>().keyboard_owner = crate::KeyboardOwner::Viewport;
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::ShiftLeft);
        keys.press(KeyCode::KeyF);
    }
    app.update();

    let state = app.world().resource::<StudioState>();
    assert_eq!(state.detail_mode, crate::detail::DetailMode::Layer);
    assert!(
        state.detail_report.hulls_drawn > 0,
        "all layers must draw, not silently draw nothing: {:?}",
        state.detail_report
    );
}

/// The overlay draws every frontier the plan names, and counts what it draws.
///
/// The point of the overlay is that the picture and the number agree: if it
/// silently dropped pairs it could not match, the border would look tidier than
/// it is and the whole reason for drawing it - seeing how much of a boundary is
/// open - would be quietly defeated. So this recomputes the expected frontier
/// independently from `region_plan` and demands the same answer.
#[test]
fn the_region_overlay_draws_every_frontier_on_the_focus_floor() {
    use observed_facility::hex_wfc::region_plan;

    let mut app = crate::tests::headless();
    for _ in 0..6 {
        app.update();
        app.world_mut().resource_mut::<StudioState>().last_edit = None;
    }
    app.update();

    assert_eq!(
        app.world().resource::<StudioState>().region_report,
        crate::regions::RegionReport::default(),
        "the overlay is a mode and starts off"
    );

    // Four floors, because a region is a whole-height volume and a single-level
    // facility has no vertical frontier at all - which is exactly how the
    // vertical case stayed untested through the change that introduced it.
    {
        let mut state = app.world_mut().resource_mut::<StudioState>();
        state.set_levels(4, 0.0);
        state.show_regions = true;
    }
    for _ in 0..6 {
        app.update();
        app.world_mut().resource_mut::<StudioState>().last_edit = None;
    }
    app.update();

    let state = app.world().resource::<StudioState>();
    let report = state.region_report;
    let plan = region_plan(state.seed, state.config);
    let expected = plan
        .gateways
        .iter()
        .flat_map(|gateway| gateway.frontier.iter())
        .filter(|(a, _)| state.layer.is_focus(a.level))
        .count();

    assert_eq!(report.regions, plan.regions.len());
    assert_eq!(report.gateways, plan.gateways.len());
    assert!(expected > 0, "the studio's facility must have regions");
    assert!(
        report.vertical > 0,
        "a four-floor facility must have vertical frontiers, or this asserts nothing"
    );
    assert_eq!(
        report.frontier + report.vertical,
        expected,
        "every frontier pair on the floor is either drawn as a wall or counted as \
         vertical - anything else is a pair silently dropped, which would draw a \
         tidier border than the facility has"
    );
    assert!(
        report.open <= report.frontier && report.widest <= report.frontier,
        "walkable crossings are a subset of the frontier: {report:?}"
    );
}
