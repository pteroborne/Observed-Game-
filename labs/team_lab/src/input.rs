use bevy::prelude::*;

/// Hardware input for the one human-controlled player, collapsed into abstract
/// values. Edge-triggered actions target whatever is nearest in range when the
/// simulation builds the player's intent.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct HumanInput {
    pub movement: Vec2,
    pub use_pressed: bool,
    pub release_pressed: bool,
    pub grab_pressed: bool,
    pub drop_pressed: bool,
}

pub(crate) fn sample_hardware(keyboard: Res<ButtonInput<KeyCode>>, mut input: ResMut<HumanInput>) {
    let x = axis(&keyboard, KeyCode::KeyD, KeyCode::KeyA)
        + axis(&keyboard, KeyCode::ArrowRight, KeyCode::ArrowLeft);
    let y = axis(&keyboard, KeyCode::KeyW, KeyCode::KeyS)
        + axis(&keyboard, KeyCode::ArrowUp, KeyCode::ArrowDown);
    input.movement = Vec2::new(x.clamp(-1.0, 1.0), y.clamp(-1.0, 1.0));
    input.use_pressed = keyboard.just_pressed(KeyCode::KeyE);
    input.release_pressed = keyboard.just_pressed(KeyCode::KeyQ);
    input.grab_pressed = keyboard.just_pressed(KeyCode::KeyF);
    input.drop_pressed = keyboard.just_pressed(KeyCode::KeyG);
}

fn axis(keyboard: &ButtonInput<KeyCode>, positive: KeyCode, negative: KeyCode) -> f32 {
    f32::from(u8::from(keyboard.pressed(positive)))
        - f32::from(u8::from(keyboard.pressed(negative)))
}
