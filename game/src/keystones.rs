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
use observed_facility::map_spec::MapSpec;

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
    pub exit_room: RoomId,
}

impl KeystoneState {
    pub fn new(seed: u64) -> Self {
        Self::for_map(seed, &crate::map_catalog::active_map_spec(seed))
    }

    pub fn for_map(seed: u64, spec: &MapSpec) -> Self {
        let mut rooms = spec.keystone_rooms();
        rooms.sort_by_key(|room| room.0);
        let required = rooms.len() as u32;
        let exit_room = spec.exit_room().unwrap_or_else(|| {
            panic!(
                "active map spec `{}` is missing a required Exit room; \
                 every catalog map must satisfy MapSpec::validate()",
                spec.name
            )
        });
        Self {
            rooms,
            collected: HashSet::new(),
            held: 0,
            required,
            seed,
            exit_room,
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

    /// The generated map's keystone placement (spec-driven: whatever
    /// `MapSpec::keystone_rooms` returns, never the map's own start/exit) is
    /// deterministic and self-consistent for a given seed.
    #[test]
    fn placement_is_deterministic_and_never_the_entrance_or_exit() {
        for seed in 0..40u64 {
            let spec = crate::map_catalog::default_map_spec(seed);
            let a = KeystoneState::for_map(seed, &spec);
            let b = KeystoneState::for_map(seed, &spec);
            assert_eq!(a.rooms, b.rooms, "deterministic for a given (seed, spec)");
            assert_eq!(
                a.rooms.len(),
                a.required as usize,
                "required is derived from placement, so it is always attainable"
            );
            let mut d = a.rooms.clone();
            d.dedup();
            assert_eq!(d.len(), a.rooms.len(), "distinct rooms");
            if let Some(start) = spec.start_room() {
                assert!(!a.rooms.contains(&start), "never the entrance");
            }
            assert!(!a.rooms.contains(&a.exit_room), "never the exit");
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
