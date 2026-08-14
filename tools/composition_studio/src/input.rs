//! Keyboard handling: who owns input, and what each key does.
//!
//! Split from `chrome.rs` so neither file outgrows the 600-line review budget
//! the rest of the WFC path lives under.
//!
//! The one rule everything here serves: **exactly one thing owns the keyboard
//! at a time**. A pending confirmation outranks the menu, the menu outranks the
//! viewport, so a key never means two things at once.

use bevy::prelude::*;

use crate::chrome::{LabMenuState, PendingConfirm};
use crate::persist::{promote_to_corpus, save_working_profile};
use crate::tunables::{TUNABLE_FIELDS, TunableField};
fn request_save(state: &mut crate::StudioState, menu: &mut LabMenuState, promote: bool) {
    if promote {
        menu.confirm = Some(PendingConfirm {
            prompt: crate::persist::promotion_summary(state),
        });
        return;
    }
    state.status = match save_working_profile(state) {
        Ok(()) => format!(
            "saved to {} (not shipped; Ctrl+Shift+S promotes)",
            crate::persist::working_dir().display()
        ),
        Err(detail) => format!("ERROR: {detail}"),
    };
}

/// Handle keyboard navigation and hotkey gating.
pub fn handle_chrome_input(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu_state: ResMut<LabMenuState>,
    mut state: ResMut<crate::StudioState>,
) {
    let now = time.elapsed_secs();
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    // A pending confirmation owns the keyboard outright: nothing else may fire
    // while an irreversible action is on screen awaiting an answer.
    if menu_state.confirm.is_some() {
        if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::NumpadEnter) {
            menu_state.confirm = None;
            state.status = match promote_to_corpus(&mut state) {
                Ok(()) => format!(
                    "promoted to corpus; simulation hash is now {}",
                    crate::persist::simulation_hash(&state, &state.profile.clone())
                ),
                Err(detail) => format!("ERROR: {detail}"),
            };
        } else if keyboard.just_pressed(KeyCode::Escape) {
            menu_state.confirm = None;
            state.status = String::from("promotion cancelled; corpus untouched");
        }
        return;
    }

    // F2 collapses the panel to give the facility the whole window. It no
    // longer changes what the keyboard can reach - that is ownership's job.
    if keyboard.just_pressed(KeyCode::F2) {
        state.panel_open = !state.panel_open;
        menu_state.is_open = state.panel_open;
        state.touch_view();
    }

    // Keys go to whichever region was last clicked. Both stay live, so you
    // can drag a value and watch the facility answer - which is the entire
    // reason the panel stopped being modal.
    if state.keyboard_owner == crate::KeyboardOwner::Panel {
        if keyboard.just_pressed(KeyCode::Tab) {
            if shift {
                menu_state.prev_tab();
            } else {
                menu_state.next_tab();
            }
        }

        // Driven by `field.tab`, not by a hardcoded tab. Any tab that declares
        // fields gets selection and adjustment for free, and a new tunable
        // cannot land somewhere the keyboard cannot reach it.
        let tab = menu_state.tab();
        {
            let fields_on_tab: Vec<(usize, &TunableField)> = TUNABLE_FIELDS
                .iter()
                .enumerate()
                .filter(|(_, f)| f.tab == tab)
                .collect();

            let count = fields_on_tab.len();

            if keyboard.just_pressed(KeyCode::ArrowUp) && menu_state.selected_item > 0 {
                menu_state.selected_item -= 1;
            }
            if keyboard.just_pressed(KeyCode::ArrowDown) && menu_state.selected_item + 1 < count {
                menu_state.selected_item += 1;
            }

            if count > 0 {
                let selected_idx = menu_state.selected_item % count;
                let (_, field) = fields_on_tab[selected_idx];
                let current_val = (field.get)(&state.profile);

                let delta = if keyboard.just_pressed(KeyCode::ArrowRight)
                    || keyboard.just_pressed(KeyCode::NumpadAdd)
                    || keyboard.just_pressed(KeyCode::Equal)
                {
                    field.step
                } else if keyboard.just_pressed(KeyCode::ArrowLeft)
                    || keyboard.just_pressed(KeyCode::NumpadSubtract)
                    || keyboard.just_pressed(KeyCode::Minus)
                {
                    -field.step
                } else {
                    0.0
                };

                if delta != 0.0 {
                    let new_val = (current_val + delta).clamp(field.min, field.max);
                    (field.set)(&mut state.profile, new_val);
                    state.touch_profile(now);
                }
            }
        }

        // Global shortcuts like save/promote work inside menu too
        if ctrl && keyboard.just_pressed(KeyCode::KeyS) {
            request_save(&mut state, &mut menu_state, shift);
        }
        return;
    }

    // --- MENU IS CLOSED: Viewport hotkeys are active ---

    // Changing which levels are drawn is a view change, not a re-solve.
    let levels = state.config.levels;
    if keyboard.just_pressed(KeyCode::Tab) {
        state.layer = if shift {
            state.layer.previous(levels)
        } else {
            state.layer.next(levels)
        };
        state.touch_view();
    }
    if keyboard.just_pressed(KeyCode::PageUp) {
        state.layer = state.layer.next(levels);
        state.touch_view();
    }
    if keyboard.just_pressed(KeyCode::PageDown) {
        state.layer = state.layer.previous(levels);
        state.touch_view();
    }

    // The brackets walk the five pinned evidence seeds; `M` leaves them for a
    // seed nobody chose. The presets are a contract shared with
    // `iso_observer_lab` and stay exactly as they were - they just stop being
    // the only facilities this tool can be pointed at.
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        state.step_preset_seed(-1, now);
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        state.step_preset_seed(1, now);
    }
    if keyboard.just_pressed(KeyCode::KeyM) {
        state.roll_seed(now);
    }

    if keyboard.just_pressed(KeyCode::KeyR) && !(ctrl && shift) {
        if ctrl {
            state.reload(now);
        } else {
            state.touch_profile(now);
        }
    }

    // The solver's topology, laid over the authored geometry. Off by default:
    // it answers "do these two agree", which is a question you ask, not the
    // view you work in.
    if keyboard.just_pressed(KeyCode::KeyG) {
        state.show_schematic = !state.show_schematic;
        state.touch_view();
    }

    // How many floors the working facility has. Not a view control: it changes
    // the facility, so it re-solves. It exists because two of a cell's eight
    // faces are up and down, and on a one-level lattice they cannot be asked
    // about at all.
    if keyboard.just_pressed(KeyCode::KeyL) {
        let levels = state.config.levels;
        let next = if shift {
            levels.saturating_sub(1)
        } else {
            levels.saturating_add(1)
        };
        state.set_levels(next, now);
    }

    if keyboard.just_pressed(KeyCode::Home) || keyboard.just_pressed(KeyCode::Digit0) {
        state.zoom = crate::viewport::DEFAULT_ZOOM;
        state.pan = Vec2::ZERO;
    }

    if ctrl && keyboard.just_pressed(KeyCode::KeyS) {
        request_save(&mut state, &mut menu_state, shift);
    }

    // One edit at a time. `Ctrl+Z` used to throw away everything since the last
    // save, which mid-way through painting a run of pins is the opposite of
    // what the key means everywhere else. Revert-to-saved is still here, under
    // its own name, next to the reload it resembles.
    if ctrl && keyboard.just_pressed(KeyCode::KeyZ) {
        let moved = if shift {
            state.redo(now)
        } else {
            state.undo(now)
        };
        if !moved {
            state.status = String::from(if shift {
                "nothing to redo"
            } else {
                "nothing to undo"
            });
        }
    }

    if ctrl && shift && keyboard.just_pressed(KeyCode::KeyR) {
        state.profile = state.saved.clone();
        state.status = String::from("reverted to the last saved profile");
        state.touch_profile(now);
    }

    if keyboard.just_pressed(KeyCode::KeyB) {
        state.show_baseline_compare = !state.show_baseline_compare;
        state.touch_view();
    }

    // --- detail rendering ---
    // Scope of the authored deck. `Layer` is the view; `Focus` isolates one
    // cell and what it connects to; `Off` is the schematic-only view this tool
    // used to open on, kept reachable rather than deleted.
    //
    // The old "layer mode needs a single layer" refusal is gone: it was written
    // against production's 5,600 cells, and the studio caps at ten levels of a
    // 12x9 lattice. The status reports the count instead.
    if keyboard.just_pressed(KeyCode::KeyF) {
        use crate::detail::DetailMode;
        state.detail_mode = if shift {
            DetailMode::Layer
        } else {
            match state.detail_mode {
                DetailMode::Layer => DetailMode::Focus,
                DetailMode::Focus => DetailMode::Off,
                _ => DetailMode::Layer,
            }
        };
        state.status = format!("deck scope: {}", state.detail_mode.label());
        state.touch_view();
    }
    // The neighbourhood explorer. Entering it re-runs the query, because the
    // whole mode is meaningless without one and the alternative is a first
    // frame that draws nothing and looks broken.
    if keyboard.just_pressed(KeyCode::KeyN) {
        use crate::detail::DetailMode;
        state.detail_mode = if state.detail_mode == DetailMode::Neighborhood {
            DetailMode::Off
        } else {
            DetailMode::Neighborhood
        };
        if state.detail_mode == DetailMode::Neighborhood {
            crate::neighbors::refresh(&mut state);
            menu_state.set_tab(crate::StudioTab::Neighbours);
            state.status = crate::panels::neighbors::headline(&state.neighbors);
        }
        state.touch_view();
    }

    if state.detail_mode == crate::detail::DetailMode::Neighborhood {
        handle_neighbor_keys(&keyboard, &mut state, shift);
    } else {
        // Arrows are the replay's scrub, but the neighbourhood explorer is a
        // list with a cursor on it and arrows are what a list answers to. One
        // key, one meaning: the explorer keeps them while it is open.
        handle_timeline_keys(&keyboard, &mut state);
    }

    // Three named projections rather than a cutaway on/off, because "partial"
    // is a real answer that neither of the other two gives: a whole deck
    // readable at once from any bearing, with nothing cut away.
    if keyboard.just_pressed(KeyCode::KeyC) {
        state.walls = state.walls.next();
        state.status = format!("walls: {}", state.walls.label());
        state.touch_view();
    }
    // Six detents, so the set of walls the cutaway removes changes coherently
    // with the six hex faces instead of popping at arbitrary angles.
    if keyboard.just_pressed(KeyCode::KeyQ) {
        state.detent = (state.detent + crate::viewport::AZIMUTH_DETENTS - 1)
            % crate::viewport::AZIMUTH_DETENTS;
        state.touch_view();
    }
    if keyboard.just_pressed(KeyCode::KeyE) {
        state.detent = (state.detent + 1) % crate::viewport::AZIMUTH_DETENTS;
        state.touch_view();
    }

    // --- pin editing ---
    if keyboard.just_pressed(KeyCode::Comma) {
        state.brush = state.brush.previous();
        state.touch_view();
    }
    if keyboard.just_pressed(KeyCode::Period) {
        state.brush = state.brush.next();
        state.touch_view();
    }
    if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        if ctrl {
            let removed = crate::brush::clear(&mut state.profile);
            state.status = format!("cleared {removed} pin(s)");
            state.refresh_pin_diagnostics();
            state.touch_profile(now);
        } else if let Some(coord) = state.selected
            && crate::brush::unpin(&mut state.profile, coord)
        {
            state.status = format!("unpinned ({},{},{})", coord.q, coord.r, coord.level);
            state.refresh_pin_diagnostics();
            state.touch_profile(now);
        }
    }

    // The seam audit recompiles every authored `.map`, so it is explicit rather
    // than automatic. Running it per solve would make tuning unusable.
    if keyboard.just_pressed(KeyCode::KeyA) {
        run_seam_audit(&mut state);
    }
}

/// Keys that drive the solve replay.
///
/// `T`, `End` and the arrows rather than the `Space`/`.`/`Home` vocabulary
/// `hex_wfc_lab` uses for the same job: every one of those already means
/// something here — rolling a neighbour ring, cycling the pin brush, resetting
/// the camera — and this tool's one input rule is that a key never means two
/// things at once. `+`/`-` are free in the viewport because the panel's own
/// handling of them returns before this point.
fn handle_timeline_keys(keyboard: &ButtonInput<KeyCode>, state: &mut crate::StudioState) {
    let steps = crate::timeline::trace_len(state);
    if steps == 0 {
        return;
    }
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let stride = if shift {
        crate::timeline::COARSE_STEP
    } else {
        1
    };
    let mut moved = false;

    if keyboard.just_pressed(KeyCode::KeyT) {
        state.timeline.toggle(steps);
        moved = true;
    }
    if keyboard.just_pressed(KeyCode::End) {
        state.timeline.to_end(steps);
        moved = true;
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        state.timeline.step(steps, stride);
        moved = true;
    }
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        state.timeline.step(steps, -stride);
        moved = true;
    }
    // Speed alone changes nothing on screen, so it does not redraw.
    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        state.timeline.change_speed(true);
    }
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        state.timeline.change_speed(false);
    }

    if moved {
        state.status = if state.timeline.settled(steps) {
            format!("replay settled on the finished solve ({steps} steps)")
        } else {
            format!("replaying step {} of {steps}", state.timeline.cursor)
        };
        state.touch_view();
    }
}

/// Keys that only mean something while the neighbourhood explorer is open.
///
/// Deliberately arrow keys and `Space` rather than more letters: in this mode
/// the viewport is a list of six-to-eight faces with a cursor on one of them,
/// and arrows are what a list cursor answers to. They are free here because the
/// panel's own arrow handling returns before this point — ownership, not
/// mode-switching, is what keeps a key meaning one thing.
fn handle_neighbor_keys(
    keyboard: &ButtonInput<KeyCode>,
    state: &mut crate::StudioState,
    shift: bool,
) {
    let mut moved = false;
    if let Some(hood) = state.neighbors.hood.as_ref() {
        let faces = hood.faces.len().max(1);
        if keyboard.just_pressed(KeyCode::ArrowDown) {
            state.neighbors.cursor = (state.neighbors.cursor + 1) % faces;
            moved = true;
        }
        if keyboard.just_pressed(KeyCode::ArrowUp) {
            state.neighbors.cursor = (state.neighbors.cursor + faces - 1) % faces;
            moved = true;
        }
    }
    // Jump straight to a face. Six lateral, then up and down, in the same
    // `HexFace::ALL` order the panel lists them, so the digit beside a row is
    // the digit that selects it.
    for (index, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
    ]
    .into_iter()
    .enumerate()
    {
        if keyboard.just_pressed(key)
            && let Some(hood) = state.neighbors.hood.as_ref()
            && index < hood.faces.len()
        {
            state.neighbors.cursor = index;
            moved = true;
        }
    }
    if moved && let Some(face) = state.neighbors.selected_face() {
        state.status = format!(
            "{:?}: {} option(s) here, {} if only this tile mattered",
            face.face,
            face.candidates.len(),
            face.from_centre
        );
    }

    let delta = i32::from(keyboard.just_pressed(KeyCode::ArrowRight))
        - i32::from(keyboard.just_pressed(KeyCode::ArrowLeft));
    if delta != 0 {
        if let Some(note) = state.neighbors.step_candidate(delta as isize) {
            state.status = note;
        }
        state.touch_view();
    }

    if keyboard.just_pressed(KeyCode::Space) {
        if shift {
            state.neighbors.reset_to_solved();
            state.status = String::from("showing the ring exactly as this seed solved it");
        } else {
            let profile = state.profile.clone();
            let rolled = state
                .solved
                .as_ref()
                .map(|solved| solved.world.clone())
                .map(|world| state.neighbors.roll_ring(&world, &profile));
            if let Some(note) = rolled {
                state.status = note;
            }
        }
        state.touch_view();
    }
}

/// Audit every authored seam and keep the result until asked again.
pub fn run_seam_audit(state: &mut crate::StudioState) {
    use crate::panels::coverage::SeamAudit;

    state.seam_audit = SeamAudit::Running;
    let root = crate::persist::corpus_dir();
    state.seam_audit = match observed_authoring::seam_auditor::audit_seams(&root) {
        Ok(report) => SeamAudit::Done {
            valid: report.valid_seams,
            mismatched: report.mismatched_seams,
            report: report.report,
        },
        Err(detail) => SeamAudit::Failed(detail),
    };
    state.status = format!(
        "seam audit: {}",
        match &state.seam_audit {
            SeamAudit::Done {
                valid, mismatched, ..
            } => format!("{valid} agree, {mismatched} mismatched"),
            SeamAudit::Failed(detail) => format!("failed: {detail}"),
            _ => String::from("running"),
        }
    );
}
