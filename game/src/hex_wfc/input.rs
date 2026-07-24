//! Local input capture for the hex match: keyboard/mouse/gamepad into a sanitized
//! [`PlayerIntent`]. The hex match is a spawn→exit traversal race — movement, look,
//! sprint, ordinary jump, and interaction.

use bevy::input::gamepad::GamepadButton;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use observed_match::hex_wfc::HexActionButtons;
use player_input::PlayerIntent;

use super::sim::HexWfcIntent;
use crate::GameState;

const KEY_LOOK_STEP: f32 = 0.035;

pub(super) fn map_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Option<Res<AccumulatedMouseMotion>>,
    gamepads: Query<&Gamepad>,
    settings: Res<crate::settings::Settings>,
    mut intent: ResMut<HexWfcIntent>,
    spectator_bot: Option<Res<crate::sim::state::SpectatorBot>>,
) {
    if spectator_bot.is_some() {
        intent.intent = PlayerIntent::default();
        intent.actions = HexActionButtons::default();
        intent.toggle_map = false;
        intent.browse_map_level = 0;
        return;
    }
    let bindings = &settings.bindings;
    let axis = |negative: KeyCode, positive: KeyCode| {
        (keyboard.pressed(positive) as i32 - keyboard.pressed(negative) as i32) as f32
    };
    let movement = Vec2::new(
        axis(bindings.move_left, bindings.move_right),
        axis(bindings.move_back, bindings.move_forward),
    )
    .normalize_or_zero();
    let mouse_delta = mouse.map_or(Vec2::ZERO, |motion| motion.delta);
    let mut gamepad_intent = PlayerIntent::default();
    let mut gamepad_deploy = false;
    let mut gamepad_recover = false;
    for gamepad in &gamepads {
        let (command, items) = crate::screens::input::read_gamepad_match(gamepad);
        gamepad_intent.movement += command.movement;
        gamepad_intent.look += command.look;
        gamepad_intent.jump_pressed |= command.jump_pressed;
        gamepad_intent.sprint_held |= command.sprint_held;
        gamepad_intent.interact_held |= command.interact_held;
        gamepad_deploy |= items.torch_action;
        gamepad_recover |= gamepad.just_pressed(GamepadButton::East);
    }
    intent.intent = PlayerIntent {
        movement: (movement + gamepad_intent.movement).clamp_length_max(1.0),
        look: mouse_delta * (settings.mouse_sensitivity * 0.018_333)
            + Vec2::new(
                axis(bindings.look_left, bindings.look_right) * KEY_LOOK_STEP,
                axis(bindings.look_up, bindings.look_down) * KEY_LOOK_STEP,
            )
            + gamepad_intent.look,
        // Jump always uses the ordinary physical controller. Vertical routes
        // are walked on authored ramps and stairs.
        jump_pressed: keyboard.pressed(bindings.jump) || gamepad_intent.jump_pressed,
        sprint_held: keyboard.pressed(bindings.sprint)
            || keyboard.pressed(bindings.sprint_alt)
            || gamepad_intent.sprint_held,
        interact_held: keyboard.pressed(bindings.interact) || gamepad_intent.interact_held,
        ..Default::default()
    };
    intent.actions = HexActionButtons {
        interact: keyboard.just_pressed(bindings.interact) || gamepad_intent.interact_pressed,
        deploy_lantern: keyboard.just_pressed(bindings.torch) || gamepad_deploy,
        recover_lantern: keyboard.just_pressed(KeyCode::KeyR) || gamepad_recover,
    };
    intent.toggle_map |= keyboard.just_pressed(bindings.tac_map);
    intent.toggle_map |= crate::screens::input::gamepad_map_pressed(&gamepads);
    let keyboard_up =
        keyboard.just_pressed(KeyCode::PageUp) || keyboard.just_pressed(KeyCode::BracketRight);
    let keyboard_down =
        keyboard.just_pressed(KeyCode::PageDown) || keyboard.just_pressed(KeyCode::BracketLeft);
    let gamepad_up = gamepads
        .iter()
        .any(|gamepad| gamepad.just_pressed(GamepadButton::DPadUp));
    let gamepad_down = gamepads
        .iter()
        .any(|gamepad| gamepad.just_pressed(GamepadButton::DPadDown));
    intent.browse_map_level = match (keyboard_up || gamepad_up, keyboard_down || gamepad_down) {
        (true, false) => 1,
        (false, true) => -1,
        _ => 0,
    };
}

pub(super) fn mode_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<GameState>>,
    runtime: Option<Res<super::sim::HexWfcRuntime>>,
    mut lan: ResMut<crate::lan::LanRuntime>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        if runtime.is_some_and(|runtime| runtime.networked) {
            lan.leave();
        }
        next.set(GameState::MainMenu);
    }
}

pub(super) fn grab_cursor(
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
    spectator_bot: Option<Res<crate::sim::state::SpectatorBot>>,
) {
    if spectator_bot.is_none()
        && let Ok(mut cursor) = cursors.single_mut()
    {
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
