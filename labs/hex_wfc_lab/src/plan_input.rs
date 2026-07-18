//! Keyboard controls for the retained Phase 90 solve replay.

use bevy::prelude::*;

use crate::{LabState, PRESET_SEEDS, relayout_demo};

pub(super) fn handle(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<LabState>,
    mut cancel_relayout: MessageWriter<relayout_demo::CancelRelayout>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        state.playing = !state.playing;
        state.status = if state.playing { "playing" } else { "paused" }.to_string();
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyN) {
        state.playing = false;
        state.advance(1);
        state.status = "single step".to_string();
    }
    if keyboard.just_pressed(KeyCode::KeyI) {
        state.cursor = state.trace.len();
        state.status = "instant solve".to_string();
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        state.steps_per_tick = (state.steps_per_tick * 2).min(64);
        state.status = format!("speed {} steps/tick", state.steps_per_tick);
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        state.steps_per_tick = (state.steps_per_tick / 2).max(1);
        state.status = format!("speed {} steps/tick", state.steps_per_tick);
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        let next = state.world.seed.wrapping_add(1);
        *state = LabState::new(next);
        state.status = format!("reset to seed {next:#018x}");
        cancel_relayout.write(Default::default());
    }
    if keyboard.just_pressed(KeyCode::F1) {
        state.overlay = !state.overlay;
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::PageUp) || keyboard.just_pressed(KeyCode::BracketRight) {
        state.current_level =
            (state.current_level + 1).min(state.world.config.levels.saturating_sub(1));
        state.status = format!("viewing level {}", state.current_level);
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::PageDown) || keyboard.just_pressed(KeyCode::BracketLeft) {
        state.current_level = state.current_level.saturating_sub(1);
        state.status = format!("viewing level {}", state.current_level);
        state.dirty = true;
    }

    for (index, key) in PRESET_KEYS.into_iter().enumerate() {
        if keyboard.just_pressed(key) {
            *state = LabState::new(PRESET_SEEDS[index]);
            state.status = format!("preset seed {}", index + 1);
            // The same seed is still a new world: stale relayout work must die.
            cancel_relayout.write(Default::default());
        }
    }
}

const PRESET_KEYS: [KeyCode; 9] = [
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
