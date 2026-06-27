use bevy::prelude::*;

/// One-shot equipment actions issued by the human-controlled player.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EquipAction {
    PickUp,
    Drop,
    Place,
    HandOff,
    Recover,
    ToggleLeave,
    ReplaceRoom,
}

#[derive(Resource, Default)]
pub struct HumanInput {
    pub movement: Vec2,
    pub actions: Vec<EquipAction>,
}

pub(crate) fn sample_hardware(keyboard: Res<ButtonInput<KeyCode>>, mut input: ResMut<HumanInput>) {
    let x = axis(&keyboard, KeyCode::KeyD, KeyCode::KeyA)
        + axis(&keyboard, KeyCode::ArrowRight, KeyCode::ArrowLeft);
    let y = axis(&keyboard, KeyCode::KeyW, KeyCode::KeyS)
        + axis(&keyboard, KeyCode::ArrowUp, KeyCode::ArrowDown);
    input.movement = Vec2::new(x.clamp(-1.0, 1.0), y.clamp(-1.0, 1.0));

    for (key, action) in [
        (KeyCode::KeyE, EquipAction::PickUp),
        (KeyCode::KeyG, EquipAction::Drop),
        (KeyCode::KeyC, EquipAction::Place),
        (KeyCode::KeyF, EquipAction::HandOff),
        (KeyCode::KeyV, EquipAction::Recover),
        (KeyCode::KeyL, EquipAction::ToggleLeave),
        (KeyCode::KeyT, EquipAction::ReplaceRoom),
    ] {
        if keyboard.just_pressed(key) {
            input.actions.push(action);
        }
    }
}

fn axis(keyboard: &ButtonInput<KeyCode>, positive: KeyCode, negative: KeyCode) -> f32 {
    f32::from(u8::from(keyboard.pressed(positive)))
        - f32::from(u8::from(keyboard.pressed(negative)))
}
