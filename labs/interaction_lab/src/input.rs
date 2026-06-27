use bevy::prelude::*;
use observed_core::{PlayerId, PlayerIntent};

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct InteractionInput {
    pub p1: PlayerIntent,
    pub p2: PlayerIntent,
}

pub(crate) fn sample_hardware(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut input: ResMut<InteractionInput>,
) {
    input.p1 = keyboard_intent(
        &keyboard,
        [KeyCode::KeyA, KeyCode::KeyD, KeyCode::KeyS, KeyCode::KeyW],
        KeyCode::KeyE,
        KeyCode::KeyQ,
    );
    input.p2 = keyboard_intent(
        &keyboard,
        [
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
            KeyCode::ArrowDown,
            KeyCode::ArrowUp,
        ],
        KeyCode::Numpad1,
        KeyCode::Numpad2,
    );
}

pub(crate) fn distribute_intents(
    time: Res<Time>,
    input: Res<InteractionInput>,
    mut players: Query<(&PlayerId, &mut PlayerIntent)>,
) {
    for (player, mut intent) in &mut players {
        *intent = match player.0 {
            0 => input.p1,
            1 => input.p2,
            _ => scripted_intent(*player, time.elapsed_secs()),
        };
    }
}

fn keyboard_intent(
    keyboard: &ButtonInput<KeyCode>,
    movement: [KeyCode; 4],
    interact: KeyCode,
    climb: KeyCode,
) -> PlayerIntent {
    PlayerIntent {
        movement: Vec2::new(
            f32::from(u8::from(keyboard.pressed(movement[1])))
                - f32::from(u8::from(keyboard.pressed(movement[0]))),
            f32::from(u8::from(keyboard.pressed(movement[3])))
                - f32::from(u8::from(keyboard.pressed(movement[2]))),
        )
        .normalize_or_zero(),
        sprint_held: keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight),
        interact_pressed: keyboard.just_pressed(interact),
        interact_held: keyboard.pressed(interact),
        climb_pressed: keyboard.just_pressed(climb),
        ..default()
    }
}

fn scripted_intent(player: PlayerId, elapsed: f32) -> PlayerIntent {
    let phase = elapsed * 0.55 + player.0 as f32;
    PlayerIntent {
        movement: Vec2::new(phase.cos(), phase.sin()) * 0.32,
        ..default()
    }
}
