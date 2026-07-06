//! Match audio: footstep cadence from the controller's stride, a door thunk on each
//! teleport between places, an escape sting, and rival **sound bleed** — each played as
//! a drop-in cue if its sound was provided, silent otherwise.

use crate::layout::PLACE_TILE;
use bevy::audio::Volume;
use bevy::prelude::*;
use observed_core::RoomId;

use crate::GameState;
use crate::settings::Settings;
use crate::sim::director::MatchDirector;
use crate::sim::nav::rival_signals;
use crate::sim::state::{MatchPaused, RivalSightings, SightingKind, TeleportState};
use crate::teleport::Place;
use crate::view::assets::MatchAssets;
use crate::view::components::{MatchAudioCue, MatchAudioState, RivalBleedState};

const FOOTSTEP_STRIDE: f32 = 1.8;
/// Sound-bleed volume clamp (Design ruling): attenuated by distance to the threshold's
/// gap centre, but never silent (a rival is always at least faintly audible through the
/// wall) nor as loud as a footstep in the same room.
const BLEED_VOLUME_MIN: f32 = 0.15;
const BLEED_VOLUME_MAX: f32 = 0.45;
/// Distance (world units) at which attenuation bottoms out at [`BLEED_VOLUME_MIN`].
const BLEED_ATTENUATION_RANGE: f32 = 12.0;

pub(crate) fn play_one_shot(
    commands: &mut Commands,
    sound: &Option<Handle<AudioSource>>,
    cue: MatchAudioCue,
    name: &'static str,
    volume: f32,
) {
    if let Some(sound) = sound {
        commands.spawn((
            cue,
            DespawnOnExit(GameState::Match),
            AudioPlayer(sound.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
            Name::new(name),
        ));
    }
}

/// Start the facility ambience on entering the Match. The static set-pieces of the old
/// whole-maze view (exit gate, control device, objective beacon) are gone: in the
/// teleport model the per-place renderer (`rebuild_place`) builds whatever is in the
/// current place, and `sync_rival_avatars` brings rival figures into the room you share
/// with them.
pub(crate) fn spawn_match_setpieces(
    assets: Res<MatchAssets>,
    settings: Res<Settings>,
    mut commands: Commands,
) {
    if let Some(ambience) = &assets.ambience {
        commands.spawn((
            MatchAudioCue::Ambience,
            DespawnOnExit(GameState::Match),
            AudioPlayer(ambience.clone()),
            PlaybackSettings::LOOP
                .with_volume(Volume::Linear(0.24 * settings.effective_music_volume())),
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
    settings: Res<Settings>,
    mut audio_state: ResMut<MatchAudioState>,
) {
    let game = runtime.live.host_match();
    let position = tp.body.position;
    let sfx_volume = settings.effective_sfx_volume();

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
                    sfx_volume,
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
        play_one_shot(
            &mut commands,
            &assets.door,
            MatchAudioCue::Door,
            "Door",
            sfx_volume,
        );
        audio_state.last_place = tp.place;
    }
    let escaped = game.competitive.escaped_count();
    if escaped > audio_state.escaped_count {
        play_one_shot(
            &mut commands,
            &assets.escape,
            MatchAudioCue::Escape,
            "Escape success",
            sfx_volume,
        );
        audio_state.escaped_count = escaped;
    }
}

/// Rival **sound bleed** (Phase 42c): when a rival team's presence first appears (or
/// changes room) among the current place's `rival_signals` since the last frame, play
/// the same footstep cue at reduced, distance-attenuated volume and record a `Heard`
/// sighting. Diegetic, procedural (no new assets — reuses `assets.footstep`), and
/// presentation-only: this is the sighting ledger's *other* writer, but it only ever
/// records [`SightingKind::Heard`] through the same [`RivalSightings::record`] rule the
/// witnessing system uses, so there remains exactly one *rule* even though two systems
/// call it for two different evidence sources.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bleed_rival_sound(
    mut commands: Commands,
    runtime: Res<MatchDirector>,
    tp: Res<TeleportState>,
    paused: Res<MatchPaused>,
    assets: Res<MatchAssets>,
    settings: Res<Settings>,
    mut sightings: ResMut<RivalSightings>,
    mut bleed: ResMut<RivalBleedState>,
) {
    if paused.0 {
        return;
    }
    let game = runtime.live.host_match();
    let commits = game.reroute_commits;
    let local_team = crate::flow::LOCAL_TEAM.0 as usize;
    let signal_room = match tp.place {
        Place::Room(room) => room,
        Place::Hallway { from, .. } => from,
    };
    let body = Vec2::new(tp.body.position.x, tp.body.position.z);

    let present: Vec<(usize, RoomId)> = rival_signals(game, local_team, signal_room)
        .into_iter()
        .filter_map(|signal| {
            signal
                .presence
                .map(|team| (team.0 as usize, signal.neighbor))
        })
        .collect();

    for &(team_index, room) in &present {
        let first_appearance_or_room_change = !bleed
            .last_heard
            .iter()
            .any(|&(t, r)| t == team_index && r == room);
        if first_appearance_or_room_change {
            let gap_center = tp
                .geom
                .gaps
                .iter()
                .find(|gap| gap.target == room)
                .map(|gap| gap.center)
                .unwrap_or(body);
            let distance = body.distance(gap_center);
            let t = (distance / BLEED_ATTENUATION_RANGE).clamp(0.0, 1.0);
            let volume = (BLEED_VOLUME_MAX - t * (BLEED_VOLUME_MAX - BLEED_VOLUME_MIN))
                * settings.effective_sfx_volume();
            if let Some(sound) = &assets.footstep {
                commands.spawn((
                    MatchAudioCue::RivalBleed,
                    DespawnOnExit(GameState::Match),
                    AudioPlayer(sound.clone()),
                    PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
                    Name::new("Rival sound bleed"),
                ));
            }
            let team_id = game.competitive.teams[team_index].id;
            sightings.record(team_id, room, SightingKind::Heard, commits);
        }
    }
    bleed.last_heard = present;
}
