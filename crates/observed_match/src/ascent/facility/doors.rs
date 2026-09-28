//! Deployed doors on the real facility.
//!
//! The rules own every door (design section 4): a door card deploys one closed on a known
//! threshold, any loyal Observer may open or close it, an open door pins its connection, a
//! closed one blocks travel and sight, and a floor without power freezes its doors. After
//! every step the rules' doors are handed to the physical match, where a closed door is a
//! panel across its doorway (`hex_wfc::model::doors`).
//!
//! A body works a door the way it works a generator: interact within
//! [`DOOR_REACH`](crate::hex_wfc::DOOR_REACH) of the doorway's middle, through the rules'
//! own Observer command, so reach, power and refusals are theirs.

use observed_core::PlayerId;

use super::AscentRules;
use crate::ascent::sim::{DoorState, ObserverAction, ObserverCommand, ThresholdKey};
use crate::hex_wfc::{HexDoor, HexInputFrame, HexWfcMatch};

/// A door a body stands at, and what interact would do to it: what the prompt says.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtDoor {
    pub closed: bool,
    /// Whether the rules would let this body open or close it now.
    pub operable: bool,
    /// Whether its floor has power: a dark floor's doors are frozen.
    pub powered: bool,
}

fn toggle(door: &HexDoor) -> ObserverCommand {
    ObserverCommand {
        facing: None,
        action: ObserverAction::Door(
            ThresholdKey {
                cell: door.cell,
                face: door.face,
            },
            if door.closed {
                DoorState::Open
            } else {
                DoorState::Closed
            },
        ),
    }
}

impl AscentRules {
    /// The door `player`'s body stands at, and what interact would do to it.
    #[must_use]
    pub fn at_door(&self, physical: &HexWfcMatch, player: PlayerId) -> Option<(HexDoor, AtDoor)> {
        let id = self.observer_for(player)?;
        let door = *physical.door_in_reach(player)?;
        let rules = &self.session.sim;
        Some((
            door,
            AtDoor {
                closed: door.closed,
                operable: rules.observer_refusal(id, toggle(&door)).is_none(),
                powered: rules.economy.is_powered(door.cell.level),
            },
        ))
    }

    /// Every body that pressed interact at a door, away from its floor's generator, opens
    /// or closes it as the rules allow.
    pub(super) fn operate_doors(&mut self, physical: &HexWfcMatch, bodies: &HexInputFrame) {
        for (player, command) in &bodies.commands {
            if !command.actions.interact || self.at_generator(physical, *player) {
                continue;
            }
            let (Some(&id), Some(door)) =
                (self.bodies.get(player), physical.door_in_reach(*player))
            else {
                continue;
            };
            // Refused - no power, out of reach, not active - is the rules' answer.
            let _ = self.session.sim.submit_observer(id, toggle(door));
        }
    }

    /// Deploy a door, `closed` or not, on `face` of `cell`, as a door card would. For evidence
    /// captures, as `stage_power` is: play deploys a door only by a legal card.
    pub fn stage_door(
        &mut self,
        cell: observed_hex::HexCoord,
        face: observed_hex::HexFace,
        closed: bool,
    ) {
        let key = self
            .session
            .sim
            .threshold_key(cell, face)
            .expect("a lateral face with a neighbour");
        let state = if closed {
            DoorState::Closed
        } else {
            DoorState::Open
        };
        self.session.sim.doors.insert(key, state);
        self.session.sim.refresh_observation();
    }

    /// Hand the rules' doors to the physical match.
    pub(super) fn place_doors(&self, physical: &mut HexWfcMatch) {
        physical.set_doors(
            self.session
                .sim
                .doors
                .iter()
                .map(|(key, &state)| ((key.cell, key.face), state == DoorState::Closed)),
        );
    }
}
