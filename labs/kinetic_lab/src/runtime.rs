//! Bevy scheduling adapter. Hardware produces commands; simulation owns outcomes.
use crate::model::{Action, ActorId, Command, Event, KineticWorld, Mode, Pose};
use bevy::prelude::*;
use player_input::PlayerIntent;
use std::collections::{BTreeMap, VecDeque};

#[derive(Resource)]
pub struct Runtime {
    pub world: KineticWorld,
    pub demo_label: Option<&'static str>,
    pub paused: bool,
    pub diagnostics: bool,
    pub generation: u32,
    pub movement: PlayerIntent,
    pub pending: VecDeque<Action>,
    pub events: Vec<Event>,
    pub previous: BTreeMap<ActorId, Pose>,
    pub previous_player: Vec3,
    pub step_once: bool,
}
impl Default for Runtime {
    fn default() -> Self {
        Self::new(Mode::Practice)
    }
}
impl Runtime {
    pub fn new(mode: Mode) -> Self {
        let world = KineticWorld::new(mode);
        let previous_player = world.player.position;
        Self {
            world,
            demo_label: None,
            paused: true,
            diagnostics: false,
            generation: 0,
            movement: PlayerIntent::default(),
            pending: VecDeque::new(),
            events: Vec::new(),
            previous: BTreeMap::new(),
            previous_player,
            step_once: false,
        }
    }
    pub fn reset(&mut self, mode: Mode) {
        let generation = self.generation + 1;
        let diagnostics = self.diagnostics;
        *self = Self::new(mode);
        self.generation = generation;
        self.diagnostics = diagnostics;
    }
    pub fn tick(&mut self) {
        if self.paused && !self.step_once {
            return;
        }
        self.step_once = false;
        self.previous = self
            .world
            .actors
            .keys()
            .map(|id| (*id, self.world.pose(*id)))
            .collect();
        self.previous_player = self.world.player.position;
        let command = Command {
            movement: self.movement,
            action: self.pending.pop_front().unwrap_or_default(),
        };
        self.movement.look = Vec2::ZERO;
        self.movement.jump_pressed = false;
        self.world.step(command);
        self.events.extend(self.world.events.iter().cloned());
    }
    pub fn pause(&mut self) {
        self.paused = true;
        self.movement = PlayerIntent::default();
        self.pending.clear();
    }
}
pub fn fixed_step(mut runtime: ResMut<Runtime>, capture: Option<Res<crate::evidence::Capture>>) {
    if capture.is_none() {
        runtime.tick();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Runtime>()
            .init_schedule(Update)
            .add_systems(FixedUpdate, fixed_step);
        app
    }
    #[test]
    fn render_schedules_do_not_change_tick_results_or_repeat_a_press() {
        let mut a = app();
        let mut b = app();
        for app in [&mut a, &mut b] {
            let mut r = app.world_mut().resource_mut::<Runtime>();
            r.paused = false;
            r.pending.push_back(Action::Push);
        }
        for tick in 0..120 {
            a.world_mut().run_schedule(FixedUpdate);
            if tick % 3 == 0 {
                a.world_mut().run_schedule(Update);
            }
            b.world_mut().run_schedule(FixedUpdate);
            assert_eq!(
                a.world().resource::<Runtime>().world.digest(),
                b.world().resource::<Runtime>().world.digest()
            );
        }
        assert!(a.world().resource::<Runtime>().pending.is_empty());
        assert_eq!(
            a.world()
                .resource::<Runtime>()
                .events
                .iter()
                .filter(|e| matches!(e, Event::Fired(..) | Event::Refused(..)))
                .count(),
            1
        );
    }
    #[test]
    fn repeated_modes_reset_all_rules_and_queued_state() {
        let mut r = Runtime::default();
        for i in 1..=10 {
            let mode = if i % 2 == 0 {
                Mode::Practice
            } else {
                Mode::Encounter
            };
            r.paused = false;
            r.pending.push_back(Action::Push);
            r.tick();
            r.reset(mode);
            let fresh = KineticWorld::new(mode);
            assert_eq!(r.world.digest(), fresh.digest());
            assert!(r.pending.is_empty());
            assert!(r.events.is_empty());
            assert_eq!(r.world.physics.bodies.len(), fresh.physics.bodies.len());
            assert_eq!(
                r.world.physics.colliders.len(),
                fresh.physics.colliders.len()
            );
            assert_eq!(r.generation, i);
        }
    }
}
