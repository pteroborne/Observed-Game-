//! Multi-hex room blueprints: fixed footprints with named exterior ports.
//!
//! A blueprint is the solver-side contract for an authored room shape
//! (Phase 91 authors the geometry against these exact signatures). Interior
//! faces between footprint cells are open; the exterior offers `Door` only at
//! the named ports, so the footprint reads and routes as one room while halls
//! attach exclusively through explicit thresholds — the hex analogue of
//! `full_wfc`'s threshold identity.

use std::collections::BTreeMap;

use observed_hex::{HexCoord, HexFace, PortClass, PortSignature};

use crate::map_spec::RoomRole;

/// Relative footprint cell: `(dq, dr, dlevel)` from the anchor.
pub type CellOffset = (i32, i32, i32);

/// A named exterior port: `(name, cell_offset, face)`.
pub type NamedPort = (&'static str, CellOffset, HexFace);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomBlueprint {
    pub role: RoomRole,
    pub name: &'static str,
    /// Footprint cells relative to the anchor; `(0, 0, 0)` is the anchor.
    pub cells: Vec<CellOffset>,
    /// Vertical port classes on interior faces (e.g. an atrium's internal
    /// shaft). Keyed `(cell_offset, face)`; faces absent here are sealed.
    pub vertical_ports: BTreeMap<(CellOffset, HexFace), PortClass>,
    /// Exterior door ports, the only lateral openings the room offers.
    pub named_ports: Vec<NamedPort>,
}

impl RoomBlueprint {
    /// The port signature of one footprint cell. Lateral faces shared with a
    /// sibling cell are open so the footprint is one continuous room. On the
    /// perimeter, only declared [`RoomBlueprint::named_ports`] are `Door`;
    /// every other face is a solid room wall. Declared vertical ports carry
    /// their class.
    #[must_use]
    pub fn cell_signature(&self, offset: CellOffset) -> PortSignature {
        let mut ports = [PortClass::Sealed; 8];
        for face in HexFace::LATERAL {
            let delta = face.delta();
            let neighbor = (offset.0 + delta.0, offset.1 + delta.1, offset.2 + delta.2);
            if self.cells.contains(&neighbor) {
                ports[face.index()] = PortClass::Door;
            }
        }
        for &(_, port_cell, face) in &self.named_ports {
            if port_cell == offset {
                ports[face.index()] = PortClass::Door;
            }
        }
        for (&(port_cell, face), &class) in &self.vertical_ports {
            if port_cell == offset {
                ports[face.index()] = class;
            }
        }
        PortSignature::try_from_ports(ports).expect("blueprint signatures use valid face classes")
    }
}

/// One blueprint stamped into a concrete grid position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StampedBlueprint {
    /// Stamp order within the attempt (0-based); stable per `(seed, generation)`.
    pub id: u32,
    pub role: RoomRole,
    pub anchor: HexCoord,
    /// Absolute footprint cells, in blueprint cell order.
    pub cells: Vec<HexCoord>,
}

/// Authored tile archetype for one cell of a room blueprint.
///
/// The mapping is indexed in the same order as [`RoomBlueprint::cells`]. It is
/// the shared coordination seam between the solver's multi-cell room stamp and
/// the manifest projector: callers must not infer a room wing from its door
/// mask because boundary-adjusted stamps deliberately seal out-of-grid faces.
///
/// This used to discard both arguments and answer `"sanctuary"` for everything —
/// bug backlog #15. Every room in the facility, of every role, was the same
/// single-hex shape repeated across its footprint, which is why a three-hex
/// Decision room read as three rooms rather than one. The wing geometry it now
/// asks for is not new: `room_double_*`, `room_tri_*`, `room_fork_*` and the
/// atrium pair have been generated in every register since Arc L. Nothing ever
/// asked for them, and `compatibility_archetype` flattened them back to
/// `sanctuary` on the way in, so they could not have been reached even by
/// accident.
#[must_use]
pub fn blueprint_cell_archetype(role: RoomRole, cell_index: usize) -> Option<&'static str> {
    Some(match (role, cell_index) {
        // Rooms that occupy a single hex. Their one cell is walled on every
        // face the stamp does not open, so one shape serves all of them.
        (
            RoomRole::Start
            | RoomRole::Exit
            | RoomRole::TeleportRelay
            | RoomRole::Keystone
            | RoomRole::Monitor
            | RoomRole::Recovery,
            0,
        ) => "room_single",

        // Two hexes side by side along q: each opens toward its sibling.
        (RoomRole::DualStation, 0) => "room_double_west",
        (RoomRole::DualStation, 1) => "room_double_east",

        // Two hexes along r, which runs north-west to south-east.
        (RoomRole::AnchorCheckpoint, 0) => "room_double_nw",
        (RoomRole::AnchorCheckpoint, 1) => "room_double_se",

        // Three hexes in an L: two openings each, facing the other two.
        (RoomRole::Decision, 0) => "room_tri_a",
        (RoomRole::Decision, 1) => "room_tri_b",
        (RoomRole::Decision, 2) => "room_tri_c",

        // Four hexes in a rhombus; the two interior cells open three ways.
        (RoomRole::DecoherenceFork, 0) => "room_fork_a",
        (RoomRole::DecoherenceFork, 1) => "room_fork_b",
        (RoomRole::DecoherenceFork, 2) => "room_fork_c",
        (RoomRole::DecoherenceFork, 3) => "room_fork_d",

        // The one room that stacks: a two-storey atrium open through its floor.
        (RoomRole::GuardianControl, 0) => "room_atrium_lower",
        (RoomRole::GuardianControl, 1) => "room_atrium_upper",

        _ => return None,
    })
}

impl StampedBlueprint {
    /// Stable identity of this room instance across relayouts:
    /// `hash(blueprint_id, anchor_cell)` (mirrors
    /// `full_wfc::catalog::corridor_generation_key` discipline).
    #[must_use]
    pub fn generation_key(&self) -> u64 {
        let role_key = blueprint_for_role(self.role)
            .name
            .bytes()
            .fold(0xCBF2_9CE4_8422_2325u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3)
            });
        let anchor_key = u64::from(self.anchor.q)
            | (u64::from(self.anchor.r) << 16)
            | (u64::from(self.anchor.level) << 32);
        role_key.rotate_left(7) ^ anchor_key.wrapping_mul(0x9E37_79B9_7F4A_7C15)
    }
}

/// Assemble a blueprint from its parts (keeps `blueprint_for_role` readable and
/// avoids a clippy `type_complexity` tuple).
fn make(
    role: RoomRole,
    name: &'static str,
    cells: &[CellOffset],
    vertical_ports: &[((CellOffset, HexFace), PortClass)],
    named_ports: &[NamedPort],
) -> RoomBlueprint {
    RoomBlueprint {
        role,
        name,
        cells: cells.to_vec(),
        vertical_ports: vertical_ports.iter().copied().collect(),
        named_ports: named_ports.to_vec(),
    }
}

/// The authored blueprint for a role. Shapes and named ports are the frozen
/// coordination contract with Phase 91 (see `docs/arc_l/phase_90_91_alignment.md`).
#[must_use]
pub fn blueprint_for_role(role: RoomRole) -> RoomBlueprint {
    match role {
        RoomRole::Start => make(
            role,
            "Start",
            &[(0, 0, 0)],
            &[],
            &[
                ("entrance", (0, 0, 0), HexFace::West),
                ("exit", (0, 0, 0), HexFace::East),
            ],
        ),
        RoomRole::Exit => make(
            role,
            "Exit",
            &[(0, 0, 0)],
            &[],
            &[("entrance", (0, 0, 0), HexFace::West)],
        ),
        RoomRole::Decision => make(
            role,
            "Decision",
            &[(0, 0, 0), (1, 0, 0), (0, 1, 0)],
            &[],
            &[
                ("port_a", (0, 0, 0), HexFace::West),
                ("port_b", (1, 0, 0), HexFace::East),
                ("port_c", (0, 1, 0), HexFace::SouthEast),
            ],
        ),
        RoomRole::DecoherenceFork => make(
            role,
            "DecoherenceFork",
            &[(0, 0, 0), (1, 0, 0), (0, 1, 0), (1, 1, 0)],
            &[],
            &[
                ("port_a", (0, 0, 0), HexFace::West),
                ("port_b", (1, 0, 0), HexFace::East),
                ("port_c", (0, 1, 0), HexFace::West),
                ("port_d", (1, 1, 0), HexFace::East),
            ],
        ),
        RoomRole::AnchorCheckpoint => make(
            role,
            "AnchorCheckpoint",
            &[(0, 0, 0), (0, 1, 0)],
            &[],
            &[
                ("port_a", (0, 0, 0), HexFace::West),
                ("port_b", (0, 1, 0), HexFace::East),
            ],
        ),
        RoomRole::TeleportRelay => make(
            role,
            "TeleportRelay",
            &[(0, 0, 0)],
            &[],
            &[("port_a", (0, 0, 0), HexFace::West)],
        ),
        RoomRole::Keystone => make(
            role,
            "Keystone",
            &[(0, 0, 0)],
            &[],
            &[
                ("port_a", (0, 0, 0), HexFace::West),
                ("port_b", (0, 0, 0), HexFace::East),
            ],
        ),
        RoomRole::DualStation => make(
            role,
            "DualStation",
            &[(0, 0, 0), (1, 0, 0)],
            &[],
            &[
                ("port_a", (0, 0, 0), HexFace::West),
                ("port_b", (1, 0, 0), HexFace::East),
            ],
        ),
        RoomRole::GuardianControl => make(
            role,
            "GuardianControl",
            &[(0, 0, 0), (0, 0, 1)],
            &[
                (((0, 0, 0), HexFace::Up), PortClass::ShaftOpen),
                (((0, 0, 1), HexFace::Down), PortClass::ShaftOpen),
            ],
            &[
                ("lower_port", (0, 0, 0), HexFace::West),
                ("upper_port", (0, 0, 1), HexFace::East),
            ],
        ),
        RoomRole::Monitor => make(
            role,
            "Monitor",
            &[(0, 0, 0)],
            &[],
            &[
                ("port_a", (0, 0, 0), HexFace::West),
                ("port_b", (0, 0, 0), HexFace::East),
            ],
        ),
        RoomRole::Recovery => make(
            role,
            "Recovery",
            &[(0, 0, 0)],
            &[],
            &[
                ("port_a", (0, 0, 0), HexFace::West),
                ("port_b", (0, 0, 0), HexFace::East),
            ],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_footprints_open_inside_and_only_named_thresholds_open_outside() {
        let decision = blueprint_for_role(RoomRole::Decision);

        let anchor = decision.cell_signature((0, 0, 0));
        assert_eq!(anchor.port(HexFace::East), PortClass::Door);
        assert_eq!(anchor.port(HexFace::SouthEast), PortClass::Door);
        assert_eq!(anchor.port(HexFace::West), PortClass::Door);
        assert_eq!(anchor.port(HexFace::NorthWest), PortClass::Sealed);
        assert_eq!(anchor.port(HexFace::NorthEast), PortClass::Sealed);
        assert_eq!(anchor.port(HexFace::SouthWest), PortClass::Sealed);

        for &(name, offset, face) in &decision.named_ports {
            assert_eq!(
                decision.cell_signature(offset).port(face),
                PortClass::Door,
                "named threshold {name} must be open"
            );
            let delta = face.delta();
            assert!(
                !decision.cells.contains(&(
                    offset.0 + delta.0,
                    offset.1 + delta.1,
                    offset.2 + delta.2,
                )),
                "named threshold {name} must leave the footprint"
            );
        }
    }
}
