use bevy::light::VolumetricFog;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use observed_match::facility::CollapseState;
use observed_match::hybrid::HybridMatch;
use observed_style as style;

use crate::flow::MATCH_SEED;
use crate::screens::audio::AudioDirector;
use crate::sim::director::MatchDirector;
use crate::sim::state::{MatchPaused, TeleportState};
use crate::teleport::Place;
use crate::view::assets::MatchAssets;
use crate::view::components::{
    DecohereFx, DoorLeaf, FlickerLight, GameCam, GameSun, MENU_SUN_ILLUMINANCE, MatchAudioCue,
};

/// How long (seconds) the first-person decoherence feedback — the diegetic light flicker
/// and door slam — lasts after a reroute commits. Shared so the flicker driver
/// ([`flicker_lights`]) and the feedback driver ([`sync_decohere_fx`]) agree.
pub(crate) const ROUTE_SHIFT_FLASH_SECS: f32 = 0.7;

/// Ease the global ambient fill and the camera's distance fog toward the current place's
/// district palette each frame, giving the megastructure visibly distinct neighbourhoods
/// (cold archive, warm reactor, overgrown atrium …) from cheap param changes alone — within
/// the Legibility Contract, since districts touch only atmosphere. Presentation-only.
pub(crate) fn apply_match_atmosphere(
    mut commands: Commands,
    camera: Query<Entity, With<GameCam>>,
    mut sun: Query<&mut DirectionalLight, With<GameSun>>,
) {
    if let Ok(camera) = camera.single() {
        commands.entity(camera).insert((
            Hdr,
            Bloom {
                intensity: 0.08,
                ..Bloom::NATURAL
            },
            DistanceFog {
                color: Color::srgb(0.01, 0.015, 0.03),
                falloff: FogFalloff::Linear {
                    start: 10.0,
                    end: 28.0,
                },
                ..default()
            },
        ));
    }
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.35, 0.42, 0.6),
        brightness: 80.0,
        ..default()
    });
    for mut light in &mut sun {
        light.illuminance = 0.0;
    }
}

pub(crate) fn clear_match_atmosphere(
    mut commands: Commands,
    camera: Query<Entity, With<GameCam>>,
    mut sun: Query<&mut DirectionalLight, With<GameSun>>,
) {
    if let Ok(camera) = camera.single() {
        commands
            .entity(camera)
            .remove::<(Hdr, Bloom, DistanceFog)>();
    }
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.7, 0.74, 0.85),
        brightness: 900.0,
        ..default()
    });
    for mut light in &mut sun {
        light.illuminance = MENU_SUN_ILLUMINANCE;
    }
}

pub(crate) fn district_for_place(seed: u64, place: Place) -> style::District {
    let key = match place {
        Place::Room(room) => room.0,
        Place::Hallway { from, .. } => from.0,
    };
    style::district_for(seed, key)
}

fn architecture_register_for_place(
    game: &HybridMatch,
    place: Place,
) -> Option<observed_content::ArchitectureRegister> {
    let spec = game.competitive.map_spec.as_ref()?;
    match place {
        Place::Room(room) => spec.room_design(room).map(|design| design.register),
        Place::Hallway { corridor, .. } => {
            spec.corridor_design(corridor).map(|design| design.register)
        }
    }
}

pub(crate) fn district_for_game(seed: u64, place: Place, game: &HybridMatch) -> style::District {
    architecture_register_for_place(game, place)
        .map(style::district_for_architecture)
        .unwrap_or_else(|| district_for_place(seed, place))
}

pub(crate) fn collapse_state_for_place(game: &HybridMatch, place: Place) -> CollapseState {
    match place {
        Place::Room(room) => game.competitive.room_collapse(room),
        Place::Hallway { from, to, .. } => {
            let from_state = game.competitive.room_collapse(from);
            let to_state = game.competitive.room_collapse(to);
            if from_state == CollapseState::Collapsed || to_state == CollapseState::Collapsed {
                CollapseState::Collapsed
            } else if from_state == CollapseState::Dying || to_state == CollapseState::Dying {
                CollapseState::Dying
            } else {
                CollapseState::Intact
            }
        }
    }
}

pub(crate) fn countdown_klaxon_active(runtime: &MatchDirector) -> bool {
    runtime.series.current.remaining_countdown().is_some()
}

pub(crate) fn palette_for_game(
    seed: u64,
    place: Place,
    game: &HybridMatch,
    klaxon_active: bool,
) -> style::DistrictPalette {
    let mut palette = architecture_register_for_place(game, place)
        .map(style::architecture)
        .unwrap_or_else(|| style::district(district_for_place(seed, place)));
    if collapse_state_for_place(game, place) != CollapseState::Intact {
        palette = style::drained(&palette);
    }
    if klaxon_active {
        palette = style::klaxon_modulate(palette);
    }
    palette
}

pub(crate) fn palette_for_match(
    seed: u64,
    place: Place,
    runtime: &MatchDirector,
) -> style::DistrictPalette {
    palette_for_game(
        seed,
        place,
        runtime.live.host_match(),
        countdown_klaxon_active(runtime),
    )
}

const DISTRICT_BLEND_RATE: f32 = 2.5;

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

pub(crate) fn apply_place_atmosphere(
    time: Res<Time>,
    tp: Res<TeleportState>,
    runtime: Res<MatchDirector>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut fog: Query<(Entity, &mut DistanceFog, Has<VolumetricFog>), With<GameCam>>,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
    mut commands: Commands,
) {
    let seed_val = seed.map(|s| s.0).unwrap_or(MATCH_SEED);
    let mut pal = palette_for_match(seed_val, tp.place, &runtime);
    // Specialized structure treatments remain style-owned. The minimum ambient bound
    // preserves the Legibility Contract while pool rhythm and fog carry the depth read.
    if tp.geom.is_wellshaft() {
        pal = style::wellshaft(pal);
    }
    if tp.geom.structure_kind == crate::teleport::PlaceStructureKind::GantryExpanse {
        // At the 36 m deck height the ground lies beyond the opaque fog range, while
        // nearby column silhouettes and the semantic deck-edge treatment stay readable.
        pal = style::gantry_expanse(pal);
    }
    let t = (time.delta_secs() * DISTRICT_BLEND_RATE).clamp(0.0, 1.0);
    ambient.color = lerp_color(ambient.color, pal.ambient_color, t);
    ambient.brightness = lerp_f(ambient.brightness, pal.ambient_brightness, t);
    if let Ok((camera, mut f, has_volumetric)) = fog.single_mut() {
        f.color = lerp_color(f.color, pal.fog_color, t);
        if let FogFalloff::Linear { start, end } = &mut f.falloff {
            *start = lerp_f(*start, pal.fog_start, t);
            *end = lerp_f(*end, pal.fog_end, t);
        }
        let wants_volumetric = tp.geom.architecture_register
            == Some(observed_content::ArchitectureRegister::FacetMonument);
        if wants_volumetric && !has_volumetric {
            commands.entity(camera).insert(VolumetricFog {
                ambient_intensity: 0.0,
                ..default()
            });
        } else if !wants_volumetric && has_volumetric {
            commands.entity(camera).remove::<VolumetricFog>();
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_decohere_fx(
    time: Res<Time>,
    runtime: Res<MatchDirector>,
    paused: Res<MatchPaused>,
    assets: Res<MatchAssets>,
    settings: Res<crate::settings::Settings>,
    mut fx: ResMut<DecohereFx>,
    mut leaves: Query<(&DoorLeaf, &mut Transform)>,
    mut commands: Commands,
    mut audio_director: ResMut<AudioDirector>,
) {
    if paused.0 {
        return;
    }
    let commits = runtime.live.host_match().reroute_commits;
    if commits > fx.last_commits {
        let was_idle = fx.flash <= 0.0;
        fx.flash = ROUTE_SHIFT_FLASH_SECS;
        fx.last_commits = commits;
        if was_idle {
            audio_director.request(
                &mut commands,
                &assets.reroute,
                MatchAudioCue::Reroute,
                "Route shift",
                None,
                &settings,
            );
        }
        for (leaf, mut transform) in &mut leaves {
            transform.translation.y = leaf.closed_y;
        }
    }
    if fx.flash > 0.0 {
        fx.flash = (fx.flash - time.delta_secs()).max(0.0);
    }
}

pub(crate) fn flicker_lights(
    time: Res<Time>,
    fx: Res<DecohereFx>,
    mut point_lights: Query<(&FlickerLight, &mut PointLight)>,
    mut spot_lights: Query<(&FlickerLight, &mut SpotLight)>,
) {
    let t = time.elapsed_secs();
    let k = (fx.flash / ROUTE_SHIFT_FLASH_SECS).clamp(0.0, 1.0);
    let reroute = if k > 0.0 {
        let blink = 0.5 + 0.5 * (t * 37.0).sin() * (t * 19.0).cos();
        1.0 - 0.8 * k * (1.0 - blink)
    } else {
        1.0
    };
    let update_intensity = |flicker: &FlickerLight| -> f32 {
        let idle = if flicker.idle > 0.0 {
            let slow =
                (t * 6.3 + flicker.phase).sin() + 0.6 * (t * 11.0 + flicker.phase * 1.7).sin();
            let dip = if slow > 1.1 {
                0.3 + 0.7 * ((t * 46.0 + flicker.phase).sin() * 0.5 + 0.5)
            } else {
                1.0
            };
            1.0 - flicker.idle * (1.0 - dip)
        } else {
            1.0
        };
        flicker.base * reroute * idle
    };
    for (flicker, mut light) in &mut point_lights {
        light.intensity = update_intensity(flicker);
    }
    for (flicker, mut light) in &mut spot_lights {
        light.intensity = update_intensity(flicker);
    }
}
