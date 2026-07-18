//! Event one-shots for the hex match, routed through the shared SFX volume.

use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;

use super::cues::{HexWfcSound, cue_for};
use super::sim::HexWfcRuntime;
use crate::GameState;

#[derive(Resource)]
pub(super) struct HexWfcAudioAssets {
    reroute: Handle<AudioSource>,
    hold: Handle<AudioSource>,
    recover: Handle<AudioSource>,
    escape: Handle<AudioSource>,
    complete: Handle<AudioSource>,
}

#[derive(Resource, Default)]
pub(super) struct HexWfcAudioState {
    last_event_tick: u64,
}

pub(super) fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(HexWfcAudioAssets {
        reroute: assets.load(observed_assets::REROUTE.path),
        hold: assets.load(observed_assets::TOOL_INTERACT.path),
        recover: assets.load(observed_assets::REROUTE.path),
        escape: assets.load(observed_assets::ESCAPE.path),
        complete: assets.load(observed_assets::EXIT_UNLOCK.path),
    });
    commands.insert_resource(HexWfcAudioState::default());
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
    for event in &runtime.match_state.recent_events {
        let definition = cue_for(event.kind);
        play(
            &mut commands,
            sound(&assets, definition.sound),
            0.62 * master,
            "Hex WFC event cue",
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
    }
}

fn play(commands: &mut Commands, source: Handle<AudioSource>, volume: f32, name: &'static str) {
    if volume <= 0.0 {
        return;
    }
    commands.spawn((
        DespawnOnExit(GameState::HexWfc),
        AudioPlayer(source),
        PlaybackSettings {
            mode: PlaybackMode::Despawn,
            volume: Volume::Linear(volume),
            ..PlaybackSettings::DESPAWN
        },
        Name::new(name),
    ));
}
