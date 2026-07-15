use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use player_input::PlayerIntent;

use super::sim::FullWfcIntent;
use crate::GameState;

const MOUSE_SENSITIVITY: f32 = 0.0022;
const KEY_LOOK_STEP: f32 = 0.035;

pub(super) fn map_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Option<Res<AccumulatedMouseMotion>>,
    mut intent: ResMut<FullWfcIntent>,
) {
    let axis = |negative: KeyCode, positive: KeyCode| {
        (keyboard.pressed(positive) as i32 - keyboard.pressed(negative) as i32) as f32
    };
    let movement = Vec2::new(
        axis(KeyCode::KeyA, KeyCode::KeyD),
        axis(KeyCode::KeyS, KeyCode::KeyW),
    )
    .normalize_or_zero();
    let mouse_delta = mouse.map_or(Vec2::ZERO, |motion| motion.delta);
    intent.0 = PlayerIntent {
        movement,
        look: mouse_delta * MOUSE_SENSITIVITY
            + Vec2::new(
                axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) * KEY_LOOK_STEP,
                axis(KeyCode::ArrowDown, KeyCode::ArrowUp) * KEY_LOOK_STEP,
            ),
        // These are intent fields, not direct gameplay reads. Holding Space climbs
        // upward in an authored shaft; holding Ctrl descends.
        jump_pressed: keyboard.pressed(KeyCode::Space),
        sprint_held: keyboard.pressed(KeyCode::ShiftLeft),
        interact_held: keyboard.pressed(KeyCode::ControlLeft),
        ..Default::default()
    };
}

pub(super) fn mode_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next.set(GameState::MainMenu);
    }
}

pub(super) fn grab_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor) = cursors.single_mut() {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

pub(super) fn release_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor) = cursors.single_mut() {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}
