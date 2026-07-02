//! First-person input sampling: keyboard, mouse, and gamepad into the shared
//! [`PlayerIntent`] for the frame, consumed by the fixed-timestep controller in
//! [`super::match_runtime`].

use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use player_input::PlayerIntent;

use super::*;

const MOUSE_SENSITIVITY: f32 = 0.12;
const GAMEPAD_DEADZONE: f32 = 0.18;
const GAMEPAD_LOOK_SCALE: f32 = 2.2;
const MENU_STICK_THRESHOLD: f32 = 0.55;

/// Map a Steam Deck / standard gamepad into match movement and local item actions.
///
/// Steam Input exposes the Deck's built-in controls as a conventional gamepad, so the
/// same South/East/West/North/D-pad/stick vocabulary covers both the Deck and Xbox-like
/// controllers without adding a controller plugin.
pub(crate) fn read_gamepad_match(gamepad: &Gamepad) -> (PlayerIntent, ItemIntent) {
    let movement = clamp_unit(apply_deadzone(gamepad.left_stick()) + gamepad.dpad());
    let look = apply_deadzone(gamepad.right_stick());
    (
        PlayerIntent {
            movement,
            // Right-stick up should look up. The traversal controller treats negative
            // `look.y` as pitch-up, matching mouse-up and ArrowUp.
            look: Vec2::new(look.x, -look.y) * GAMEPAD_LOOK_SCALE,
            jump_pressed: gamepad.pressed(GamepadButton::South),
            sprint_held: gamepad.pressed(GamepadButton::LeftTrigger2)
                || gamepad.pressed(GamepadButton::LeftThumb),
            interact_pressed: gamepad.just_pressed(GamepadButton::West),
            interact_held: gamepad.pressed(GamepadButton::West),
            climb_pressed: gamepad.just_pressed(GamepadButton::North),
        },
        ItemIntent {
            torch_action: gamepad.just_pressed(GamepadButton::LeftTrigger),
            pad_action: gamepad.just_pressed(GamepadButton::North),
            activate_pad: gamepad.just_pressed(GamepadButton::West),
        },
    )
}

pub(crate) fn gamepad_confirm_pressed(gamepads: &Query<&Gamepad>) -> bool {
    gamepad_just_pressed(gamepads, &[GamepadButton::South, GamepadButton::Start])
}

pub(crate) fn gamepad_back_pressed(gamepads: &Query<&Gamepad>) -> bool {
    gamepad_just_pressed(gamepads, &[GamepadButton::East])
}

pub(crate) fn gamepad_pause_pressed(gamepads: &Query<&Gamepad>) -> bool {
    gamepad_just_pressed(gamepads, &[GamepadButton::Start, GamepadButton::East])
}

pub(crate) fn gamepad_quit_pressed(gamepads: &Query<&Gamepad>) -> bool {
    gamepad_just_pressed(gamepads, &[GamepadButton::North])
}

pub(crate) fn gamepad_map_pressed(gamepads: &Query<&Gamepad>) -> bool {
    gamepad_just_pressed(
        gamepads,
        &[GamepadButton::RightTrigger, GamepadButton::Select],
    )
}

pub(crate) fn gamepad_menu_axis(gamepads: &Query<&Gamepad>) -> i8 {
    for gamepad in gamepads {
        if gamepad.just_pressed(GamepadButton::DPadDown) {
            return -1;
        }
        if gamepad.just_pressed(GamepadButton::DPadUp) {
            return 1;
        }
    }
    gamepads
        .iter()
        .map(|gamepad| stick_direction(gamepad.left_stick().y))
        .find(|direction| *direction != 0)
        .unwrap_or(0)
}

fn gamepad_just_pressed(gamepads: &Query<&Gamepad>, buttons: &[GamepadButton]) -> bool {
    gamepads
        .iter()
        .any(|gamepad| buttons.iter().any(|button| gamepad.just_pressed(*button)))
}

fn stick_direction(y: f32) -> i8 {
    if y >= MENU_STICK_THRESHOLD {
        1
    } else if y <= -MENU_STICK_THRESHOLD {
        -1
    } else {
        0
    }
}

fn apply_deadzone(value: Vec2) -> Vec2 {
    let length = value.length();
    if length <= GAMEPAD_DEADZONE {
        return Vec2::ZERO;
    }
    let normalized = ((length - GAMEPAD_DEADZONE) / (1.0 - GAMEPAD_DEADZONE)).clamp(0.0, 1.0);
    value.normalize_or_zero() * normalized
}

fn clamp_unit(value: Vec2) -> Vec2 {
    if value.length_squared() > 1.0 {
        value.normalize_or_zero()
    } else {
        value
    }
}

/// Map keyboard + mouse + gamepads into the first-person `PlayerIntent` for this frame.
#[allow(clippy::too_many_arguments)]
pub(crate) fn match_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Option<Res<AccumulatedMouseMotion>>,
    gamepads: Query<&Gamepad>,
    paused: Res<MatchPaused>,
    mut intent: ResMut<MatchIntent>,
    mut item_intent: ResMut<ItemIntent>,
    bot_capture: Option<Res<crate::capture::BotPovCaptureRequest>>,
    spectator_bot: Option<Res<SpectatorBot>>,
) {
    if bot_capture.is_some() || spectator_bot.is_some() {
        return;
    }
    if crate::diagnostics::freecam_enabled() {
        intent.0 = PlayerIntent::default();
        *item_intent = ItemIntent::default();
        return;
    }
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
    let mut look = Vec2::new(
        axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) + delta.x * MOUSE_SENSITIVITY,
        axis(KeyCode::ArrowUp, KeyCode::ArrowDown) + delta.y * MOUSE_SENSITIVITY,
    );
    let mut gamepad_intent = PlayerIntent::default();
    let mut gamepad_item = ItemIntent::default();
    for gamepad in &gamepads {
        let (pad_intent, pad_item) = read_gamepad_match(gamepad);
        gamepad_intent.movement += pad_intent.movement;
        gamepad_intent.look += pad_intent.look;
        gamepad_intent.jump_pressed |= pad_intent.jump_pressed;
        gamepad_intent.sprint_held |= pad_intent.sprint_held;
        gamepad_intent.interact_pressed |= pad_intent.interact_pressed;
        gamepad_intent.interact_held |= pad_intent.interact_held;
        gamepad_intent.climb_pressed |= pad_intent.climb_pressed;
        gamepad_item.torch_action |= pad_item.torch_action;
        gamepad_item.pad_action |= pad_item.pad_action;
        gamepad_item.activate_pad |= pad_item.activate_pad;
    }
    movement = clamp_unit(movement + gamepad_intent.movement);
    look += gamepad_intent.look;

    intent.0 = PlayerIntent {
        movement,
        look,
        jump_pressed: keyboard.pressed(KeyCode::Space) || gamepad_intent.jump_pressed,
        sprint_held: keyboard.pressed(KeyCode::ShiftLeft)
            || keyboard.pressed(KeyCode::ShiftRight)
            || gamepad_intent.sprint_held,
        interact_pressed: keyboard.just_pressed(KeyCode::KeyE) || gamepad_intent.interact_pressed,
        interact_held: keyboard.pressed(KeyCode::KeyE) || gamepad_intent.interact_held,
        climb_pressed: gamepad_intent.climb_pressed,
    };
    *item_intent = ItemIntent {
        torch_action: keyboard.just_pressed(KeyCode::KeyF) || gamepad_item.torch_action,
        pad_action: keyboard.just_pressed(KeyCode::KeyC) || gamepad_item.pad_action,
        activate_pad: keyboard.just_pressed(KeyCode::KeyE) || gamepad_item.activate_pad,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::gamepad::GamepadAxis;

    fn gamepad_with(
        buttons: &[GamepadButton],
        left: Vec2,
        right: Vec2,
        dpad: &[GamepadButton],
    ) -> Gamepad {
        let mut gamepad = Gamepad::default();
        {
            let digital = gamepad.digital_mut();
            for button in buttons.iter().chain(dpad.iter()) {
                digital.press(*button);
            }
        }
        {
            let analog = gamepad.analog_mut();
            analog.set(GamepadAxis::LeftStickX, left.x);
            analog.set(GamepadAxis::LeftStickY, left.y);
            analog.set(GamepadAxis::RightStickX, right.x);
            analog.set(GamepadAxis::RightStickY, right.y);
            if dpad.contains(&GamepadButton::DPadUp) {
                analog.set(GamepadButton::DPadUp, 1.0);
            }
            if dpad.contains(&GamepadButton::DPadDown) {
                analog.set(GamepadButton::DPadDown, 1.0);
            }
            if dpad.contains(&GamepadButton::DPadLeft) {
                analog.set(GamepadButton::DPadLeft, 1.0);
            }
            if dpad.contains(&GamepadButton::DPadRight) {
                analog.set(GamepadButton::DPadRight, 1.0);
            }
        }
        gamepad
    }

    #[test]
    fn gamepad_deadzone_ignores_stick_drift() {
        assert_eq!(apply_deadzone(Vec2::splat(0.05)), Vec2::ZERO);
        assert!(
            apply_deadzone(Vec2::X).x > 0.99,
            "full stick deflection survives the deadzone rescale"
        );
    }

    #[test]
    fn steam_deck_layout_maps_to_match_intent_and_items() {
        let gamepad = gamepad_with(
            &[
                GamepadButton::South,
                GamepadButton::West,
                GamepadButton::North,
                GamepadButton::LeftTrigger,
                GamepadButton::LeftTrigger2,
            ],
            Vec2::new(0.8, 0.0),
            Vec2::new(0.5, 0.5),
            &[GamepadButton::DPadUp],
        );

        let (intent, item) = read_gamepad_match(&gamepad);

        assert!(
            intent.movement.x > 0.5 && intent.movement.y > 0.5,
            "left stick and D-pad both feed movement"
        );
        assert!(intent.look.x > 0.5, "right stick looks right");
        assert!(intent.look.y < -0.5, "right stick up maps to pitch-up");
        assert!(intent.jump_pressed, "A / South jumps");
        assert!(intent.sprint_held, "left trigger sprints");
        assert!(
            intent.interact_pressed && intent.interact_held,
            "X / West interacts"
        );
        assert!(intent.climb_pressed, "Y / North remains available as climb");
        assert!(item.torch_action, "left shoulder drops or picks the torch");
        assert!(item.pad_action, "Y / North drops or picks a pad");
        assert!(item.activate_pad, "X / West activates a nearby pad link");
    }

    #[test]
    fn menu_stick_direction_uses_a_clear_edge_threshold() {
        assert_eq!(stick_direction(0.2), 0);
        assert_eq!(stick_direction(0.7), 1);
        assert_eq!(stick_direction(-0.7), -1);
    }
}
