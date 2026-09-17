//! Shared sound design. Reads events and motion; never writes simulation state.
//!
//! The palette, the mixing and the motion cues are lab-agnostic: a lab supplies
//! a [`SoundWorld`] and chooses which [`Cue`] its own events map to, and this
//! module owns everything else. That is what lets a second lab have the same
//! sound without a second copy of it drifting out of sync.
use crate::{
    arena,
    model::{Action, Event, Kind, Outcome, Refusal},
    runtime::Runtime,
    view::Scene,
};
use bevy::{
    audio::{SpatialScale, Volume},
    prelude::*,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Push,
    Pull,
    Miss,
    Empty,
    Impact,
    Step,
    GuardianStep,
    GuardianVoice,
    Void,
    Warning,
    Retract,
    PowerOn,
    PowerOff,
    Wave,
    Clear,
    Capture,
    Recharge,
    ChargeTick,
    Land,
}
const CUES: [(Cue, &str); 19] = [
    (Cue::Push, "push"),
    (Cue::Pull, "pull"),
    (Cue::Miss, "miss"),
    (Cue::Empty, "empty"),
    (Cue::Impact, "impact"),
    (Cue::Step, "step"),
    (Cue::GuardianStep, "guardian_step"),
    (Cue::GuardianVoice, "guardian_voice"),
    (Cue::Void, "void"),
    (Cue::Warning, "warning"),
    (Cue::Retract, "retract"),
    (Cue::PowerOn, "power_on"),
    (Cue::PowerOff, "power_off"),
    (Cue::Wave, "wave"),
    (Cue::Clear, "clear"),
    (Cue::Capture, "capture"),
    (Cue::Recharge, "recharge"),
    (Cue::ChargeTick, "charge_tick"),
    (Cue::Land, "land"),
];
#[derive(Resource)]
pub struct Bank(Vec<Handle<AudioSource>>);
#[derive(Component)]
pub struct Voice {
    gain: f32,
    generation: u32,
}

/// One body, as the sound design needs to see it.
#[derive(Clone, Copy, Debug)]
pub struct MotionSample {
    pub position: Vec3,
    pub velocity: Vec3,
    pub grounded: bool,
    pub staggered: bool,
    /// Minors have footsteps and a voice; crates do not.
    pub is_minor: bool,
}

/// The Observer, as the sound design needs to see them.
#[derive(Clone, Copy, Debug)]
pub struct PlayerSample {
    pub position: Vec3,
    pub velocity: Vec3,
    pub grounded: bool,
}

/// What a lab has to be able to answer for its world to be audible.
pub trait SoundWorld {
    fn tick(&self) -> u64;
    /// Bumped on reset, so voices from a previous attempt can be cut.
    fn generation(&self) -> u32;
    fn paused(&self) -> bool;
    fn finished(&self) -> bool;
    fn ear(&self) -> Vec3;
    /// Whether architecture stands between the ear and a point.
    fn occluded(&self, at: Vec3) -> bool;
    fn player(&self) -> PlayerSample;
    fn charge(&self) -> f32;
    /// Every live body, keyed by a stable id.
    fn bodies(&self) -> Vec<(u32, MotionSample)>;
}

#[derive(Default)]
struct Motion {
    position: Vec3,
    velocity: Vec3,
    grounded: bool,
    distance: f32,
    impact_tick: u64,
    voice_tick: u64,
}
#[derive(Resource, Default)]
pub struct SoundState {
    pub muted: bool,
    generation: Option<u32>,
    tick: u64,
    refused_tick: Option<u64>,
    actors: BTreeMap<u32, Motion>,
    player: Option<Motion>,
    charge: f32,
    charge_tick: u64,
}
pub fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Bank(
        CUES.iter()
            .map(|(_, name)| assets.load(format!("sounds/kinetic/{name}.ogg")))
            .collect(),
    ));
}
fn event_cue(event: &Event) -> Cue {
    match event {
        Event::Fired(Action::Pull, ..) => Cue::Pull,
        Event::Fired(..) => Cue::Push,
        Event::Refused(Refusal::EmptyCharge) => Cue::Empty,
        Event::Refused(_) => Cue::Miss,
        Event::Eliminated(_) => Cue::Void,
        Event::Power(true) => Cue::PowerOn,
        Event::Power(false) => Cue::PowerOff,
        Event::Retracting => Cue::Warning,
        Event::Retracted => Cue::Retract,
        Event::Wave(_) => Cue::Wave,
        Event::Ended(Outcome::Cleared) => Cue::Clear,
        Event::Ended(Outcome::Fell) => Cue::Void,
        Event::Ended(_) => Cue::Capture,
        Event::Recharge => Cue::Recharge,
    }
}
/// Spawn one voice. Returns the entity so a lab can tag it with its own
/// scene marker.
///
/// Distance falloff, occlusion damping and the small pitch jitter live here
/// rather than in either lab, so the two cannot drift apart.
pub fn play_cue(
    commands: &mut Commands,
    bank: &Bank,
    world: &impl SoundWorld,
    cue: Cue,
    gain: f32,
    position: Option<Vec3>,
    variant: u32,
) -> Option<Entity> {
    let index = CUES.iter().position(|(c, _)| *c == cue)?;
    let mut settings = PlaybackSettings::DESPAWN;
    settings.speed = 0.97 + (variant % 7) as f32 * 0.01;
    let mut gain = gain;
    if let Some(at) = position {
        let distance = world.ear().distance(at);
        if distance > 24. {
            return None;
        }
        gain *= 1. / (1. + distance * distance / 36.);
        if world.occluded(at) {
            gain *= 0.55;
        }
        settings.spatial = true;
        settings.spatial_scale = Some(SpatialScale::new(0.06));
    }
    settings.volume = Volume::Linear(gain);
    Some(
        commands
            .spawn((
                Voice {
                    gain,
                    generation: world.generation(),
                },
                AudioPlayer::new(bank.0[index].clone()),
                settings,
                Transform::from_translation(position.unwrap_or_default()),
            ))
            .id(),
    )
}

fn play(
    commands: &mut Commands,
    bank: &Bank,
    runtime: &Runtime,
    cue: Cue,
    gain: f32,
    position: Option<Vec3>,
    variant: u32,
) {
    if let Some(entity) = play_cue(commands, bank, runtime, cue, gain, position, variant) {
        commands.entity(entity).insert(Scene);
    }
}

/// Whether a refusal should be heard, or is one of a burst held down on a key.
pub fn refusal_is_audible(sound: &mut SoundState, world: &impl SoundWorld) -> bool {
    if sound.generation == Some(world.generation())
        && sound
            .refused_tick
            .is_some_and(|at| world.tick().saturating_sub(at) < 12)
    {
        return false;
    }
    sound.refused_tick = Some(world.tick());
    true
}

pub fn event(
    commands: &mut Commands,
    bank: &Bank,
    sound: &mut SoundState,
    runtime: &Runtime,
    event: &Event,
) {
    if matches!(event, Event::Refused(_)) && !refusal_is_audible(sound, runtime) {
        return;
    }
    let position = match event {
        Event::Eliminated(id) => Some(runtime.world.pose(*id).position),
        Event::Power(_) => Some(arena::GENERATOR + Vec3::Y),
        Event::Retracting | Event::Retracted => Some(arena::PANEL + Vec3::Y),
        _ => None,
    };
    play(
        commands,
        bank,
        runtime,
        event_cue(event),
        0.8,
        position,
        runtime.world.tick as u32,
    );
}
pub fn motion(
    mut commands: Commands,
    runtime: Res<Runtime>,
    bank: Res<Bank>,
    mut sound: ResMut<SoundState>,
) {
    let spawned = motion_from(&mut commands, &bank, &mut sound, &*runtime);
    for entity in spawned {
        commands.entity(entity).insert(Scene);
    }
}

/// Footsteps, impacts, Guardian voices, landings and charge ticks, from one
/// lab-agnostic view of the world. Returns the voices it spawned so the caller
/// can tag them.
pub fn motion_from(
    commands: &mut Commands,
    bank: &Bank,
    sound: &mut SoundState,
    world: &impl SoundWorld,
) -> Vec<Entity> {
    let mut spawned = Vec::new();
    let emit = |commands: &mut Commands, cue, gain, at, variant, spawned: &mut Vec<Entity>| {
        if let Some(entity) = play_cue(commands, bank, world, cue, gain, at, variant) {
            spawned.push(entity);
        }
    };
    let tick = world.tick();
    if sound.generation != Some(world.generation()) {
        let muted = sound.muted;
        *sound = SoundState {
            muted,
            generation: Some(world.generation()),
            charge: world.charge(),
            ..default()
        };
    }
    if world.paused() || sound.tick == tick {
        return spawned;
    }
    sound.tick = tick;
    for (id, body) in world.bodies() {
        let Some(previous) = sound.actors.get_mut(&id) else {
            sound.actors.insert(
                id,
                Motion {
                    position: body.position,
                    velocity: body.velocity,
                    grounded: body.grounded,
                    voice_tick: tick + u64::from(id % 5) * 41,
                    ..default()
                },
            );
            continue;
        };
        let travelled = body.position.distance(previous.position);
        let mut steps = None;
        if body.grounded && previous.grounded && travelled < 1. && !body.staggered {
            previous.distance += travelled;
            if body.is_minor && previous.distance > 0.8 {
                previous.distance %= 0.8;
                steps = Some(id + tick as u32);
            }
        }
        let impact = previous.velocity.length() > 3.5
            && previous.velocity.length() - body.velocity.length() > 2.5
            && tick.saturating_sub(previous.impact_tick) > 18;
        if impact {
            previous.impact_tick = tick;
        }
        let voice = body.is_minor
            && tick > previous.voice_tick + 240
            && body.position.distance(world.ear()) < 12.;
        if voice {
            previous.voice_tick = tick;
        }
        previous.position = body.position;
        previous.velocity = body.velocity;
        previous.grounded = body.grounded;
        if let Some(variant) = steps {
            emit(
                commands,
                Cue::GuardianStep,
                0.6,
                Some(body.position),
                variant,
                &mut spawned,
            );
        }
        if impact {
            emit(
                commands,
                Cue::Impact,
                0.65,
                Some(body.position),
                id,
                &mut spawned,
            );
        }
        if voice {
            emit(
                commands,
                Cue::GuardianVoice,
                0.45,
                Some(body.position),
                id * 3,
                &mut spawned,
            );
        }
    }
    let player = world.player();
    let mut player_cue = None;
    if let Some(previous) = &mut sound.player {
        let travelled = player.position.distance(previous.position);
        if player.grounded && previous.grounded && travelled < 1. {
            previous.distance += travelled;
            if previous.distance > 1.65 {
                previous.distance %= 1.65;
                player_cue = Some(Cue::Step);
            }
        }
        if player.grounded && !previous.grounded && previous.velocity.y < -2. {
            player_cue = Some(Cue::Land);
        }
        previous.position = player.position;
        previous.velocity = player.velocity;
        previous.grounded = player.grounded;
    } else {
        sound.player = Some(Motion {
            position: player.position,
            velocity: player.velocity,
            grounded: player.grounded,
            ..default()
        });
    }
    if let Some(cue) = player_cue {
        let gain = if cue == Cue::Land { 0.7 } else { 0.55 };
        emit(commands, cue, gain, None, tick as u32, &mut spawned);
    }
    let charge = world.charge();
    let ticked =
        charge > sound.charge && charge < 100. && tick.saturating_sub(sound.charge_tick) > 30;
    if ticked {
        sound.charge_tick = tick;
        emit(
            commands,
            Cue::ChargeTick,
            0.5,
            None,
            charge as u32,
            &mut spawned,
        );
    }
    sound.charge = charge;
    spawned
}

/// Every playing voice, with whichever sink kind it happens to own.
pub type Voices<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Voice,
        &'static mut PlaybackSettings,
        Option<&'static mut AudioSink>,
        Option<&'static mut SpatialAudioSink>,
    ),
>;

pub fn mix(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    runtime: Res<Runtime>,
    mut sound: ResMut<SoundState>,
    mut voices: Voices,
) {
    if keys.just_pressed(KeyCode::KeyM) {
        sound.muted = !sound.muted;
    }
    mix_voices(&mut commands, &sound, &*runtime, &mut voices);
}

/// Hold every live voice at the right volume, pause it with the simulation, and
/// cut anything left over from a previous attempt.
pub fn mix_voices(
    commands: &mut Commands,
    sound: &SoundState,
    world: &impl SoundWorld,
    voices: &mut Voices,
) {
    for (entity, voice, mut settings, sink, spatial) in voices.iter_mut() {
        if voice.generation != world.generation() {
            commands.entity(entity).despawn();
            continue;
        }
        let volume = Volume::Linear(if sound.muted { 0. } else { voice.gain });
        let paused = world.paused() && !world.finished();
        settings.volume = volume;
        settings.paused = paused;
        if let Some(mut sink) = sink {
            sink.set_volume(volume);
            if paused {
                sink.pause();
            } else {
                sink.play();
            }
        }
        if let Some(mut sink) = spatial {
            sink.set_volume(volume);
            if paused {
                sink.pause();
            } else {
                sink.play();
            }
        }
    }
}

impl SoundWorld for Runtime {
    fn tick(&self) -> u64 {
        self.world.tick
    }
    fn generation(&self) -> u32 {
        self.generation
    }
    fn paused(&self) -> bool {
        self.paused
    }
    fn finished(&self) -> bool {
        self.world.outcome != Outcome::Playing
    }
    fn ear(&self) -> Vec3 {
        self.world.eye()
    }
    fn occluded(&self, at: Vec3) -> bool {
        !self.world.line_clear(self.world.eye(), at)
    }
    fn player(&self) -> PlayerSample {
        PlayerSample {
            position: self.world.player.position,
            velocity: self.world.player.velocity,
            grounded: self.world.player.grounded,
        }
    }
    fn charge(&self) -> f32 {
        self.world.charge
    }
    fn bodies(&self) -> Vec<(u32, MotionSample)> {
        self.world
            .actors
            .iter()
            .filter(|(_, actor)| actor.alive)
            .map(|(id, actor)| {
                (
                    id.0,
                    MotionSample {
                        position: self.world.pose(*id).position,
                        velocity: self.world.velocity(*id),
                        grounded: actor.grounded,
                        staggered: actor.stagger > 0,
                        is_minor: actor.kind == Kind::Minor,
                    },
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ActorId;
    #[test]
    fn palette_is_complete_and_success_does_not_sound_like_capture() {
        for (_, name) in CUES {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../assets/sounds/kinetic/{name}.ogg"));
            let bytes = std::fs::read(path).unwrap();
            assert!(bytes.starts_with(b"OggS"));
        }
        assert_ne!(
            event_cue(&Event::Ended(Outcome::Cleared)),
            event_cue(&Event::Ended(Outcome::Captured))
        );
        assert_ne!(
            event_cue(&Event::Fired(Action::Push, ActorId(1), Vec3::ZERO)),
            event_cue(&Event::Fired(Action::Pull, ActorId(1), Vec3::ZERO))
        );
    }
}
