//! Validated Observer actions shared by human seats and simulation adapters.

use observed_hex::{HexCoord, HexFace};

use super::{
    ArchitectLab, DoorState, GuardianId, MatchOutcome, ObserverId, ObserverIntent, ObserverState,
    ThresholdKey,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ObserverAction {
    #[default]
    None,
    Step(HexCoord),
    Door(ThresholdKey, DoorState),
    Shove(GuardianId),
    ToggleGenerator,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ObserverCommand {
    pub facing: Option<HexFace>,
    pub action: ObserverAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObserverRefusal {
    UnknownObserver,
    MatchFinished,
    Corrupted,
    NoPassage,
    OutOfReach,
    NoPower,
    InvalidTarget,
}

impl ArchitectLab {
    /// Read-only validation used by network and UI adapters.
    pub fn observer_refusal(
        &self,
        id: ObserverId,
        command: ObserverCommand,
    ) -> Option<ObserverRefusal> {
        self.observer_action(id, command).err()
    }

    fn observer_action(
        &self,
        id: ObserverId,
        command: ObserverCommand,
    ) -> Result<Option<ObserverIntent>, ObserverRefusal> {
        if self.outcome != MatchOutcome::Running {
            return Err(ObserverRefusal::MatchFinished);
        }
        let observer = self
            .observers
            .get(&id)
            .ok_or(ObserverRefusal::UnknownObserver)?;
        if observer.state == ObserverState::Corrupted {
            return Err(ObserverRefusal::Corrupted);
        }
        let cell = observer.cell;
        let intent = match command.action {
            ObserverAction::None => None,
            ObserverAction::Step(next) => {
                let reachable = if observer.state == ObserverState::Jailed {
                    self.prison
                        .graph
                        .get(&cell)
                        .is_some_and(|edges| edges.values().any(|&neighbor| neighbor == next))
                } else {
                    self.exits(cell).contains(&next)
                };
                if !reachable {
                    return Err(ObserverRefusal::NoPassage);
                }
                Some(ObserverIntent::Step(next))
            }
            ObserverAction::Door(key, state) => {
                if !self.doors.contains_key(&key) {
                    return Err(ObserverRefusal::InvalidTarget);
                }
                if !super::threshold_touches(key, cell, &self.world) {
                    return Err(ObserverRefusal::OutOfReach);
                }
                if !self.economy.is_powered(key.cell.level) {
                    return Err(ObserverRefusal::NoPower);
                }
                Some(ObserverIntent::SetDoor(key, state))
            }
            ObserverAction::Shove(target) => {
                if !self.can_shove(id, target) {
                    return Err(ObserverRefusal::InvalidTarget);
                }
                Some(ObserverIntent::Shove(target))
            }
            ObserverAction::ToggleGenerator => {
                let powered = self.economy.is_powered(cell.level);
                if observer.state != ObserverState::Active
                    || !self.economy.is_at_generator(cell)
                    || (!powered && self.power_policy == super::PowerPolicy::OneWay)
                    || (powered && self.power_policy == super::PowerPolicy::AlwaysOn)
                {
                    return Err(ObserverRefusal::InvalidTarget);
                }
                Some(ObserverIntent::ToggleGenerator)
            }
        };
        Ok(intent)
    }

    /// Cell-level action boundary. Continuous movement adapters resolve physical
    /// movement first; arbitrary client positions must never be passed as steps.
    pub fn submit_observer(
        &mut self,
        id: ObserverId,
        command: ObserverCommand,
    ) -> Result<(), ObserverRefusal> {
        let intent = self.observer_action(id, command)?;
        if let Some(intent) = intent {
            self.apply_observer_intent(id, intent);
        }
        if let Some(facing) = command.facing {
            self.observers
                .get_mut(&id)
                .expect("validated Observer")
                .facing = facing;
        }
        self.refresh_observation();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ascent::sim::{ACTOR_BEAT_TICKS, ArchitectMode};
    use std::collections::BTreeMap;

    #[test]
    fn neutral_human_input_never_runs_the_observer_bot() {
        let mut sim = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
        sim.guardians.clear();
        let id = ObserverId(0);
        let before = sim.observers[&id].clone();
        for _ in 0..ACTOR_BEAT_TICKS * 2 {
            sim.tick_with_observers(&BTreeMap::from([(id, ObserverCommand::default())]));
        }
        assert_eq!(sim.observers[&id], before);
        assert!(!sim.traces.contains_key("Observer 0"));
    }

    #[test]
    fn impossible_step_is_atomic_even_when_facing_is_supplied() {
        let mut sim = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
        let id = ObserverId(0);
        let before = sim.observers[&id].clone();
        let result = sim.submit_observer(
            id,
            ObserverCommand {
                facing: Some(HexFace::West),
                action: ObserverAction::Step(HexCoord {
                    q: 100,
                    r: 100,
                    level: 0,
                }),
            },
        );
        assert_eq!(result, Err(ObserverRefusal::NoPassage));
        assert_eq!(sim.observers[&id], before);
    }
}
