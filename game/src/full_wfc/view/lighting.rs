use std::collections::BTreeSet;

use bevy::light::{FogVolume, VolumetricFog, VolumetricLight};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::full_wfc::{CellCoord, ModuleFace};
use observed_match::full_wfc::{GameplayEventKind, MatchStatus, WALL_HEIGHT, cell_origin};
use observed_style::{self as style, MarkerRole};

use super::{CandleLight, CellPractical, FullWfcFogVolume, FullWfcKeyLight};
use crate::GameState;
use crate::full_wfc::sim::{EYE_OFFSET, FullWfcRuntime};
use crate::full_wfc::{FullWfcCapture, FullWfcCaptureMode};
use crate::view::components::GameCam;

const DISTRICT_BLEND_RATE: f32 = 2.5;

type KeyLightQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut SpotLight,
        &'static mut Transform,
        Has<VolumetricLight>,
    ),
    (With<FullWfcKeyLight>, Without<FullWfcFogVolume>),
>;
type FogVolumeQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut FogVolume,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    (With<FullWfcFogVolume>, Without<FullWfcKeyLight>),
>;

pub(super) fn spawn_rig(commands: &mut Commands) {
    commands.spawn((
        FullWfcKeyLight,
        DespawnOnExit(GameState::FullWfc),
        SpotLight {
            intensity: 0.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::default(),
        Name::new("budgeted full-WFC key light"),
    ));
    commands.spawn((
        FullWfcFogVolume,
        DespawnOnExit(GameState::FullWfc),
        FogVolume {
            density_factor: 0.0,
            absorption: 0.32,
            scattering: 0.18,
            ..default()
        },
        Transform::default(),
        Visibility::Inherited,
        Name::new("Facet Monument bounded shaft air"),
    ));
    commands.spawn((
        CandleLight,
        DespawnOnExit(GameState::FullWfc),
        PointLight {
            color: style::marker(MarkerRole::NextRoom).base_color,
            intensity: 220.0,
            range: 9.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::default(),
        Name::new("A* proximity candle"),
    ));
}

pub(in crate::full_wfc) fn sync_camera_and_candle(
    runtime: Res<FullWfcRuntime>,
    capture: Option<Res<FullWfcCapture>>,
    mut camera: Query<&mut Transform, (With<GameCam>, Without<CandleLight>)>,
    mut candle: Query<(&mut Transform, &mut PointLight), With<CandleLight>>,
) {
    let player = runtime.local();
    let f = Vec3::new(player.yaw.sin(), 0.0, -player.yaw.cos());
    let (sp, cp) = player.pitch.sin_cos();
    let look_dir = Vec3::new(f.x * cp, sp, f.z * cp);
    let rotation = Transform::from_translation(Vec3::ZERO)
        .looking_to(look_dir, Vec3::Y)
        .rotation;
    let pressure = runtime.match_state.guardian_pressure(runtime.local_player);
    let style_focus = capture
        .as_deref()
        .filter(|capture| capture.mode == FullWfcCaptureMode::Style)
        .map(|capture| super::presentation_focus(&runtime, Some(capture)));
    if let Ok(mut transform) = camera.single_mut() {
        if let Some(focus) = style_focus {
            let origin = cell_origin(focus);
            let placement = runtime.match_state.facility.placement(focus);
            let face = placement
                .and_then(|placement| {
                    [
                        ModuleFace::North,
                        ModuleFace::East,
                        ModuleFace::South,
                        ModuleFace::West,
                    ]
                    .into_iter()
                    .find(|face| placement.is_open(*face))
                })
                .unwrap_or(ModuleFace::North);
            let direction = match face {
                ModuleFace::East => Vec3::X,
                ModuleFace::West => Vec3::NEG_X,
                ModuleFace::South => Vec3::Z,
                ModuleFace::North => Vec3::NEG_Z,
                ModuleFace::Up | ModuleFace::Down => Vec3::NEG_Z,
            };
            *transform =
                Transform::from_translation(origin - direction * 3.4 + Vec3::Y * EYE_OFFSET * 2.35)
                    .looking_at(origin + direction * 4.5 + Vec3::Y * 1.9, Vec3::Y);
        } else {
            transform.translation = player.position + Vec3::Y * EYE_OFFSET;
            transform.rotation = rotation;
        }
    }
    let proximity = runtime.match_state.facility.candle_proximity(player.cell);
    let warning = runtime.match_state.mutation_warning_progress();
    if let Ok((mut transform, mut light)) = candle.single_mut() {
        if style_focus.is_some() {
            light.intensity = 0.0;
            return;
        }
        let forward = rotation * Vec3::NEG_Z;
        let right = rotation * Vec3::X;
        transform.translation =
            player.position + Vec3::Y * EYE_OFFSET + forward * 0.35 + right * 0.28 - Vec3::Y * 0.28;
        let guardian_flicker = if pressure > 0.0 && runtime.match_state.tick % 17 < 3 {
            0.35
        } else {
            1.0
        };
        let breathing = 1.0 + (warning * std::f32::consts::TAU * 2.0).sin().abs() * 0.28;
        let mutation_cut = if runtime
            .match_state
            .recent_events
            .iter()
            .any(|event| event.kind == GameplayEventKind::MutationCommitted)
        {
            0.08
        } else {
            1.0
        };
        light.intensity = (180.0 + proximity.powf(1.6) * 2_300.0)
            * (1.0 - pressure * 0.62)
            * guardian_flicker
            * breathing
            * mutation_cut;
        light.range = (8.0 + proximity * 11.0) * (1.0 - pressure * 0.55);
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::full_wfc) fn sync_lighting_and_atmosphere(
    time: Res<Time>,
    runtime: Res<FullWfcRuntime>,
    capture: Option<Res<FullWfcCapture>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut camera: Query<(Entity, &mut DistanceFog, Has<VolumetricFog>), With<GameCam>>,
    mut key: KeyLightQuery,
    mut volume: FogVolumeQuery,
    mut practicals: Query<(&CellPractical, &mut PointLight)>,
    mut commands: Commands,
) {
    let current = super::presentation_focus(&runtime, capture.as_deref());
    let Some(placement) = runtime.match_state.facility.placement(current) else {
        return;
    };
    let mut palette = style::architecture(placement.architecture);
    if matches!(runtime.match_state.status, MatchStatus::Countdown { .. }) {
        palette = style::klaxon_modulate(palette);
    }
    let pressure = runtime.match_state.guardian_pressure(runtime.local_player);
    let warning = runtime.match_state.mutation_warning_progress();
    let t = (time.delta_secs() * DISTRICT_BLEND_RATE).clamp(0.0, 1.0);
    ambient.color = lerp_color(ambient.color, palette.ambient_color, t);
    ambient.brightness = lerp_f(ambient.brightness, palette.ambient_brightness, t);

    let warning_pulse = (warning * std::f32::consts::TAU * 2.0).sin().abs();
    let fog_start = (palette.fog_start - pressure * 3.0).max(style::DISTRICT_MIN_FOG_START);
    let fog_end = (palette.fog_end - pressure * 10.0 - warning_pulse * 2.5).max(fog_start + 10.1);
    let wants_volumetric = placement.architecture == ArchitectureRegister::FacetMonument;
    if let Ok((camera_entity, mut fog, has_volumetric)) = camera.single_mut() {
        fog.color = lerp_color(fog.color, palette.fog_color, t);
        if let FogFalloff::Linear { start, end } = &mut fog.falloff {
            *start = lerp_f(*start, fog_start, t);
            *end = lerp_f(*end, fog_end, t);
        }
        if wants_volumetric && !has_volumetric {
            commands.entity(camera_entity).insert(VolumetricFog {
                ambient_intensity: 0.0,
                ..default()
            });
        } else if !wants_volumetric && has_volumetric {
            commands.entity(camera_entity).remove::<VolumetricFog>();
        }
    }

    let origin = cell_origin(current);
    if let Ok((entity, mut light, mut transform, has_volumetric)) = key.single_mut() {
        let local_key = if wants_volumetric {
            Vec3::new(0.0, WALL_HEIGHT - 0.20, 0.0)
        } else {
            Vec3::new(3.25, WALL_HEIGHT - 0.45, 3.25)
        };
        let target_translation = origin + local_key;
        let target_aim = if wants_volumetric {
            origin + Vec3::Y * 0.1
        } else {
            origin + Vec3::new(-1.1, 0.2, -1.1)
        };
        let target_rotation = Transform::from_translation(target_translation)
            .looking_at(target_aim, Vec3::Y)
            .rotation;

        if transform.translation == Vec3::ZERO {
            transform.translation = target_translation;
            transform.rotation = target_rotation;
        } else {
            transform.translation = transform.translation.lerp(target_translation, t);
            transform.rotation = transform.rotation.slerp(target_rotation, t);
        }

        let threat = style::marker(MarkerRole::Collapse).base_color;
        let target_color = lerp_color(palette.key_color, threat, pressure * 0.42);
        light.color = lerp_color(light.color, target_color, t);

        let key_scale = if wants_volumetric { 0.16 } else { 0.68 };
        let target_intensity = palette.key_intensity * key_scale * (1.0 - pressure * 0.20);
        light.intensity = lerp_f(light.intensity, target_intensity, t);

        light.range = lerp_f(light.range, palette.key_range, t);
        light.radius = lerp_f(light.radius, palette.key_radius, t);
        light.inner_angle = lerp_f(light.inner_angle, palette.key_inner_angle, t);
        light.outer_angle = lerp_f(light.outer_angle, palette.key_outer_angle, t);
        light.shadows_enabled = palette.key_shadows_enabled;

        if wants_volumetric && !has_volumetric {
            commands.entity(entity).insert(VolumetricLight);
        } else if !wants_volumetric && has_volumetric {
            commands.entity(entity).remove::<VolumetricLight>();
        }
    }
    if let Ok((mut fog, mut transform, mut visibility)) = volume.single_mut() {
        let target_fog_color = lerp_color(palette.fog_color, palette.light_color, 0.18);
        fog.fog_color = lerp_color(fog.fog_color, target_fog_color, t);

        let target_translation = origin + Vec3::Y * (WALL_HEIGHT * 0.5);
        let target_scale = Vec3::new(8.0, WALL_HEIGHT, 8.0);

        if transform.translation == Vec3::ZERO {
            transform.translation = target_translation;
            transform.scale = target_scale;
        } else {
            transform.translation = transform.translation.lerp(target_translation, t);
            transform.scale = transform.scale.lerp(target_scale, t);
        }

        let target_density = if wants_volumetric { 0.055 } else { 0.0 };
        fog.density_factor = lerp_f(fog.density_factor, target_density, t);

        *visibility = Visibility::Inherited;
    }

    let active = active_light_cells(&runtime.match_state.facility, current);
    let tick_time = runtime.match_state.tick as f32 * 0.04;
    let mutation_cut = if runtime
        .match_state
        .recent_events
        .iter()
        .any(|event| event.kind == GameplayEventKind::MutationCommitted)
    {
        0.12
    } else {
        1.0
    };
    for (practical, mut light) in &mut practicals {
        let (target_intensity, target_color) = if active.contains(&practical.cell) {
            let current_scale = if practical.cell == current { 1.0 } else { 0.52 };
            let mut practical_palette = style::architecture(practical.architecture);
            if matches!(runtime.match_state.status, MatchStatus::Countdown { .. }) {
                practical_palette = style::klaxon_modulate(practical_palette);
            }
            let wave = (tick_time * 3.7 + practical.phase).sin()
                + 0.55 * (tick_time * 7.1 + practical.phase * 1.7).sin();
            let flicker = if wave > 1.12 {
                0.38 + 0.62 * (tick_time * 31.0 + practical.phase).sin().abs()
            } else {
                1.0
            };
            let intensity = 2_600.0
                * current_scale
                * practical.detail
                * flicker
                * (1.0 - pressure * 0.28)
                * mutation_cut;

            light.range = if practical_palette.pools_rhythm {
                7.5
            } else {
                11.0
            };
            light.shadows_enabled = false;

            (intensity, practical_palette.light_color)
        } else {
            (0.0, light.color)
        };

        light.color = lerp_color(light.color, target_color, t);
        light.intensity = lerp_f(light.intensity, target_intensity, t);
    }
}

fn active_light_cells(
    world: &observed_facility::full_wfc::FullWfcWorld,
    current: CellCoord,
) -> BTreeSet<CellCoord> {
    let mut active = BTreeSet::from([current]);
    let Some(placement) = world.placement(current) else {
        return active;
    };
    for face in ModuleFace::ALL {
        if !placement.is_open(face) {
            continue;
        }
        if let Some(neighbor) = world.config.neighbor(current, face)
            && world.placement(neighbor).is_some_and(|placement| {
                placement.space != observed_facility::full_wfc::ModuleSpace::Void
            })
        {
            active.insert(neighbor);
        }
    }
    debug_assert!(active.len() <= 7);
    active
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
    use observed_facility::full_wfc::{FullWfcConfig, FullWfcWorld};

    use super::*;

    #[test]
    fn practical_budget_never_exceeds_current_plus_six_neighbors() {
        let world = FullWfcWorld::new(7, FullWfcConfig::default()).expect("world");
        for placement in world.placements.values() {
            assert!(active_light_cells(&world, placement.coord).len() <= 7);
        }
    }
}
