//! Keys for `module_studio`.
//!
//! Split from `app.rs` so neither file outgrows the 600-line review budget the
//! rest of this tool lives under, and because the key table is the one thing in
//! this studio a person reads to answer "what can I press".

use bevy::prelude::*;

use crate::module::app::ModuleState;
use crate::module::neighbor_view::{NeighbourView, toggle};

/// Keys: page modules, jump to the next failure, orbit, zoom.
pub fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<ModuleState>,
    mut view: ResMut<NeighbourView>,
    scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
) {
    if keyboard.just_pressed(KeyCode::BracketRight) || keyboard.just_pressed(KeyCode::ArrowDown) {
        let next = state.selected + 1;
        state.select(next);
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) || keyboard.just_pressed(KeyCode::ArrowUp) {
        let next = state.selected + state.diagnoses.len().saturating_sub(1);
        state.select(next);
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        state.select_next_failing();
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        state.toggle_rapier_audit();
    }
    // `C` is already cutaway, and stays cutaway; certification takes `K`.
    if keyboard.just_pressed(KeyCode::KeyK) {
        state.toggle_certification();
    }
    // `C` is cutaway in the sibling tool too. A key that means one thing in
    // one studio and another thing in the other is a trap for the person
    // who uses both.
    if keyboard.just_pressed(KeyCode::KeyC) {
        state.cutaway = !state.cutaway;
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyQ) {
        state.detent = (state.detent + 1) % crate::viewport::AZIMUTH_DETENTS;
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyE) {
        state.detent = (state.detent + crate::viewport::AZIMUTH_DETENTS - 1)
            % crate::viewport::AZIMUTH_DETENTS;
        state.dirty = true;
    }
    // The neighbour ring. `N` matches the sibling tool's binding for the same
    // job, and so do the digits, `Space` and `Shift+Space`.
    if keyboard.just_pressed(KeyCode::KeyN) {
        state.status = toggle(&mut state, &mut view);
    }
    if view.on {
        handle_ring_keys(&keyboard, &mut state, &mut view);
    }

    // Recipe editing. Deliberately different keys from the module paging
    // above, so a key never means two things at once - the same rule the
    // sibling tool enforces with ownership.
    if state.current().is_some_and(|d| d.is_parametric()) {
        if keyboard.just_pressed(KeyCode::KeyX) {
            state.step += 1;
            state.param = 0;
            state.clamp_cursor();
        }
        if keyboard.just_pressed(KeyCode::KeyS) && !keyboard.pressed(KeyCode::ControlLeft) {
            state.step = state.step.saturating_sub(1);
            state.param = 0;
            state.clamp_cursor();
        }
        if keyboard.just_pressed(KeyCode::KeyD) {
            state.param += 1;
            state.clamp_cursor();
        }
        if keyboard.just_pressed(KeyCode::KeyA) {
            state.param = state.param.saturating_sub(1);
            state.clamp_cursor();
        }
        let coarse = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
        let unit = if coarse { 8.0 } else { 1.0 };
        if keyboard.just_pressed(KeyCode::ArrowRight) {
            state.nudge_param(unit);
        }
        if keyboard.just_pressed(KeyCode::ArrowLeft) {
            state.nudge_param(-unit);
        }
    }

    if scroll.delta.y.abs() > f32::EPSILON {
        state.zoom = (state.zoom * 1.14_f32.powf(-scroll.delta.y)).clamp(0.05, 4.0);
    }
}

/// Keys that only mean something while the ring is open.
///
/// Deliberately not the arrow keys, even though the sibling tool uses them for
/// the same job here: `Up`/`Down` page modules unconditionally and
/// `Left`/`Right` nudge recipe parameters, and mode-scoping either would stop
/// you editing a recipe while watching its ring update - which is one of the
/// better uses of this mode. Digits address a cell directly, which for six
/// faces beats a cursor anyway.
fn handle_ring_keys(
    keyboard: &ButtonInput<KeyCode>,
    state: &mut ModuleState,
    view: &mut NeighbourView,
) {
    const DIGITS: [KeyCode; 9] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (index, key) in DIGITS.into_iter().enumerate() {
        if keyboard.just_pressed(key)
            && let Some(note) = view.select(index)
        {
            state.status = note;
            state.dirty = true;
        }
    }

    let step = i32::from(keyboard.just_pressed(KeyCode::Period))
        - i32::from(keyboard.just_pressed(KeyCode::Comma));
    if step != 0 {
        if let Some(note) = view.step_candidate(step as isize) {
            state.status = note;
        }
        state.dirty = true;
    }

    if keyboard.just_pressed(KeyCode::Space) {
        let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
        state.status = if shift {
            view.reset();
            String::from("every cell back to its heaviest option")
        } else {
            view.roll()
        };
        state.dirty = true;
    }
}
