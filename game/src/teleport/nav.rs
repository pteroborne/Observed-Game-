//! Navigation and connection tracking.

use super::{RoomConnectionSlot, ThresholdSlotId, corridor_id_for};
use observed_core::{CorridorId, RoomId};
use observed_facility::map_spec::{CorridorRole, RoomRole};

/// An edge `(a, b)` whose hallway variation is frozen at `version` â€” an **anchor torch**
/// pins the structure so the corridor there stops re-rolling, even as the rest of the
/// maze decoheres. Edge-unordered (`(a, b)` == `(b, a)`). This is the *item-level* pin
/// record ([`crate::items::ItemsState::pins`]); the nav projection carries the
/// socket-keyed [`PinnedCorridor`] derived from it.
#[derive(Clone, Copy, Debug)]
pub struct PinnedEdge {
    pub a: RoomId,
    pub b: RoomId,
    pub version: u32,
}

/// A pin expressed as **corridor (place) identity** rather than an `(a, b)` room pair:
/// the derived corridor `corridor_id_for(a, b)` whose hallway variation is frozen at
/// `version`. This is the socket/attachment-keyed pin state the connectivity authority
/// reads — the crossing resolver freezes a corridor's variation by looking the corridor
/// up in the active junction topology (its stable id), never by reconstructing the room
/// pair. Anchor pins live persistently in [`crate::items::ItemsState`]; the nav producer
/// projects them into this corridor-keyed form each frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PinnedCorridor {
    pub corridor: CorridorId,
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
    /// Corridors pinned by a dropped anchor torch: each names the derived corridor whose
    /// hallway variation is frozen (see [`PinnedCorridor`]). Expressed as corridor/socket
    /// identity, not `(a, b)` room pairs, so the crossing resolver freezes a variation by
    /// the corridor the junction topology resolved it into.
    pub pinned_corridors: Vec<PinnedCorridor>,
    /// The active map specification, when available.
    pub map_spec: Option<observed_facility::map_spec::MapSpec>,
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

    /// The decohere version to use for `corridor`: the pinned version if an anchor torch
    /// froze that corridor's variation, otherwise the live `version`. This is the stable
    /// **corridor-identity** form the crossing resolver uses — it already resolved the
    /// corridor through the junction topology, so it never reconstructs the room pair.
    pub fn effective_version_for_corridor(&self, corridor: CorridorId) -> u32 {
        self.pinned_corridors
            .iter()
            .find(|p| p.corridor == corridor)
            .map(|p| p.version)
            .unwrap_or(self.version)
    }

    /// Whether `corridor` is **tethered** — frozen by a dropped anchor torch. The stable
    /// corridor-identity form.
    pub fn is_tethered_corridor(&self, corridor: CorridorId) -> bool {
        self.pinned_corridors.iter().any(|p| p.corridor == corridor)
    }

    /// The decohere version for the edge `(x, y)`, keyed by the derived corridor. Thin
    /// pair-shaped wrapper over [`Self::effective_version_for_corridor`]; retained only
    /// for the pinned-variation regression test (`teleport::test`) — no deferred consumer
    /// reads it, so it needs no 75b migration.
    pub fn effective_version(&self, x: RoomId, y: RoomId) -> u32 {
        self.effective_version_for_corridor(corridor_id_for(x, y))
    }
}
