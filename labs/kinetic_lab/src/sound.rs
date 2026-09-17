//! Lab-owned sound design. Reads events and motion; never writes simulation state.
use crate::{
    arena,
    model::{Action, ActorId, Event, Kind, Outcome, Refusal},
    runtime::Runtime,
    view::Scene,
};
use bevy::{
    audio::{SpatialScale, Volume},
    prelude::*,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cue {
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
    actors: BTreeMap<ActorId, Motion>,
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
fn play(
    commands: &mut Commands,
    bank: &Bank,
    runtime: &Runtime,
    cue: Cue,
    gain: f32,
    position: Option<Vec3>,
    variant: u32,
) {
    let index = CUES.iter().position(|(c, _)| *c == cue).unwrap();
    let mut settings = PlaybackSettings::DESPAWN;
    settings.speed = 0.97 + (variant % 7) as f32 * 0.01;
    let mut gain = gain;
    if let Some(p) = position {
        let distance = runtime.world.eye().distance(p);
        if distance > 24. {
            return;
        }
        gain *= 1. / (1. + distance * distance / 36.);
        if !runtime.world.line_clear(runtime.world.eye(), p) {
            gain *= 0.55;
        }
        settings.spatial = true;
        settings.spatial_scale = Some(SpatialScale::new(0.06));
    }
    settings.volume = Volume::Linear(gain);
    commands.spawn((
        Scene,
        Voice {
            gain,
            generation: runtime.generation,
        },
        AudioPlayer::new(bank.0[index].clone()),
        settings,
        Transform::from_translation(position.unwrap_or_default()),
    ));
}
pub fn event(
    commands: &mut Commands,
    bank: &Bank,
    sound: &mut SoundState,
    runtime: &Runtime,
    event: &Event,
) {
    if matches!(event, Event::Refused(_)) {
        if sound.generation == Some(runtime.generation)
            && sound
                .refused_tick
                .is_some_and(|t| runtime.world.tick.saturating_sub(t) < 12)
        {
            return;
        }
        sound.refused_tick = Some(runtime.world.tick);
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
    let w = &runtime.world;
    if sound.generation != Some(runtime.generation) {
        let muted = sound.muted;
        *sound = SoundState {
            muted,
            generation: Some(runtime.generation),
            charge: w.charge,
            ..default()
        };
    }
    if runtime.paused || sound.tick == w.tick {
        return;
    }
    sound.tick = w.tick;
    for (&id, actor) in &w.actors {
        if !actor.alive {
            continue;
        }
        let p = w.pose(id).position;
        let v = w.velocity(id);
        let Some(previous) = sound.actors.get_mut(&id) else {
            sound.actors.insert(
                id,
                Motion {
                    position: p,
                    velocity: v,
                    grounded: actor.grounded,
                    voice_tick: w.tick + u64::from(id.0 % 5) * 41,
                    ..default()
                },
            );
            continue;
        };
        let distance = p.distance(previous.position);
        if actor.grounded && previous.grounded && distance < 1. && actor.stagger == 0 {
            previous.distance += distance;
            if actor.kind == Kind::Minor && previous.distance > 0.8 {
                previous.distance %= 0.8;
                play(
                    &mut commands,
                    &bank,
                    &runtime,
                    Cue::GuardianStep,
                    0.6,
                    Some(p),
                    id.0 + w.tick as u32,
                );
            }
        }
        if previous.velocity.length() > 3.5
            && previous.velocity.length() - v.length() > 2.5
            && w.tick.saturating_sub(previous.impact_tick) > 18
        {
            play(
                &mut commands,
                &bank,
                &runtime,
                Cue::Impact,
                0.65,
                Some(p),
                id.0,
            );
            previous.impact_tick = w.tick;
        }
        if actor.kind == Kind::Minor
            && w.tick > previous.voice_tick + 240
            && p.distance(w.eye()) < 12.
        {
            play(
                &mut commands,
                &bank,
                &runtime,
                Cue::GuardianVoice,
                0.45,
                Some(p),
                id.0 * 3,
            );
            previous.voice_tick = w.tick;
        }
        previous.position = p;
        previous.velocity = v;
        previous.grounded = actor.grounded;
    }
    let p = w.player.position;
    if let Some(previous) = &mut sound.player {
        let distance = p.distance(previous.position);
        if w.player.grounded && previous.grounded && distance < 1. {
            previous.distance += distance;
            if previous.distance > 1.65 {
                previous.distance %= 1.65;
                play(
                    &mut commands,
                    &bank,
                    &runtime,
                    Cue::Step,
                    0.55,
                    None,
                    w.tick as u32,
                );
            }
        }
        if w.player.grounded && !previous.grounded && previous.velocity.y < -2. {
            play(
                &mut commands,
                &bank,
                &runtime,
                Cue::Land,
                0.7,
                None,
                w.tick as u32,
            );
        }
        previous.position = p;
        previous.velocity = w.player.velocity;
        previous.grounded = w.player.grounded;
    } else {
        sound.player = Some(Motion {
            position: p,
            velocity: w.player.velocity,
            grounded: w.player.grounded,
            ..default()
        });
    }
    if w.charge > sound.charge && w.charge < 100. && w.tick.saturating_sub(sound.charge_tick) > 30 {
        play(
            &mut commands,
            &bank,
            &runtime,
            Cue::ChargeTick,
            0.5,
            None,
            w.charge as u32,
        );
        sound.charge_tick = w.tick;
    }
    sound.charge = w.charge;
}
/// Every playing voice, with whichever sink kind it happens to own.
type Voices<'w, 's> = Query<
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
    for (entity, voice, mut settings, sink, spatial) in &mut voices {
        if voice.generation != runtime.generation {
            commands.entity(entity).despawn();
            continue;
        }
        let volume = Volume::Linear(if sound.muted { 0. } else { voice.gain });
        let paused = runtime.paused && runtime.world.outcome == Outcome::Playing;
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
#[cfg(test)]
mod tests {
    use super::*;
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
