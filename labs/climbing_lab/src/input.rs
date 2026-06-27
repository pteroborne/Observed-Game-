use bevy::prelude::*;
use observed_core::{PlayerId, PlayerIntent};

use crate::lab::ClimbLabRuntime;

/// Hardware input collapsed into the abstract values the simulation consumes.
/// Edge-triggered actions are latched here until a simulation step takes them.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct HumanInput {
    pub movement: Vec2,
    pub jump_queued: bool,
    pub climb_queued: bool,
}

pub(crate) fn sample_hardware(keyboard: Res<ButtonInput<KeyCode>>, mut buffer: ResMut<HumanInput>) {
    let x = axis(&keyboard, KeyCode::KeyD, KeyCode::KeyA)
        + axis(&keyboard, KeyCode::ArrowRight, KeyCode::ArrowLeft);
    let y = axis(&keyboard, KeyCode::KeyW, KeyCode::KeyS)
        + axis(&keyboard, KeyCode::ArrowUp, KeyCode::ArrowDown);
    buffer.movement = Vec2::new(x.clamp(-1.0, 1.0), y.clamp(-1.0, 1.0));
    buffer.jump_queued |= keyboard.just_pressed(KeyCode::Space);
    buffer.climb_queued |= keyboard.just_pressed(KeyCode::KeyQ);
}

pub(crate) fn distribute_intents(
    runtime: Res<ClimbLabRuntime>,
    mut buffer: ResMut<HumanInput>,
    mut bodies: Query<(&PlayerId, &mut PlayerIntent)>,
) {
    let movement = buffer.movement;
    let jump = std::mem::take(&mut buffer.jump_queued);
    let climb = std::mem::take(&mut buffer.climb_queued);
    for (player, mut intent) in &mut bodies {
        *intent = if *player == runtime.selected_player {
            PlayerIntent {
                movement,
                jump_pressed: jump,
                climb_pressed: climb,
                ..default()
            }
            .sanitized()
        } else {
            // Unselected bodies hold their authored showcase pose, so each
            // traversal mode stays visible until the player drives one.
            PlayerIntent::default()
        };
    }
}

fn axis(keyboard: &ButtonInput<KeyCode>, positive: KeyCode, negative: KeyCode) -> f32 {
    f32::from(u8::from(keyboard.pressed(positive)))
        - f32::from(u8::from(keyboard.pressed(negative)))
}
