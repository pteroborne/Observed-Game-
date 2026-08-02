//! Gates for `composition_studio`.
//!
//! Two kinds live here. The behaviour tests drive real state transitions and
//! assert an observable consequence — a test that passes because the thing it
//! watches never moves is worse than no test, because it reports a feature as
//! covered. The ratchets walk the source tree rather than a hand-written file
//! list, so a new module cannot escape them by being new.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use observed_facility::hex_wfc::profile::{
    ArchetypeBias, CompositionTendencies, HexCompositionProfile, ScoreWeights,
};
use observed_facility::hex_wfc::{HexArchetype, HexWfcConfig, HexWfcWorld};

use crate::draw::StudioVisual;
use crate::tunables::TUNABLE_FIELDS;
use crate::{LabMenuState, Layer, PRESET_SEEDS, ProfileOrigin, StudioPlugin, StudioState};

pub fn headless() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(StudioPlugin);
    app
}

/// Step past the solve debounce so a queued edit actually lands.
fn settle(app: &mut App) {
    for _ in 0..4 {
        app.update();
        let mut state = app.world_mut().resource_mut::<StudioState>();
        state.last_edit = None;
    }
    app.update();
}

// ---------------------------------------------------------------- solver seam

#[test]
fn baseline_studio_solve_matches_generate() {
    let config = HexWfcConfig::default();
    let profile = HexCompositionProfile::baseline();
    for &seed in &PRESET_SEEDS {
        let (traced, _) =
            HexWfcWorld::generate_traced_with_profile(seed, config, &profile).expect("traced");
        let direct = HexWfcWorld::generate(seed, config).expect("direct");
        assert_eq!(traced.placements, direct.placements, "seed {seed:#x}");
        assert_eq!(traced.architecture, direct.architecture, "seed {seed:#x}");
    }
}

/// The tool's whole purpose: an edit must reach the facility. If this passes
/// while the viewport looks unchanged, the drawer is at fault, not the solver.
#[test]
fn an_authored_bias_changes_the_solved_layout() {
    let mut state = StudioState::default();
    crate::solve::run_solve(&mut state);
    let baseline = state
        .solved
        .as_ref()
        .expect("baseline solves")
        .world
        .placements
        .clone();

    state.profile.archetype_bias = state
        .profile
        .archetype_bias
        .with(HexArchetype::Junction, 4.0)
        .with(HexArchetype::Corner, 0.25);
    assert_eq!(state.profile.validate(), Ok(()));
    crate::solve::run_solve(&mut state);

    let steered = &state
        .solved
        .as_ref()
        .expect("authored solves")
        .world
        .placements;
    assert_ne!(&baseline, steered, "an authored bias must move the layout");
}

#[test]
fn a_failed_solve_keeps_the_previous_layout_on_screen() {
    let mut state = StudioState::default();
    crate::solve::run_solve(&mut state);
    let kept = state
        .solved
        .as_ref()
        .expect("first solve")
        .world
        .placements
        .clone();

    state.config.cols = 1; // below the solver's minimum
    crate::solve::run_solve(&mut state);

    let still = &state
        .solved
        .as_ref()
        .expect("layout is kept")
        .world
        .placements;
    assert_eq!(&kept, still, "a failed solve must not blank the viewport");
    assert!(state.status.contains("FAILED"), "{}", state.status);
}

/// A changed seed must drop the cached baseline, or the compare overlay and the
/// score delta would describe a different facility than the one on screen.
#[test]
fn changing_the_seed_invalidates_the_cached_baseline() {
    let mut state = StudioState::default();
    crate::solve::run_solve(&mut state);
    assert!(state.baseline_world.is_some());

    state.seed_index += 1;
    state.invalidate_baseline();
    assert!(state.baseline_world.is_none());
    assert!(state.baseline_score.is_none());

    crate::solve::run_solve(&mut state);
    let rebuilt = state.baseline_world.as_ref().expect("baseline re-solved");
    let shown = &state.solved.as_ref().expect("solved").world;
    assert_eq!(
        rebuilt.seed, shown.seed,
        "the cached baseline must describe the seed on screen"
    );
}

// ------------------------------------------------------------------- lifecycle

/// The house reset rule. Asserts the drawer produced something *and* that a
/// rebuild does not accumulate — a count-equality check that never verifies the
/// count is non-zero passes just as happily on a drawer that draws nothing.
#[test]
fn reset_rebuilds_the_projection_without_leaking_entities() {
    let mut app = headless();
    settle(&mut app);

    let count = |app: &mut App| {
        app.world_mut()
            .query_filtered::<Entity, With<StudioVisual>>()
            .iter(app.world_mut())
            .count()
    };

    let first = count(&mut app);
    assert!(
        first > 0,
        "the drawer must emit visuals, or this test proves nothing"
    );

    for _ in 0..3 {
        app.world_mut().resource_mut::<StudioState>().geometry_dirty = true;
        app.update();
    }

    assert_eq!(
        first,
        count(&mut app),
        "repeated rebuilds must not accumulate entities"
    );
}

/// A reload re-reads the profile and re-solves, but must not throw away where
/// the author was looking.
#[test]
fn reloading_keeps_the_view_you_were_using() {
    let mut state = StudioState {
        zoom: 0.55,
        pan: Vec2::new(12.0, -8.0),
        layer: Layer::Single(0),
        ..StudioState::default()
    };

    state.reload(0.0);

    assert!((state.zoom - 0.55).abs() < f32::EPSILON);
    assert_eq!(state.pan, Vec2::new(12.0, -8.0));
    assert_eq!(state.layer, Layer::Single(0));
    assert_eq!(state.reset_count, 1);
    assert!(state.solve_dirty, "a reload must re-solve");
}

/// "A key never means two things at once" — now enforced by **ownership**
/// rather than by freezing the world.
///
/// The panel used to be modal, which made the tool's core loop impossible: you
/// cannot watch a facility answer an edit while the edit screen has stopped it.
/// So both regions stay live and keys go to whichever you last clicked.
///
/// Pinned by driving the *same* key under both owners and requiring the
/// outcomes to differ. Asserting only that nothing happened under one owner
/// would pass just as well if the key were unbound entirely.
#[test]
fn the_keyboard_owner_decides_where_a_key_lands() {
    let press_tab = |owner: crate::KeyboardOwner| {
        let mut app = headless();
        {
            let mut state = app.world_mut().resource_mut::<StudioState>();
            state.keyboard_owner = owner;
            state.layer = Layer::All;
        }
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Tab);
        app.update();
        app.world().resource::<StudioState>().layer
    };

    assert_eq!(
        press_tab(crate::KeyboardOwner::Panel),
        Layer::All,
        "Tab must not reach the facility while the panel owns the keyboard"
    );
    assert_ne!(
        press_tab(crate::KeyboardOwner::Viewport),
        Layer::All,
        "Tab must cycle the layer once the facility owns the keyboard"
    );
}

/// Docking means the facility is drawn *beside* the panel, so a cursor position
/// has to be rebased before it can be compared against projected cell centres.
/// Getting this wrong offsets every pick by the panel width, which reads as
/// "clicking selects the wrong cell" rather than as a coordinate-space bug.
#[test]
fn cursor_positions_are_rebased_into_the_inset_viewport() {
    let open = StudioState::default();
    assert!(open.panel_open);
    assert_eq!(open.viewport_origin(), crate::PANEL_WIDTH);
    assert!(!open.cursor_in_viewport(Vec2::new(10.0, 300.0)));
    assert!(open.cursor_in_viewport(Vec2::new(crate::PANEL_WIDTH + 5.0, 300.0)));
    assert_eq!(
        open.cursor_to_viewport(Vec2::new(crate::PANEL_WIDTH + 40.0, 300.0)),
        Vec2::new(40.0, 300.0)
    );

    // Collapsed, the facility owns the whole window and no rebasing applies.
    let collapsed = StudioState {
        panel_open: false,
        ..StudioState::default()
    };
    assert_eq!(collapsed.viewport_origin(), 0.0);
    assert!(collapsed.cursor_in_viewport(Vec2::new(10.0, 300.0)));
    assert_eq!(
        collapsed.cursor_to_viewport(Vec2::new(40.0, 300.0)),
        Vec2::new(40.0, 300.0)
    );
}

/// Promotion is the one irreversible action, so it must never fire directly off
/// a keystroke — it has to raise a confirmation first.
#[test]
fn promoting_requires_a_confirmation() {
    let mut app = headless();
    app.update();
    assert!(app.world().resource::<LabMenuState>().confirm.is_none());

    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::ControlLeft);
        keys.press(KeyCode::ShiftLeft);
        keys.press(KeyCode::KeyS);
    }
    app.update();

    let confirm = app
        .world()
        .resource::<LabMenuState>()
        .confirm
        .clone()
        .expect("Ctrl+Shift+S must raise a confirmation, not write the corpus");
    assert!(
        confirm.prompt.contains("simulation hash"),
        "the prompt must name the hash it changes: {}",
        confirm.prompt
    );
}

/// Overwriting a corpus profile the tool could not parse would destroy authored
/// work in order to paper over a loading bug.
#[test]
fn promotion_is_refused_when_the_corpus_profile_was_unreadable() {
    let mut state = StudioState {
        origin: ProfileOrigin::Unreadable(String::from("synthetic")),
        ..StudioState::default()
    };
    let error = crate::persist::promote_to_corpus(&mut state)
        .expect_err("promotion must be refused after a failed load");
    assert!(error.contains("refusing to promote"), "{error}");
}

// ------------------------------------------------------------------- tunables

#[test]
fn every_tunable_field_round_trips() {
    for field in TUNABLE_FIELDS {
        let mut profile = HexCompositionProfile::baseline();
        for value in [field.min, field.max] {
            (field.set)(&mut profile, value);
            assert!(
                ((field.get)(&profile) - value).abs() < 1e-9,
                "{} did not round-trip {value}",
                field.label
            );
            assert_eq!(profile.validate(), Ok(()), "{} @ {value}", field.label);
        }
    }
}

/// Every control must say what it does. The complaint this answers is "I don't
/// know what it would do if I adjust the values", and a placeholder would
/// satisfy the type system while leaving the author exactly where they were.
#[test]
fn every_tunable_field_explains_its_consequence() {
    for field in TUNABLE_FIELDS {
        let text = field.consequence;
        assert!(
            text.len() >= 40,
            "{} has a stub consequence: {text:?}",
            field.label
        );
        assert!(
            text.ends_with('.'),
            "{} reads as a fragment, not a sentence: {text:?}",
            field.label
        );
        // The label is the control's name; repeating it back teaches nothing.
        assert!(
            !text.eq_ignore_ascii_case(field.label),
            "{} just restates its own name",
            field.label
        );
        assert!(text.is_ascii(), "{} is not ASCII: {text:?}", field.label);
    }

    // Distinct text per field: copy-pasting one line across a category would
    // pass every check above while telling an author nothing about the control
    // in front of them.
    let mut seen: Vec<&str> = TUNABLE_FIELDS.iter().map(|f| f.consequence).collect();
    seen.sort_unstable();
    let total = seen.len();
    seen.dedup();
    assert_eq!(total, seen.len(), "two tunables share a consequence string");
}

#[test]
fn every_profile_scalar_has_a_tunable_entry() {
    let expected = CompositionTendencies::baseline().fields().len()
        + ArchetypeBias::neutral().fields().len()
        + ScoreWeights::baseline().fields().len();
    assert_eq!(
        TUNABLE_FIELDS.len(),
        expected,
        "a new profile scalar must gain a TunableField, or it ships with no UI"
    );
}

/// Invariant 1, from the tool's side: no reachable slider position can starve a
/// legal variant. The floor is deliberately `PROFILE_MIN`, not zero — a genuine
/// prohibition is a pin, which is checked, not a weight, which is not.
#[test]
fn no_slider_position_can_zero_a_legal_variant() {
    use observed_facility::hex_wfc::PROFILE_MIN;
    for field in TUNABLE_FIELDS {
        // Score weights are post-hoc: they rank already-legal layouts, so zero
        // there disables a criterion rather than starving the lottery.
        if field.category == "Score Weights" {
            assert!(field.min >= 0.0, "{} may not go negative", field.label);
            continue;
        }
        assert!(
            field.min >= PROFILE_MIN,
            "{} can be dragged to {}, below the solvability floor {PROFILE_MIN}",
            field.label,
            field.min
        );
    }
}

// -------------------------------------------------------------------- persist

#[test]
fn saving_writes_a_loadable_profile_and_sidecar() {
    let dir = std::env::temp_dir().join("composition_studio_save_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");

    let mut profile = HexCompositionProfile::baseline();
    profile.archetype_bias = profile.archetype_bias.with(HexArchetype::Shaft, 1.8);
    let build =
        observed_authoring::composition::CompositionBuild::new(profile.clone()).expect("hashes");
    observed_authoring::composition::write_profile_build(&build, &dir).expect("writes");

    let loaded = observed_authoring::composition::load_profile(&dir).expect("loads");
    assert_eq!(loaded.profile, profile);
    std::fs::remove_dir_all(&dir).ok();
}

/// There is no honest placeholder for a content hash. A zeroed one folds into a
/// plausible-looking wrong answer, which is worse than admitting ignorance.
#[test]
fn an_unreadable_catalog_reports_unavailable_rather_than_a_fabricated_hash() {
    let state = StudioState {
        catalog_hash: crate::CatalogHash::Unavailable(String::from("synthetic")),
        ..StudioState::default()
    };
    let shown = crate::persist::simulation_hash(&state, &state.profile);
    assert_eq!(shown, "unavailable");
}

#[test]
fn the_simulation_hash_moves_when_the_profile_does() {
    let state = StudioState {
        catalog_hash: crate::CatalogHash::Known("a".repeat(64)),
        ..StudioState::default()
    };
    let before = crate::persist::simulation_hash(&state, &state.profile);

    let mut edited = state.profile.clone();
    edited.archetype_bias = edited.archetype_bias.with(HexArchetype::Shaft, 2.0);
    let after = crate::persist::simulation_hash(&state, &edited);

    assert_ne!(before, after, "an edit must be visible in the status bar");
}

// -------------------------------------------------------------------- ratchets

/// Every `.rs` under `src/`, discovered rather than listed — a hand-written file
/// list silently exempts whatever is added next.
fn studio_sources() -> Vec<(PathBuf, String)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
        let entries = std::fs::read_dir(dir).expect("studio src is readable");
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).expect("source is readable");
                out.push((path, text));
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    walk(&root, &mut out);
    assert!(out.len() >= 12, "source discovery found only {}", out.len());
    out
}

/// The tool ships no font asset, so Bevy's default ASCII subset is all it can
/// draw. A non-ASCII glyph renders as a blank box; this repo has lost one that
/// way before.
#[test]
fn rendered_studio_strings_stay_ascii() {
    for (path, text) in studio_sources() {
        for (index, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") || line.is_ascii() {
                continue;
            }
            panic!(
                "non-ASCII in a rendered string at {}:{}",
                path.display(),
                index + 1
            );
        }
    }
}

/// The Legibility Contract as a ratchet: colour comes from `observed_style`, so
/// presentation code never invents one.
#[test]
fn no_locally_invented_colours() {
    const BANNED: [&str; 5] = [
        "Color::srgb",
        "Color::rgb",
        "Color::hsl",
        "Srgba::new",
        "LinearRgba::rgb",
    ];
    for (path, text) in studio_sources() {
        // This file names the banned patterns in order to look for them.
        if path.ends_with("tests.rs") {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for needle in BANNED {
                assert!(
                    !line.contains(needle),
                    "{} invents a colour at {}:{}; ask observed_style instead",
                    needle,
                    path.display(),
                    index + 1
                );
            }
        }
    }
}

/// The studio is a tool, not a lab, but the review ratchet the rest of the WFC
/// path lives under is a good norm and `collapse.rs` at ~1000 lines is why.
#[test]
fn studio_files_stay_under_six_hundred_lines() {
    for (path, text) in studio_sources() {
        let lines = text.lines().count();
        assert!(
            lines <= 600,
            "{} is {lines} lines; split it before extending",
            path.display()
        );
    }
}
