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

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_hex::hex_origin;
use observed_style::{self as style, HexComposition};

use super::spectate::Cutaway;
use super::{HexPractical, HexWfcKeyLight};
use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;
use crate::view::components::GameCam;

/// Per-tile fill fixtures allowed to cast shadows at once (the district key casts on top
/// of this). Bounded because point-light shadows are six-face cubemaps; kept small to
/// hold GPU margin while still giving real cast-shadow contrast around the runner.
///
/// Three since the climb compositions. Measured on the Phase 101 arc gate (2026-10-02):
/// each shadowed fixture redraws every caster within its 14 m range into six faces, about
/// 2.5 ms a frame, and the shadow map's size makes no difference - it is draws, not fill.
/// Climbs put half as many cells again within reach, and four took frame p95 to 18.3 ms
/// against a 16.7 ms budget. Three: 14.0 ms. Two: 10.2 ms. None: 6.9 ms.
///
/// Open, seen while choosing three: in the verticals capture's `backrooms_up` pose, a
/// hall fixture 12 m behind the camera, near the edge of its 14 m range, lights the
/// floor ahead only while it casts shadows. That holds under CPU and GPU light
/// clustering alike, and with no clustering resize in the log, so it looks engine-side.
/// At four it happens to be in the budget, and the floor there is brighter.
const PRACTICAL_SHADOW_BUDGET: usize = 3;

const BLEND_RATE: f32 = 2.5;
/// The key trim, which now lives in `observed_style` beside the palette it
/// trims: a preview that reproduces this rig needs the same number or it is
/// previewing a different building. The per-cell practicals in [`super::shell`]
/// carry the interior read the deleted eye headlamp used to fake.
use observed_style::HEX_KEY_INTENSITY_SCALE;

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

/// A drawn mesh that has just streamed in.
type JustStreamed = (With<Mesh3d>, Added<Cutaway>);

/// Only geometry on the viewed body's storey and above casts shadows.
///
/// Every light that casts sits in or above that storey: the district key hangs 6.4 m up
/// in the body's own cell, and the moon is overhead. A cell below can shadow only itself
/// and what is lower still, which the storey's own floor hides from the key and which a
/// body standing on it does not see; but it was rendered into every shadow map anyway,
/// and since the climb compositions a storey has about half as many cells again within
/// reach. Cells above keep casting: an overhang shading a moonlit loggia is a shadow
/// a body sees.
///
/// Re-tagged whole when the storey changes, and otherwise only what has just streamed in.
pub(in crate::hex_wfc) fn sync_storey_shadow_casters(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    mut last_level: Local<Option<u8>>,
    added: Query<(Entity, &Cutaway), JustStreamed>,
    meshes: Query<(Entity, &Cutaway, Has<NotShadowCaster>), With<Mesh3d>>,
) {
    let level = runtime.viewed().cell.level;
    let casts = |cutaway: &Cutaway| cutaway.cell_level >= level;
    if *last_level != Some(level) {
        *last_level = Some(level);
        for (entity, cutaway, silent) in &meshes {
            match (casts(cutaway), silent) {
                (true, true) => {
                    commands.entity(entity).remove::<NotShadowCaster>();
                }
                (false, false) => {
                    commands.entity(entity).insert(NotShadowCaster);
                }
                _ => {}
            }
        }
        return;
    }
    for (entity, cutaway) in &added {
        if !casts(cutaway) {
            commands.entity(entity).insert(NotShadowCaster);
        }
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
    mut practicals: Query<(
        Entity,
        &HexPractical,
        &mut PointLight,
        Option<&GlobalTransform>,
    )>,
    streamed: Query<(), Added<HexPractical>>,
) {
    let current = runtime.viewed().cell;
    // Again when fixtures stream in, as well as when the runner moves: a cell that
    // arrives after the runner does would otherwise wait for its next step.
    if *last_cell == Some(current) && streamed.is_empty() {
        return;
    }
    *last_cell = Some(current);
    let focus = crate::hex_wfc::ascent::presented_position(runtime.viewed());

    // Nearest fixtures by squared distance to the runner (small budget → cheap select).
    let mut ranked: Vec<(f32, Entity)> = practicals
        .iter()
        .filter(|(_, practical, ..)| {
            let register = runtime
                .match_state
                .facility
                .architecture
                .get(&practical.0)
                .copied()
                .unwrap_or(ArchitectureRegister::ALL[0]);
            style::hex_practical_light(register, HexComposition::Hall, 1).shadows_allowed
        })
        // By where each fixture is, not its cell: a cell's fixtures share its origin,
        // so ranked by cell they tied, and the tie went by entity - two lights at 10 and
        // 12 m kept their shadows while one at 8 m, right by the runner, had none.
        .map(|(entity, practical, _, placed)| {
            let at = placed.map_or_else(
                || Vec3::from_array(hex_origin(practical.0)),
                GlobalTransform::translation,
            );
            (at.distance_squared(focus), entity)
        })
        .collect();
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let want: Vec<Entity> = ranked
        .into_iter()
        .take(PRACTICAL_SHADOW_BUDGET)
        .map(|(_, entity)| entity)
        .collect();

    // Turn off any fixture that was casting and is no longer chosen, then turn on the
    // chosen set. Guarded assignments keep change detection quiet on the steady state.
    for entity in std::mem::take(&mut *shadowed) {
        if !want.contains(&entity)
            && let Ok((_, _, mut light, _)) = practicals.get_mut(entity)
            && light.shadow_maps_enabled
        {
            light.shadow_maps_enabled = false;
        }
    }
    for &entity in &want {
        if let Ok((_, _, mut light, _)) = practicals.get_mut(entity)
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
    // What the view stands in: the overview's frame, if any, and the floor's sky.
    (frame, sky): (
        Res<super::camera::OverviewFrame>,
        Option<Res<super::sky::HexSky>>,
    ),
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    mut camera: Query<&mut DistanceFog, With<GameCam>>,
    mut key: Query<(&mut SpotLight, &mut Transform), With<HexWfcKeyLight>>,
) {
    let current = runtime.viewed().cell;
    let architecture = runtime
        .match_state
        .facility
        .architecture
        .get(&current)
        .copied()
        .unwrap_or(observed_content::ArchitectureRegister::ALL[0]);
    let composition = composition_at(&runtime.match_state.facility, current);
    let night = style::open_air::night();
    let palette = outdoors_if_open(
        &runtime.match_state.facility,
        current,
        style::architecture_for_composition(architecture, composition),
        sky.as_deref().map_or(&night, |sky| &sky.now),
    );
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

/// A hall that opens onto the outside is outdoors: its fog reaches across the air and
/// fades into the floor's horizon rather than into the dark, and the light comes from its
/// sky. Everywhere else keeps its district palette, tuned for a body in a corridor.
pub(super) fn outdoors_if_open(
    world: &observed_facility::hex_wfc::HexWfcWorld,
    coord: observed_facility::hex_wfc::HexCoord,
    palette: style::DistrictPalette,
    sky: &style::open_air::SkyMood,
) -> style::DistrictPalette {
    if observed_match::hex_wfc::open_edges(world, coord).is_some() {
        style::open_air::open_air_under(palette, sky)
    } else {
        palette
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
        // A climb is the facility's vertical circulation: lit to stay readable the
        // whole length of its flight.
        Some(HexArchetype::Climb { .. }) => HexComposition::Vertical,
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
    fn nearby_shadow_budget_does_not_enable_noon_practical_shadows() {
        use observed_match::hex_wfc::{HexBotDriver, HexMatchConfig, HexWfcMatch};
        use std::collections::{BTreeMap, BTreeSet};
        let mut game = HexWfcMatch::new(
            44,
            HexMatchConfig::default(),
            &crate::hex_wfc::sim::load_prototypes(),
        )
        .expect("fixture solves");
        let local_player = *game.players.keys().next().expect("player");
        let noon = game.players[&local_player].cell;
        let other = observed_hex::HexCoord {
            q: noon.q + 1,
            ..noon
        };
        game.facility
            .architecture
            .insert(noon, ArchitectureRegister::OverlitGrid);
        game.facility
            .architecture
            .insert(other, ArchitectureRegister::Monolith);
        let mut app = App::new();
        app.insert_resource(HexWfcRuntime {
            match_state: game,
            bot_driver: HexBotDriver::new(),
            local_player,
            pending_visual_cells: BTreeSet::new(),
            presented_revisions: BTreeMap::new(),
            status: String::new(),
            map_open: false,
            map_level: noon.level,
            results_delay_frames: 0,
            networked: false,
            resync_attempts: 0,
            ascent: None,
            viewed_player: None,
        })
        .add_systems(Update, sync_practical_shadow_budget);
        for _ in 0..4 {
            app.world_mut()
                .spawn((HexPractical(noon), PointLight::default()));
        }
        app.world_mut()
            .spawn((HexPractical(other), PointLight::default()));
        for current in [noon, other] {
            app.world_mut()
                .resource_mut::<HexWfcRuntime>()
                .match_state
                .players
                .get_mut(&local_player)
                .expect("player")
                .cell = current;
            app.update();
            let world = app.world_mut();
            for (fixture, light) in world.query::<(&HexPractical, &PointLight)>().iter(world) {
                assert_eq!(light.shadow_maps_enabled, fixture.0 == other);
            }
        }
    }

    /// The shadows go to the fixtures nearest the runner by where each one is, not by
    /// its cell: every fixture in a cell shares the cell's origin, so ranked by cell they
    /// tied, and the farther ones of a cell could take the shadows from the nearest.
    #[test]
    fn the_nearest_fixtures_in_one_cell_get_the_shadows() {
        use observed_match::hex_wfc::{HexBotDriver, HexMatchConfig, HexWfcMatch};
        use std::collections::{BTreeMap, BTreeSet};
        let mut game = HexWfcMatch::new(
            44,
            HexMatchConfig::default(),
            &crate::hex_wfc::sim::load_prototypes(),
        )
        .expect("fixture solves");
        let local_player = *game.players.keys().next().expect("player");
        let cell = game.players[&local_player].cell;
        game.facility
            .architecture
            .insert(cell, ArchitectureRegister::Monolith);
        let focus = crate::hex_wfc::ascent::presented_position(&game.players[&local_player]);
        let mut app = App::new();
        app.insert_resource(HexWfcRuntime {
            match_state: game,
            bot_driver: HexBotDriver::new(),
            local_player,
            pending_visual_cells: BTreeSet::new(),
            presented_revisions: BTreeMap::new(),
            status: String::new(),
            map_open: false,
            map_level: cell.level,
            results_delay_frames: 0,
            networked: false,
            resync_attempts: 0,
            ascent: None,
            viewed_player: None,
        })
        .add_systems(Update, sync_practical_shadow_budget);
        // Spawned farthest first, so entity order would pick the farthest on a tie.
        let count = PRACTICAL_SHADOW_BUDGET + 2;
        let mut by_distance = BTreeMap::new();
        for step in (0..count).rev() {
            #[allow(clippy::cast_precision_loss)]
            let at = focus + Vec3::X * (1.0 + step as f32);
            let entity = app
                .world_mut()
                .spawn((
                    HexPractical(cell),
                    PointLight::default(),
                    GlobalTransform::from_translation(at),
                ))
                .id();
            by_distance.insert(step, entity);
        }
        app.update();
        for (step, entity) in by_distance {
            let light = app.world().get::<PointLight>(entity).expect("a light");
            assert_eq!(
                light.shadow_maps_enabled,
                step < PRACTICAL_SHADOW_BUDGET,
                "the fixture {} m out",
                step + 1
            );
        }
    }

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
