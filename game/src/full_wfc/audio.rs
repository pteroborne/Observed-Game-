//! Event one-shots and Guardian-proximity Geiger cadence.

use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;

use super::cues::{FullWfcSound, cue_for};
use super::sim::FullWfcRuntime;
use crate::GameState;

#[derive(Resource)]
pub(super) struct FullWfcAudioAssets {
    reroute: Handle<AudioSource>,
    tool: Handle<AudioSource>,
    keystone: Handle<AudioSource>,
    unlock: Handle<AudioSource>,
    guardian: Handle<AudioSource>,
    collapse: Handle<AudioSource>,
    escape_cue: Handle<AudioSource>,
}

#[derive(Resource, Default)]
pub(super) struct FullWfcAudioState {
    last_event_tick: u64,
    last_geiger_tick: u64,
}

pub(super) fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(FullWfcAudioAssets {
        reroute: assets.load(observed_assets::REROUTE.path),
        tool: assets.load(observed_assets::TOOL_INTERACT.path),
        keystone: assets.load(observed_assets::KEYSTONE.path),
        unlock: assets.load(observed_assets::EXIT_UNLOCK.path),
        guardian: assets.load(observed_assets::GUARDIAN_DREAD.path),
        collapse: assets.load(observed_assets::COLLAPSE_STING.path),
        escape_cue: assets.load(observed_assets::ESCAPE.path),
    });
    commands.insert_resource(FullWfcAudioState::default());
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<FullWfcAudioAssets>();
    commands.remove_resource::<FullWfcAudioState>();
}

pub(super) fn sync(
    mut commands: Commands,
    runtime: Res<FullWfcRuntime>,
    assets: Res<FullWfcAudioAssets>,
    settings: Res<crate::settings::Settings>,
    mut state: ResMut<FullWfcAudioState>,
) {
    let tick = runtime.match_state.tick;
    let master = settings.effective_sfx_volume();
    if tick != state.last_event_tick {
        state.last_event_tick = tick;
        for event in &runtime.match_state.recent_events {
            let definition = cue_for(event.kind);
            play(
                &mut commands,
                sound(&assets, definition.sound),
                0.62 * master,
                "Full WFC event cue",
            );
        }
    }
    let pressure = runtime.match_state.guardian_pressure(runtime.local_player);
    if pressure < 0.08 {
        return;
    }
    let period = (96.0 - pressure * 84.0).round().max(12.0) as u64;
    if tick.saturating_sub(state.last_geiger_tick) >= period {
        state.last_geiger_tick = tick;
        play(
            &mut commands,
            assets.guardian.clone(),
            (0.16 + pressure * 0.48) * master,
            "Guardian Geiger tick",
        );
    }
}

fn sound(assets: &FullWfcAudioAssets, cue: FullWfcSound) -> Handle<AudioSource> {
    match cue {
        FullWfcSound::Reroute => assets.reroute.clone(),
        FullWfcSound::Tool => assets.tool.clone(),
        FullWfcSound::Keystone => assets.keystone.clone(),
        FullWfcSound::Unlock => assets.unlock.clone(),
        FullWfcSound::Guardian => assets.guardian.clone(),
        FullWfcSound::Collapse => assets.collapse.clone(),
        FullWfcSound::Escape => assets.escape_cue.clone(),
    }
}

fn play(commands: &mut Commands, source: Handle<AudioSource>, volume: f32, name: &'static str) {
    if volume <= 0.0 {
        return;
    }
    commands.spawn((
        DespawnOnExit(GameState::FullWfc),
        AudioPlayer(source),
        PlaybackSettings {
            mode: PlaybackMode::Despawn,
            volume: Volume::Linear(volume),
            ..PlaybackSettings::DESPAWN
        },
        Name::new(name),
    ));
}
