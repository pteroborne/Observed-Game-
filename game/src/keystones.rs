//! The **keystone-gated exit**: a dead-simple item-and-inventory layer over the match.
//!
//! Keystones are pickup **items** placed in some of the facility's spine rooms. Walking
//! over one collects it. The exit door stays locked until you **hold** the required
//! number — the gate is a plain inventory check ([`KeystoneState::gate_open`]), with no
//! room-visit / discovery tracking. The match brain is untouched; this only gates the
//! local player's final crossing into the exit (see `screens.rs`).

use std::collections::HashSet;

use bevy::prelude::*;
use observed_core::RoomId;
use observed_match::mutable::{START_ROOM, spine_next};

/// How many keystones the exit demands (clamped to the available spine rooms).
pub const REQUIRED: usize = 3;

/// The spine, in order from the entrance to the exit (`0,1,2,5,4,3,6,7,8`).
fn spine_sequence() -> Vec<RoomId> {
    let mut seq = vec![RoomId(START_ROOM)];
    let mut room = RoomId(START_ROOM);
    while let Some((next, _)) = spine_next(room) {
        seq.push(next);
        room = next;
    }
    seq
}

/// The spine rooms that hold a keystone item: a deterministic spread of `REQUIRED`
/// distinct *intermediate* rooms (never the entrance or the exit), always including the
/// last room before the exit so the objective is collectable on the way through.
pub fn keystone_rooms(seed: u64) -> Vec<RoomId> {
    let seq = spine_sequence();
    if seq.len() < 3 {
        return Vec::new();
    }
    let inter: Vec<RoomId> = seq[1..seq.len() - 1].to_vec();
    let required = REQUIRED.min(inter.len());
    let mut chosen = vec![*inter.last().expect("intermediates non-empty")];
    let mut i = (seed % inter.len() as u64) as usize;
    while chosen.len() < required {
        let room = inter[i % inter.len()];
        if !chosen.contains(&room) {
            chosen.push(room);
        }
        i += 1;
    }
    chosen.sort_by_key(|r| r.0);
    chosen
}

/// The player's keystone inventory + the fixed placement for this match.
#[derive(Resource, Clone, Debug)]
pub struct KeystoneState {
    /// The spine rooms that hold a keystone item.
    pub rooms: Vec<RoomId>,
    /// Rooms whose keystone has been picked up (so an item is never taken twice).
    pub collected: HashSet<RoomId>,
    /// Keystones held.
    pub held: u32,
    /// Keystones the exit demands (derived from placement, so it is always attainable).
    pub required: u32,
    pub seed: u64,
}

impl KeystoneState {
    pub fn new(seed: u64) -> Self {
        let rooms = keystone_rooms(seed);
        let required = rooms.len() as u32;
        Self {
            rooms,
            collected: HashSet::new(),
            held: 0,
            required,
            seed,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.seed);
    }

    /// Does `room` hold a keystone the player has not yet taken?
    pub fn has_uncollected(&self, room: RoomId) -> bool {
        self.rooms.contains(&room) && !self.collected.contains(&room)
    }

    /// Pick up the keystone in `room` (once). Returns true if one was collected.
    pub fn collect(&mut self, room: RoomId) -> bool {
        if self.has_uncollected(room) {
            self.collected.insert(room);
            self.held += 1;
            true
        } else {
            false
        }
    }

    /// The exit is open once enough keystones are held.
    pub fn gate_open(&self) -> bool {
        self.held >= self.required
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_match::mutable::EXIT_ROOM;

    #[test]
    fn placement_is_deterministic_and_required_distinct_intermediate_rooms() {
        for seed in 0..40u64 {
            let a = keystone_rooms(seed);
            assert_eq!(a, keystone_rooms(seed), "deterministic");
            assert_eq!(a.len(), REQUIRED, "exactly REQUIRED keystones placed");
            // Distinct, and never the entrance or the exit.
            let mut d = a.clone();
            d.dedup();
            assert_eq!(d.len(), a.len(), "distinct rooms");
            assert!(!a.contains(&RoomId(START_ROOM)), "never the entrance");
            assert!(!a.contains(&RoomId(EXIT_ROOM)), "never the exit");
        }
    }

    #[test]
    fn the_last_room_before_the_exit_always_holds_a_keystone() {
        // spine = ...,7,8 — room 7 is the last intermediate, always reachable on the way.
        let last_intermediate = {
            let seq = spine_sequence();
            seq[seq.len() - 2]
        };
        for seed in 0..40u64 {
            assert!(
                keystone_rooms(seed).contains(&last_intermediate),
                "the last pre-exit room must hold a keystone (seed {seed})"
            );
        }
    }

    #[test]
    fn the_gate_opens_exactly_at_the_requirement() {
        let mut state = KeystoneState::new(7);
        assert!(!state.gate_open());
        let rooms = state.rooms.clone();
        for (i, room) in rooms.iter().enumerate() {
            assert!(state.collect(*room));
            assert!(!state.collect(*room), "a keystone is taken only once");
            let expected_open = (i as u32 + 1) >= state.required;
            assert_eq!(state.gate_open(), expected_open);
        }
        assert!(state.gate_open());
        assert_eq!(state.held, state.required);
    }

    #[test]
    fn reset_clears_the_inventory_but_keeps_the_placement() {
        let mut state = KeystoneState::new(3);
        let rooms = state.rooms.clone();
        for room in &rooms {
            state.collect(*room);
        }
        state.reset();
        assert_eq!(state.held, 0);
        assert!(state.collected.is_empty());
        assert_eq!(state.rooms, rooms, "placement is stable across reset");
    }
}
