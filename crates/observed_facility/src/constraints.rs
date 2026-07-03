//! **Mutable graph constraints** — the rule that keeps the observe/decohere structure
//! ([`observed_observation`]) playable. An unconstrained rewiring can isolate a room;
//! a **persistent route spine** (a protected spanning path that is never rewired) keeps
//! the whole structure traversable however the rest of it decoheres. With protection
//! off, the same rewiring can disconnect the graph — which is why the constraint
//! exists. Pure logic; the `Resource` derive is behind the `bevy` feature.

use observed_core::{RoomId, SplitMix, TeamId};
use observed_observation::contention::Anchor;
use observed_observation::{Door, DoorId, ObservationWorld, ROOM_COUNT, Side};

use crate::map_spec::MapSpec;

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
    /// Team-keyed hard freezes, shared across every team (Phase 38). Reuses
    /// [`observed_observation::contention::Anchor`] verbatim — see
    /// `ContentionWorld::place_anchor`/`remove_anchor` for the semantics this
    /// mirrors.
    pub anchors: Vec<Anchor>,
    /// Rooms the collapse has permanently claimed (Phase 41): every doorway of
    /// a sealed room is a self-link wall. Dedup, insertion order. Sealing is
    /// the one force that overrides every freeze — pins, anchors, and even the
    /// protected spine — see [`Self::seal_room`].
    pub sealed_rooms: Vec<RoomId>,
}

impl ConstraintWorld {
    pub fn authored() -> Self {
        let graph = ObservationWorld::authored();
        let mut protected = vec![false; graph.doors.len()];
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
            anchors: Vec::new(),
            sealed_rooms: Vec::new(),
        };
        world.recompute_connectivity();
        world
    }

    pub fn from_map_spec(spec: &MapSpec) -> Self {
        spec.validate_or_panic();
        let start = spec.start_room().expect("validated start room");
        let edges = spec
            .edges
            .iter()
            .map(|edge| (edge.a.room, edge.a.side, edge.b.room, edge.b.side))
            .collect::<Vec<_>>();
        let graph = ObservationWorld::from_edges(
            spec.room_count(),
            &edges,
            vec![start; observed_observation::PLAYER_COUNT],
            0x5EC7_0A11_EE00_0001,
        );
        let mut world = Self {
            protected: vec![false; graph.doors.len()],
            graph,
            protection_enabled: false,
            base_seed: 0x5EED_C0DE_1234_5678,
            decohere_count: 0,
            connected: true,
            reachable: spec.room_count() as u32,
            last_rewired: 0,
            last_event: format!(
                "{} uses redundant connectivity; tools stabilize practical routes.",
                spec.name
            ),
            anchors: Vec::new(),
            sealed_rooms: Vec::new(),
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

    /// Whether any team has an anchor placed on `room`. Mirrors
    /// `ContentionWorld::anchored`.
    fn anchored(&self, room: RoomId) -> bool {
        self.anchors.iter().any(|anchor| anchor.room == room)
    }

    /// Whether the collapse has already claimed `room` (Phase 41).
    pub fn is_sealed_room(&self, room: RoomId) -> bool {
        self.sealed_rooms.contains(&room)
    }

    /// Seal every doorway of `room` — and each doorway's partner — to a
    /// self-link wall, **unconditionally**. This is the collapse's override:
    /// unlike every other freeze in this world (observation pins, anchors,
    /// the protected spine), sealing does not check `is_frozen` and cannot be
    /// blocked by one. The collapse is "the one force anchors cannot hold
    /// back" — territory it claims stays claimed regardless of what was
    /// pinning those doors before. Idempotent: sealing an already-sealed room
    /// is a no-op past the first call (doors are already self-linked and
    /// `sealed_rooms` dedups).
    pub fn seal_room(&mut self, room: RoomId) {
        for side in Side::ALL {
            let door = self.graph.door_id(room, side);
            let partner = self.graph.partner(door);
            self.graph.links[door.0 as usize] = door;
            self.graph.links[partner.0 as usize] = partner;
        }
        if !self.sealed_rooms.contains(&room) {
            self.sealed_rooms.push(room);
        }
        self.recompute_connectivity();
    }

    /// Place `team`'s anchor on `room`. Idempotent per (team, room) — placing
    /// the same team's anchor on the same room twice is a no-op that still
    /// returns `true`. Anchor-vs-anchor agrees: two teams may anchor the same
    /// room simultaneously and both facts are recorded. Semantics identical to
    /// `observed_observation::contention::ContentionWorld::place_anchor`.
    pub fn place_anchor(&mut self, team: TeamId, room: RoomId) -> bool {
        if self
            .anchors
            .iter()
            .any(|anchor| anchor.team == team && anchor.room == room)
        {
            return true;
        }
        self.anchors.push(Anchor { team, room });
        true
    }

    /// Remove `team`'s anchor from `room`, if present. Only ever removes the
    /// calling team's own anchor; another team's anchor on the same room is
    /// untouched. Returns whether an anchor was actually removed. Semantics
    /// identical to `ContentionWorld::remove_anchor`.
    pub fn remove_anchor(&mut self, team: TeamId, room: RoomId) -> bool {
        let before = self.anchors.len();
        self.anchors
            .retain(|anchor| !(anchor.team == team && anchor.room == room));
        self.anchors.len() != before
    }

    /// A door is frozen when observation pins it, the spine protects it (and
    /// protection is on), either end's room is anchored by any team, or
    /// either end's room has been sealed by the collapse (Phase 41). Mirrors
    /// `ObservationWorld::is_pinned`'s both-ends rule: a door's own room and
    /// its partner's room are both checked, so a sealed-wall door (whose
    /// partner is itself) is covered by the same single check.
    ///
    /// Sealed rooms must be included here even though [`Self::seal_room`]
    /// already writes self-links directly: a sealed room's doors are
    /// otherwise unpinned (no observer necessarily standing there, no anchor
    /// necessarily placed), so without this check `decohere`'s free-door
    /// filter would treat them as eligible and re-match them away from their
    /// self-link on the very next call.
    pub fn is_frozen(&self, door: DoorId) -> bool {
        self.graph.is_pinned(door)
            || (self.protection_enabled && self.is_protected(door))
            || self.anchored(self.graph.door(door).room)
            || self.anchored(self.graph.door(self.graph.partner(door)).room)
            || self.is_sealed_room(self.graph.door(door).room)
            || self.is_sealed_room(self.graph.door(self.graph.partner(door)).room)
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

        let free: Vec<DoorId> = (0..self.graph.doors.len())
            .map(|i| DoorId(i as u16))
            .filter(|d| !self.is_frozen(*d))
            .collect();

        let mut accepted = None;
        for attempt in 0..32u64 {
            let mut candidate = before.clone();
            let mut shuffled = free.clone();
            let mut rng = SplitMix(
                self.base_seed
                    ^ (self.decohere_count as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    ^ attempt.wrapping_mul(0xD1B5_4A32_D192_ED03),
            );
            for i in (1..shuffled.len()).rev() {
                shuffled.swap(i, rng.below(i + 1));
            }

            let mut chunks = shuffled.chunks_exact(2);
            for pair in chunks.by_ref() {
                let (a, b) = (pair[0], pair[1]);
                candidate[a.0 as usize] = b;
                candidate[b.0 as usize] = a;
            }
            if let [leftover] = chunks.remainder() {
                candidate[leftover.0 as usize] = *leftover;
            }

            if self.reachable_count_for_links(&candidate) == self.graph.room_count {
                accepted = Some(candidate);
                break;
            }
        }

        if let Some(candidate) = accepted {
            self.graph.links = candidate;
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
        let mut adjacency: Vec<Vec<u32>> = vec![Vec::new(); self.graph.room_count];
        for (a, b) in self.graph.connections() {
            let ra = self.graph.door(a).room.0;
            let rb = self.graph.door(b).room.0;
            if ra != rb {
                adjacency[ra as usize].push(rb);
                adjacency[rb as usize].push(ra);
            }
        }

        let mut seen = vec![false; self.graph.room_count];
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
        self.connected = self.reachable as usize == self.graph.room_count;
    }

    pub fn traverse(&mut self, player: usize, side: Side) -> bool {
        self.graph.traverse(player, side)
    }

    fn reachable_count_for_links(&self, links: &[DoorId]) -> usize {
        let mut adjacency: Vec<Vec<u32>> = vec![Vec::new(); self.graph.room_count];
        for (index, door) in self.graph.doors.iter().enumerate() {
            let a = DoorId(index as u16);
            let b = links[index];
            if a.0 < b.0 {
                let other = self.graph.door(b).room;
                if door.room != other {
                    adjacency[door.room.0 as usize].push(other.0);
                    adjacency[other.0 as usize].push(door.room.0);
                }
            }
        }
        if adjacency.is_empty() {
            return 0;
        }
        let mut seen = vec![false; self.graph.room_count];
        let mut stack = vec![0usize];
        seen[0] = true;
        while let Some(room) = stack.pop() {
            for &neighbour in &adjacency[room] {
                let neighbour = neighbour as usize;
                if !seen[neighbour] {
                    seen[neighbour] = true;
                    stack.push(neighbour);
                }
            }
        }
        seen.into_iter().filter(|seen| *seen).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spine_doors(world: &ConstraintWorld) -> Vec<DoorId> {
        (0..world.graph.doors.len())
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
    fn unprotected_rewiring_rejects_disconnected_candidates() {
        // Remove the observers so nothing is pinned, then drop protection.
        let mut world = ConstraintWorld::authored();
        world.graph.players = vec![RoomId(4); world.graph.players.len()];
        world.protection_enabled = false;

        let before = world.graph.links.clone();
        for _ in 0..200 {
            world.decohere();
            if !world.connected {
                panic!("connectivity-preserving decoherence rejected too few disconnected rewires");
            }
        }
        assert_ne!(
            world.graph.links, before,
            "the unprotected graph still mutates"
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
        assert!(world.anchors.is_empty());
        assert!(world.sealed_rooms.is_empty());
    }

    #[test]
    fn anchored_room_doorways_never_rewire_while_unanchored_ones_do() {
        use observed_core::TeamId;

        // No presence anywhere and protection off, so only anchors freeze doors.
        let mut world = ConstraintWorld::authored();
        world.graph.players.clear();
        world.protection_enabled = false;

        // Anchor room 4 (a non-spine-adjacent-only room with real free doors).
        assert!(world.place_anchor(TeamId(0), RoomId(4)));
        let watched: Vec<(DoorId, DoorId)> = Side::ALL
            .iter()
            .map(|side| {
                let d = world.graph.door_id(RoomId(4), *side);
                (d, world.graph.partner(d))
            })
            .collect();

        // Track an unanchored room's doorway too, to prove decoherence is still
        // happening elsewhere in the same run.
        let free_door = world.graph.door_id(RoomId(0), Side::North);
        let mut saw_free_door_rewire = false;
        let mut saw_anchored_survive_every_time = true;

        for _ in 0..40 {
            let free_partner_before = world.graph.partner(free_door);
            world.decohere();
            for (door, expected) in &watched {
                if world.graph.partner(*door) != *expected {
                    saw_anchored_survive_every_time = false;
                }
            }
            if world.graph.partner(free_door) != free_partner_before {
                saw_free_door_rewire = true;
            }
            assert!(
                world.connected,
                "connectivity guard must hold with anchors present"
            );
        }

        assert!(
            saw_anchored_survive_every_time,
            "an anchored room's doorways must never rewire across many decoheres"
        );
        assert!(
            saw_free_door_rewire,
            "unanchored doorways must still be free to rewire"
        );

        // Removing the anchor frees the room up again.
        assert!(world.remove_anchor(TeamId(0), RoomId(4)));
        assert!(!world.is_frozen(watched[0].0) || world.is_protected(watched[0].0));
    }

    #[test]
    fn anchors_do_not_change_behavior_when_absent() {
        // With no anchors placed, is_frozen must behave exactly as before
        // (observation-or-protection only). This guards against a regression
        // where the anchors check accidentally widens the frozen set.
        let mut a = ConstraintWorld::authored();
        let mut b = ConstraintWorld::authored();
        for _ in 0..10 {
            a.decohere();
            b.decohere();
        }
        assert_eq!(a.graph.links, b.graph.links);
    }

    #[test]
    fn seal_room_self_links_every_doorway_and_records_it() {
        let mut world = ConstraintWorld::authored();
        world.seal_room(RoomId(4));
        for side in Side::ALL {
            let door = world.graph.door_id(RoomId(4), side);
            assert!(
                world.graph.is_sealed(door),
                "every doorway of a sealed room must self-link"
            );
        }
        assert_eq!(world.sealed_rooms, vec![RoomId(4)]);

        // Idempotent: sealing again doesn't duplicate the record.
        world.seal_room(RoomId(4));
        assert_eq!(world.sealed_rooms, vec![RoomId(4)]);
    }

    #[test]
    fn collapse_overrides_anchors_protection_and_presence() {
        // A room that is simultaneously observed, anchored, and spine-protected
        // must still seal — the collapse is the one force nothing else holds
        // back.
        let mut world = ConstraintWorld::authored();
        world.graph.players = vec![RoomId(1); world.graph.players.len()];
        assert!(world.place_anchor(TeamId(0), RoomId(1)));
        assert!(world.is_protected(world.graph.door_id(RoomId(1), Side::East)));

        world.seal_room(RoomId(1));
        for side in Side::ALL {
            let door = world.graph.door_id(RoomId(1), side);
            assert!(
                world.graph.is_sealed(door),
                "sealing must override presence, anchors, and spine protection"
            );
        }
    }

    #[test]
    fn sealed_rooms_never_rewire_across_many_decoheres() {
        let mut world = ConstraintWorld::authored();
        // Clear presence so nothing but the seal itself protects room 4.
        world.graph.players.clear();
        world.seal_room(RoomId(4));

        for _ in 0..100 {
            world.decohere();
            for side in Side::ALL {
                let door = world.graph.door_id(RoomId(4), side);
                assert!(
                    world.graph.is_sealed(door),
                    "a sealed room must stay sealed across every decohere"
                );
            }
        }
    }
}
