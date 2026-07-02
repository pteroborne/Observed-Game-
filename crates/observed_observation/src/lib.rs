//! The game's defining concept as a **pure production model**: a megastructure whose
//! connections change *when unobserved* and collapse to a fixed state *when observed*.
//!
//! The structure is a grid of rooms; each room has four doorways. Doorways are
//! paired into a perfect matching (a doorway linked to itself is a sealed wall).
//! A room is **observed** while a player occupies it. Observation pins every
//! doorway of an observed room — and its partner — so observed connections are
//! frozen. A **decoherence** event re-matches only the unobserved, unpinned
//! doorways, deterministically from a seed, so the same observation produces the
//! same rewiring (replayable, testable). Walking through a doorway follows its
//! current link, so where a door leads depends on what has been watched.
//!
//! This is the durable model (promoted out of `observation_lab` in refactor R5). It
//! depends only on `glam` for vector math and `observed_core` for `RoomId`. The
//! optional, default-on `bevy` feature is the adapter: it derives `Resource` on
//! [`ObservationWorld`] so the labs/game can insert it directly. `observation_lab`
//! re-exports this crate as its `model` and is the debug projection.

use glam::Vec2;
use observed_core::{RoomId, SplitMix};

pub const ROWS: u32 = 3;
pub const COLS: u32 = 3;
pub const ROOM_COUNT: usize = (ROWS * COLS) as usize;
pub const DOOR_COUNT: usize = ROOM_COUNT * 4;
pub const PLAYER_COUNT: usize = 4;
pub const ROOM_HALF: f32 = 120.0;
const ROOM_SPACING: f32 = 320.0;

pub use observed_core::Direction as Side;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DoorId(pub u16);

#[derive(Clone, Copy, Debug)]
pub struct Door {
    pub id: DoorId,
    pub room: RoomId,
    pub side: Side,
}

// Replaced duplicate SplitMix with shared observed_core::SplitMix

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct ObservationWorld {
    pub room_count: usize,
    pub doors: Vec<Door>,
    /// `links[d]` is the partner of door `d`; `links[d] == d` means a sealed wall.
    pub links: Vec<DoorId>,
    /// Which room each player occupies.
    pub players: Vec<RoomId>,
    pub base_seed: u64,
    pub decoherence_count: u32,
    pub rewires_last: u32,
    pub locked_last: u32,
}

impl ObservationWorld {
    pub fn authored() -> Self {
        let mut world = Self::from_edges(
            ROOM_COUNT,
            &authored_edges(),
            vec![RoomId(0), RoomId(2), RoomId(6), RoomId(8)],
            0xA11C_E5EE_D5EE_D000,
        );
        world.rewires_last = 0;
        world.locked_last = 0;
        world
    }

    pub fn from_edges(
        room_count: usize,
        edges: &[(RoomId, Side, RoomId, Side)],
        players: Vec<RoomId>,
        base_seed: u64,
    ) -> Self {
        let door_count = room_count * 4;
        assert!(door_count <= u16::MAX as usize, "door ids are u16-backed");
        let mut doors = Vec::with_capacity(door_count);
        for room_index in 0..room_count {
            for side in Side::ALL {
                doors.push(Door {
                    id: DoorId((room_index * 4 + side.index()) as u16),
                    room: RoomId(room_index as u32),
                    side,
                });
            }
        }
        let mut links: Vec<DoorId> = (0..door_count).map(|i| DoorId(i as u16)).collect();
        let mut link = |a: DoorId, b: DoorId| {
            links[a.0 as usize] = b;
            links[b.0 as usize] = a;
        };
        for &(room_a, side_a, room_b, side_b) in edges {
            assert!(
                (room_a.0 as usize) < room_count && (room_b.0 as usize) < room_count,
                "edge endpoints must be in range"
            );
            link(
                DoorId((room_a.0 as usize * 4 + side_a.index()) as u16),
                DoorId((room_b.0 as usize * 4 + side_b.index()) as u16),
            );
        }

        Self {
            room_count,
            doors,
            links,
            players,
            base_seed,
            decoherence_count: 0,
            rewires_last: 0,
            locked_last: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::authored();
    }

    // -- queries ----------------------------------------------------------

    pub fn door(&self, id: DoorId) -> &Door {
        &self.doors[id.0 as usize]
    }

    pub fn partner(&self, id: DoorId) -> DoorId {
        self.links[id.0 as usize]
    }

    pub fn is_sealed(&self, id: DoorId) -> bool {
        self.links[id.0 as usize] == id
    }

    pub fn observed(&self, room: RoomId) -> bool {
        self.players.contains(&room)
    }

    /// A doorway is pinned when its room is observed, or when its partner's room
    /// is observed (observing one end collapses the whole connection).
    pub fn is_pinned(&self, id: DoorId) -> bool {
        if self.observed(self.door(id).room) {
            return true;
        }
        let partner = self.partner(id);
        partner != id && self.observed(self.door(partner).room)
    }

    pub fn door_id(&self, room: RoomId, side: Side) -> DoorId {
        DoorId((room.0 as usize * 4 + side.index()) as u16)
    }

    pub fn room_center(&self, room: RoomId) -> Vec2 {
        let cols = layout_cols(self.room_count) as u32;
        let rows = (self.room_count as u32).div_ceil(cols);
        let r = room.0 / cols;
        let c = room.0 % cols;
        Vec2::new(
            (c as f32 - (cols as f32 - 1.0) * 0.5) * ROOM_SPACING,
            ((rows as f32 - 1.0) * 0.5 - r as f32) * ROOM_SPACING,
        )
    }

    pub fn door_position(&self, id: DoorId) -> Vec2 {
        let door = self.door(id);
        self.room_center(door.room) + door.side.vector() * ROOM_HALF
    }

    /// Unique active passages (a < b, linked, not sealed).
    pub fn connections(&self) -> Vec<(DoorId, DoorId)> {
        let mut out = Vec::new();
        for index in 0..self.doors.len() {
            let a = DoorId(index as u16);
            let b = self.partner(a);
            if a.0 < b.0 {
                out.push((a, b));
            }
        }
        out
    }

    pub fn free_door_count(&self) -> usize {
        (0..self.doors.len())
            .filter(|i| !self.is_pinned(DoorId(*i as u16)))
            .count()
    }

    // -- evolution --------------------------------------------------------

    /// Re-match every unobserved, unpinned doorway into a fresh perfect matching.
    /// Observed connections are untouched. Deterministic for a given state.
    pub fn decohere(&mut self) {
        self.decoherence_count += 1;
        let before = self.links.clone();

        let mut free: Vec<DoorId> = (0..self.doors.len())
            .map(|i| DoorId(i as u16))
            .filter(|d| !self.is_pinned(*d))
            .collect();
        self.locked_last = (self.doors.len() - free.len()) as u32;

        // Deterministic Fisher–Yates shuffle of the free set.
        let mut rng = SplitMix(
            self.base_seed ^ (self.decoherence_count as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
        );
        for i in (1..free.len()).rev() {
            free.swap(i, rng.below(i + 1));
        }

        // Pair consecutively; seal a leftover if the free count is odd.
        let mut iter = free.chunks_exact(2);
        for pair in iter.by_ref() {
            let (a, b) = (pair[0], pair[1]);
            self.links[a.0 as usize] = b;
            self.links[b.0 as usize] = a;
        }
        if let [leftover] = iter.remainder() {
            self.links[leftover.0 as usize] = *leftover;
        }

        self.rewires_last = (0..self.links.len())
            .filter(|i| self.links[*i] != before[*i])
            .count() as u32;
    }

    /// Walk the selected player through the doorway on `side`, following its
    /// current link. Returns false for a sealed wall.
    pub fn traverse(&mut self, player: usize, side: Side) -> bool {
        let Some(&room) = self.players.get(player) else {
            return false;
        };
        let door = self.door_id(room, side);
        if self.is_sealed(door) {
            return false;
        }
        let destination = self.door(self.partner(door)).room;
        self.players[player] = destination;
        true
    }
}

fn layout_cols(room_count: usize) -> usize {
    if room_count == ROOM_COUNT {
        return COLS as usize;
    }
    (room_count as f32).sqrt().ceil().max(1.0) as usize
}

fn authored_edges() -> Vec<(RoomId, Side, RoomId, Side)> {
    let mut edges = Vec::new();
    for r in 0..ROWS {
        for c in 0..COLS {
            let room = RoomId(r * COLS + c);
            if c + 1 < COLS {
                edges.push((room, Side::East, RoomId(r * COLS + c + 1), Side::West));
            }
            if r + 1 < ROWS {
                edges.push((room, Side::South, RoomId((r + 1) * COLS + c), Side::North));
            }
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_valid_matching(world: &ObservationWorld) {
        for index in 0..world.doors.len() {
            let a = DoorId(index as u16);
            let b = world.partner(a);
            assert_eq!(
                world.partner(b),
                a,
                "matching must be a symmetric involution"
            );
        }
    }

    #[test]
    fn authored_structure_is_a_valid_matching_with_observers() {
        let world = ObservationWorld::authored();
        assert_eq!(world.doors.len(), DOOR_COUNT);
        assert_eq!(world.players.len(), PLAYER_COUNT);
        assert_valid_matching(&world);
        // The four authored observers sit in distinct rooms.
        assert_eq!(
            world
                .players
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            4
        );
    }

    #[test]
    fn observed_rooms_pin_their_doorways_across_decoherence() {
        let mut world = ObservationWorld::authored();
        // Observe only room 4 (the centre).
        world.players = vec![RoomId(4), RoomId(4), RoomId(4), RoomId(4)];
        let watched: Vec<(DoorId, DoorId)> = Side::ALL
            .iter()
            .map(|side| {
                let d = world.door_id(RoomId(4), *side);
                (d, world.partner(d))
            })
            .collect();

        world.decohere();
        assert_valid_matching(&world);
        for (door, partner) in watched {
            assert_eq!(
                world.partner(door),
                partner,
                "observed doorway must not change"
            );
        }
    }

    #[test]
    fn unobserved_doorways_rewire_on_decoherence() {
        let mut world = ObservationWorld::authored();
        let before = world.links.clone();
        world.decohere();
        assert_valid_matching(&world);
        assert!(world.rewires_last > 0, "some unobserved link should change");
        assert_ne!(world.links, before);
    }

    #[test]
    fn decoherence_is_deterministic_for_the_same_state() {
        let mut a = ObservationWorld::authored();
        let mut b = ObservationWorld::authored();
        a.decohere();
        b.decohere();
        assert_eq!(a.links, b.links);
        // A different seed diverges.
        let mut c = ObservationWorld::authored();
        c.base_seed = 12345;
        c.decohere();
        assert_ne!(a.links, c.links);
    }

    #[test]
    fn observing_a_room_collapses_it_then_releasing_lets_it_change() {
        let mut world = ObservationWorld::authored();
        // No observers anywhere: everything is free.
        world.players = vec![RoomId(0); PLAYER_COUNT]; // all watch room 0
        let room0_door = world.door_id(RoomId(0), Side::East);
        let pinned_partner = world.partner(room0_door);
        world.decohere();
        assert_eq!(world.partner(room0_door), pinned_partner);

        // Move the watchers elsewhere; room 0 is now free and can rewire.
        world.players = vec![RoomId(8); PLAYER_COUNT];
        let mut changed = false;
        for _ in 0..8 {
            world.decohere();
            if world.partner(room0_door) != pinned_partner {
                changed = true;
                break;
            }
        }
        assert!(changed, "an unobserved room should eventually rewire");
    }

    #[test]
    fn traversal_follows_the_current_link() {
        let mut world = ObservationWorld::authored();
        world.players[0] = RoomId(0);
        // Room 0 East links to room 1 West in the authored lattice.
        assert!(world.traverse(0, Side::East));
        assert_eq!(world.players[0], RoomId(1));
    }

    #[test]
    fn traversing_a_sealed_wall_is_blocked() {
        let mut world = ObservationWorld::authored();
        world.players[0] = RoomId(0);
        // Room 0 North is on the boundary — sealed in the authored lattice.
        assert!(world.is_sealed(world.door_id(RoomId(0), Side::North)));
        assert!(!world.traverse(0, Side::North));
        assert_eq!(world.players[0], RoomId(0));
    }

    #[test]
    fn reset_restores_the_authored_structure() {
        let mut world = ObservationWorld::authored();
        for _ in 0..5 {
            world.decohere();
        }
        world.players[0] = RoomId(4);
        world.reset();
        assert_eq!(world.decoherence_count, 0);
        assert_eq!(
            world.players,
            vec![RoomId(0), RoomId(2), RoomId(6), RoomId(8)]
        );
        assert_valid_matching(&world);
    }
}
