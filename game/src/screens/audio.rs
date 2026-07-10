//! Match audio: footstep cadence from the controller's stride, a door thunk on each
//! teleport between places, an escape sting, and rival **sound bleed** — each played as
//! a drop-in cue if its sound was provided, silent otherwise.

use crate::layout::PLACE_TILE;
use bevy::audio::Volume;
use bevy::prelude::*;
use observed_core::RoomId;
use std::collections::HashMap;

use crate::GameState;
use crate::settings::Settings;
use crate::sim::director::MatchDirector;
use crate::sim::nav::{connections_for, rival_signals};
use crate::sim::state::{MatchPaused, RivalSightings, SightingKind, TeleportState};
use crate::teleport::Place;
use crate::view::assets::MatchAssets;
use crate::view::components::{MatchAudioCue, MatchAudioState, RivalBleedState};
use observed_match::facility::CollapseState;

const FOOTSTEP_STRIDE: f32 = 1.8;
const DISTRICT_AMBIENCE_VOLUME: f32 = 0.24;
const DISTRICT_AMBIENCE_BLEND_RATE: f32 = 1.5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AudioBus {
    Music,
    Sfx,
    Ui,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AudioSourceRelation {
    SamePlace,
    ThroughThreshold,
    ThroughWall,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RolloffClass {
    None,
    Near,
    Room,
    Far,
}

impl RolloffClass {
    fn gain(self, distance: f32) -> f32 {
        let d = distance.max(0.0);
        match self {
            Self::None => 1.0,
            Self::Near => rolloff_gain(d, 5.0, 0.68),
            Self::Room => rolloff_gain(d, 12.0, 0.38),
            Self::Far => rolloff_gain(d, 18.0, 0.28),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OcclusionClass {
    None,
    Threshold,
    Dread,
}

impl OcclusionClass {
    fn gain(self, relation: AudioSourceRelation) -> f32 {
        match (self, relation) {
            (Self::None, _) => 1.0,
            (Self::Threshold, AudioSourceRelation::SamePlace) => 1.0,
            (Self::Threshold, AudioSourceRelation::ThroughThreshold) => 0.78,
            (Self::Threshold, AudioSourceRelation::ThroughWall) => 0.48,
            (Self::Dread, AudioSourceRelation::SamePlace) => 0.9,
            (Self::Dread, AudioSourceRelation::ThroughThreshold) => 0.54,
            (Self::Dread, AudioSourceRelation::ThroughWall) => 0.24,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AttenuationClass {
    pub(crate) rolloff: RolloffClass,
    pub(crate) occlusion: OcclusionClass,
    pub(crate) floor: f32,
}

impl AttenuationClass {
    pub(crate) const NONE: Self = Self {
        rolloff: RolloffClass::None,
        occlusion: OcclusionClass::None,
        floor: 1.0,
    };
    pub(crate) const LOCAL: Self = Self {
        rolloff: RolloffClass::Near,
        occlusion: OcclusionClass::None,
        floor: 0.55,
    };
    pub(crate) const STRUCTURE: Self = Self {
        rolloff: RolloffClass::Near,
        occlusion: OcclusionClass::Threshold,
        floor: 0.42,
    };
    pub(crate) const RIVAL: Self = Self {
        rolloff: RolloffClass::Far,
        occlusion: OcclusionClass::Threshold,
        floor: 0.34,
    };
    pub(crate) const GUARDIAN: Self = Self {
        rolloff: RolloffClass::Room,
        occlusion: OcclusionClass::Dread,
        floor: 0.12,
    };

    pub(crate) fn gain(self, relation: AudioSourceRelation, distance: f32) -> f32 {
        (self.rolloff.gain(distance) * self.occlusion.gain(relation))
            .max(self.floor)
            .clamp(0.0, 1.0)
    }
}

fn rolloff_gain(distance: f32, range: f32, floor: f32) -> f32 {
    let t = (distance / range).clamp(0.0, 1.0);
    1.0 - t * (1.0 - floor)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DuckConfig {
    pub(crate) bus: AudioBus,
    pub(crate) target_factor: f32,
    pub(crate) ease_in: f32,
    pub(crate) duration: f32,
    pub(crate) ease_out: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DuckState {
    Active,
    EasingOut { start_factor: f32 },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ActiveDuck {
    pub(crate) bus: AudioBus,
    pub(crate) target_factor: f32,
    pub(crate) ease_in: f32,
    pub(crate) duration: f32,
    pub(crate) ease_out: f32,
    pub(crate) elapsed: f32,
    pub(crate) state: DuckState,
    pub(crate) source_entity: Option<Entity>,
}

impl ActiveDuck {
    pub(crate) fn current_factor(&self) -> f32 {
        match self.state {
            DuckState::Active => {
                if self.elapsed < self.ease_in {
                    let p = if self.ease_in > 0.0 {
                        self.elapsed / self.ease_in
                    } else {
                        1.0
                    };
                    1.0 + (self.target_factor - 1.0) * p
                } else {
                    self.target_factor
                }
            }
            DuckState::EasingOut { start_factor } => {
                let p = if self.ease_out > 0.0 {
                    (self.elapsed / self.ease_out).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                start_factor + (1.0 - start_factor) * p
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct CueConfig {
    pub(crate) bus: AudioBus,
    pub(crate) base_volume: f32,
    pub(crate) cooldown: f32,
    pub(crate) max_instances: usize,
    pub(crate) duck: Option<DuckConfig>,
    pub(crate) is_loop: bool,
    pub(crate) attenuation: AttenuationClass,
}

#[derive(Resource)]
pub(crate) struct AudioDirector {
    pub(crate) last_fire: HashMap<MatchAudioCue, f32>,
    pub(crate) active_instances: HashMap<MatchAudioCue, Vec<Entity>>,
    pub(crate) volume_overrides: HashMap<Entity, f32>,
    pub(crate) active_ducks: Vec<ActiveDuck>,
    pub(crate) elapsed_secs: f32,
}

impl Default for AudioDirector {
    fn default() -> Self {
        Self {
            last_fire: HashMap::default(),
            active_instances: HashMap::default(),
            volume_overrides: HashMap::default(),
            active_ducks: Vec::default(),
            elapsed_secs: 0.0,
        }
    }
}

impl AudioDirector {
    pub(crate) fn get_config(&self, cue: MatchAudioCue) -> Option<CueConfig> {
        match cue {
            MatchAudioCue::Ambience => Some(CueConfig {
                bus: AudioBus::Music,
                base_volume: DISTRICT_AMBIENCE_VOLUME,
                cooldown: 0.0,
                max_instances: 6,
                duck: None,
                is_loop: true,
                attenuation: AttenuationClass::NONE,
            }),
            MatchAudioCue::Footstep => Some(CueConfig {
                bus: AudioBus::Sfx,
                base_volume: 1.0,
                cooldown: 0.1,
                max_instances: 3,
                duck: None,
                is_loop: false,
                attenuation: AttenuationClass::LOCAL,
            }),
            MatchAudioCue::Door => Some(CueConfig {
                bus: AudioBus::Sfx,
                base_volume: 1.0,
                cooldown: 0.1,
                max_instances: 2,
                duck: None,
                is_loop: false,
                attenuation: AttenuationClass::STRUCTURE,
            }),
            MatchAudioCue::Escape => Some(CueConfig {
                bus: AudioBus::Sfx,
                base_volume: 1.0,
                cooldown: 0.5,
                max_instances: 1,
                duck: Some(DuckConfig {
                    bus: AudioBus::Music,
                    target_factor: 0.3,
                    ease_in: 0.1,
                    duration: 2.0,
                    ease_out: 1.5,
                }),
                is_loop: false,
                attenuation: AttenuationClass::NONE,
            }),
            MatchAudioCue::Reroute => Some(CueConfig {
                bus: AudioBus::Sfx,
                base_volume: 1.0,
                cooldown: 0.5,
                max_instances: 1,
                duck: Some(DuckConfig {
                    bus: AudioBus::Music,
                    target_factor: 0.3,
                    ease_in: 0.2,
                    duration: 1.5,
                    ease_out: 1.0,
                }),
                is_loop: false,
                attenuation: AttenuationClass::STRUCTURE,
            }),
            MatchAudioCue::RivalBleed => Some(CueConfig {
                bus: AudioBus::Sfx,
                base_volume: 0.45,
                cooldown: 0.1,
                max_instances: 4,
                duck: None,
                is_loop: false,
                attenuation: AttenuationClass::RIVAL,
            }),
            MatchAudioCue::UiClick => Some(CueConfig {
                bus: AudioBus::Ui,
                base_volume: 1.0,
                cooldown: 0.05,
                max_instances: 5,
                duck: None,
                is_loop: false,
                attenuation: AttenuationClass::NONE,
            }),
            MatchAudioCue::UiHover => Some(CueConfig {
                bus: AudioBus::Ui,
                base_volume: 1.0,
                cooldown: 0.05,
                max_instances: 5,
                duck: None,
                is_loop: false,
                attenuation: AttenuationClass::NONE,
            }),
            MatchAudioCue::Jump => Some(CueConfig {
                bus: AudioBus::Sfx,
                base_volume: 0.7,
                cooldown: 0.1,
                max_instances: 2,
                duck: None,
                is_loop: false,
                attenuation: AttenuationClass::LOCAL,
            }),
            MatchAudioCue::Land => Some(CueConfig {
                bus: AudioBus::Sfx,
                base_volume: 0.9,
                cooldown: 0.1,
                max_instances: 2,
                duck: None,
                is_loop: false,
                attenuation: AttenuationClass::LOCAL,
            }),
            MatchAudioCue::Klaxon => Some(CueConfig {
                bus: AudioBus::Sfx,
                base_volume: 0.35,
                cooldown: 0.5,
                max_instances: 1,
                duck: Some(DuckConfig {
                    bus: AudioBus::Music,
                    target_factor: 0.5,
                    ease_in: 0.5,
                    duration: f32::INFINITY,
                    ease_out: 0.5,
                }),
                is_loop: true,
                attenuation: AttenuationClass::NONE,
            }),
            MatchAudioCue::CollapseSting => Some(CueConfig {
                bus: AudioBus::Sfx,
                base_volume: 0.8,
                cooldown: 1.0,
                max_instances: 1,
                duck: Some(DuckConfig {
                    bus: AudioBus::Music,
                    target_factor: 0.3,
                    ease_in: 0.2,
                    duration: 2.0,
                    ease_out: 1.5,
                }),
                is_loop: false,
                attenuation: AttenuationClass::STRUCTURE,
            }),
            MatchAudioCue::ToolInteract => Some(CueConfig {
                bus: AudioBus::Sfx,
                base_volume: 0.65,
                cooldown: 0.0,
                max_instances: 2,
                duck: None,
                is_loop: false,
                attenuation: AttenuationClass::LOCAL,
            }),
            MatchAudioCue::Keystone => Some(CueConfig {
                bus: AudioBus::Sfx,
                base_volume: 0.74,
                cooldown: 0.12,
                max_instances: 2,
                duck: None,
                is_loop: false,
                attenuation: AttenuationClass::LOCAL,
            }),
            MatchAudioCue::ExitUnlock => Some(CueConfig {
                bus: AudioBus::Sfx,
                base_volume: 0.9,
                cooldown: 0.5,
                max_instances: 1,
                duck: Some(DuckConfig {
                    bus: AudioBus::Music,
                    target_factor: 0.55,
                    ease_in: 0.15,
                    duration: 1.1,
                    ease_out: 0.8,
                }),
                is_loop: false,
                attenuation: AttenuationClass::NONE,
            }),
            MatchAudioCue::GuardianDread => Some(CueConfig {
                bus: AudioBus::Sfx,
                base_volume: 0.55,
                cooldown: 3.0,
                max_instances: 1,
                duck: None,
                is_loop: false,
                attenuation: AttenuationClass::GUARDIAN,
            }),
        }
    }

    pub(crate) fn bus_duck_factor(&self, bus: AudioBus) -> f32 {
        let mut factor: f32 = 1.0;
        for duck in &self.active_ducks {
            if duck.bus == bus {
                factor = factor.min(duck.current_factor());
            }
        }
        factor
    }

    #[cfg(test)]
    pub(crate) fn spatial_gain_for(
        &self,
        cue: MatchAudioCue,
        relation: AudioSourceRelation,
        distance: f32,
    ) -> Option<f32> {
        self.get_config(cue)
            .map(|config| config.attenuation.gain(relation, distance))
    }

    pub(crate) fn request(
        &mut self,
        commands: &mut Commands,
        sound: &Option<Handle<AudioSource>>,
        cue: MatchAudioCue,
        name: &'static str,
        volume_override: Option<f32>,
        settings: &Settings,
    ) -> bool {
        self.request_spatial(
            commands,
            sound,
            cue,
            name,
            volume_override,
            AudioSourceRelation::SamePlace,
            0.0,
            settings,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn request_spatial(
        &mut self,
        commands: &mut Commands,
        sound: &Option<Handle<AudioSource>>,
        cue: MatchAudioCue,
        name: &'static str,
        volume_override: Option<f32>,
        relation: AudioSourceRelation,
        distance: f32,
        settings: &Settings,
    ) -> bool {
        let Some(config) = self.get_config(cue) else {
            return false;
        };
        let bus = config.bus;

        // 1. Cooldown check
        if config.cooldown > 0.0
            && self
                .last_fire
                .get(&cue)
                .is_some_and(|&last| self.elapsed_secs - last < config.cooldown)
        {
            return false;
        }

        // 2. Max instances check
        if self
            .active_instances
            .get(&cue)
            .is_some_and(|ins| ins.len() >= config.max_instances)
        {
            return false;
        }

        // 3. Settings channel check
        let settings_volume = match bus {
            AudioBus::Music => settings.effective_music_volume(),
            AudioBus::Sfx | AudioBus::Ui => settings.effective_sfx_volume(),
        };

        if settings_volume <= 0.0 {
            return false;
        }

        // 4. Determine base volume
        let spatial_gain = config.attenuation.gain(relation, distance);
        let base_vol = volume_override.unwrap_or(config.base_volume) * spatial_gain;
        let duck_factor = self.bus_duck_factor(bus);
        let volume = base_vol * settings_volume * duck_factor;

        let Some(audible_vol) = audible(volume) else {
            return false;
        };

        if let Some(sound) = sound {
            let entity_cmds = commands.spawn((
                cue,
                DespawnOnExit(GameState::Match),
                AudioPlayer(sound.clone()),
                if config.is_loop {
                    PlaybackSettings::LOOP.with_volume(Volume::Linear(audible_vol))
                } else {
                    PlaybackSettings::DESPAWN.with_volume(Volume::Linear(audible_vol))
                },
                Name::new(name),
            ));
            let entity = entity_cmds.id();

            // Track instance
            self.active_instances.entry(cue).or_default().push(entity);
            if volume_override.is_some() || (base_vol - config.base_volume).abs() > f32::EPSILON {
                self.volume_overrides.insert(entity, base_vol);
            }

            // Record fire time
            if config.cooldown > 0.0 {
                self.last_fire.insert(cue, self.elapsed_secs);
            }

            // Apply ducking if configured
            if let Some(duck_cfg) = config.duck {
                self.active_ducks.push(ActiveDuck {
                    bus: duck_cfg.bus,
                    target_factor: duck_cfg.target_factor,
                    ease_in: duck_cfg.ease_in,
                    duration: duck_cfg.duration,
                    ease_out: duck_cfg.ease_out,
                    elapsed: 0.0,
                    state: DuckState::Active,
                    source_entity: Some(entity),
                });
            }

            true
        } else {
            false
        }
    }

    pub(crate) fn stop(&mut self, commands: &mut Commands, cue: MatchAudioCue) {
        if let Some(entities) = self.active_instances.remove(&cue) {
            for entity in entities {
                commands.entity(entity).despawn();
            }
        }
    }
}

pub(crate) fn update_audio_director(
    time: Res<Time>,
    mut director: ResMut<AudioDirector>,
    settings: Res<Settings>,
    entity_query: Query<Entity>,
    // Ambience beds are excluded: `fade_ambience_beds` is their only volume writer
    // (it applies the music volume and duck itself). Without this filter the two
    // systems write the same sinks every frame — the director's un-faded value
    // (all beds audible at once) is exposed to the audio thread between the writes.
    mut sink_query: Query<(Entity, &mut AudioSink, &MatchAudioCue), Without<AmbienceBed>>,
) {
    director.elapsed_secs += time.delta_secs();
    let dt = time.delta_secs();

    // 1. Clean up despawned entities from active_instances and volume_overrides
    for instances in director.active_instances.values_mut() {
        instances.retain(|entity| entity_query.get(*entity).is_ok());
    }
    director
        .volume_overrides
        .retain(|entity, _| entity_query.get(*entity).is_ok());

    // 2. Update active ducks
    for duck in &mut director.active_ducks {
        duck.elapsed += dt;
        if let DuckState::Active = duck.state {
            let source_alive = duck
                .source_entity
                .map(|e| entity_query.get(e).is_ok())
                .unwrap_or(true);
            if !source_alive {
                let current = duck.current_factor();
                duck.state = DuckState::EasingOut {
                    start_factor: current,
                };
                duck.elapsed = 0.0;
            }
        }
    }

    // Transition ducks that naturally reached their active duration/hold end
    for duck in &mut director.active_ducks {
        if duck.state == DuckState::Active && duck.elapsed >= duck.ease_in + duck.duration {
            duck.state = DuckState::EasingOut {
                start_factor: duck.target_factor,
            };
            duck.elapsed = 0.0;
        }
    }

    // Retain only ducks that are not finished easing out
    director.active_ducks.retain(|duck| match duck.state {
        DuckState::Active => true,
        DuckState::EasingOut { .. } => duck.elapsed < duck.ease_out,
    });

    // 3. Update volumes of all active sinks based on settings and ducking
    for (entity, mut sink, cue) in &mut sink_query {
        if let Some(config) = director.get_config(*cue) {
            let bus = config.bus;
            let settings_volume = match bus {
                AudioBus::Music => settings.effective_music_volume(),
                AudioBus::Sfx | AudioBus::Ui => settings.effective_sfx_volume(),
            };
            let duck_factor = director.bus_duck_factor(bus);
            let base_vol = director
                .volume_overrides
                .get(&entity)
                .copied()
                .unwrap_or(config.base_volume);
            let final_vol = base_vol * settings_volume * duck_factor;
            sink.set_volume(Volume::Linear(final_vol.clamp(0.0, 1.0)));
        }
    }
}

fn audible(volume: f32) -> Option<f32> {
    volume
        .is_finite()
        .then_some(volume.clamp(0.0, 1.0))
        .filter(|v| *v > 0.0)
}

/// Which looping ambience bed an [`AmbienceBed`] entity carries: one per district for
/// rooms, plus the two hallway flavours (a generic corridor bed, and the gantry bed
/// for the two-level jump hall).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AmbienceBedKind {
    District(observed_style::District),
    Corridor,
    Gantry,
}

impl AmbienceBedKind {
    fn label(self) -> String {
        match self {
            AmbienceBedKind::District(district) => format!("district {}", district.label()),
            AmbienceBedKind::Corridor => "corridor".to_string(),
            AmbienceBedKind::Gantry => "gantry".to_string(),
        }
    }
}

/// The active bed for a place: rooms take their district's bed; hallways take the
/// gantry bed when the hall has raised decks, the corridor bed otherwise. Pure, so the
/// selection is testable without audio.
pub(crate) fn active_ambience_bed(seed: u64, place: Place, has_decks: bool) -> AmbienceBedKind {
    match place {
        Place::Room(_) => AmbienceBedKind::District(
            crate::screens::match_runtime::ambience::district_for_place(seed, place),
        ),
        Place::Hallway { .. } if has_decks => AmbienceBedKind::Gantry,
        Place::Hallway { .. } => AmbienceBedKind::Corridor,
    }
}

/// A looping ambience bed. Spawned once per kind at Match entry and **volume-faded
/// only** — never restarted — as the player moves (the Phase-49 stutter lesson).
/// `fade_ambience_beds` is the ONLY writer of these sinks' volumes:
/// `update_audio_director` skips them (`Without<AmbienceBed>`) so the two systems
/// never fight over the same sink within a frame.
#[derive(Component)]
pub(crate) struct AmbienceBed {
    pub(crate) kind: AmbienceBedKind,
    pub(crate) fade_factor: f32,
}

/// Start the facility ambience on entering the Match. The static set-pieces of the old
/// whole-maze view (exit gate, control device, objective beacon) are gone: in the
/// teleport model the per-place renderer (`rebuild_place`) builds whatever is in the
/// current place, and `sync_rival_avatars` brings rival figures into the room you share
/// with them.
pub(crate) fn spawn_match_setpieces(
    assets: Res<MatchAssets>,
    _settings: Res<Settings>,
    mut commands: Commands,
) {
    let mut beds: Vec<(AmbienceBedKind, &Option<Handle<AudioSource>>)> =
        observed_style::District::ALL
            .iter()
            .map(|&district| {
                (
                    AmbienceBedKind::District(district),
                    &assets.district_ambience[district.index()],
                )
            })
            .collect();
    beds.push((AmbienceBedKind::Corridor, &assets.ambience_corridor));
    beds.push((AmbienceBedKind::Gantry, &assets.ambience_gantry));

    for (kind, sound) in beds {
        let fallback = &assets.ambience;
        let selected = sound.as_ref().or(fallback.as_ref());
        if let Some(selected) = selected {
            commands.spawn((
                AmbienceBed {
                    kind,
                    fade_factor: 0.0,
                },
                MatchAudioCue::Ambience,
                DespawnOnExit(GameState::Match),
                AudioPlayer(selected.clone()),
                PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
                Name::new(format!("Ambience bed: {}", kind.label())),
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_match_audio(
    mut commands: Commands,
    runtime: Res<MatchDirector>,
    tp: Res<TeleportState>,
    paused: Res<MatchPaused>,
    assets: Res<MatchAssets>,
    settings: Res<Settings>,
    mut audio_state: ResMut<MatchAudioState>,
    mut director: ResMut<AudioDirector>,
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
                director.request(
                    &mut commands,
                    &assets.footstep,
                    MatchAudioCue::Footstep,
                    "Player footstep",
                    None,
                    &settings,
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
        director.request(
            &mut commands,
            &assets.door,
            MatchAudioCue::Door,
            "Door",
            None,
            &settings,
        );
        audio_state.last_place = tp.place;
    }
    let escaped = game.competitive.escaped_count();
    if escaped > audio_state.escaped_count {
        director.request(
            &mut commands,
            &assets.escape,
            MatchAudioCue::Escape,
            "Escape success",
            None,
            &settings,
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
    mut director: ResMut<AudioDirector>,
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
            let (source, relation) = tp
                .geom
                .gaps
                .iter()
                .find(|gap| gap.target == room)
                .map(|gap| (gap.center, AudioSourceRelation::ThroughThreshold))
                .unwrap_or((body, AudioSourceRelation::ThroughWall));
            let distance = body.distance(source);
            director.request_spatial(
                &mut commands,
                &assets.footstep,
                MatchAudioCue::RivalBleed,
                "Rival sound bleed",
                None,
                relation,
                distance,
                &settings,
            );
            let team_id = game.competitive.teams[team_index].id;
            sightings.record(team_id, room, SightingKind::Heard, commits);
        }
    }
    bleed.last_heard = present;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_match_stings(
    mut commands: Commands,
    runtime: Res<MatchDirector>,
    assets: Res<MatchAssets>,
    settings: Res<Settings>,
    tp: Res<TeleportState>,
    guardian: Option<Res<crate::guardian::Guardian>>,
    mut audio_state: ResMut<MatchAudioState>,
    mut director: ResMut<AudioDirector>,
) {
    let klaxon_active = crate::screens::match_runtime::ambience::countdown_klaxon_active(&runtime);
    if klaxon_active {
        director.request(
            &mut commands,
            &assets.klaxon,
            MatchAudioCue::Klaxon,
            "Klaxon alarm",
            None,
            &settings,
        );
    } else {
        director.stop(&mut commands, MatchAudioCue::Klaxon);
    }

    let game = runtime.live.host_match();
    let collapse_state =
        crate::screens::match_runtime::ambience::collapse_state_for_place(game, tp.place);
    if collapse_state == CollapseState::Dying {
        if audio_state.collapse_sting_place != Some(tp.place) {
            let triggered = director.request(
                &mut commands,
                &assets.collapse_sting,
                MatchAudioCue::CollapseSting,
                "Collapse warning sting",
                None,
                &settings,
            );
            if triggered {
                audio_state.collapse_sting_place = Some(tp.place);
            }
        }
    } else if collapse_state == CollapseState::Intact {
        audio_state.collapse_sting_place = None;
    }

    if let Some(guardian) = guardian.as_ref()
        && let Some((relation, distance)) =
            guardian_audio_relation(runtime.live.host_match(), &tp, guardian)
    {
        director.request_spatial(
            &mut commands,
            &assets.guardian_dread,
            MatchAudioCue::GuardianDread,
            "Guardian dread",
            None,
            relation,
            distance,
            &settings,
        );
    }
}

fn guardian_audio_relation(
    game: &observed_match::hybrid::HybridMatch,
    tp: &TeleportState,
    guardian: &crate::guardian::Guardian,
) -> Option<(AudioSourceRelation, f32)> {
    let body = Vec2::new(tp.body.position.x, tp.body.position.z);
    match tp.place {
        Place::Room(room) if room == guardian.room => {
            let pos = Vec2::new(guardian.pos.x, guardian.pos.z);
            Some((AudioSourceRelation::SamePlace, body.distance(pos)))
        }
        Place::Room(room) => {
            if let Some(gap) = tp.geom.gaps.iter().find(|gap| gap.target == guardian.room) {
                Some((
                    AudioSourceRelation::ThroughThreshold,
                    body.distance(gap.center),
                ))
            } else if connections_for(game, room).contains(&guardian.room) {
                Some((AudioSourceRelation::ThroughWall, PLACE_TILE))
            } else {
                None
            }
        }
        Place::Hallway { from, to, .. } if guardian.room == from || guardian.room == to => {
            if let Some(gap) = tp.geom.gaps.iter().find(|gap| gap.target == guardian.room) {
                Some((
                    AudioSourceRelation::ThroughThreshold,
                    body.distance(gap.center),
                ))
            } else {
                Some((AudioSourceRelation::ThroughThreshold, PLACE_TILE * 0.5))
            }
        }
        Place::Hallway { .. } => None,
    }
}

pub(crate) fn play_ui_sound(
    commands: &mut Commands,
    director: Option<&mut AudioDirector>,
    sound: &Option<Handle<AudioSource>>,
    cue: MatchAudioCue,
    settings: &Settings,
) -> bool {
    if let Some(dir) = director {
        dir.request(commands, sound, cue, "UI sound", None, settings)
    } else {
        // Fallback when outside GameState::Match (no AudioDirector)
        let volume = settings.effective_sfx_volume();
        let Some(volume) = audible(volume) else {
            return false;
        };
        if let Some(sound) = sound {
            commands.spawn((
                cue,
                AudioPlayer(sound.clone()),
                PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
            ));
            true
        } else {
            false
        }
    }
}

pub(crate) fn fade_ambience_beds(
    time: Res<Time>,
    tp: Res<TeleportState>,
    settings: Res<Settings>,
    director: Res<AudioDirector>,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
    mut query: Query<(&mut AmbienceBed, &mut AudioSink)>,
) {
    let seed_val = seed.map(|s| s.0).unwrap_or(crate::flow::MATCH_SEED);
    let active_bed = active_ambience_bed(seed_val, tp.place, !tp.geom.decks.is_empty());
    let music_volume = settings.effective_music_volume();
    let music_duck = director.bus_duck_factor(AudioBus::Music);
    let dt = (time.delta_secs() * DISTRICT_AMBIENCE_BLEND_RATE).clamp(0.0, 1.0);

    for (mut bed, mut sink) in &mut query {
        let target = if bed.kind == active_bed { 1.0 } else { 0.0 };
        bed.fade_factor += (target - bed.fade_factor) * dt;
        let volume = bed.fade_factor * DISTRICT_AMBIENCE_VOLUME * music_volume * music_duck;
        sink.set_volume(Volume::Linear(volume.clamp(0.0, 1.0)));
    }
}
