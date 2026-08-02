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

    // A different seed is a different facility, so the cached baseline solve
    // no longer describes anything the compare overlay should trust.
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        state.seed_index = state.seed_index.saturating_sub(1);
        state.invalidate_baseline();
        state.touch_profile(now);
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        state.seed_index += 1;
        state.invalidate_baseline();
        state.touch_profile(now);
    }

    if keyboard.just_pressed(KeyCode::KeyR) {
        if ctrl {
            state.reload(now);
        } else {
            state.touch_profile(now);
        }
    }

    if keyboard.just_pressed(KeyCode::KeyG) {
        state.show_walls = !state.show_walls;
        state.touch_view();
    }

    if keyboard.just_pressed(KeyCode::Home) || keyboard.just_pressed(KeyCode::Digit0) {
        state.zoom = crate::viewport::DEFAULT_ZOOM;
        state.pan = Vec2::ZERO;
    }

    if ctrl && keyboard.just_pressed(KeyCode::KeyS) {
        request_save(&mut state, &mut menu_state, shift);
    }

    if ctrl && keyboard.just_pressed(KeyCode::KeyZ) {
        state.profile = state.saved.clone();
        state.status = String::from("reverted to the last saved profile");
        state.touch_profile(now);
    }

    if keyboard.just_pressed(KeyCode::KeyB) {
        state.show_baseline_compare = !state.show_baseline_compare;
        state.touch_view();
    }

    // --- detail rendering ---
    if keyboard.just_pressed(KeyCode::KeyF) {
        use crate::detail::DetailMode;
        state.detail_mode = if shift {
            // A layer sweep needs a layer. Refusing loudly beats silently
            // drawing nothing and letting the author think detail is broken.
            if state.layer.level().is_some() {
                DetailMode::Layer
            } else {
                state.status = String::from(
                    "detail: layer mode needs a single layer (Tab to pick one); \
                     all-layers detail is ~120k hulls and not a diagram",
                );
                DetailMode::Off
            }
        } else if state.detail_mode == DetailMode::Focus {
            DetailMode::Off
        } else {
            DetailMode::Focus
        };
        state.touch_view();
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        state.cutaway = !state.cutaway;
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
