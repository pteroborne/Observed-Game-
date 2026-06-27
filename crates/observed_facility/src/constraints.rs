//! **Mutable graph constraints** — the rule that keeps the observe/decohere structure
//! ([`observed_observation`]) playable. An unconstrained rewiring can isolate a room;
//! a **persistent route spine** (a protected spanning path that is never rewired) keeps
//! the whole structure traversable however the rest of it decoheres. With protection
//! off, the same rewiring can disconnect the graph — which is why the constraint
//! exists. Pure logic; the `Resource` derive is behind the `bevy` feature.

use observed_core::RoomId;
use observed_observation::{DOOR_COUNT, Door, DoorId, ObservationWorld, ROOM_COUNT, Side};

/// A spanning path through every room: the persistent backbone. Each entry is the
/// two doorways of one protected connection. Because it visits all nine rooms, the
/// graph is connected via the spine alone, regardless of how the free doors wire.
const SPINE: [((u32, Side), (u32, Side)); 8] = [
    ((0, Side::East), (1, Side::West)),
    ((1, Side::East), (2, Side::West)),
    ((2, Side::South), (5, Side::North)),
    ((5, Side::West), (4, Side::East)),
    ((4, Side::West), (3, Side::East)),
    ((3, Side::South), (6, Side::North)),
    ((6, Side::East), (7, Side::West)),
    ((7, Side::East), (8, Side::West)),
];

struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct ConstraintWorld {
    /// The connection graph reused from `observed_observation`.
    pub graph: ObservationWorld,
    /// Per-door: belongs to the persistent spine and is never rewired.
    pub protected: Vec<bool>,
    pub protection_enabled: bool,
    pub base_seed: u64,
    pub decohere_count: u32,
    pub connected: bool,
    pub reachable: u32,
    pub last_rewired: u32,
    pub last_event: String,
}

impl ConstraintWorld {
    pub fn authored() -> Self {
        let graph = ObservationWorld::authored();
        let mut protected = vec![false; DOOR_COUNT];
        for ((room_a, side_a), (room_b, side_b)) in SPINE {
            protected[graph.door_id(RoomId(room_a), side_a).0 as usize] = true;
            protected[graph.door_id(RoomId(room_b), side_b).0 as usize] = true;
        }

        let mut world = Self {
            graph,
            protected,
            protection_enabled: true,
            base_seed: 0x5EED_C0DE_1234_5678,
            decohere_count: 0,
            connected: true,
            reachable: ROOM_COUNT as u32,
            last_rewired: 0,
            last_event: "Gold routes persist; the rest rewires but stays connected.".to_string(),
        };
        world.recompute_connectivity();
        world
    }

    pub fn reset(&mut self) {
        *self = Self::authored();
    }

    pub fn is_protected(&self, door: DoorId) -> bool {
        self.protected[door.0 as usize]
    }

    /// A door is frozen when observation pins it, or when the spine protects it
    /// (and protection is on).
    pub fn is_frozen(&self, door: DoorId) -> bool {
        self.graph.is_pinned(door) || (self.protection_enabled && self.is_protected(door))
    }

    pub fn door(&self, door: DoorId) -> &Door {
        self.graph.door(door)
    }

    pub fn toggle_protection(&mut self) {
        self.protection_enabled = !self.protection_enabled;
        self.last_event = if self.protection_enabled {
            "Route protection ON — connectivity is guaranteed.".to_string()
        } else {
            "Route protection OFF — rewiring may isolate rooms.".to_string()
        };
    }

    /// Re-match every door that is neither observed nor (optionally) protected.
    pub fn decohere(&mut self) {
        self.decohere_count += 1;
        let before = self.graph.links.clone();

        let mut free: Vec<DoorId> = (0..DOOR_COUNT)
            .map(|i| DoorId(i as u16))
            .filter(|d| !self.is_frozen(*d))
            .collect();

        let mut rng = SplitMix(
            self.base_seed ^ (self.decohere_count as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
        );
        for i in (1..free.len()).rev() {
            free.swap(i, rng.below(i + 1));
        }

        let mut chunks = free.chunks_exact(2);
        for pair in chunks.by_ref() {
            let (a, b) = (pair[0], pair[1]);
            self.graph.links[a.0 as usize] = b;
            self.graph.links[b.0 as usize] = a;
        }
        if let [leftover] = chunks.remainder() {
            self.graph.links[leftover.0 as usize] = *leftover;
        }

        self.last_rewired = (0..self.graph.links.len())
            .filter(|i| self.graph.links[*i] != before[*i])
            .count() as u32;
        self.recompute_connectivity();

        self.last_event = format!(
            "Decohered: {} doors rewired; {}.",
            self.last_rewired,
            if self.connected {
                "every room still reachable"
            } else {
                "a room was isolated!"
            }
        );
    }

    /// Breadth-first reachability over the active passages, from room 0.
    pub fn reachable_set(&self) -> Vec<bool> {
        let mut adjacency: Vec<Vec<u32>> = vec![Vec::new(); ROOM_COUNT];
        for (a, b) in self.graph.connections() {
            let ra = self.graph.door(a).room.0;
            let rb = self.graph.door(b).room.0;
            if ra != rb {
                adjacency[ra as usize].push(rb);
                adjacency[rb as usize].push(ra);
            }
        }

        let mut seen = vec![false; ROOM_COUNT];
        let mut stack = vec![0usize];
        seen[0] = true;
        while let Some(room) = stack.pop() {
            for &neighbour in &adjacency[room] {
                if !seen[neighbour as usize] {
                    seen[neighbour as usize] = true;
                    stack.push(neighbour as usize);
                }
            }
        }
        seen
    }

    pub fn recompute_connectivity(&mut self) {
        let seen = self.reachable_set();
        self.reachable = seen.iter().filter(|s| **s).count() as u32;
        self.connected = self.reachable as usize == ROOM_COUNT;
    }

    pub fn traverse(&mut self, player: usize, side: Side) -> bool {
        self.graph.traverse(player, side)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spine_doors(world: &ConstraintWorld) -> Vec<DoorId> {
        (0..DOOR_COUNT)
            .map(|i| DoorId(i as u16))
            .filter(|d| world.is_protected(*d))
            .collect()
    }

    #[test]
    fn authored_world_is_connected_with_a_protected_spine() {
        let world = ConstraintWorld::authored();
        assert!(world.connected);
        assert_eq!(world.reachable as usize, ROOM_COUNT);
        // Eight spine connections protect sixteen doors.
        assert_eq!(spine_doors(&world).len(), 16);
    }

    #[test]
    fn protected_routes_persist_across_decoherence() {
        let mut world = ConstraintWorld::authored();
        let spine = spine_doors(&world);
        let before: Vec<DoorId> = spine.iter().map(|d| world.graph.partner(*d)).collect();
        for _ in 0..20 {
            world.decohere();
        }
        for (door, expected) in spine.iter().zip(before) {
            assert_eq!(
                world.graph.partner(*door),
                expected,
                "spine link must persist"
            );
        }
    }

    #[test]
    fn connectivity_holds_for_every_decoherence_with_protection() {
        let mut world = ConstraintWorld::authored();
        for _ in 0..200 {
            world.decohere();
            assert!(
                world.connected,
                "protected spine must keep the graph connected"
            );
            assert_eq!(world.reachable as usize, ROOM_COUNT);
        }
    }

    #[test]
    fn unprotected_rewiring_can_disconnect_the_graph() {
        // Remove the observers so nothing is pinned, then drop protection.
        let mut world = ConstraintWorld::authored();
        world.graph.players = vec![RoomId(4); world.graph.players.len()];
        world.protection_enabled = false;

        let mut isolated = false;
        for _ in 0..200 {
            world.decohere();
            if !world.connected {
                isolated = true;
                break;
            }
        }
        assert!(
            isolated,
            "without the spine, some rewiring should isolate a room — that's why the constraint exists"
        );
    }

    #[test]
    fn the_constraint_still_leaves_doors_mutable() {
        let mut world = ConstraintWorld::authored();
        world.decohere();
        assert!(
            world.last_rewired > 0,
            "non-spine doors should still rewire"
        );
    }

    #[test]
    fn observed_rooms_stay_frozen_under_the_constraint() {
        let mut world = ConstraintWorld::authored();
        world.graph.players = vec![RoomId(4), RoomId(4), RoomId(4), RoomId(4)];
        let watched: Vec<(DoorId, DoorId)> = Side::ALL
            .iter()
            .map(|side| {
                let d = world.graph.door_id(RoomId(4), *side);
                (d, world.graph.partner(d))
            })
            .collect();
        world.decohere();
        for (door, partner) in watched {
            assert_eq!(world.graph.partner(door), partner);
        }
    }

    #[test]
    fn decoherence_is_deterministic() {
        let mut a = ConstraintWorld::authored();
        let mut b = ConstraintWorld::authored();
        a.decohere();
        b.decohere();
        assert_eq!(a.graph.links, b.graph.links);
    }

    #[test]
    fn reset_restores_the_authored_structure() {
        let mut world = ConstraintWorld::authored();
        for _ in 0..5 {
            world.decohere();
        }
        world.protection_enabled = false;
        world.reset();
        assert_eq!(world.decohere_count, 0);
        assert!(world.protection_enabled);
        assert!(world.connected);
    }
}
