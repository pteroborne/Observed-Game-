//! Free-fly and shared-controller input for the 3D facility mode.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use observed_traversal::rapier_controller::step_character;
use player_input::PlayerIntent;

use super::{CameraMode, FacilityState, LabViewMode};
use crate::LabState;

pub(super) fn handle_input(
    mode: Res<LabViewMode>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut motions: MessageReader<MouseMotion>,
    mut state: ResMut<FacilityState>,
) {
    if *mode != LabViewMode::Facility3d {
        return;
    }
    let mut look = Vec2::ZERO;
    for motion in motions.read() {
        look += motion.delta;
    }
    state.look = look * 0.06;
    if keyboard.just_pressed(KeyCode::KeyV) {
        state.camera_mode = match state.camera_mode {
            CameraMode::Walk => CameraMode::FreeFly,
            CameraMode::FreeFly => CameraMode::Walk,
        };
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        state.collider_view = !state.collider_view;
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        state.overlay = !state.overlay;
    }
    if keyboard.just_pressed(KeyCode::Backspace) {
        state.body.reset();
    }
    if state.camera_mode == CameraMode::FreeFly {
        let look = state.look;
        state.fly_yaw = (state.fly_yaw + look.x * 0.035).rem_euclid(std::f32::consts::TAU);
        state.fly_pitch = (state.fly_pitch - look.y * 0.035).clamp(-1.5, 1.5);
        state.look = Vec2::ZERO;
        let forward = Vec3::new(state.fly_yaw.sin(), 0.0, -state.fly_yaw.cos());
        let right = forward.cross(Vec3::Y);
        let mut direction = Vec3::ZERO;
        direction += forward * axis(&keyboard, KeyCode::KeyW, KeyCode::KeyS);
        direction += right * axis(&keyboard, KeyCode::KeyD, KeyCode::KeyA);
        direction.y += axis(&keyboard, KeyCode::KeyE, KeyCode::KeyQ);
        let speed = if keyboard.pressed(KeyCode::ShiftLeft) {
            42.0
        } else {
            16.0
        };
        state.fly_position += direction.normalize_or_zero() * speed / 60.0;
    }
}

pub(super) fn step_walk(keyboard: Res<ButtonInput<KeyCode>>, mut state: ResMut<FacilityState>) {
    if state.camera_mode != CameraMode::Walk {
        return;
    }
    let intent = player_intent(&keyboard, state.look);
    state.look = Vec2::ZERO;
    let FacilityState {
        scene,
        body,
        config,
        ..
    } = &mut *state;
    step_character(scene, body, intent, config, 1.0 / 60.0);
    let safety = &state.snapshot.arena;
    let delta = (state.body.position - safety.safety_center).abs();
    if !delta.cmplt(safety.safety_half).all() {
        state.body.reset();
    }
}

pub(super) fn reset_world(
    mode: Res<LabViewMode>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut world: ResMut<LabState>,
    mut state: ResMut<FacilityState>,
    mut cancel_relayout: MessageWriter<crate::relayout_demo::CancelRelayout>,
) {
    if *mode == LabViewMode::Facility3d && keyboard.just_pressed(KeyCode::KeyR) {
        let next = world.world.seed.wrapping_add(1);
        *world = LabState::new(next);
        state.rebuild(&world.world);
        cancel_relayout.write(Default::default());
    }
}

fn axis(keys: &ButtonInput<KeyCode>, positive: KeyCode, negative: KeyCode) -> f32 {
    let positive = if keys.pressed(positive) { 1.0 } else { 0.0 };
    let negative = if keys.pressed(negative) { 1.0 } else { 0.0 };
    positive - negative
}

fn player_intent(keys: &ButtonInput<KeyCode>, look: Vec2) -> PlayerIntent {
    PlayerIntent {
        movement: Vec2::new(
            axis(keys, KeyCode::KeyD, KeyCode::KeyA),
            axis(keys, KeyCode::KeyW, KeyCode::KeyS),
        ),
        look,
        jump_pressed: keys.just_pressed(KeyCode::Space),
        sprint_held: keys.pressed(KeyCode::ShiftLeft),
        ..PlayerIntent::default()
    }
}
