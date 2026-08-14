//! Lighting-lab register rig for the hex facility (Arc I "Light & Line" language).
//!
//! Three staged tiers, all driven by the per-register `observed_style` palette — the
//! artifact into which the lighting lab's findings were transferred as parameters:
//!   1. a shadow-casting **district key** spotlight over the runner's current cell,
//!      giving each register its dramatic directional read (overlit-grid alone runs it
//!      flat, `key_shadows_enabled = false`);
//!   2. per-cell **practical pools** (see [`super::shell`]) tinted by the cell's
//!      `light_color`, staged as pools-in-dark on `pools_rhythm` registers (places lit,
//!      connective halls dark) or as an even fill elsewhere;
//!   3. district **ambient + distance fog** for depth.
//!
//! There is deliberately no eye-follow headlamp: a flat player-locked fill washed out
//! the very shadows this rig exists to cast. The caged lantern remains the only
//! discretionary player-following light, so spending the last one still has a cost.

use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_hex::hex_origin;
use observed_style::{self as style, HexComposition};

use super::{HexPractical, HexWfcKeyLight};
use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;
use crate::view::components::GameCam;

/// Per-tile fill fixtures allowed to cast shadows at once (the district key casts on top
/// of this). Bounded because point-light shadows are six-face cubemaps; kept small to
/// hold GPU margin while still giving real cast-shadow contrast around the runner.
const PRACTICAL_SHADOW_BUDGET: usize = 4;

const BLEND_RATE: f32 = 2.5;
/// Hex cells are somewhat tighter than the teleport-era rooms that established the
/// style palette's absolute lumen values, so the shadow-casting key is trimmed a little
/// to keep a nearby wall in material contrast without blowing out. Value sits in the
/// proven `full_wfc` key range (`0.16..=0.68`); the per-cell practicals in
/// [`super::shell`] now carry the interior read the deleted eye headlamp used to fake.
const HEX_KEY_INTENSITY_SCALE: f32 = 0.62;

/// Spawn the complete semantic rig at its final treatment for the initial cell.
///
/// Phase 95 spawned a default-white, zero-intensity key and eased it toward the
/// current register. That made the first visible seconds desaturated. Initial state is
/// not a transition: every light starts at the exact `observed_style` target, while
/// [`sync_lighting_and_atmosphere`] retains easing for later cell changes.
pub(super) fn spawn_rig(
    commands: &mut Commands,
    architecture: ArchitectureRegister,
    composition: HexComposition,
    current: observed_facility::hex_wfc::HexCoord,
    player: &observed_match::hex_wfc::HexPlayerState,
) {
    let _ = player;
    let (key_translation, key_rotation) = key_pose(current);
    commands.spawn((
        HexWfcKeyLight,
        DespawnOnExit(GameState::HexWfc),
        primed_key_light(architecture, composition),
        Transform::from_translation(key_translation).with_rotation(key_rotation),
        Name::new("budgeted hex key light"),
    ));
}

fn primed_key_light(architecture: ArchitectureRegister, composition: HexComposition) -> SpotLight {
    let palette = style::architecture_for_composition(architecture, composition);
    SpotLight {
        color: palette.key_color,
        intensity: palette.key_intensity * HEX_KEY_INTENSITY_SCALE,
        range: palette.key_range,
        radius: palette.key_radius,
        inner_angle: palette.key_inner_angle,
        outer_angle: palette.key_outer_angle,
        shadow_maps_enabled: palette.key_shadows_enabled,
        ..default()
    }
}

/// Enable shadows on the [`HexPractical`] downlights nearest the runner and disable the
/// rest — the lighting lab's "per-place shadow-casting staging". Recomputed only when the
/// runner's cell changes, so cast-shadow contrast follows the player across every tile
/// without paying for a shadow map on all ~thousands of fixtures.
pub(in crate::hex_wfc) fn sync_practical_shadow_budget(
    runtime: Res<HexWfcRuntime>,
    mut last_cell: Local<Option<observed_facility::hex_wfc::HexCoord>>,
    mut shadowed: Local<Vec<Entity>>,
    mut practicals: Query<(Entity, &HexPractical, &mut PointLight)>,
) {
    let current = runtime.local().cell;
    if *last_cell == Some(current) {
        return;
    }
    *last_cell = Some(current);
    let focus = runtime.local().position;

    // Nearest fixtures by squared distance to the runner (small budget → cheap select).
    let mut ranked: Vec<(f32, Entity)> = practicals
        .iter()
        .map(|(entity, practical, _)| {
            (
                Vec3::from_array(hex_origin(practical.0)).distance_squared(focus),
                entity,
            )
        })
        .collect();
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
    let want: Vec<Entity> = ranked
        .into_iter()
        .take(PRACTICAL_SHADOW_BUDGET)
        .map(|(_, entity)| entity)
        .collect();

    // Turn off any fixture that was casting and is no longer chosen, then turn on the
    // chosen set. Guarded assignments keep change detection quiet on the steady state.
    for entity in std::mem::take(&mut *shadowed) {
        if !want.contains(&entity)
            && let Ok((_, _, mut light)) = practicals.get_mut(entity)
            && light.shadow_maps_enabled
        {
            light.shadow_maps_enabled = false;
        }
    }
    for &entity in &want {
        if let Ok((_, _, mut light)) = practicals.get_mut(entity)
            && !light.shadow_maps_enabled
        {
            light.shadow_maps_enabled = true;
        }
    }
    *shadowed = want;
}

pub(in crate::hex_wfc) fn sync_lighting_and_atmosphere(
    time: Res<Time>,
    runtime: Res<HexWfcRuntime>,
    frame: Res<super::camera::OverviewFrame>,
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
    let composition = composition_at(&runtime.match_state.facility, current);
    let palette = style::architecture_for_composition(architecture, composition);
    let t = (time.delta_secs() * BLEND_RATE).clamp(0.0, 1.0);
    let overview_active = frame.0.is_some();

    ambient.color = lerp_color(ambient.color, palette.ambient_color, t);
    // A cut-open interior needs fill or it is a black hole with one bright
    // spot. Play's ambient is tuned for a body standing inside a lit pool; the
    // overview is looking at a dozen opened tiles at once, so it takes the
    // studio's fill - the same view of the same building, so the same answer.
    ambient.brightness = if overview_active {
        observed_style::iso::light::AMBIENT_BRIGHTNESS
    } else {
        lerp_f(ambient.brightness, palette.ambient_brightness, t)
    };
    clear.0 = lerp_color(clear.0, palette.fog_color, t);

    // The overview stands hundreds of metres out; play fog is tuned for 10 to
    // 28 m. Eased toward the palette from up there, every pixel is 100 percent
    // fog and the view is a flat sheet of `fog_color` - which is exactly what
    // "spectate mode seems blank" was. Depth cue and total occlusion are the
    // same setting at different scales, so the overview gets its own scale
    // rather than losing the atmosphere entirely.
    let overview_fog = frame.0.map(|iso| {
        (
            iso.far * super::camera::OVERVIEW_FOG_START,
            iso.far * super::camera::OVERVIEW_FOG_END,
        )
    });
    if let Ok(mut fog) = camera.single_mut() {
        fog.color = lerp_color(fog.color, palette.fog_color, t);
        if let bevy::pbr::FogFalloff::Linear { start, end } = &mut fog.falloff {
            let (target_start, target_end) =
                overview_fog.unwrap_or((palette.fog_start, palette.fog_end));
            // Snapped, not eased: easing across two orders of magnitude leaves
            // the view blank for the second it takes to arrive.
            if overview_fog.is_some() {
                *start = target_start;
                *end = target_end;
            } else {
                *start = lerp_f(*start, target_start, t);
                *end = lerp_f(*end, target_end, t);
            }
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
        light.shadow_maps_enabled = palette.key_shadows_enabled;
    }
}

pub(super) fn composition_at(
    world: &observed_facility::hex_wfc::HexWfcWorld,
    coord: observed_facility::hex_wfc::HexCoord,
) -> HexComposition {
    use observed_facility::hex_wfc::HexArchetype;

    if world
        .blueprints
        .iter()
        .any(|blueprint| blueprint.cells.contains(&coord))
    {
        return HexComposition::Room;
    }
    match world
        .placements
        .get(&coord)
        .map(|placement| placement.archetype)
    {
        Some(HexArchetype::Room | HexArchetype::Expanse) => HexComposition::Room,
        Some(HexArchetype::RampUp | HexArchetype::RampHead | HexArchetype::Shaft) => {
            HexComposition::Vertical
        }
        _ => HexComposition::Hall,
    }
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
    use super::*;
    use observed_content::ArchitectureRegister;
    use observed_style::{self as style, HexComposition};

    #[test]
    fn initial_key_values_are_style_owned_targets() {
        for architecture in ArchitectureRegister::ALL {
            let palette = style::architecture_for_composition(architecture, HexComposition::Hall);
            let key = primed_key_light(architecture, HexComposition::Hall);

            assert_eq!(key.color, palette.key_color);
            assert_eq!(
                key.intensity,
                palette.key_intensity * HEX_KEY_INTENSITY_SCALE
            );
            assert_eq!(key.range, palette.key_range);
            assert_eq!(key.radius, palette.key_radius);
            assert_eq!(key.inner_angle, palette.key_inner_angle);
            assert_eq!(key.outer_angle, palette.key_outer_angle);
            assert_eq!(key.shadow_maps_enabled, palette.key_shadows_enabled);
        }
    }
}
