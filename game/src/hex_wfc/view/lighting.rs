//! Deterministic local-light budget and district atmosphere for the hex facility.
//!
//! The rig is bounded: one district key spotlight staged above the runner's current
//! cell plus district ambient and distance fog. The caged lantern is the only
//! player-following light, so spending the last one has a real visibility cost.
//! Authored tile hulls carry surface detail; the district rig carries atmosphere.

use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_hex::hex_origin;
use observed_style::{self as style};

use super::HexWfcKeyLight;
use crate::GameState;
use crate::hex_wfc::sim::{EYE_OFFSET, HexWfcRuntime};
use crate::view::components::GameCam;

const BLEND_RATE: f32 = 2.5;
/// Hex cells are much tighter than the teleport-era rooms that established the
/// style palette's absolute lumen values. Keep the semantic colour/cone/range,
/// but scale the practical key so a nearby wall retains material contrast.
const HEX_KEY_INTENSITY_SCALE: f32 = 0.000_01;

/// Spawn the complete semantic rig at its final treatment for the initial cell.
///
/// Phase 95 spawned a default-white, zero-intensity key and eased it toward the
/// current register. That made the first visible seconds desaturated. Initial state is
/// not a transition: every light starts at the exact `observed_style` target, while
/// [`sync_lighting_and_atmosphere`] retains easing for later cell changes.
pub(super) fn spawn_rig(
    commands: &mut Commands,
    architecture: ArchitectureRegister,
    current: observed_facility::hex_wfc::HexCoord,
    player: &observed_match::hex_wfc::HexPlayerState,
) {
    let (key_translation, key_rotation) = key_pose(current);
    let _ = player;
    commands.spawn((
        HexWfcKeyLight,
        DespawnOnExit(GameState::HexWfc),
        primed_key_light(architecture),
        Transform::from_translation(key_translation).with_rotation(key_rotation),
        Name::new("budgeted hex key light"),
    ));
}

fn primed_key_light(architecture: ArchitectureRegister) -> SpotLight {
    let palette = style::architecture(architecture);
    SpotLight {
        color: palette.key_color,
        intensity: palette.key_intensity * HEX_KEY_INTENSITY_SCALE,
        range: palette.key_range,
        radius: palette.key_radius,
        inner_angle: palette.key_inner_angle,
        outer_angle: palette.key_outer_angle,
        shadows_enabled: palette.key_shadows_enabled,
        ..default()
    }
}

/// Put the shared camera at the authoritative eye pose before shell entities are
/// enqueued. This removes the one-frame menu-camera vantage from state entry.
pub(super) fn prime_camera(
    transform: &mut Transform,
    player: &observed_match::hex_wfc::HexPlayerState,
) {
    let (eye, rotation) = player_eye_pose(player);
    transform.translation = eye;
    transform.rotation = rotation;
}

pub(in crate::hex_wfc) fn sync_camera(
    runtime: Res<HexWfcRuntime>,
    mut camera: Query<&mut Transform, With<GameCam>>,
) {
    let player = runtime.local();
    let (eye, rotation) = player_eye_pose(player);
    if let Ok(mut transform) = camera.single_mut() {
        transform.translation = eye;
        transform.rotation = rotation;
    }
}

pub(in crate::hex_wfc) fn sync_lighting_and_atmosphere(
    time: Res<Time>,
    runtime: Res<HexWfcRuntime>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    mut camera: Query<&mut DistanceFog, With<GameCam>>,
    mut key: Query<(&mut SpotLight, &mut Transform), With<HexWfcKeyLight>>,
) {
    let current = runtime.local().cell;
    let architecture = runtime
        .match_state
        .facility
        .architecture
        .get(&current)
        .copied()
        .unwrap_or(observed_content::ArchitectureRegister::ALL[0]);
    let palette = style::architecture(architecture);
    let t = (time.delta_secs() * BLEND_RATE).clamp(0.0, 1.0);

    ambient.color = lerp_color(ambient.color, palette.ambient_color, t);
    ambient.brightness = lerp_f(ambient.brightness, palette.ambient_brightness, t);
    clear.0 = lerp_color(clear.0, palette.fog_color, t);

    if let Ok(mut fog) = camera.single_mut() {
        fog.color = lerp_color(fog.color, palette.fog_color, t);
        if let bevy::pbr::FogFalloff::Linear { start, end } = &mut fog.falloff {
            *start = lerp_f(*start, palette.fog_start, t);
            *end = lerp_f(*end, palette.fog_end, t);
        }
    }

    if let Ok((mut light, mut transform)) = key.single_mut() {
        let (target_translation, target_rotation) = key_pose(current);
        if transform.translation == Vec3::ZERO {
            transform.translation = target_translation;
            transform.rotation = target_rotation;
        } else {
            transform.translation = transform.translation.lerp(target_translation, t);
            transform.rotation = transform.rotation.slerp(target_rotation, t);
        }
        let target_color = lerp_color(light.color, palette.key_color, t);
        light.color = target_color;
        light.intensity = lerp_f(
            light.intensity,
            palette.key_intensity * HEX_KEY_INTENSITY_SCALE,
            t,
        );
        light.range = lerp_f(light.range, palette.key_range, t);
        light.radius = lerp_f(light.radius, palette.key_radius, t);
        light.inner_angle = lerp_f(light.inner_angle, palette.key_inner_angle, t);
        light.outer_angle = lerp_f(light.outer_angle, palette.key_outer_angle, t);
        light.shadows_enabled = palette.key_shadows_enabled;
    }
}

fn player_eye_pose(player: &observed_match::hex_wfc::HexPlayerState) -> (Vec3, Quat) {
    let eye = player.position + Vec3::Y * EYE_OFFSET;
    let rotation = Quat::from_rotation_y(-player.yaw) * Quat::from_rotation_x(player.pitch);
    (eye, rotation)
}

fn key_pose(current: observed_facility::hex_wfc::HexCoord) -> (Vec3, Quat) {
    let origin = Vec3::from_array(hex_origin(current));
    let translation = origin + Vec3::new(2.6, 6.4, 2.6);
    let rotation = Transform::from_translation(translation)
        .looking_at(origin + Vec3::new(-1.0, 0.2, -1.0), Vec3::Y)
        .rotation;
    (translation, rotation)
}

fn lerp_f(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    Color::srgb(
        lerp_f(a.red, b.red, t),
        lerp_f(a.green, b.green, t),
        lerp_f(a.blue, b.blue, t),
    )
}

#[cfg(test)]
mod tests {
    use observed_core::PlayerId;

    use super::*;

    #[test]
    fn initial_key_values_are_style_owned_targets() {
        for architecture in ArchitectureRegister::ALL {
            let palette = style::architecture(architecture);
            let key = primed_key_light(architecture);

            assert_eq!(key.color, palette.key_color);
            assert_eq!(
                key.intensity,
                palette.key_intensity * HEX_KEY_INTENSITY_SCALE
            );
            assert_eq!(key.range, palette.key_range);
            assert_eq!(key.radius, palette.key_radius);
            assert_eq!(key.inner_angle, palette.key_inner_angle);
            assert_eq!(key.outer_angle, palette.key_outer_angle);
            assert_eq!(key.shadows_enabled, palette.key_shadows_enabled);
        }
    }

    #[test]
    fn camera_is_primed_to_the_authoritative_eye_pose() {
        let player = observed_match::hex_wfc::HexPlayerState {
            id: PlayerId(0),
            cell: observed_facility::hex_wfc::HexCoord {
                q: 2,
                r: 3,
                level: 1,
            },
            position: Vec3::new(4.0, 8.5, -2.0),
            yaw: 0.7,
            pitch: -0.2,
            climb_target: None,
            transit_target: None,
            escaped: false,
        };
        let mut camera = Transform::default();
        prime_camera(&mut camera, &player);
        let (eye, rotation) = player_eye_pose(&player);

        assert_eq!(camera.translation, eye);
        assert_eq!(camera.rotation, rotation);
    }
}
