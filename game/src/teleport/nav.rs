//! Navigation and connection tracking.

use super::{RoomConnectionSlot, ThresholdSlotId};
use observed_core::RoomId;
use observed_facility::map_spec::{CorridorRole, RoomRole};

/// An edge `(a, b)` whose hallway variation is frozen at `version` â€” an **anchor torch**
/// pins the structure so the corridor there stops re-rolling, even as the rest of the
/// maze decoheres. Edge-unordered (`(a, b)` == `(b, a)`).
#[derive(Clone, Copy, Debug)]
pub struct PinnedEdge {
    pub a: RoomId,
    pub b: RoomId,
    pub version: u32,
}

/// A snapshot of the brain's navigation state the place model reads (supplied by the
/// match each frame; constructed directly in tests).
#[derive(Clone, Debug)]
pub struct Nav {
    /// Rooms connected to the current room (its open doorways' partners).
    pub connections: Vec<RoomId>,
    /// Fixed room threshold slots for the connections above, when the caller can resolve
    /// them from the authoritative door IDs.
    pub connection_slots: Vec<RoomConnectionSlot>,
    /// Fixed room threshold slots sealed by the collapse. They render as rubble and are
    /// never crossable, even if an anchor previously froze that relation.
    pub sealed_slots: Vec<ThresholdSlotId>,
    /// For a rendered hallway, the room-side slot at the entry/back end.
    pub hallway_entry_room_slot: Option<ThresholdSlotId>,
    /// For a rendered hallway, the room-side slot at the exit/forward end.
    pub hallway_exit_room_slot: Option<ThresholdSlotId>,
    /// The spine-forward objective room, if the local team is still running.
    pub target_room: Option<RoomId>,
    /// The active map role for a rendered room, when known. Geometry uses this for
    /// role-shaped footprints without reaching back into a global map singleton.
    pub room_role: Option<RoomRole>,
    /// The active map's [`CorridorRole`] for each of this room's connections (the edge
    /// to that neighbour), when known from the map spec. A room can have several
    /// doorways to distinct edges with distinct roles, so this is a per-neighbour list
    /// (paralleling `connection_slots`) rather than one scalar — [`Nav::corridor_role_for`]
    /// resolves the role for a specific hallway's `to` room. Geometry uses this to pick
    /// a hallway's interior generator (WFC vs. DFS+braid maze) without reaching back
    /// into a global map singleton. Empty when the current map has no spec
    /// (authored/dev fallbacks).
    pub corridor_roles: Vec<(RoomId, CorridorRole)>,
    pub seed: u64,
    /// Increments when the graph decoheres, so an edge re-rolls its hallway.
    pub version: u32,
    /// The keystone gate is shut: a hallway toward the facility exit shows a solid
    /// `LockedExit` instead of an open `Exit` until enough keystones are held.
    pub exit_locked: bool,
    /// The active map's exit room.
    pub exit_room: RoomId,
    /// Edges pinned by a dropped anchor torch (their variation is frozen).
    pub pins: Vec<PinnedEdge>,
}

impl Nav {
    pub fn slot_for(&self, target: RoomId) -> Option<ThresholdSlotId> {
        self.connection_slots
            .iter()
            .find(|connection| connection.target == target)
            .map(|connection| connection.slot)
    }

    /// The [`CorridorRole`] of the edge from this room to `target`, if the active map
    /// spec has one for that pair.
    pub fn corridor_role_for(&self, target: RoomId) -> Option<CorridorRole> {
        self.corridor_roles
            .iter()
            .find(|(room, _)| *room == target)
            .map(|(_, role)| *role)
    }

    /// The decohere version to use for the edge `(x, y)`: the pinned version if an anchor
    /// torch froze it, otherwise the live `version`.
    pub fn effective_version(&self, x: RoomId, y: RoomId) -> u32 {
        let key = |a: RoomId, b: RoomId| if a.0 <= b.0 { (a.0, b.0) } else { (b.0, a.0) };
        let want = key(x, y);
        self.pins
            .iter()
            .find(|p| key(p.a, p.b) == want)
            .map(|p| p.version)
            .unwrap_or(self.version)
    }

    /// Whether the edge `(x, y)` is **tethered** â€” frozen by a dropped anchor torch (its
    /// variation pinned). A doorway's frame light reads this so a glance shows which edges
    /// are anchored. Edge-unordered.
    pub fn is_tethered(&self, x: RoomId, y: RoomId) -> bool {
        let key = |a: RoomId, b: RoomId| if a.0 <= b.0 { (a.0, b.0) } else { (b.0, a.0) };
        let want = key(x, y);
        self.pins.iter().any(|p| key(p.a, p.b) == want)
    }
}
