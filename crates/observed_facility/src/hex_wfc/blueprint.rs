//! Multi-hex room blueprints: fixed footprints with named exterior ports.
//!
//! A blueprint is the solver-side contract for an authored room shape
//! (Phase 91 authors the geometry against these exact signatures). Interior
//! faces between footprint cells are sealed; the exterior offers `Door` only
//! at the named ports, so halls attach to rooms exclusively through ports —
//! the hex analogue of `full_wfc`'s threshold identity.

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
    /// The exterior port signature of one footprint cell. Every lateral face
    /// that does **not** border another footprint cell is a `Door` (rooms
    /// present their whole exterior to the surrounding halls, so multi-hex
    /// rooms have several entrances); interior faces between footprint cells
    /// are `Sealed`; declared vertical ports carry their class. The
    /// `named_ports` are a labeled subset of these exterior doors, used for
    /// stable threshold identity — not a restriction of the openings.
    #[must_use]
    pub fn cell_signature(&self, offset: CellOffset) -> PortSignature {
        let mut ports = [PortClass::Sealed; 8];
        for face in HexFace::LATERAL {
            let delta = face.delta();
            let neighbor = (offset.0 + delta.0, offset.1 + delta.1, offset.2 + delta.2);
            if !self.cells.contains(&neighbor) {
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
#[must_use]
pub fn blueprint_cell_archetype(role: RoomRole, cell_index: usize) -> Option<&'static str> {
    let archetypes: &[&str] = match role {
        RoomRole::Start
        | RoomRole::Exit
        | RoomRole::TeleportRelay
        | RoomRole::Keystone
        | RoomRole::Monitor
        | RoomRole::Recovery => &["room_single"],
        RoomRole::Decision => &["room_tri_a", "room_tri_b", "room_tri_c"],
        RoomRole::DecoherenceFork => &["room_fork_a", "room_fork_b", "room_fork_c", "room_fork_d"],
        RoomRole::AnchorCheckpoint => &["room_double_nw", "room_double_se"],
        RoomRole::DualStation => &["room_double_west", "room_double_east"],
        RoomRole::GuardianControl => &["room_atrium_lower", "room_atrium_upper"],
    };
    archetypes.get(cell_index).copied()
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
            &[("port_a", (0, 0, 0), HexFace::West)],
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
            &[("port_a", (0, 0, 0), HexFace::West)],
        ),
        RoomRole::Recovery => make(
            role,
            "Recovery",
            &[(0, 0, 0)],
            &[],
            &[("port_a", (0, 0, 0), HexFace::West)],
        ),
    }
}
