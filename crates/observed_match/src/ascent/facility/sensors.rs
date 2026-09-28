//! The Rogue's sensors on the real facility.
//!
//! The rules own every sensor (`ascent::sim::sensor`). After every step they are handed to
//! the physical match, which hangs each one where a body can see it and walk up to it
//! (`hex_wfc::model::sensors`). A loyal body takes one down the way it works a door:
//! interact within [`SENSOR_REACH`](crate::hex_wfc::SENSOR_REACH) of it, away from a
//! generator and a door, through the rules' own Observer command, so reach and refusals
//! are theirs.

use observed_core::PlayerId;
use observed_hex::HexCoord;

use super::AscentRules;
use crate::ascent::sim::{ObserverAction, ObserverCommand};
use crate::hex_wfc::{HexInputFrame, HexWfcMatch};

/// A sensor a body stands under, and what the prompt says of it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtSensor {
    pub cell: HexCoord,
    /// Whether the rules would let this body take it down now.
    pub dismantlable: bool,
    /// Whether its floor's power lets it watch.
    pub live: bool,
}

const fn dismantle(cell: HexCoord) -> ObserverCommand {
    ObserverCommand {
        facing: None,
        action: ObserverAction::Dismantle(cell),
    }
}

impl AscentRules {
    /// The sensor `player`'s body stands under, and what interact would do to it.
    #[must_use]
    pub fn at_sensor(&self, physical: &HexWfcMatch, player: PlayerId) -> Option<AtSensor> {
        let id = self.observer_for(player)?;
        let cell = physical.sensor_in_reach(player)?;
        let rules = &self.session.sim;
        Some(AtSensor {
            cell,
            dismantlable: rules.observer_refusal(id, dismantle(cell)).is_none(),
            live: rules.sensor_live(cell),
        })
    }

    /// Every body that pressed interact under a sensor, away from its floor's generator and
    /// any door, takes it down as the rules allow.
    pub(super) fn operate_sensors(&mut self, physical: &HexWfcMatch, bodies: &HexInputFrame) {
        for (player, command) in &bodies.commands {
            if !command.actions.interact
                || self.at_generator(physical, *player)
                || physical.door_in_reach(*player).is_some()
            {
                continue;
            }
            let (Some(&id), Some(cell)) =
                (self.bodies.get(player), physical.sensor_in_reach(*player))
            else {
                continue;
            };
            // Refused - out of reach, not active - is the rules' answer.
            let _ = self.session.sim.submit_observer(id, dismantle(cell));
        }
    }

    /// Install a sensor on `cell` as a Rogue seat would. For evidence captures, as
    /// `stage_door` is: play installs one only by the Rogue's legal command.
    pub fn stage_sensor(&mut self, cell: HexCoord) {
        let tick = self.session.sim.tick;
        self.session.sim.sensors.insert(cell, tick);
    }

    /// Hand the rules' sensors to the physical match.
    pub(super) fn place_sensors(&self, physical: &mut HexWfcMatch) {
        physical.set_sensors(self.session.sim.sensors.keys().copied());
    }
}
