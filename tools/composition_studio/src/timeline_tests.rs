//! Gates for the solve replay.
//!
//! The load-bearing claim is that this is a *view*: a studio with the replay
//! settled must behave exactly as it did before the replay existed, because
//! that is the state it is in for all but a few seconds of any session. The
//! rest guards the two things a replay can silently get wrong — drawing cells
//! the solver has not reached, and reporting numbers it did not compute.

use observed_facility::hex_wfc::HexArchetype;

use crate::timeline::{self, Timeline};
use crate::{StudioState, StudioTab};

fn solved_state() -> StudioState {
    let mut state = StudioState::default();
    crate::solve::run_solve(&mut state);
    assert!(state.solved.is_some(), "the compact fixture solves");
    state
}

#[test]
fn a_fresh_solve_lands_settled_on_its_finished_world() {
    let state = solved_state();
    let steps = timeline::trace_len(&state);
    assert!(steps > 0, "the studio asks for a traced solve");
    assert!(
        state.timeline.settled(steps),
        "tuning a value is a request to see the result, not to watch it built"
    );
    assert!(!state.timeline.playing);
    assert!(
        !timeline::replaying(&state),
        "a settled studio is the studio that existed before the replay did"
    );
    assert!(
        timeline::replay_cells(&state).is_none(),
        "and the draw path must not pay for a fold it does not need"
    );
}

#[test]
fn re_solving_settles_a_replay_that_was_part_way_through() {
    let mut state = solved_state();
    state.timeline.cursor = 5;
    assert!(timeline::replaying(&state));

    crate::solve::run_solve(&mut state);
    let steps = timeline::trace_len(&state);
    assert!(
        state.timeline.settled(steps),
        "a new solve must not leave the cursor pointing into the old trace"
    );
}

#[test]
fn playing_from_the_end_starts_again_rather_than_doing_nothing() {
    let mut state = solved_state();
    let steps = timeline::trace_len(&state);
    assert!(state.timeline.settled(steps));

    state.timeline.toggle(steps);
    assert_eq!(state.timeline.cursor, 0, "the end is where a replay starts");
    assert!(state.timeline.playing);

    // And from part-way through it is an ordinary pause.
    state.timeline.cursor = steps / 2;
    state.timeline.toggle(steps);
    assert!(!state.timeline.playing);
    assert_eq!(state.timeline.cursor, steps / 2, "pausing does not move");
}

#[test]
fn stepping_pauses_and_stays_inside_the_trace() {
    let mut state = solved_state();
    let steps = timeline::trace_len(&state);

    state.timeline.cursor = 0;
    state.timeline.playing = true;
    state.timeline.step(steps, 1);
    assert!(
        !state.timeline.playing,
        "a cursor that ran out from under the key"
    );
    assert_eq!(state.timeline.cursor, 1);

    state.timeline.step(steps, -50);
    assert_eq!(
        state.timeline.cursor, 0,
        "stepping back off the start clamps"
    );

    state.timeline.step(steps, isize::MAX);
    assert_eq!(
        state.timeline.cursor, steps,
        "and forward off the end settles"
    );
}

#[test]
fn a_replay_only_draws_cells_the_solver_has_observed() {
    let mut state = solved_state();
    let steps = timeline::trace_len(&state);
    let placed = state
        .solved
        .as_ref()
        .expect("solved")
        .world
        .placements
        .len();

    // Both cursors sit inside the **last** attempt, and that is not a detail.
    // A retry throws the lattice away and the fold clears with it, so a cursor
    // before a restart and one after it are counting two different solves; a
    // contradiction does the same thing on a smaller scale, un-resolving the
    // cell it lands on. Neither is the replay drawing something it has not
    // observed, which is what this test is named for.
    //
    // It held on a single cursor pair for as long as every fixture solved first
    // time. Routing does not always, and the third and two-thirds marks landed
    // either side of an `AttemptStart`.
    let last_attempt = state
        .solved
        .as_ref()
        .expect("solved")
        .steps
        .iter()
        .rposition(|step| {
            matches!(
                step,
                observed_facility::hex_wfc::SolveStep::AttemptStart { .. }
            )
        })
        .unwrap_or(0);
    let span = steps - last_attempt;

    state.timeline.cursor = last_attempt + span / 3;
    let cells = timeline::replay_cells(&state).expect("part-way through is a replay");
    let resolved = cells.values().filter(|c| c.resolved.is_some()).count();
    assert!(
        resolved < placed,
        "a third of the way in, {resolved} of {placed} cells must not all be settled"
    );

    // And the count only grows.
    state.timeline.cursor = last_attempt + (span * 2) / 3;
    let later = timeline::replay_cells(&state).expect("still a replay");
    let later_resolved = later.values().filter(|c| c.resolved.is_some()).count();
    assert!(later_resolved >= resolved);
}

#[test]
fn the_readout_counts_up_and_ends_on_the_solve_it_reports() {
    let mut state = solved_state();
    let steps = timeline::trace_len(&state);
    let placed = state
        .solved
        .as_ref()
        .expect("solved")
        .world
        .placements
        .len();

    state.timeline.cursor = steps / 4;
    let early = timeline::summary(&state).expect("a solve has a summary");
    state.timeline.to_end(steps);
    let settled = timeline::summary(&state).expect("a solve has a summary");

    assert!(early.collapsed < settled.collapsed);
    assert_eq!(
        settled.collapsed as usize, placed,
        "the settled readout must agree with the world it is describing"
    );
    assert_eq!(settled.open, 0);
}

#[test]
fn the_solver_state_panel_names_the_seed_it_solved() {
    let state = solved_state();
    let panel = timeline::format_solver_state(&state);
    assert!(panel.contains(&format!("{:#018x}", state.seed())));
    assert!(panel.contains("collapsed"));
    assert!(panel.contains("attempt"));
    // The scrub bar is drawn full when settled.
    assert!(panel.contains("[===="), "settled reads as a full bar");
}

#[test]
fn a_cell_report_says_which_domain_number_it_is() {
    // The neighbourhood explorer's number and this one are both "how many
    // things could be here" and they are not the same quantity. A reader who
    // confused them would tune against variety that cannot occur, so the label
    // is part of the contract rather than a nicety.
    let state = solved_state();
    let coord = *state
        .solved
        .as_ref()
        .expect("solved")
        .world
        .placements
        .keys()
        .next()
        .expect("the fixture places cells");
    let described = timeline::describe_cell(&state, coord).expect("a traced cell reports");
    assert!(
        described.contains("domain at collapse") || described.contains("domain never narrowed"),
        "unlabelled domain number: {described}"
    );
}

#[test]
fn the_solve_tab_shows_the_solver_state() {
    // Through the real chrome, not the formatter alone: a panel that is correct
    // and never rendered is not a feature.
    let mut app = crate::tests::headless();
    {
        let mut menu = app.world_mut().resource_mut::<crate::LabMenuState>();
        menu.set_tab(StudioTab::Solve);
    }
    for _ in 0..6 {
        app.update();
        app.world_mut().resource_mut::<StudioState>().last_edit = None;
    }
    app.update();

    let state = app.world().resource::<StudioState>();
    assert!(state.solved.is_some(), "the studio solves on startup");
    assert!(timeline::format_solver_state(state).contains("SOLVER STATE"));
}

#[test]
fn speed_stays_inside_its_bounds() {
    let mut timeline = Timeline::default();
    for _ in 0..20 {
        timeline.change_speed(true);
    }
    assert!(timeline.steps_per_tick <= 128);
    for _ in 0..20 {
        timeline.change_speed(false);
    }
    assert_eq!(
        timeline.steps_per_tick, 1,
        "a replay that can reach zero steps per frame is a replay that hangs"
    );
}

// ------------------------------------------------------------- seeds & history

#[test]
fn the_brackets_walk_the_pinned_presets_and_wrap() {
    let mut state = StudioState::default();
    assert_eq!(state.seed_preset, Some(0));
    assert_eq!(state.seed(), crate::PRESET_SEEDS[0]);

    state.step_preset_seed(1, 0.0);
    assert_eq!(state.seed(), crate::PRESET_SEEDS[1]);

    // Backwards off the start wraps to the end rather than sticking at zero,
    // which is what the old saturating index did.
    state.step_preset_seed(-1, 0.0);
    state.step_preset_seed(-1, 0.0);
    assert_eq!(state.seed(), *crate::PRESET_SEEDS.last().expect("presets"));
}

#[test]
fn a_rolled_seed_leaves_the_pinned_set_alone() {
    // `PRESET_SEEDS` is an evidence contract shared with `iso_observer_lab`.
    // Free seeds are additive: they must not renumber or replace it.
    let before = crate::PRESET_SEEDS;
    let mut state = StudioState::default();
    state.roll_seed(0.0);

    assert_eq!(before, crate::PRESET_SEEDS);
    assert_eq!(state.seed_preset, None);
    assert!(state.seed_label().contains("free"));
    assert!(
        state.baseline_world.is_none(),
        "a different seed is a different facility"
    );

    // And a preset is still one bracket away.
    state.step_preset_seed(1, 0.0);
    assert_eq!(state.seed(), crate::PRESET_SEEDS[0]);
}

#[test]
fn a_seed_is_not_part_of_the_profile_hash() {
    // Rolling a seed must not lock out a LAN peer. Seeds are not authored
    // content; the profile is.
    let mut state = StudioState::default();
    let before = crate::persist::simulation_hash(&state, &state.profile.clone());
    state.roll_seed(0.0);
    let after = crate::persist::simulation_hash(&state, &state.profile.clone());
    assert_eq!(before, after);
}

#[test]
fn undo_steps_back_one_edit_rather_than_to_the_last_save() {
    let mut state = StudioState::default();
    let original = state.profile.clone();

    // Two edits, separated past the coalescing window.
    state.profile.archetype_bias = state
        .profile
        .archetype_bias
        .with(HexArchetype::Junction, 3.0);
    state.touch_profile(0.0);
    let after_first = state.profile.clone();

    state.profile.archetype_bias = state.profile.archetype_bias.with(HexArchetype::Corner, 0.5);
    state.touch_profile(10.0);
    assert_eq!(state.undo_stack.len(), 2);

    assert!(state.undo(20.0));
    assert_eq!(state.profile, after_first, "one edit back, not all of them");
    assert!(state.undo(30.0));
    assert_eq!(state.profile, original);
    assert!(!state.undo(40.0), "and it stops at the beginning");
}

#[test]
fn redo_retraces_an_undo_and_a_fresh_edit_forgets_it() {
    let mut state = StudioState::default();
    state.profile.archetype_bias = state
        .profile
        .archetype_bias
        .with(HexArchetype::Junction, 3.0);
    state.touch_profile(0.0);
    let edited = state.profile.clone();

    assert!(state.undo(10.0));
    assert!(state.redo(20.0));
    assert_eq!(state.profile, edited);

    // A new edit after an undo makes the abandoned branch unreachable, which is
    // what every editor does and what the redo stack must reflect.
    assert!(state.undo(30.0));
    state.profile.archetype_bias = state.profile.archetype_bias.with(HexArchetype::Shaft, 2.0);
    state.touch_profile(40.0);
    assert!(state.redo_stack.is_empty());
}

#[test]
fn one_gesture_is_one_undo() {
    // Painting a run of pins, or sweeping a slider, is one edit to the person
    // doing it. Coalescing rides the same window the solver debounces on.
    let mut state = StudioState::default();
    for step in 0..8u8 {
        state.profile.archetype_bias = state
            .profile
            .archetype_bias
            .with(HexArchetype::Junction, 1.0 + f64::from(step) * 0.1);
        // All well inside SOLVE_DEBOUNCE_SECONDS of each other.
        state.touch_profile(f32::from(step) * 0.02);
    }
    assert_eq!(
        state.undo_stack.len(),
        1,
        "a swept slider must not cost eight undo steps"
    );
}

#[test]
fn a_re_solve_that_changes_nothing_records_no_edit() {
    // `R` re-solves without editing. A history that filled up with identical
    // entries would push real edits off the end of the bounded stack.
    let mut state = StudioState::default();
    for tick in 0..10u8 {
        state.touch_profile(f32::from(tick));
    }
    assert!(state.undo_stack.is_empty());
}

#[test]
fn the_history_is_bounded() {
    let mut state = StudioState::default();
    for step in 0..u32::try_from(crate::state::UNDO_DEPTH + 20).expect("small") {
        state.profile.archetype_bias = state
            .profile
            .archetype_bias
            .with(HexArchetype::Shaft, 1.0 + f64::from(step) * 0.01);
        // Past the coalescing window each time, so each is its own entry.
        state.touch_profile(f64::from(step) as f32);
    }
    assert_eq!(state.undo_stack.len(), crate::state::UNDO_DEPTH);
}
