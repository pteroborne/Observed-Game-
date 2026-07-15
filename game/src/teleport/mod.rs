//! The **teleport place model**: the player occupies exactly one discrete space at a
//! time â€” a [`Place::Room`] or a [`Place::Hallway`] â€” each in its own local frame
//! centred at the origin. Crossing a doorway gap teleports you to the next place
//! (room â†’ its edge's hallway â†’ destination room). Because only the current place
//! exists, everything else is unobserved by construction, and a doorway can point at
//! a freshly-rolled hallway variation / destination whenever you are not inside it.
//!
//! This module is pure geometry + the transition state machine: it builds the current
//! place's footprint + doorway gaps, detects when a moving body crosses a gap, and
//! computes the resulting place. The real fixed-step controller (`observed_traversal`)
//! drives the body; the presentation renders the geometry. It is deliberately
//! controller- and render-agnostic so it can be unit-tested.

use bevy::math::{Vec2, Vec3};
pub use observed_core::ThresholdSlotId;
use observed_core::{CorridorId, PlaceId, RoomId};

/// Half-extent of a room's square footprint (world units). Generous so rooms read as
/// breathable volumes, not cells, and so the wider doorway gaps still fit on a polygon edge.
pub const ROOM_HALF: f32 = 8.5;
/// The **standard threshold width** (world units): every crossable doorway is this wide,
/// clamped narrower only where a tight space (a maze corridor, a short polygon edge) forces
/// it. One module so a doorway reads the same everywhere and rooms/halls line up cleanly.
pub const THRESHOLD_WIDTH: f32 = 4.5;
/// How far inside a place the body spawns from the doorway it entered through.
pub const ENTRY_INSET: f32 = 1.2;
/// Side length of one labyrinth cell (world units). The clear corridor is
/// `MAZE_CELL - 2*MAZE_WALL_T`; with the controller's 0.4 body radius that stays roomy.
pub const MAZE_CELL: f32 = 4.2;
/// Half-thickness of a labyrinth interior wall (world units).
pub const MAZE_WALL_T: f32 = 0.3;

/// Where the player currently is.
///
/// A hallway's **identity** is its [`CorridorId`] (see [`Place::place_id`]), not the
/// `(from, to)` room pair: both traversal directions of a two-socket corridor share one
/// corridor id, and a corridor's id never changes when its threshold sockets rewire. The
/// retained `from`/`to` are directional/orientation data (which socket you entered, which
/// way the piece faces) that deferred Phase-75 consumers still read; `variation` is
/// presentation state (the rolled hallway flavour).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Room(RoomId),
    Hallway {
        corridor: CorridorId,
        entered_socket: ThresholdSlotId,
        variation: usize,
        from: RoomId,
        to: RoomId,
    },
}

impl Place {
    pub fn legacy_hallway(from: RoomId, to: RoomId, variation: usize) -> Self {
        Self::Hallway {
            corridor: corridor_id_for(from, to),
            entered_socket: ThresholdSlotId(0),
            variation,
            from,
            to,
        }
    }

    /// The canonical stable identity of this place: a room maps to its `RoomId`, a
    /// hallway to its `CorridorId` (derived from the unordered endpoint pair via
    /// [`corridor_id_for`]). This is the identity the junction topology, crossing
    /// resolver, and future multi-exit corridors key on — never the `(from, to)` pair.
    pub fn place_id(self) -> PlaceId {
        match self {
            Place::Room(room) => PlaceId::Room(room),
            Place::Hallway { corridor, .. } => PlaceId::Corridor(corridor),
        }
    }

    /// The corridor identity of a hallway place (`None` for a room).
    pub fn corridor_id(self) -> Option<CorridorId> {
        match self {
            Place::Room(_) => None,
            Place::Hallway { corridor, .. } => Some(corridor),
        }
    }
}

/// The stable [`CorridorId`] of the derived single-exit corridor spanning the unordered
/// room pair `(a, b)`. Deterministic and direction-independent (`corridor_id_for(a, b)`
/// == `corridor_id_for(b, a)`), so both traversal directions and every consumer resolve
/// the same corridor. The runtime graph carries at most one corridor per unordered room
/// pair, so packing the sorted pair is a collision-free identity.
///
/// Seam note: `MapSpec::corridors()` assigns corridor ids by *authored edge index*; the
/// live runtime graph decoheres, so runtime corridor identity is derived from the current
/// room pair instead. The two id spaces are intentionally not unified in Phase 74.
pub fn corridor_id_for(a: RoomId, b: RoomId) -> CorridorId {
    let (lo, hi) = if a.0 <= b.0 { (a.0, b.0) } else { (b.0, a.0) };
    debug_assert!(
        lo < (1 << 16) && hi < (1 << 16),
        "room ids fit in 16 bits for corridor packing"
    );
    CorridorId((lo << 16) | (hi & 0xFFFF))
}

/// The corridor-side threshold slot that `room` occupies on the derived two-socket
/// corridor spanning `(a, b)`: socket `0` for the lower-numbered endpoint, `1` for the
/// higher. Direction-independent so the room-side and hallway-side topologies agree.
pub fn corridor_socket_for(a: RoomId, b: RoomId, room: RoomId) -> ThresholdSlotId {
    let lo = a.0.min(b.0);
    ThresholdSlotId(if room.0 == lo { 0 } else { 1 })
}

pub fn place_y_offset(place: Place) -> f32 {
    match place {
        Place::Room(_) => 0.0,
        Place::Hallway { .. } => -8.0,
    }
}

/// What a doorway gap does / means in the current place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GapKind {
    /// The spine-forward doorway of a room (toward your next objective room).
    Forward,
    /// A non-spine doorway of a room (a side route).
    Side,
    /// A hallway's entry (back toward the room you came from).
    Entry,
    /// A hallway's exit (onward to the destination room).
    Exit,
    /// A hallway's exit toward the facility exit while the **keystone gate is locked** —
    /// a solid, closed door (not a passage) until enough keystones are held.
    LockedExit,
    /// A collapse-sealed threshold: rubble fills the doorway, and no observation or
    /// anchor can reopen it.
    Collapsed,
    /// A direct one-way exit from the Master Room.
    OneWayExit,
    /// An invisible, non-traversable incoming connection from the Master Room.
    OneWayEntry,
}

impl GapKind {
    /// A passage you can actually cross to another place (a `Side` door, or a
    /// `LockedExit`, is a closed door on a solid wall — you can't pass it).
    pub fn is_passage(self) -> bool {
        matches!(
            self,
            GapKind::Forward | GapKind::Entry | GapKind::Exit | GapKind::OneWayExit
        )
    }
}

// `ThresholdSlotId` is the canonical `observed_core::ThresholdSlotId(u16)`, re-exported
// above. Room slots mirror the authored observation sides (N/E/S/W as 0/1/2/3 when that
// data is available); hallway slots distinguish multiple apertures on the same end of a
// generated corridor.

/// A room-side threshold endpoint.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RoomThreshold {
    pub room: RoomId,
    pub slot: ThresholdSlotId,
}

/// Stable identity for the graph edge whose hallway is being projected. The hallway
/// variation is presentation state; the logical hall is the unordered edge between rooms.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HallId {
    pub a: RoomId,
    pub b: RoomId,
}

impl HallId {
    pub fn new(a: RoomId, b: RoomId) -> Self {
        if a.0 <= b.0 {
            Self { a, b }
        } else {
            Self { a: b, b: a }
        }
    }
}

/// A hallway-side threshold endpoint.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HallThreshold {
    pub corridor: CorridorId,
    pub slot: ThresholdSlotId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThresholdLocalSide {
    Room,
    Hall,
}

/// Full threshold identity: every rendered doorway knows both the room slot and the
/// hallway slot it connects. Geometry matching uses this instead of "nearest centre".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThresholdLink {
    pub room: RoomThreshold,
    pub hall: HallThreshold,
    pub local_side: ThresholdLocalSide,
}

/// A connected room plus the fixed room threshold slot used by that connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoomConnectionSlot {
    pub target: RoomId,
    pub slot: ThresholdSlotId,
}

/// A crossable gap on the current place's footprint edge, in the place's local frame.
#[derive(Clone, Copy, Debug)]
pub struct DoorGap {
    /// Centre of the gap on the footprint edge (XZ).
    pub center: Vec2,
    /// Outward unit normal â€” the direction you exit through.
    pub normal: Vec2,
    pub width: f32,
    /// The room this gap ultimately heads toward.
    pub target: RoomId,
    pub kind: GapKind,
    pub threshold: ThresholdLink,
    /// The local floor height a body must have its feet at (within a small tolerance) to
    /// use this gap. `0.0` for every ground-level doorway; a raised-deck exit (the gantry's
    /// upper exit) sets this above zero so a ground-level body walking under its XZ span
    /// does not also "cross" it.
    pub floor_y: f32,
}

pub(crate) fn is_point_on_segment(p: Vec2, a: Vec2, b: Vec2, tolerance: f32) -> bool {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-6 {
        return (p - a).length() < tolerance;
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let projection = a + ab * t;
    (p - projection).length() < tolerance
}

/// An interior wall segment inside a place's footprint (local frame, centred at 0).
/// Used to carve a labyrinth inside a hallway; rooms and simple halls have none.
#[derive(Clone, Copy, Debug)]
pub struct WallSeg {
    pub center: Vec2,
    pub half: Vec2,
}

/// A walkable raised deck inside a place's footprint (local frame, centred at 0): solid
/// from the place's floor up to `top_y`, so a body can stand on top of it (and walk
/// underneath, if nothing else blocks). Used to project the gantry's jump-map platforms
/// and its mount stair; empty everywhere else.
#[derive(Clone, Copy, Debug)]
pub struct DeckSeg {
    pub center: Vec2,
    pub half: Vec2,
    /// Local bottom height relative to the place floor.
    pub bottom_y: f32,
    pub top_y: f32,
}

/// Explicit semantic structure kind for a frozen place. Consumers must use this rather
/// than inferring architecture from deck counts, polygon shape, or elevation.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum PlaceStructureKind {
    Room,
    #[default]
    Corridor,
    CurvedChicane,
    Orthogonal,
    Colonnade,
    PressureGate,
    LegacyGantry,
    GantryExpanse,
    Wellshaft,
}

/// A yawed box footprint in a place's local XZ plane. This is the common structural
/// shape used by smooth sampled walls, rotated platforms, and faceted architecture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrientedBoxSolid {
    pub center: Vec2,
    pub half: Vec2,
    pub yaw: f32,
    pub bottom_y: f32,
    pub top_y: f32,
}

/// A convex-prism structural solid. `footprint` is CCW in the place-local XZ plane.
/// Hexagonal columns and non-rectangular structural masses use this so presentation
/// and Rapier consume the exact same authored vertices.
#[derive(Clone, Debug, PartialEq)]
pub struct ConvexPrismSolid {
    pub footprint: Vec<Vec2>,
    pub bottom_y: f32,
    pub top_y: f32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlaceRouteKind {
    JumpLine,
    HighBridge,
    Understory,
}

/// Stable ordered traversal hints projected from structural generation. They are debug
/// and bot inputs only; simulation validity remains owned by the collision solids.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceRouteGuide {
    pub kind: PlaceRouteKind,
    pub nodes: Vec<Vec3>,
}

/// The current place's footprint + its doorway gaps (local frame, centred at 0).
///
/// `half` is always the bounding half-extent (used for floor/light/bounds sizing). A
/// hallway is an axis-aligned box (`poly: None`) whose walls come from `interior` +
/// `place_arena`'s perimeter. A room is a convex **polygon** (`poly: Some(vertices)`,
/// CCW, centred at 0); its walls are the polygon edges projected as yawed Rapier
/// colliders, and it has no `interior` or AABB perimeter.
#[derive(Clone, Debug)]
pub struct PlaceGeom {
    pub structure_kind: PlaceStructureKind,
    pub architecture_register: Option<observed_content::ArchitectureRegister>,
    pub design_key: Option<u64>,
    pub half: Vec2,
    pub gaps: Vec<DoorGap>,
    pub interior: Vec<WallSeg>,
    pub poly: Option<Vec<Vec2>>,
    /// Walkable raised decks (the gantry's platforms + mount stair); empty everywhere else.
    pub decks: Vec<DeckSeg>,
    /// General yawed structural boxes. Legacy axis-aligned walls/decks remain in their
    /// compact fields; new architecture uses this list.
    pub oriented_solids: Vec<OrientedBoxSolid>,
    /// General convex structural prisms, including true hexagonal column fields.
    pub convex_solids: Vec<ConvexPrismSolid>,
    pub route_guides: Vec<PlaceRouteGuide>,
}

impl PlaceGeom {
    pub fn forward_gap(&self) -> Option<&DoorGap> {
        self.gaps.iter().find(|g| g.kind == GapKind::Forward)
    }

    /// Structural discriminator for the authored vertical connector. Kept derived from
    /// geometry so persistent simulation state does not acquire a presentation flavor.
    pub fn is_wellshaft(&self) -> bool {
        self.structure_kind == PlaceStructureKind::Wellshaft
    }
}

pub mod aperture;
pub mod geom;
pub mod nav;
pub mod transition;

#[cfg(test)]
pub mod test;

pub use aperture::{
    AperturePlan, AperturePlanError, PlannedAperture, ThresholdClosure, WallPanel, plan_boundary,
};
pub use geom::{
    HallwayGeomEndpoints, contain, geom_for, hallway_geom, hallway_geom_with_slots,
    hallway_geom_with_slots_and_role, open_entry, open_entry_threshold, room_geom,
    room_geom_with_slots, room_preview_geom,
};
pub use nav::{LiveCorridorConnection, Nav, PinnedCorridor, PinnedEdge};
pub use transition::{
    Align2d, Crossing, GAP_FLOOR_TOLERANCE, PREVIEW_OUTSET, apply_crossing, arrival_gap,
    capsule_crossing_fraction, crossed, crossing_alignment, entry_spawn, feet_at_gap_floor,
    hallway_alignment, hallway_gap_alignment, inside_footprint, place_arena, place_arena_spec,
    place_boundary_primitives, place_junction, place_rapier_scene, place_structural_primitives,
    resolve_crossing, room_alignment, structural_height,
};
