//! Match audio: footstep cadence from the controller's stride, a door thunk on each
//! teleport between places, and an escape sting — each played as a drop-in cue if its
//! sound was provided, silent otherwise.

use crate::layout::PLACE_TILE;
use bevy::audio::Volume;
use bevy::prelude::*;

use crate::GameState;
use crate::sim::director::MatchDirector;
use crate::sim::state::{MatchPaused, TeleportState};
use crate::view::assets::MatchAssets;
use crate::view::components::{MatchAudioCue, MatchAudioState};

const FOOTSTEP_STRIDE: f32 = 1.8;

pub(crate) fn play_one_shot(
    commands: &mut Commands,
    sound: &Option<Handle<AudioSource>>,
    cue: MatchAudioCue,
    name: &'static str,
) {
    if let Some(sound) = sound {
        commands.spawn((
            cue,
            DespawnOnExit(GameState::Match),
            AudioPlayer(sound.clone()),
            PlaybackSettings::DESPAWN,
            Name::new(name),
        ));
    }
}

/// Start the facility ambience on entering the Match. The static set-pieces of the old
/// whole-maze view (exit gate, control device, objective beacon) are gone: in the
/// teleport model the per-place renderer (`rebuild_place`) builds whatever is in the
/// current place, and `sync_rival_avatars` brings rival figures into the room you share
/// with them.
pub(crate) fn spawn_match_setpieces(assets: Res<MatchAssets>, mut commands: Commands) {
    if let Some(ambience) = &assets.ambience {
        commands.spawn((
            MatchAudioCue::Ambience,
            DespawnOnExit(GameState::Match),
            AudioPlayer(ambience.clone()),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.24)),
            Name::new("Facility ambience"),
        ));
    }
}

pub(crate) fn sync_match_audio(
    mut commands: Commands,
    runtime: Res<MatchDirector>,
    tp: Res<TeleportState>,
    paused: Res<MatchPaused>,
    assets: Res<MatchAssets>,
    mut audio_state: ResMut<MatchAudioState>,
) {
    let game = runtime.live.host_match();
    let position = tp.body.position;

    if !paused.0 {
        let horizontal_delta = Vec2::new(position.x, position.z)
            - Vec2::new(audio_state.last_position.x, audio_state.last_position.z);
        let distance = horizontal_delta.length();
        // The width guard skips the position jump on a teleport between places.
        if tp.body.grounded && distance < PLACE_TILE * 0.5 {
            audio_state.stride_distance += distance;
            if audio_state.stride_distance >= FOOTSTEP_STRIDE {
                play_one_shot(
                    &mut commands,
                    &assets.footstep,
                    MatchAudioCue::Footstep,
                    "Player footstep",
                );
                audio_state.stride_distance -= FOOTSTEP_STRIDE;
            }
        } else if !tp.body.grounded {
            audio_state.stride_distance = 0.0;
        }
    }
    audio_state.last_position = position;

    // A door thunk on entering/leaving a place (a teleport) — not the old per-round
    // high-pitch reroute zap. Silent until a `door.ogg` is dropped in.
    if tp.place != audio_state.last_place {
        play_one_shot(&mut commands, &assets.door, MatchAudioCue::Door, "Door");
        audio_state.last_place = tp.place;
    }
    let escaped = game.competitive.escaped_count();
    if escaped > audio_state.escaped_count {
        play_one_shot(
            &mut commands,
            &assets.escape,
            MatchAudioCue::Escape,
            "Escape success",
        );
        audio_state.escaped_count = escaped;
    }
}
