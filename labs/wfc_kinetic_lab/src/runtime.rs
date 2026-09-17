//! Bevy scheduling adapter. Hardware produces commands; simulation owns outcomes.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use bevy::prelude::*;
use player_input::PlayerIntent;

use crate::model::{Action, ActorId, Command, Event, Mode, Pose, WfcKineticWorld};
use crate::site::Site;

#[derive(Resource)]
pub struct Runtime {
    pub site: Arc<Site>,
    pub world: WfcKineticWorld,
    pub paused: bool,
    pub diagnostics: bool,
    /// Bumped on every reset so presentation knows to rebuild its entities.
    pub generation: u32,
    pub movement: PlayerIntent,
    pub pending: VecDeque<Action>,
    pub events: Vec<Event>,
    pub previous: BTreeMap<ActorId, Pose>,
    pub previous_player: Vec3,
    pub step_once: bool,
}

impl Runtime {
    #[must_use]
    pub fn new(site: Arc<Site>, mode: Mode) -> Self {
        let world = WfcKineticWorld::new(Arc::clone(&site), mode);
        let previous_player = world.player.position;
        Self {
            site,
            world,
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

    /// Rebuild the simulation on the same floor. The solve is not repeated: a
    /// reset is meant to restore the board, not deal a different one.
    pub fn reset(&mut self, mode: Mode) {
        let generation = self.generation + 1;
        let diagnostics = self.diagnostics;
        *self = Self::new(Arc::clone(&self.site), mode);
        self.generation = generation;
        self.diagnostics = diagnostics;
    }

    /// Solve a different floor and start on it.
    pub fn reseat(&mut self, site: Arc<Site>, mode: Mode) {
        let generation = self.generation + 1;
        let diagnostics = self.diagnostics;
        *self = Self::new(site, mode);
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
        // Look and jump are edge-triggered: consuming them here stops one press
        // from being replayed across a frame that renders more than once.
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
    use crate::site::load_content;
    use std::sync::OnceLock;

    fn site() -> Arc<Site> {
        static SITE: OnceLock<Arc<Site>> = OnceLock::new();
        Arc::clone(SITE.get_or_init(|| {
            Arc::new(Site::solve(crate::DEFAULT_SEED, &load_content()).expect("solves"))
        }))
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Runtime::new(site(), Mode::Encounter))
            .init_schedule(Update)
            .add_systems(FixedUpdate, fixed_step);
        app
    }

    #[test]
    fn render_schedules_do_not_change_tick_results_or_repeat_a_press() {
        let mut a = app();
        let mut b = app();
        for app in [&mut a, &mut b] {
            let mut runtime = app.world_mut().resource_mut::<Runtime>();
            runtime.paused = false;
            runtime.pending.push_back(Action::Push);
        }
        for tick in 0..120 {
            a.world_mut().run_schedule(FixedUpdate);
            if tick % 3 == 0 {
                a.world_mut().run_schedule(Update);
            }
            b.world_mut().run_schedule(FixedUpdate);
            assert_eq!(
                a.world().resource::<Runtime>().world.digest(),
                b.world().resource::<Runtime>().world.digest(),
                "extra render frames changed the simulation at tick {tick}"
            );
        }
        assert!(a.world().resource::<Runtime>().pending.is_empty());
        assert_eq!(
            a.world()
                .resource::<Runtime>()
                .events
                .iter()
                .filter(|event| matches!(event, Event::Fired(..) | Event::Refused(..)))
                .count(),
            1,
            "one press produced more than one outcome"
        );
    }

    #[test]
    fn repeated_resets_restore_the_floor_without_leaking_state() {
        let mut runtime = Runtime::new(site(), Mode::Practice);
        for index in 1..=8 {
            let mode = if index % 2 == 0 {
                Mode::Practice
            } else {
                Mode::Encounter
            };
            runtime.paused = false;
            runtime.pending.push_back(Action::Push);
            runtime.tick();
            runtime.reset(mode);
            let fresh = WfcKineticWorld::new(site(), mode);
            assert_eq!(runtime.world.digest(), fresh.digest());
            assert!(runtime.pending.is_empty());
            assert!(runtime.events.is_empty());
            assert_eq!(
                runtime.world.physics.bodies.len(),
                fresh.physics.bodies.len()
            );
            assert_eq!(
                runtime.world.physics.colliders.len(),
                fresh.physics.colliders.len(),
                "a reset leaked or dropped colliders"
            );
            assert_eq!(runtime.generation, index);
            // The floor is the same floor: a reset re-deals nothing.
            assert_eq!(runtime.world.site.seed, fresh.site.seed);
        }
    }
}
