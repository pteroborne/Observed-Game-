//! Deterministic local-light budget and district atmosphere for the hex facility.
//!
//! The rig is bounded: one district key spotlight staged above the runner's current
//! cell, one eye-follow headlamp so the interior always reads, and a district ambient +
//! distance-fog blend keyed by the cell's architecture. No per-cell practical swarm —
//! the authored tile hulls carry the surface detail; the lights carry the mood.

use bevy::prelude::*;
use observed_hex::hex_origin;
use observed_style::{self as style};

use super::{HexHeadlamp, HexWfcKeyLight};
use crate::GameState;
use crate::hex_wfc::sim::{EYE_OFFSET, HexWfcRuntime};
use crate::view::components::GameCam;

const BLEND_RATE: f32 = 2.5;

pub(super) fn spawn_rig(commands: &mut Commands) {
    commands.spawn((
        HexWfcKeyLight,
        DespawnOnExit(GameState::HexWfc),
        SpotLight {
            intensity: 0.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::default(),
        Name::new("budgeted hex key light"),
    ));
    commands.spawn((
        HexHeadlamp,
        DespawnOnExit(GameState::HexWfc),
        PointLight {
            // A close readability fill, not a second district key. The previous
            // 220k-lumen/42m light flattened nearby authored ramp hulls to white and
            // erased their slope; this bounded fill preserves normals and colour.
            intensity: 2_200.0,
            range: 14.0,
            color: Color::srgb(0.82, 0.91, 1.0),
            shadows_enabled: false,
            ..default()
        },
        Transform::default(),
        Name::new("hex eye headlamp"),
    ));
}

pub(in crate::hex_wfc) fn sync_camera_and_headlamp(
    runtime: Res<HexWfcRuntime>,
    mut camera: Query<&mut Transform, (With<GameCam>, Without<HexHeadlamp>)>,
    mut headlamp: Query<&mut Transform, (With<HexHeadlamp>, Without<GameCam>)>,
) {
    let player = runtime.local();
    let eye = player.position + Vec3::Y * EYE_OFFSET;
    let rotation = Quat::from_rotation_y(-player.yaw) * Quat::from_rotation_x(player.pitch);
    if let Ok(mut transform) = camera.single_mut() {
        transform.translation = eye;
        transform.rotation = rotation;
    }
    if let Ok(mut transform) = headlamp.single_mut() {
        transform.translation = eye + rotation * Vec3::new(0.0, 0.0, -0.4);
    }
}

pub(in crate::hex_wfc) fn sync_lighting_and_atmosphere(
    time: Res<Time>,
    runtime: Res<HexWfcRuntime>,
    mut ambient: ResMut<GlobalAmbientLight>,
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

    if let Ok(mut fog) = camera.single_mut() {
        fog.color = lerp_color(fog.color, palette.fog_color, t);
        if let bevy::pbr::FogFalloff::Linear { start, end } = &mut fog.falloff {
            *start = lerp_f(*start, palette.fog_start, t);
            *end = lerp_f(*end, palette.fog_end, t);
        }
    }

    let origin = Vec3::from_array(hex_origin(current));
    if let Ok((mut light, mut transform)) = key.single_mut() {
        let target_translation = origin + Vec3::new(2.6, 6.4, 2.6);
        let target_rotation = Transform::from_translation(target_translation)
            .looking_at(origin + Vec3::new(-1.0, 0.2, -1.0), Vec3::Y)
            .rotation;
        if transform.translation == Vec3::ZERO {
            transform.translation = target_translation;
            transform.rotation = target_rotation;
        } else {
            transform.translation = transform.translation.lerp(target_translation, t);
            transform.rotation = transform.rotation.slerp(target_rotation, t);
        }
        let target_color = lerp_color(light.color, palette.key_color, t);
        light.color = target_color;
        light.intensity = lerp_f(light.intensity, palette.key_intensity * 0.62, t);
        light.range = lerp_f(light.range, palette.key_range, t);
        light.radius = lerp_f(light.radius, palette.key_radius, t);
        light.inner_angle = lerp_f(light.inner_angle, palette.key_inner_angle, t);
        light.outer_angle = lerp_f(light.outer_angle, palette.key_outer_angle, t);
        light.shadows_enabled = palette.key_shadows_enabled;
    }
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
