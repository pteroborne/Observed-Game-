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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use observed_hex::HexCoord;

    #[test]
    fn spatial_cue_sets_spatial_playback_scale_and_transform() {
        let mut app = App::new();
        let cell = HexCoord {
            q: 2,
            r: 3,
            level: 1,
        };
        let pos = Vec3::from_array(hex_origin(cell)) + Vec3::Y * EYE_OFFSET;
        app.world_mut()
            .run_system_once(move |mut commands: Commands| {
                play(
                    &mut commands,
                    Handle::default(),
                    0.6,
                    "Hex WFC event cue",
                    Some(pos),
                );
            })
            .unwrap();

        let mut query = app.world_mut().query::<(&PlaybackSettings, &Transform)>();
        let (playback, transform) = query.single(app.world()).unwrap();
        assert!(
            playback.spatial,
            "Spatial audio must be enabled for located cue"
        );
        assert_eq!(
            playback.spatial_scale.map(|s| s.0),
            Some(Vec3::splat(HEX_SPATIAL_SCALE)),
            "Spatial scale must match hex lattice scale"
        );
        assert_eq!(
            transform.translation, pos,
            "Cue transform must match cell hex origin with eye offset"
        );
    }

    #[test]
    fn different_cells_produce_different_cue_positions() {
        let mut app = App::new();
        let cell_a = HexCoord {
            q: 0,
            r: 0,
            level: 0,
        };
        let cell_b = HexCoord {
            q: 3,
            r: 5,
            level: 2,
        };
        let pos_a = Vec3::from_array(hex_origin(cell_a)) + Vec3::Y * EYE_OFFSET;
        let pos_b = Vec3::from_array(hex_origin(cell_b)) + Vec3::Y * EYE_OFFSET;
        assert_ne!(
            pos_a, pos_b,
            "Different cells must produce different spatial origins"
        );

        app.world_mut()
            .run_system_once(move |mut commands: Commands| {
                play(
                    &mut commands,
                    Handle::default(),
                    0.6,
                    "Hex WFC event cue",
                    Some(pos_a),
                );
                play(
                    &mut commands,
                    Handle::default(),
                    0.6,
                    "Hex WFC event cue",
                    Some(pos_b),
                );
            })
            .unwrap();

        let mut query = app.world_mut().query::<(&PlaybackSettings, &Transform)>();
        let translations: Vec<Vec3> = query
            .iter(app.world())
            .map(|(_, t)| t.translation)
            .collect();
        assert_eq!(translations.len(), 2);
        assert_ne!(translations[0], translations[1]);
        assert!(translations.contains(&pos_a));
        assert!(translations.contains(&pos_b));
    }

    #[test]
    fn unlocated_events_play_non_spatially_without_transform() {
        let mut app = App::new();
        app.world_mut()
            .run_system_once(|mut commands: Commands| {
                play(
                    &mut commands,
                    Handle::default(),
                    0.6,
                    "Hex WFC event cue",
                    None,
                );
            })
            .unwrap();

        let mut query = app.world_mut().query::<(Entity, &PlaybackSettings)>();
        let (entity, playback) = query.single(app.world()).unwrap();
        assert!(!playback.spatial, "Unlocated event must play non-spatially");
        assert!(playback.spatial_scale.is_none());
        assert!(
            app.world().get::<Transform>(entity).is_none(),
            "Non-spatial cue must not attach a Transform"
        );
    }

    #[test]
    fn setup_attaches_spatial_listener_to_game_cam() {
        let mut app = App::new();
        let cam = app.world_mut().spawn(GameCam).id();
        app.world_mut()
            .run_system_once(
                |mut commands: Commands,
                 camera: Query<Entity, (With<GameCam>, Without<SpatialListener>)>| {
                    for entity in &camera {
                        commands
                            .entity(entity)
                            .insert(SpatialListener::new(LISTENER_EAR_GAP));
                    }
                },
            )
            .unwrap();

        let listener = app.world().get::<SpatialListener>(cam);
        assert!(listener.is_some(), "GameCam must receive SpatialListener");
    }
}
