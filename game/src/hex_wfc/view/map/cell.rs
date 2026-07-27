//! How one discovered hex is classified for the survivor map.
//!
//! A tile is a pixel. On its own it says almost nothing; what matters is what it
//! composes — a room, a corridor run, a vertical link — and whether that
//! composition can be trusted to still be there later. This module owns those
//! judgements so the builder stays a builder.

use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{HexArchetype, HexSpace, HexWfcWorld};
use observed_hex::{HexCoord, TILE_LEVEL_HEIGHT};
use observed_match::hex_wfc::{HexMapCellKnowledge, HexMapDiscovery};
use observed_style::{MarkerRole, ObservedState, SurfaceRole};

/// A room reads as one mass, so its hexes meet with no seam.
pub(super) const ROOM_INSET: f32 = 1.0;
/// A corridor reads as a ribbon between things, not as a tile.
pub(super) const HALL_INSET: f32 = 0.52;
/// Vertical circulation is a node on the run: wider than a corridor, narrower
/// than a room.
pub(super) const VERTICAL_INSET: f32 = 0.74;
/// How far a level off the focus floor is pushed down in brightness.
pub(super) const UNFOCUSED_DIM: f32 = 0.42;
/// Glimpsed cells are dimmer than traversed ones but must stay readable.
const GLIMPSED_DIM: f32 = 0.5;

/// What the tile is part of. This is the "pixels compose something" channel, and
/// it is carried by footprint width rather than colour, so it survives being
/// read at a glance and does not compete with the district hue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Composition {
    /// Part of a stamped room blueprint.
    Room,
    /// Flat circulation — the connective tissue between everything else.
    Hall,
    /// A ramp or shaft: the run's vertical joint.
    Vertical,
}

impl Composition {
    pub(super) fn of(archetype: HexArchetype, space: HexSpace, in_blueprint: bool) -> Self {
        if in_blueprint || space == HexSpace::Room {
            return Self::Room;
        }
        match archetype {
            HexArchetype::RampUp | HexArchetype::RampHead | HexArchetype::Shaft => Self::Vertical,
            _ => Self::Hall,
        }
    }

    pub(super) fn inset(self) -> f32 {
        match self {
            Self::Room => ROOM_INSET,
            Self::Hall => HALL_INSET,
            Self::Vertical => VERTICAL_INSET,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Room => "room",
            Self::Hall => "hallway",
            Self::Vertical => "vertical",
        }
    }
}

/// Whether this part of the sketch can be trusted to still be there later.
///
/// Deliberately derived from leak-free sources only. `HexWfcMatch::observation`
/// is a public field and would be the obvious input, but it is a *global* frame:
/// a cell that appears there only because a rival is standing in it would render
/// as pinned and hand the survivor that rival's position. So the inputs here are
/// the cell's own archetype, the team's own anchors, and the team's own
/// occupancy — all three of which the survivor already knows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stability {
    /// Geometry the solver never rewires — rooms, ramps, shafts.
    Permanent,
    /// Currently held: a deployed anchor, or a teammate standing on it.
    Held,
    /// Plain corridor with nobody watching. This is what relayout eats.
    Mutable,
}

impl Stability {
    /// Mirrors `collapse::placement_is_mutable_topology`, which is private to
    /// `observed_facility`. Only `Void`, `Straight`, `Corner` and `Junction` are
    /// topology-mutable; everything else is structural and survives a relayout.
    /// If that predicate ever changes, this must follow it — the map claiming
    /// permanence it cannot deliver is worse than the map saying nothing.
    pub(super) fn of(archetype: HexArchetype, anchored: bool, team_occupied: bool) -> Self {
        let mutable_topology = matches!(
            archetype,
            HexArchetype::Void
                | HexArchetype::Straight
                | HexArchetype::Corner
                | HexArchetype::Junction
        );
        if !mutable_topology {
            Self::Permanent
        } else if anchored || team_occupied {
            Self::Held
        } else {
            Self::Mutable
        }
    }

    /// Only `Held` earns a cap.
    ///
    /// Capping `Permanent` too was the first design and it was wrong twice over:
    /// it gilds roughly half the map, because rooms plus vertical circulation are
    /// most of what gets discovered, and it is redundant — the composition
    /// channel already carries it, since a room or a vertical joint is exactly
    /// the geometry the solver never rewires. `Held` is the one that earns its
    /// own mark: it is rare, it is transient, and it is the only one the survivor
    /// caused, by spending a lantern or by standing there.
    pub(super) fn cap(self) -> Option<MarkerRole> {
        match self {
            Self::Held => Some(MarkerRole::Control),
            Self::Permanent | Self::Mutable => None,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Permanent => "permanent",
            Self::Held => "held",
            Self::Mutable => "can rewire",
        }
    }
}

/// The survivor states the corner sketch carried, preserved verbatim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CellState {
    Traversed,
    Glimpsed,
    Stale,
}

impl CellState {
    /// Stale wins over discovery: a cell the facility has rewired past is no
    /// longer trustworthy no matter how well it was once known. An anchored
    /// cell is never stale, which is what makes a deployed lantern worth having.
    pub(super) fn of(known: &HexMapCellKnowledge, world: &HexWfcWorld, cell: HexCoord) -> Self {
        if known.is_stale(world, cell) {
            Self::Stale
        } else if known.discovery == HexMapDiscovery::Traversed {
            Self::Traversed
        } else {
            Self::Glimpsed
        }
    }

    pub(super) fn key(self) -> u8 {
        match self {
            Self::Traversed => 0,
            Self::Glimpsed => 1,
            Self::Stale => 2,
        }
    }

    /// Colour is the district accent; the survivor state modulates it. Stale
    /// deliberately leaves the district palette entirely and reads as inert
    /// structure, because the district is exactly what may have changed.
    pub(super) fn treatment(
        self,
        register: ArchitectureRegister,
        focused: bool,
    ) -> (Color, LinearRgba) {
        let gain = if focused { 1.0 } else { UNFOCUSED_DIM };
        let (base, emissive) = match self {
            Self::Stale => {
                let treatment = observed_style::observed_modulate(
                    observed_style::surface(SurfaceRole::Wall),
                    ObservedState::Unobserved,
                );
                (treatment.base_color, treatment.emissive)
            }
            Self::Traversed => {
                let accent = observed_style::architecture(register).accent;
                (Color::LinearRgba(accent), accent * 0.22)
            }
            Self::Glimpsed => {
                let accent = observed_style::architecture(register).accent * GLIMPSED_DIM;
                (Color::LinearRgba(accent), accent * 0.10)
            }
        };
        (Color::LinearRgba(base.to_linear() * gain), emissive * gain)
    }
}

/// Height in metres for an archetype's slab; the ordering is the legend.
#[must_use]
pub(super) fn archetype_height(archetype: HexArchetype) -> Option<f32> {
    match archetype {
        HexArchetype::Void => None,
        HexArchetype::Straight | HexArchetype::Corner => Some(0.9),
        HexArchetype::Junction => Some(1.5),
        HexArchetype::RampHead => Some(0.6),
        HexArchetype::Room => Some(2.6),
        HexArchetype::RampUp => Some(4.5),
        HexArchetype::Shaft => Some(TILE_LEVEL_HEIGHT * 0.85),
    }
}

/// Distinct cache key per signal role, so two roles never share a material.
pub(super) fn marker_key(role: MarkerRole) -> u8 {
    match role {
        MarkerRole::You => 1,
        MarkerRole::Exit => 2,
        MarkerRole::Control => 3,
        MarkerRole::NextRoom => 4,
        _ => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn void_is_the_only_archetype_the_map_does_not_draw() {
        assert!(archetype_height(HexArchetype::Void).is_none());
        for archetype in [
            HexArchetype::Room,
            HexArchetype::Straight,
            HexArchetype::Corner,
            HexArchetype::Junction,
            HexArchetype::RampUp,
            HexArchetype::RampHead,
            HexArchetype::Shaft,
        ] {
            assert!(archetype_height(archetype).is_some_and(|h| h > 0.0));
        }
    }

    #[test]
    fn vertical_circulation_reads_taller_than_flat_circulation() {
        let straight = archetype_height(HexArchetype::Straight).expect("draws");
        let junction = archetype_height(HexArchetype::Junction).expect("draws");
        let room = archetype_height(HexArchetype::Room).expect("draws");
        let shaft = archetype_height(HexArchetype::Shaft).expect("draws");
        assert!(straight < junction && junction < room && room < shaft);
    }

    #[test]
    fn a_room_fills_its_hex_and_a_hallway_does_not() {
        // This is the whole "tiles are pixels" channel: adjacent room cells must
        // meet with no seam so a multi-cell room reads as one space, while a
        // corridor must read as a ribbon strung between things.
        assert!((Composition::Room.inset() - 1.0).abs() < f32::EPSILON);
        assert!(Composition::Hall.inset() < Composition::Vertical.inset());
        assert!(Composition::Vertical.inset() < Composition::Room.inset());
    }

    #[test]
    fn blueprint_membership_beats_archetype_when_classifying_a_room() {
        // A room's footprint can contain a cell whose archetype is not `Room`;
        // the blueprint is the authority on what composes a room.
        assert_eq!(
            Composition::of(HexArchetype::Junction, HexSpace::Hall, true),
            Composition::Room
        );
        assert_eq!(
            Composition::of(HexArchetype::Junction, HexSpace::Hall, false),
            Composition::Hall
        );
        assert_eq!(
            Composition::of(HexArchetype::Shaft, HexSpace::Hall, false),
            Composition::Vertical
        );
    }

    #[test]
    fn only_topology_mutable_archetypes_can_rewire() {
        // Mirrors `collapse::placement_is_mutable_topology`. Rooms, ramps and
        // shafts are structural and survive relayout; plain circulation does not.
        for archetype in [
            HexArchetype::Room,
            HexArchetype::RampUp,
            HexArchetype::RampHead,
            HexArchetype::Shaft,
        ] {
            assert_eq!(
                Stability::of(archetype, false, false),
                Stability::Permanent,
                "{archetype:?} is structural"
            );
        }
        for archetype in [
            HexArchetype::Straight,
            HexArchetype::Corner,
            HexArchetype::Junction,
        ] {
            assert_eq!(Stability::of(archetype, false, false), Stability::Mutable);
            assert_eq!(Stability::of(archetype, true, false), Stability::Held);
            assert_eq!(Stability::of(archetype, false, true), Stability::Held);
        }
    }

    #[test]
    fn every_classification_axis_uses_a_distinct_cache_key() {
        let states = [CellState::Traversed, CellState::Glimpsed, CellState::Stale]
            .map(CellState::key)
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(states.len(), 3);
        let markers = [
            MarkerRole::You,
            MarkerRole::Exit,
            MarkerRole::Control,
            MarkerRole::NextRoom,
        ]
        .map(marker_key)
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(markers.len(), 4, "signal roles must not share a material");
    }
}
