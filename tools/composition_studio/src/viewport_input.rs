//! Viewport input: what the pointer is over, and what a click there does.
//!
//! Split from `lib.rs` so neither file outgrows the 600-line review budget the
//! rest of the WFC path lives under. Panel/menu keys live in `input.rs`; these
//! are the mouse-driven ones that act on the lattice itself.

use bevy::prelude::*;
use bevy::window::{CursorIcon, SystemCursorIcon};

use crate::{LabMenuState, StudioCamera, StudioState, brush, pick};
/// What the pointer is currently over and what a click there would do.
///
/// Two signals, deliberately: the ring says *which* cell, and the cursor shape
/// says *what will happen*. Cursor alone is a 32-pixel hint at the edge of
/// attention; ring alone does not distinguish painting from inspecting.
pub fn update_hover_and_cursor(
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<(Entity, &Window)>,
    camera_query: Query<(&Camera, &GlobalTransform), With<StudioCamera>>,
    menu_state: Res<LabMenuState>,
    mut state: ResMut<StudioState>,
    mut commands: Commands,
) {
    let Ok((window_entity, window)) = windows.single() else {
        return;
    };

    // A pending confirmation is the one genuinely modal thing here, because it
    // is asking a question that must be answered. Everything else stays live:
    // the panel no longer freezes the facility.
    let hovered = if menu_state.confirm.is_some() {
        None
    } else {
        (|| {
            let (camera, camera_transform) = camera_query.single().ok()?;
            let cursor = window.cursor_position()?;
            if !state.cursor_in_viewport(cursor) {
                return None;
            }
            let solved = state.solved.as_ref()?;
            pick::pick(
                &solved.world,
                state.layer,
                camera,
                camera_transform,
                state.cursor_to_viewport(cursor),
            )
        })()
    };

    if hovered != state.hovered {
        state.hovered = hovered;
        // Overlay only: a ring moved, no hull work.
        state.touch_overlay();
    }

    // Clicking the panel hands it the keyboard, mirroring the viewport side.
    if mouse.just_pressed(MouseButton::Left)
        && let Some(cursor) = window.cursor_position()
        && !state.cursor_in_viewport(cursor)
    {
        state.keyboard_owner = crate::KeyboardOwner::Panel;
    }

    let inspecting = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let panning = mouse.pressed(MouseButton::Right) || mouse.pressed(MouseButton::Middle);
    let icon = if panning {
        SystemCursorIcon::Grabbing
    } else if hovered.is_none() {
        SystemCursorIcon::Default
    } else if inspecting {
        SystemCursorIcon::Pointer
    } else {
        SystemCursorIcon::Crosshair
    };
    commands
        .entity(window_entity)
        .insert(CursorIcon::System(icon));
}

/// Left-drag paints the active brush; `Shift`+left selects without painting.
///
/// Painting is held on the button rather than fired once, so a stroke across a
/// corridor pins the whole run. Each newly-touched cell re-solves through the
/// ordinary debounce, so a fast drag costs one solve, not one per cell.
pub fn handle_viewport_painting(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<StudioCamera>>,
    menu_state: Res<LabMenuState>,
    mut state: ResMut<StudioState>,
) {
    if menu_state.confirm.is_some() || !mouse.pressed(MouseButton::Left) {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if !state.cursor_in_viewport(cursor) {
        return;
    }

    // Clicking the facility hands it the keyboard. That is the whole ownership
    // rule: you talk to what you touched, and both halves stay live.
    if mouse.just_pressed(MouseButton::Left) {
        state.keyboard_owner = crate::KeyboardOwner::Viewport;
    }

    let local = state.cursor_to_viewport(cursor);
    let Some(solved) = state.solved.as_ref() else {
        return;
    };
    let Some(coord) = pick::pick(&solved.world, state.layer, camera, camera_transform, local)
    else {
        return;
    };

    if state.selected != Some(coord) {
        state.selected = Some(coord);
        state.touch_overlay();
    }

    // Shift inspects without committing an edit, so an author can read a cell
    // mid-stroke without pinning it.
    if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
        return;
    }

    let now = time.elapsed_secs();
    let (config, active) = (state.config, state.brush);
    let painted = brush::paint(&mut state.profile, config, coord, active);
    if painted {
        state.refresh_pin_diagnostics();
        state.touch_profile(now);
    }
}
