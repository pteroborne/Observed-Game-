//! First-person input sampling: keyboard + mouse → the shared [`PlayerIntent`] for
//! the frame, consumed by the fixed-timestep controller in [`super::match_runtime`].

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use player_input::PlayerIntent;

use super::*;

const MOUSE_SENSITIVITY: f32 = 0.12;

/// Map keyboard + mouse into the first-person `PlayerIntent` for this frame.
pub(crate) fn match_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Option<Res<AccumulatedMouseMotion>>,
    paused: Res<MatchPaused>,
    mut intent: ResMut<MatchIntent>,
    mut item_intent: ResMut<ItemIntent>,
) {
    if paused.0 {
        intent.0 = PlayerIntent::default();
        *item_intent = ItemIntent::default();
        return;
    }
    let axis = |negative: KeyCode, positive: KeyCode| {
        (keyboard.pressed(positive) as i32 - keyboard.pressed(negative) as i32) as f32
    };
    let delta = mouse.map(|motion| motion.delta).unwrap_or(Vec2::ZERO);
    let mut movement = Vec2::new(
        axis(KeyCode::KeyA, KeyCode::KeyD),
        axis(KeyCode::KeyS, KeyCode::KeyW),
    );
    if movement.length_squared() > 1.0 {
        movement = movement.normalize_or_zero();
    }
    intent.0 = PlayerIntent {
        movement,
        look: Vec2::new(
            axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) + delta.x * MOUSE_SENSITIVITY,
            axis(KeyCode::ArrowUp, KeyCode::ArrowDown) + delta.y * MOUSE_SENSITIVITY,
        ),
        jump_pressed: keyboard.pressed(KeyCode::Space),
        sprint_held: keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight),
        interact_pressed: keyboard.just_pressed(KeyCode::KeyE),
        ..default()
    };
    *item_intent = ItemIntent {
        torch_action: keyboard.just_pressed(KeyCode::KeyF),
        pad_action: keyboard.just_pressed(KeyCode::KeyC),
        activate_pad: keyboard.just_pressed(KeyCode::KeyE),
    };
}
