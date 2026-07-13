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

use bevy::math::Vec2;
use observed_core::RoomId;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Room(RoomId),
    Hallway {
        from: RoomId,
        to: RoomId,
        variation: usize,
    },
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

/// Stable ordinal of a threshold on one side of a room or hallway. Room slots mirror the
/// authored observation sides (N/E/S/W as 0/1/2/3 when that data is available); hallway
/// slots distinguish multiple apertures on the same end of a generated corridor.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ThresholdSlotId(pub u8);

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

/// A hallway-side threshold endpoint. `side` is the room this hallway aperture faces.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HallThreshold {
    pub hall: HallId,
    pub side: RoomId,
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

/// The current place's footprint + its doorway gaps (local frame, centred at 0).
///
/// `half` is always the bounding half-extent (used for floor/light/bounds sizing). A
/// hallway is an axis-aligned box (`poly: None`) whose walls come from `interior` +
/// `place_arena`'s perimeter. A room is a convex **polygon** (`poly: Some(vertices)`,
/// CCW, centred at 0); its walls are the polygon edges projected as yawed Rapier
/// colliders, and it has no `interior` or AABB perimeter.
#[derive(Clone, Debug)]
pub struct PlaceGeom {
    pub half: Vec2,
    pub gaps: Vec<DoorGap>,
    pub interior: Vec<WallSeg>,
    pub poly: Option<Vec<Vec2>>,
    /// Walkable raised decks (the gantry's platforms + mount stair); empty everywhere else.
    pub decks: Vec<DeckSeg>,
}

impl PlaceGeom {
    pub fn forward_gap(&self) -> Option<&DoorGap> {
        self.gaps.iter().find(|g| g.kind == GapKind::Forward)
    }

    /// Structural discriminator for the authored vertical connector. Kept derived from
    /// geometry so persistent simulation state does not acquire a presentation flavor.
    pub fn is_wellshaft(&self) -> bool {
        self.decks.len() > 20
            && self
                .gaps
                .iter()
                .any(|gap| (gap.floor_y - crate::hallway::WELL_SHAFT_HEIGHT).abs() < 0.01)
    }
}

pub mod geom;
pub mod nav;
pub mod transition;

#[cfg(test)]
pub mod test;

pub use geom::{
    HallwayGeomEndpoints, contain, geom_for, hallway_geom, hallway_geom_with_slots,
    hallway_geom_with_slots_and_role, open_entry, room_geom, room_geom_with_slots,
    room_preview_geom,
};
pub use nav::{Nav, PinnedEdge};
pub use transition::{
    Align2d, Crossing, GAP_FLOOR_TOLERANCE, PREVIEW_OUTSET, apply_crossing, arrival_gap, crossed,
    crossing_alignment, entry_spawn, feet_at_gap_floor, hallway_alignment, hallway_gap_alignment,
    place_arena, place_rapier_scene, place_structural_primitives, room_alignment,
    structural_height,
};
