use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use observed_style as style;

use crate::flow::MATCH_SEED;
use crate::screens::audio::play_one_shot;
use crate::sim::state::{MatchPaused, MatchRuntime, TeleportState};
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
    mut ambient: ResMut<GlobalAmbientLight>,
    mut fog: Query<&mut DistanceFog, With<GameCam>>,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
) {
    let seed_val = seed.map(|s| s.0).unwrap_or(MATCH_SEED);
    let pal = style::district(district_for_place(seed_val, tp.place));
    let t = (time.delta_secs() * DISTRICT_BLEND_RATE).clamp(0.0, 1.0);
    ambient.color = lerp_color(ambient.color, pal.ambient_color, t);
    ambient.brightness = lerp_f(ambient.brightness, pal.ambient_brightness, t);
    if let Ok(mut f) = fog.single_mut() {
        f.color = lerp_color(f.color, pal.fog_color, t);
        if let FogFalloff::Linear { start, end } = &mut f.falloff {
            *start = lerp_f(*start, pal.fog_start, t);
            *end = lerp_f(*end, pal.fog_end, t);
        }
    }
}

pub(crate) fn sync_decohere_fx(
    time: Res<Time>,
    runtime: Res<MatchRuntime>,
    paused: Res<MatchPaused>,
    assets: Res<MatchAssets>,
    mut fx: ResMut<DecohereFx>,
    mut leaves: Query<(&DoorLeaf, &mut Transform)>,
    mut commands: Commands,
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
            play_one_shot(
                &mut commands,
                &assets.reroute,
                MatchAudioCue::Reroute,
                "Route shift",
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
    mut lights: Query<(&FlickerLight, &mut PointLight)>,
) {
    let t = time.elapsed_secs();
    let k = (fx.flash / ROUTE_SHIFT_FLASH_SECS).clamp(0.0, 1.0);
    let reroute = if k > 0.0 {
        let blink = 0.5 + 0.5 * (t * 37.0).sin() * (t * 19.0).cos();
        1.0 - 0.8 * k * (1.0 - blink)
    } else {
        1.0
    };
    for (flicker, mut light) in &mut lights {
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
        light.intensity = flicker.base * reroute * idle;
    }
}
