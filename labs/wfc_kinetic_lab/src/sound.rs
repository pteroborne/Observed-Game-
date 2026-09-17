//! This lab's voice, using the shared kinetic sound design.
//!
//! The palette, the falloff, the motion cues and the mixer all live in
//! `kinetic_lab::sound`. What belongs here is only what is particular to a
//! solved floor: which of this lab's events map to which cue, and where in the
//! facility each one happens.

#[cfg(test)]
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use kinetic_lab::sound::{
    Bank, Cue, MotionSample, PlayerSample, SoundState, SoundWorld, Voices, mix_voices, motion_from,
    play_cue, refusal_is_audible,
};
use observed_hex::hex_origin;

use crate::model::{Action, Event, Kind, Outcome, Refusal};
use crate::runtime::Runtime;
use crate::view::FacilityVisual;

/// This lab's world, as the shared sound design needs to see it.
pub struct Audible<'a>(pub &'a Runtime);

impl SoundWorld for Audible<'_> {
    fn tick(&self) -> u64 {
        self.0.world.tick
    }
    fn generation(&self) -> u32 {
        self.0.generation
    }
    fn paused(&self) -> bool {
        self.0.paused
    }
    fn finished(&self) -> bool {
        self.0.world.outcome != Outcome::Playing
    }
    fn ear(&self) -> Vec3 {
        self.0.world.eye()
    }
    fn occluded(&self, at: Vec3) -> bool {
        !self.0.world.line_clear(self.0.world.eye(), at)
    }
    fn player(&self) -> PlayerSample {
        PlayerSample {
            position: self.0.world.player.position,
            velocity: self.0.world.player.velocity,
            grounded: self.0.world.player.grounded,
        }
    }
    fn charge(&self) -> f32 {
        self.0.world.charge
    }
    fn bodies(&self) -> Vec<(u32, MotionSample)> {
        self.0
            .world
            .actors
            .iter()
            .filter(|(_, actor)| actor.alive)
            .map(|(id, actor)| {
                (
                    id.0,
                    MotionSample {
                        position: self.0.world.pose(*id).position,
                        velocity: self.0.world.velocity(*id),
                        grounded: actor.grounded,
                        staggered: actor.stagger > 0,
                        is_minor: actor.kind == Kind::Minor,
                    },
                )
            })
            .collect()
    }
}

/// Which cue an event of this lab's is.
///
/// Total by construction rather than by a catch-all arm, so an event added to
/// the simulation cannot go silent without the compiler saying so.
#[must_use]
pub fn cue(event: &Event) -> Cue {
    match event {
        Event::Fired(Action::Pull, ..) => Cue::Pull,
        Event::Fired(..) => Cue::Push,
        Event::Refused(Refusal::EmptyCharge) => Cue::Empty,
        Event::Refused(_) => Cue::Miss,
        Event::Eliminated(..) => Cue::Void,
        Event::Power(true) => Cue::PowerOn,
        Event::Power(false) => Cue::PowerOff,
        Event::Decohering(_) => Cue::Warning,
        Event::Relaid(_) => Cue::Retract,
        // The floor refusing to change is the Observer's doing. It gets the
        // power cue rather than the retraction one: something switched off,
        // and nothing fell.
        Event::Held => Cue::PowerOff,
        Event::Inert => Cue::Miss,
        Event::Retracted(_) => Cue::Retract,
        // The bank has no plumb of its own yet. Pull is the closest thing in
        // it — a sustained draw rather than an impact — and arming borrows the
        // charge tick, which is already the sound of the tool getting ready.
        Event::Plumbed(..) => Cue::Pull,
        Event::Unplumbed(_) => Cue::ChargeTick,
        Event::Armed(_) => Cue::ChargeTick,
        Event::Wave(_) => Cue::Wave,
        Event::Ended(Outcome::Cleared) => Cue::Clear,
        Event::Ended(Outcome::Fell) => Cue::Void,
        Event::Ended(_) => Cue::Capture,
        Event::Recharge => Cue::Recharge,
    }
}

/// Where in the facility an event happens, or `None` for one that belongs to
/// the Observer rather than to a place.
#[must_use]
pub fn origin(runtime: &Runtime, event: &Event) -> Option<Vec3> {
    let site = &runtime.site;
    match event {
        // A body leaving the facility is heard where it went over, which on a
        // solved floor is a cell rather than an authored prop.
        Event::Eliminated(_, Some(cell)) => {
            Some(Vec3::from_array(hex_origin(*cell)) + Vec3::Y * 0.5)
        }
        Event::Power(_) => Some(site.generator + Vec3::Y),
        // The warning is heard at the pocket that is about to go, not at the
        // panel: what matters is which way to look to save it.
        Event::Decohering(cell) | Event::Relaid(cell) | Event::Retracted(cell) => {
            Some(Vec3::from_array(hex_origin(*cell)) + Vec3::Y)
        }
        Event::Recharge => Some(site.station + Vec3::Y),
        // Heard where the body is, not at the Observer: the point of the plumb
        // is that something over there is now falling a different way.
        Event::Plumbed(id, _) | Event::Unplumbed(id) => runtime
            .world
            .actors
            .get(id)
            .filter(|actor| actor.alive)
            .map(|actor| runtime.world.pose(actor.id).position),
        _ => None,
    }
}

/// Play one event.
pub fn event(
    commands: &mut Commands,
    bank: &Bank,
    sound: &mut SoundState,
    runtime: &Runtime,
    event: &Event,
) {
    let world = Audible(runtime);
    if matches!(event, Event::Refused(_)) && !refusal_is_audible(sound, &world) {
        return;
    }
    if let Some(entity) = play_cue(
        commands,
        bank,
        &world,
        cue(event),
        0.8,
        origin(runtime, event),
        runtime.world.tick as u32,
    ) {
        commands.entity(entity).insert(FacilityVisual);
    }
}

pub fn motion(
    mut commands: Commands,
    runtime: Res<Runtime>,
    bank: Res<Bank>,
    mut sound: ResMut<SoundState>,
) {
    let spawned = motion_from(&mut commands, &bank, &mut sound, &Audible(&runtime));
    for entity in spawned {
        commands.entity(entity).insert(FacilityVisual);
    }
}

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
    mix_voices(&mut commands, &sound, &Audible(&runtime), &mut voices);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ActorId, Mode};
    use crate::site::{Site, load_content};
    use observed_hex::HexCoord;
    use std::sync::{Arc, OnceLock};

    fn site() -> Arc<Site> {
        static SITE: OnceLock<Arc<Site>> = OnceLock::new();
        Arc::clone(SITE.get_or_init(|| {
            Arc::new(Site::solve(crate::DEFAULT_SEED, &load_content()).expect("solves"))
        }))
    }

    /// The floor has to be audible in practice, not merely in principle.
    ///
    /// Drives the recorded loop and counts the voices it spawns. No audio
    /// device is involved: a voice is an entity, and this asserts the lab
    /// creates them from its own events and motion rather than silently
    /// dropping every cue on the floor.
    #[test]
    fn playing_the_loop_spawns_voices() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin {
                file_path: format!("{}/../../assets", env!("CARGO_MANIFEST_DIR")),
                ..default()
            })
            .init_asset::<AudioSource>()
            .init_resource::<SoundState>()
            .insert_resource(Runtime::new(site(), Mode::Encounter))
            .add_systems(Startup, kinetic_lab::sound::setup);
        app.finish();
        app.update();

        let mut heard = 0usize;
        for tick in 0..900u32 {
            {
                let runtime = app.world().resource::<Runtime>();
                let command = crate::demo::loop_command(&runtime.world, tick);
                let mut runtime = app.world_mut().resource_mut::<Runtime>();
                runtime.paused = false;
                runtime.movement = command.movement;
                runtime.pending.push_back(command.action);
                runtime.tick();
            }
            // Events, then continuous motion, exactly as the view orders them.
            app.world_mut()
                .run_system_once(
                    |mut commands: Commands,
                     mut runtime: ResMut<Runtime>,
                     bank: Res<Bank>,
                     mut sound: ResMut<SoundState>| {
                        for event in std::mem::take(&mut runtime.events) {
                            super::event(&mut commands, &bank, &mut sound, &runtime, &event);
                        }
                    },
                )
                .expect("event pass runs");
            app.world_mut()
                .run_system_once(super::motion)
                .expect("motion pass runs");
            app.update();
            heard = app
                .world_mut()
                .query::<&kinetic_lab::sound::Voice>()
                .iter(app.world())
                .count()
                .max(heard);
        }
        assert!(
            heard > 0,
            "the loop played for 900 ticks without spawning a single voice"
        );
    }

    #[test]
    fn every_event_has_a_voice_and_the_endings_do_not_sound_alike() {
        let cell = HexCoord {
            q: 1,
            r: 0,
            level: 0,
        };
        // Winning, being caught and falling out of the facility are three
        // different pieces of news and must not arrive as the same sound.
        assert_ne!(
            cue(&Event::Ended(Outcome::Cleared)),
            cue(&Event::Ended(Outcome::Captured))
        );
        assert_ne!(
            cue(&Event::Ended(Outcome::Fell)),
            cue(&Event::Ended(Outcome::Captured))
        );
        assert_ne!(
            cue(&Event::Fired(Action::Push, ActorId(1), Vec3::ZERO)),
            cue(&Event::Fired(Action::Pull, ActorId(1), Vec3::ZERO))
        );
        // An empty pool is a different refusal from a miss: one says walk to
        // the station and the other says aim again.
        assert_ne!(
            cue(&Event::Refused(Refusal::EmptyCharge)),
            cue(&Event::Refused(Refusal::Blocked))
        );
        // The retraction beats are placed at the tile, not at the Observer.
        assert!(matches!(cue(&Event::Decohering(cell)), Cue::Warning));
        assert!(matches!(cue(&Event::Relaid(cell)), Cue::Retract));
        // Holding the floor and losing it must not sound the same.
        assert_ne!(cue(&Event::Held), cue(&Event::Relaid(cell)));
    }
}
