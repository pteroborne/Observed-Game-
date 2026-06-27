//! Pure production model for **doors as the observation gate**.
//!
//! The game's defining mechanic is "the structure changes when unobserved." Where
//! [`observed_observation`] drives that by *occupying/seeing* a room, this model asks
//! a sharper question: can a **player-operated door** be the gate?
//!
//! * A **closed** door hides the connection and leaves it *free to rewire*.
//! * **Opening** a door observes it, **freezing** that connection (and its partner)
//!   so it cannot rewire while open.
//! * Closing a door (a "slam") releases it back to free.
//! * You may only **traverse** an open door — a closed door is a mystery until you
//!   open it, and reopening one that rewired while closed reveals a *changed*
//!   partner (the "the path changed behind you" loop).
//! * A **protected spine** (a fixed set of door pairs) is always pinned, so the exit
//!   stays reachable no matter how the rest rewires; dead-end pockets may appear but
//!   never sever the spine.
//!
//! It reuses [`observed_observation`]'s graph *structure* (rooms, doorways, the
//! authored lattice, geometry, traversal) but replaces the *pinning rule*: pinned ⇔
//! the door is open or on the spine. That rule's re-matching is implemented here
//! (deterministic, seeded — replayable). Promoted out of `door_lab` in refactor R5;
//! the lab is the debug projection.

use glam::Vec2;
use observed_core::RoomId;
use observed_observation::{COLS, DOOR_COUNT, DoorId, ObservationWorld, ROOM_COUNT, Side};

/// splitmix64 — the same deterministic PRNG `observed_observation` uses, so rewiring
/// is replayable and testable.
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

/// The protected spine: a path of rooms whose connecting doors are always pinned so
/// the exit is always reachable. Start → exit across the 3×3 lattice.
const SPINE_ROOMS: [u32; 5] = [0, 1, 2, 5, 8];
const START_ROOM: RoomId = RoomId(0);
const EXIT_ROOM: RoomId = RoomId(8);

#[derive(Clone)]
pub struct DoorWorld {
    /// Reused graph structure (doorways, links, room geometry, traversal helpers).
    graph: ObservationWorld,
    /// Per-doorway: is this door currently open (observed → frozen)?
    open: Vec<bool>,
    /// Per-doorway: on the protected spine (always pinned)?
    spine: Vec<bool>,
    /// Partner a door had the last time it was opened, to detect a changed partner
    /// on reopen.
    partner_when_opened: Vec<Option<DoorId>>,
    player: RoomId,
    base_seed: u64,
    pub decoherence_count: u32,
    pub rewires_last: u32,
    pub slams: u32,
    pub reopened_changed: u32,
}

fn side_between(a: RoomId, b: RoomId) -> Option<Side> {
    let (ar, ac) = (a.0 / COLS, a.0 % COLS);
    let (br, bc) = (b.0 / COLS, b.0 % COLS);
    if ar == br && bc == ac + 1 {
        Some(Side::East)
    } else if ar == br && ac == bc + 1 {
        Some(Side::West)
    } else if ac == bc && br == ar + 1 {
        Some(Side::South)
    } else if ac == bc && ar == br + 1 {
        Some(Side::North)
    } else {
        None
    }
}

impl DoorWorld {
    pub fn authored() -> Self {
        let graph = ObservationWorld::authored();
        let mut spine = vec![false; DOOR_COUNT];
        // Mark both doorways of every spine connection.
        for pair in SPINE_ROOMS.windows(2) {
            let (a, b) = (RoomId(pair[0]), RoomId(pair[1]));
            let side = side_between(a, b).expect("spine rooms are grid-adjacent");
            let door = graph.door_id(a, side);
            let partner = graph.partner(door);
            spine[door.0 as usize] = true;
            spine[partner.0 as usize] = true;
        }
        Self {
            graph,
            open: vec![false; DOOR_COUNT],
            spine,
            partner_when_opened: vec![None; DOOR_COUNT],
            player: START_ROOM,
            base_seed: 0xD00D_5EED_0000_0001,
            decoherence_count: 0,
            rewires_last: 0,
            slams: 0,
            reopened_changed: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::authored();
    }

    // -- queries ----------------------------------------------------------

    pub fn player(&self) -> RoomId {
        self.player
    }

    pub fn start(&self) -> RoomId {
        START_ROOM
    }

    pub fn exit(&self) -> RoomId {
        EXIT_ROOM
    }

    pub fn door_id(&self, room: RoomId, side: Side) -> DoorId {
        self.graph.door_id(room, side)
    }

    pub fn partner(&self, door: DoorId) -> DoorId {
        self.graph.partner(door)
    }

    pub fn is_sealed(&self, door: DoorId) -> bool {
        self.graph.is_sealed(door)
    }

    pub fn is_open(&self, door: DoorId) -> bool {
        self.open[door.0 as usize]
    }

    pub fn is_spine(&self, door: DoorId) -> bool {
        self.spine[door.0 as usize]
    }

    /// Pinned ⇔ open or on the spine. Pinned doors never rewire.
    pub fn is_pinned(&self, door: DoorId) -> bool {
        self.open[door.0 as usize] || self.spine[door.0 as usize]
    }

    pub fn room_center(&self, room: RoomId) -> Vec2 {
        self.graph.room_center(room)
    }

    pub fn door_position(&self, door: DoorId) -> Vec2 {
        self.graph.door_position(door)
    }

    pub fn destination(&self, door: DoorId) -> RoomId {
        self.graph.door(self.graph.partner(door)).room
    }

    /// The matching is a valid symmetric involution (every door pairs back).
    pub fn matching_valid(&self) -> bool {
        (0..DOOR_COUNT).all(|i| {
            let d = DoorId(i as u16);
            self.graph.partner(self.graph.partner(d)) == d
        })
    }

    /// BFS over non-sealed connections (you can open any door you reach, so a closed
    /// door is not a wall): is the exit reachable from the start?
    pub fn exit_reachable(&self) -> bool {
        let mut seen = [false; ROOM_COUNT];
        let mut stack = vec![START_ROOM];
        seen[START_ROOM.0 as usize] = true;
        while let Some(room) = stack.pop() {
            for side in Side::ALL {
                let door = self.graph.door_id(room, side);
                if self.graph.is_sealed(door) {
                    continue;
                }
                let next = self.destination(door);
                if !seen[next.0 as usize] {
                    seen[next.0 as usize] = true;
                    stack.push(next);
                }
            }
        }
        seen[EXIT_ROOM.0 as usize]
    }

    /// Rooms (other than start/exit) reachable by exactly one non-sealed door — a
    /// dead-end pocket.
    pub fn dead_end_rooms(&self) -> Vec<RoomId> {
        (0..ROOM_COUNT as u32)
            .map(RoomId)
            .filter(|&room| {
                if room == START_ROOM || room == EXIT_ROOM {
                    return false;
                }
                let degree = Side::ALL
                    .iter()
                    .filter(|&&side| !self.graph.is_sealed(self.graph.door_id(room, side)))
                    .count();
                degree == 1
            })
            .collect()
    }

    // -- player actions ---------------------------------------------------

    /// Open a connection (observe it → freeze). Detects a changed partner on reopen.
    pub fn open(&mut self, door: DoorId) {
        let partner = self.graph.partner(door);
        if let Some(prev) = self.partner_when_opened[door.0 as usize]
            && prev != partner
        {
            self.reopened_changed += 1;
        }
        self.open[door.0 as usize] = true;
        self.partner_when_opened[door.0 as usize] = Some(partner);
        if partner != door {
            self.open[partner.0 as usize] = true;
            self.partner_when_opened[partner.0 as usize] = Some(door);
        }
    }

    /// Close a connection (it "slams"): it becomes free to rewire again.
    pub fn close(&mut self, door: DoorId) {
        let partner = self.graph.partner(door);
        if self.open[door.0 as usize] {
            self.slams += 1;
        }
        self.open[door.0 as usize] = false;
        if partner != door {
            self.open[partner.0 as usize] = false;
        }
    }

    /// Toggle the door on the given side of the player's room.
    pub fn toggle_facing(&mut self, side: Side) {
        let door = self.graph.door_id(self.player, side);
        if self.open[door.0 as usize] {
            self.close(door);
        } else {
            self.open(door);
        }
    }

    /// Walk through the door on `side` — only if it is open (a closed door blocks).
    pub fn traverse(&mut self, side: Side) -> bool {
        let door = self.graph.door_id(self.player, side);
        if self.graph.is_sealed(door) || !self.open[door.0 as usize] {
            return false;
        }
        self.player = self.destination(door);
        true
    }

    // -- evolution --------------------------------------------------------

    /// Re-match every *unpinned* (closed, non-spine) doorway into a fresh perfect
    /// matching, leaving open and spine connections frozen. Deterministic per state.
    pub fn decohere(&mut self) {
        self.decoherence_count += 1;
        let before = self.graph.links.clone();

        let mut free: Vec<DoorId> = (0..DOOR_COUNT as u16)
            .map(DoorId)
            .filter(|&d| !self.is_pinned(d))
            .collect();

        let mut rng = SplitMix(
            self.base_seed ^ (self.decoherence_count as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
        );
        for i in (1..free.len()).rev() {
            free.swap(i, rng.below(i + 1));
        }

        let mut chunks = free.chunks_exact(2);
        for pair in chunks.by_ref() {
            self.graph.links[pair[0].0 as usize] = pair[1];
            self.graph.links[pair[1].0 as usize] = pair[0];
        }
        if let [leftover] = chunks.remainder() {
            self.graph.links[leftover.0 as usize] = *leftover;
        }

        self.rewires_last = (0..self.graph.links.len())
            .filter(|&i| self.graph.links[i] != before[i])
            .count() as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A door in the free interior (room 4 ↔ room 3); neither room is on the spine.
    fn free_door(world: &DoorWorld) -> DoorId {
        world.door_id(RoomId(4), Side::West)
    }

    #[test]
    fn authored_spine_connects_start_to_exit() {
        let world = DoorWorld::authored();
        assert!(world.matching_valid());
        assert!(world.exit_reachable());
        // Spine doorways are pinned even with nothing open.
        let spine_door = world.door_id(RoomId(0), Side::East);
        assert!(world.is_spine(spine_door));
        assert!(world.is_pinned(spine_door));
    }

    #[test]
    fn opening_a_door_freezes_its_connection_across_decoherence() {
        let mut world = DoorWorld::authored();
        let door = free_door(&world);
        world.open(door);
        let partner = world.partner(door);
        world.decohere();
        assert!(world.matching_valid());
        assert_eq!(
            world.partner(door),
            partner,
            "an open door's connection must not rewire"
        );
        assert!(
            world.rewires_last > 0,
            "closed doors elsewhere still rewired"
        );
    }

    #[test]
    fn rewiring_happens_only_behind_closed_doors() {
        let mut world = DoorWorld::authored();
        // Open a couple of interior connections.
        let opened = [
            world.door_id(RoomId(4), Side::West),
            world.door_id(RoomId(4), Side::South),
        ];
        for d in opened {
            world.open(d);
        }
        let snapshot: Vec<(DoorId, DoorId)> = (0..DOOR_COUNT as u16)
            .map(DoorId)
            .filter(|&d| world.is_open(d))
            .map(|d| (d, world.partner(d)))
            .collect();
        world.decohere();
        for (door, partner) in snapshot {
            assert_eq!(
                world.partner(door),
                partner,
                "no open door may change across a decohere"
            );
        }
    }

    #[test]
    fn closed_doors_can_rewire() {
        let mut world = DoorWorld::authored();
        let door = free_door(&world); // closed by default
        let original = world.partner(door);
        let mut changed = false;
        for _ in 0..16 {
            world.decohere();
            if world.partner(door) != original {
                changed = true;
                break;
            }
        }
        assert!(changed, "a closed, non-spine door should eventually rewire");
    }

    #[test]
    fn the_spine_keeps_the_exit_reachable_through_any_rewiring() {
        let mut world = DoorWorld::authored();
        for _ in 0..32 {
            world.decohere();
            assert!(world.matching_valid());
            assert!(
                world.exit_reachable(),
                "the protected spine must always keep the exit reachable"
            );
        }
    }

    #[test]
    fn reopening_a_closed_door_can_reveal_a_changed_partner() {
        let mut world = DoorWorld::authored();
        let door = free_door(&world);
        world.open(door);
        let original = world.partner(door);
        world.close(door);
        // Let it rewire while closed.
        for _ in 0..16 {
            world.decohere();
            if world.partner(door) != original {
                break;
            }
        }
        assert_ne!(world.partner(door), original, "it should have rewired");
        world.open(door); // reopen → notices the changed partner
        assert!(
            world.reopened_changed > 0,
            "reopening a rewired door registers the path change"
        );
    }

    #[test]
    fn closing_an_open_door_counts_as_a_slam() {
        let mut world = DoorWorld::authored();
        let door = free_door(&world);
        world.open(door);
        assert_eq!(world.slams, 0);
        world.close(door);
        assert_eq!(world.slams, 1, "a closed-while-open door slams");
        // Closing an already-closed door is not a slam.
        world.close(door);
        assert_eq!(world.slams, 1);
    }

    #[test]
    fn traversal_requires_an_open_door() {
        let mut world = DoorWorld::authored();
        // Room 0 East leads to room 1 in the authored lattice.
        let door = world.door_id(RoomId(0), Side::East);
        assert!(!world.traverse(Side::East), "a closed door blocks");
        assert_eq!(world.player(), RoomId(0));
        world.open(door);
        assert!(world.traverse(Side::East), "an open door lets you through");
        assert_eq!(world.player(), RoomId(1));
    }

    #[test]
    fn dead_ends_are_detected_and_do_not_sever_the_exit() {
        let mut world = DoorWorld::authored();
        assert!(
            world.dead_end_rooms().is_empty(),
            "the authored lattice has no dead-ends"
        );
        // Hand-seal room 6's only non-boundary connections except one (6↔3), leaving
        // it reachable by a single door — a dead-end pocket.
        let east = world.door_id(RoomId(6), Side::East); // 6 ↔ 7
        let partner = world.partner(east);
        world.graph.links[east.0 as usize] = east;
        world.graph.links[partner.0 as usize] = partner;
        let dead_ends = world.dead_end_rooms();
        assert!(dead_ends.contains(&RoomId(6)), "room 6 is now a dead-end");
        assert!(
            world.exit_reachable(),
            "a dead-end pocket must not sever the spine's exit path"
        );
    }

    #[test]
    fn decoherence_is_deterministic() {
        let mut a = DoorWorld::authored();
        let mut b = DoorWorld::authored();
        for _ in 0..10 {
            a.decohere();
            b.decohere();
        }
        assert_eq!(a.graph.links, b.graph.links);
        assert_eq!(a.rewires_last, b.rewires_last);
        // A different seed diverges.
        let mut c = DoorWorld::authored();
        c.base_seed = 999;
        for _ in 0..10 {
            c.decohere();
        }
        assert_ne!(a.graph.links, c.graph.links);
    }

    #[test]
    fn reset_restores_the_authored_world() {
        let mut world = DoorWorld::authored();
        world.open(free_door(&world));
        for _ in 0..5 {
            world.decohere();
        }
        world.reset();
        assert_eq!(world.decoherence_count, 0);
        assert_eq!(world.slams, 0);
        assert_eq!(world.player(), RoomId(0));
        assert!(world.exit_reachable());
        assert!(world.dead_end_rooms().is_empty());
    }
}
