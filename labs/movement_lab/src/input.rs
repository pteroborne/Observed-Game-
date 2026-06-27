use bevy::prelude::*;
use player_input::{PlayerId, PlayerIntent};

use crate::{lab::MovementLabRuntime, simulation::MovementBody};

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct HumanInputBuffer {
    pub movement_x: f32,
    pub sprint_held: bool,
    pub jump_queued: bool,
}

pub(crate) fn sample_hardware_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut buffer: ResMut<HumanInputBuffer>,
) {
    buffer.movement_x = f32::from(u8::from(keyboard.pressed(KeyCode::KeyD)))
        - f32::from(u8::from(keyboard.pressed(KeyCode::KeyA)));
    buffer.sprint_held =
        keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    buffer.jump_queued |= keyboard.just_pressed(KeyCode::Space);
}

pub(crate) fn prepare_player_intents(
    time: Res<Time<Fixed>>,
    runtime: Res<MovementLabRuntime>,
    mut buffer: ResMut<HumanInputBuffer>,
    mut players: Query<(&PlayerId, &MovementBody, &mut PlayerIntent)>,
) {
    let elapsed = time.elapsed_secs();
    for (player, body, mut intent) in &mut players {
        *intent = if *player == runtime.selected_player {
            PlayerIntent {
                movement: Vec2::new(buffer.movement_x, 0.0),
                jump_pressed: std::mem::take(&mut buffer.jump_queued),
                sprint_held: buffer.sprint_held,
                ..default()
            }
        } else {
            scripted_movement(*player, body, elapsed)
        };
    }
}

fn scripted_movement(player: PlayerId, body: &MovementBody, elapsed: f32) -> PlayerIntent {
    let phase = elapsed + player.0 as f32 * 0.7;
    let direction = if (phase / 3.5).floor() as i32 % 2 == 0 {
        1.0
    } else {
        -1.0
    };
    PlayerIntent {
        movement: Vec2::new(direction, 0.0),
        jump_pressed: body.grounded && phase.rem_euclid(2.4) < 0.02,
        sprint_held: player.0.is_multiple_of(2),
        ..default()
    }
}
