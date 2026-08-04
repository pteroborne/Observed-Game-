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
use crate::hex_wfc::sim::{EYE_OFFSET, HexWfcRuntime};
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

/// Where the overview's fog begins and ends, as fractions of the framing's far
/// plane. Far enough back that the whole facility is legible, close enough that
/// depth still reads across it.
const OVERVIEW_FOG_START: f32 = 0.45;
const OVERVIEW_FOG_END: f32 = 1.05;

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

/// How far behind and above the bot the spectator camera trails, in metres, and
/// how far it looks down. Matches the over-the-shoulder framing already proven
/// by `hex_wfc_lab`'s bot-POV capture.
const CHASE_BACK: f32 = 2.6;
const CHASE_RISE: f32 = 1.6;
const CHASE_PITCH: f32 = -0.26;
/// Fraction of the remaining gap the chase camera closes per second. The bot's
/// yaw is a simulation value that can still swing quickly; easing toward it
/// keeps the view readable without altering anything the simulation sees.
const CHASE_RESPONSE: f32 = 6.0;

pub(in crate::hex_wfc) fn sync_camera(
    runtime: Res<HexWfcRuntime>,
    spectating: Option<Res<crate::sim::state::SpectatorBot>>,
    overview: Option<Res<super::spectate::SpectatorOverview>>,
    time: Res<Time>,
    mut camera: Query<&mut Transform, With<GameCam>>,
    mut was_overviewing: Local<bool>,
) {
    let player = runtime.local();
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    if spectating.is_none() {
        // Human play is untouched: the eye pose, rigidly, every frame.
        let (eye, rotation) = player_eye_pose(player);
        transform.translation = eye;
        transform.rotation = rotation;
        return;
    }

    // Spectating: trail the bot instead of riding inside its head, and ease
    // rather than snap. A bot can still change heading faster than is
    // comfortable to watch from the first person, and this is presentation
    // only — no simulation state is read back or written.
    // Two spectator poses, one camera. The overview is further out and eases
    // more slowly; the chase is the close read. Which one is a question about
    // presentation, so it is answered here rather than by spawning a second
    // camera - Bevy hands UI to the highest-order camera on the window and
    // ignores `is_active`, so a dormant one would take every overlay with it.
    let overview = overview.filter(|overview| overview.active);
    let overviewing = overview.is_some();
    let iso = overview.as_ref().and_then(|overview| {
        super::spectate::framing_around(
            &runtime.match_state.facility,
            player.position,
            overview.detent,
            overview.tile_radius,
            1600.0,
            1000.0,
        )
    });
    if let Some(iso) = &iso
        && std::env::var("OBSERVED2_SPECTATE_TRACE").is_ok()
    {
        // One line a second while the trace is on. This exists because the
        // last attempt at this was debugged by reasoning and got it wrong: the
        // numbers are cheap and the guesses were not.
        if time.elapsed_secs() as u32 != (time.elapsed_secs() - time.delta_secs()) as u32 {
            let to_body = iso.translation.distance(player.position);
            info!(
                "spectate: body={:?} cam={:?} dist={to_body:.1} upp={:.3} far={:.1} fog={:.1}..{:.1}",
                player.position,
                iso.translation,
                iso.units_per_pixel,
                iso.far,
                iso.far * OVERVIEW_FOG_START,
                iso.far * OVERVIEW_FOG_END,
            );
        }
    }
    let (target_translation, target_rotation, response) = if let Some(iso) = &iso {
        (iso.translation, iso.rotation, super::spectate::response())
    } else {
        let forward = Vec3::new(player.yaw.sin(), 0.0, -player.yaw.cos());
        let eye = player.position + Vec3::Y * EYE_OFFSET;
        (
            eye + Vec3::Y * CHASE_RISE - forward * CHASE_BACK,
            Quat::from_rotation_y(-player.yaw) * Quat::from_rotation_x(CHASE_PITCH),
            CHASE_RESPONSE,
        )
    };
    // Snap on the way in and out of the overview, ease within it.
    //
    // Easing is right for following a body and for turning a detent, and wrong
    // for the mode change: the play pose and the overview pose are hundreds of
    // metres apart, so at 2.5 per second the camera is still ~50 m short a
    // second later - which reads as the whole facility sitting off-centre.
    let switched = *was_overviewing != overviewing;
    *was_overviewing = overviewing;
    if switched || transform.translation == Vec3::ZERO {
        transform.translation = target_translation;
        transform.rotation = target_rotation;
        return;
    }
    let t = (response * time.delta_secs()).clamp(0.0, 1.0);
    transform.translation = transform.translation.lerp(target_translation, t);
    transform.rotation = transform.rotation.slerp(target_rotation, t);
}

/// Perspective for play, orthographic for the overview.
///
/// An isometric read has to be orthographic: under perspective the far side of
/// the facility shrinks, parallel corridors converge, and the cutaway's whole
/// premise - that a wall's plan azimuth tells you whether it is in your way -
/// stops holding.
pub(in crate::hex_wfc) fn sync_projection(
    runtime: Res<HexWfcRuntime>,
    settings: Res<crate::settings::Settings>,
    overview: Option<Res<super::spectate::SpectatorOverview>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut projection: Query<&mut Projection, With<GameCam>>,
) {
    let Ok(mut projection) = projection.single_mut() else {
        return;
    };
    // Fit to the window that is actually there. `ScalingMode::WindowSize`
    // reads `scale` as world units per *pixel*, so framing against a nominal
    // viewport just means a wider window shows more world - the facility came
    // out a third of the frame with empty space around it. Only the scale
    // depends on the viewport; the camera's position and far plane do not.
    let (width, height) = windows.single().map_or((1600.0, 1000.0), |window| {
        (window.width().max(1.0), window.height().max(1.0))
    });
    let iso = overview
        .filter(|overview| overview.active)
        .and_then(|overview| {
            // `framing_around`, matching `sync_camera`. This called
            // `framing_fitted` - the whole facility - so the camera followed
            // the body while the *zoom* stayed set for all 340 m of building,
            // which is why the resident geometry came out a thumbnail in an
            // empty frame. Position and scale have to answer about the same
            // box or they disagree about what is being looked at.
            super::spectate::framing_around(
                &runtime.match_state.facility,
                runtime.local().position,
                overview.detent,
                overview.tile_radius,
                width,
                height,
            )
        });
    if let Some(iso) = iso {
        *projection = Projection::Orthographic(OrthographicProjection {
            scale: iso.units_per_pixel,
            near: 0.1,
            far: iso.far,
            ..OrthographicProjection::default_3d()
        });
        return;
    }
    if let Projection::Perspective(perspective) = &mut *projection {
        let target = settings.fov_degrees.clamp(50.0, 80.0).to_radians();
        if (perspective.fov - target).abs() > f32::EPSILON {
            perspective.fov = target;
        }
    } else {
        // Coming back from the overview: restore the play projection at the
        // player's chosen field of view rather than leaving them orthographic.
        *projection = Projection::Perspective(PerspectiveProjection {
            fov: settings.fov_degrees.clamp(50.0, 80.0).to_radians(),
            ..default()
        });
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
            && light.shadows_enabled
        {
            light.shadows_enabled = false;
        }
    }
    for &entity in &want {
        if let Ok((_, _, mut light)) = practicals.get_mut(entity)
            && !light.shadows_enabled
        {
            light.shadows_enabled = true;
        }
    }
    *shadowed = want;
}

pub(in crate::hex_wfc) fn sync_lighting_and_atmosphere(
    time: Res<Time>,
    runtime: Res<HexWfcRuntime>,
    overview: Option<Res<super::spectate::SpectatorOverview>>,
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
    let overview_active = overview.as_ref().is_some_and(|overview| overview.active);

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
    let overview_fog = overview
        .filter(|overview| overview.active)
        .and_then(|overview| {
            super::spectate::framing(&runtime.match_state.facility, overview.detent)
        })
        .map(|iso| (iso.far * OVERVIEW_FOG_START, iso.far * OVERVIEW_FOG_END));
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
        light.shadows_enabled = palette.key_shadows_enabled;
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
            assert_eq!(key.shadows_enabled, palette.key_shadows_enabled);
        }
    }

    #[test]
    fn camera_is_primed_to_the_authoritative_eye_pose() {
        let player = observed_match::hex_wfc::HexPlayerState {
            id: PlayerId(0),
            team: observed_core::TeamId(0),
            cell: observed_facility::hex_wfc::HexCoord {
                q: 2,
                r: 3,
                level: 1,
            },
            position: Vec3::new(4.0, 8.5, -2.0),
            yaw: 0.7,
            pitch: -0.2,
            escaped: false,
        };
        let mut camera = Transform::default();
        prime_camera(&mut camera, &player);
        let (eye, rotation) = player_eye_pose(&player);

        assert_eq!(camera.translation, eye);
        assert_eq!(camera.rotation, rotation);
    }
}
