//! Event one-shots for the hex match, routed through the shared SFX volume.
//!
//! Every event with a known place plays from that place: Bevy's spatial audio
//! (stereo panning plus inverse-square distance falloff, via `rodio`) is
//! enabled and the source `Transform` is set to the event's cell — the same
//! location `feedback.rs` blooms a beacon at (`feedback::event_cells`), so an
//! event that has a place to be seen also has a place to be heard from. A
//! player should be able to hear a keystone claimed down the hall before they
//! round the corner and see it, per the standing diegetic-feedback rule
//! (`agents.md`). Events the simulation genuinely has no location for yet
//! (`MutationWarning`, `MutationCancelled`, `MatchFinished` — see
//! `feedback::event_cells`) play non-spatially, from the listener: that is
//! what "the whole facility, no particular direction" sounds like, and it is
//! honest about not knowing where.

use bevy::audio::{PlaybackMode, SpatialListener, SpatialScale, Volume};
use bevy::prelude::*;
use observed_hex::hex_origin;

use super::cues::{HexWfcSound, cue_for};
use super::feedback::event_cells;
use super::sim::{EYE_OFFSET, HexWfcRuntime};
use crate::GameState;
use crate::view::components::GameCam;

/// Ear separation for the spatial listener. Bevy's spatial audio is stereo
/// panning only, no HRTF (`bevy_audio::SpatialListener` doc), so a
/// wider-than-anatomical gap keeps left/right direction legible rather than
/// subtle to the point of unnoticed.
const LISTENER_EAR_GAP: f32 = 0.5;

/// Shrinks world-metre distances before the inverse-square falloff
/// (`rodio::source::spatial::Spatial::set_positions`, `1.0 / distance^2`
/// clamped to 1.0 at contact) sees them. A hex cell spans 14 m
/// (`view::camera::TILE_SPAN`); at the engine's default scale of 1.0 an event
/// one cell away is already almost silent. This keeps a same-room event clear
/// and a several-cells-away one a distant, audible-but-quiet tell rather than
/// nothing. Picked by reading the falloff math, not confirmed against a
/// speaker — a human pass with real audio should retune this.
const HEX_SPATIAL_SCALE: f32 = 0.15;

#[derive(Resource)]
pub(super) struct HexWfcAudioAssets {
    reroute: Handle<AudioSource>,
    hold: Handle<AudioSource>,
    recover: Handle<AudioSource>,
    escape: Handle<AudioSource>,
    complete: Handle<AudioSource>,
    guardian: Handle<AudioSource>,
}

#[derive(Resource, Default)]
pub(super) struct HexWfcAudioState {
    last_event_tick: u64,
}

pub(super) fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    camera: Query<Entity, (With<GameCam>, Without<SpatialListener>)>,
) {
    commands.insert_resource(HexWfcAudioAssets {
        reroute: assets.load(observed_assets::REROUTE.path),
        hold: assets.load(observed_assets::TOOL_INTERACT.path),
        recover: assets.load(observed_assets::REROUTE.path),
        escape: assets.load(observed_assets::ESCAPE.path),
        complete: assets.load(observed_assets::EXIT_UNLOCK.path),
        guardian: assets.load(observed_assets::GUARDIAN_DREAD.path),
    });
    commands.insert_resource(HexWfcAudioState::default());
    // GameCam is the app's one persistent world camera (`game/src/lib.rs`), reused across
    // every screen, so this only needs to happen once and is safe to leave in place outside
    // the hex match too: it affects only sources spawned with `spatial: true`, which today
    // is exactly the events this module spatializes.
    for entity in &camera {
        commands
            .entity(entity)
            .insert(SpatialListener::new(LISTENER_EAR_GAP));
    }
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<HexWfcAudioAssets>();
    commands.remove_resource::<HexWfcAudioState>();
}

pub(super) fn sync(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    assets: Res<HexWfcAudioAssets>,
    settings: Res<crate::settings::Settings>,
    mut state: ResMut<HexWfcAudioState>,
) {
    let tick = runtime.match_state.tick;
    if tick == state.last_event_tick {
        return;
    }
    state.last_event_tick = tick;
    let master = settings.effective_sfx_volume();
    let delta = runtime.match_state.last_relayout_delta.as_ref();
    for event in &runtime.match_state.recent_events {
        let definition = cue_for(event.kind);
        // One sound per event even when a mutation touches several cells (the first —
        // deterministic, since `changed_cells` is a `BTreeSet` — stands in for the whole
        // beat); the beacon still blooms at each of them.
        let position = event_cells(event, delta)
            .first()
            .map(|&cell| Vec3::from_array(hex_origin(cell)) + Vec3::Y * EYE_OFFSET);
        play(
            &mut commands,
            sound(&assets, definition.sound),
            0.62 * master,
            "Hex WFC event cue",
            position,
        );
    }
}

fn sound(assets: &HexWfcAudioAssets, cue: HexWfcSound) -> Handle<AudioSource> {
    match cue {
        HexWfcSound::Reroute => assets.reroute.clone(),
        HexWfcSound::Hold => assets.hold.clone(),
        HexWfcSound::Recover => assets.recover.clone(),
        HexWfcSound::Escape => assets.escape.clone(),
        HexWfcSound::Complete => assets.complete.clone(),
        HexWfcSound::Guardian => assets.guardian.clone(),
    }
}

fn play(
    commands: &mut Commands,
    source: Handle<AudioSource>,
    volume: f32,
    name: &'static str,
    position: Option<Vec3>,
) {
    if volume <= 0.0 {
        return;
    }
    let mut playback = PlaybackSettings {
        mode: PlaybackMode::Despawn,
        volume: Volume::Linear(volume),
        ..PlaybackSettings::DESPAWN
    };
    let mut entity = commands.spawn((
        DespawnOnExit(GameState::HexWfc),
        AudioPlayer(source),
        Name::new(name),
    ));
    if let Some(position) = position {
        playback.spatial = true;
        playback.spatial_scale = Some(SpatialScale::new(HEX_SPATIAL_SCALE));
        entity.insert(Transform::from_translation(position));
    }
    entity.insert(playback);
}
