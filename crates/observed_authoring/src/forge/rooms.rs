//! Whole-room modules: eleven multi-cell blueprints.
//!
//! Each matches one `RoomBlueprint`, and the contract is enforced from two
//! sides - `room_contract_matches` in `observed_match` and `validate_ports`
//! here. The footprint must equal the blueprint's cells exactly, faces shared
//! by footprint cells are open continuous spans, only named exterior faces are
//! doors, and a port on an internal face is a hard error.
//!
//! `rotation_policy` is `none` because stamped blueprints are never rotated: a
//! sixfold room would compile five unreachable variants.

use super::entities::{
    Meta, Socket, ceiling_fixture, lateral_port, tile_cell, tile_light, tile_socket, worldspawn,
};
use super::geometry::{
    FACE_DELTA, FACE_NAMES, FLOOR_TOP, LEVEL, cell_origin, door_wall_default, hex_slab, translate,
    wall,
};
use super::{Builder, GENERATED_NOTE};

/// One footprint cell: axial `(q, r)` and a level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub q: i32,
    pub r: i32,
    pub level: i32,
}

impl Cell {
    #[must_use]
    pub const fn ground(q: i32, r: i32) -> Self {
        Self { q, r, level: 0 }
    }

    #[must_use]
    pub const fn at(q: i32, r: i32, level: i32) -> Self {
        Self { q, r, level }
    }
}

/// A named exterior threshold: which cell, which face, and the blueprint's own
/// name for it.
#[derive(Clone, Copy, Debug)]
pub struct NamedPort {
    pub cell: Cell,
    pub face: usize,
    pub name: &'static str,
}

/// Per-cell floor and ceiling slabs, no geometry on faces shared with a
/// sibling, framed door walls at named exterior thresholds, and solid walls
/// everywhere else on the perimeter.
///
/// Slabs are deliberately unchamfered: a rim bevel on a shared edge would cut a
/// visible groove across the middle of the room's floor.
///
/// Vertically stacked cells keep their slabs. The blueprint's internal Up/Down
/// ports may be `ShaftOpen`, but the validator forbids authoring a port on an
/// internal face and the projector never checks them, so each level is authored
/// as its own clear, walkable storey.
#[must_use]
fn room_shell(cells: &[Cell], named: &[NamedPort]) -> String {
    let mut brushes = String::new();
    for cell in cells {
        let Cell { q, r, level } = *cell;
        let mut text = format!("// --- cell q{q} r{r} L{level}: floor + ceiling\n");
        text.push_str(&hex_slab(0.0, FLOOR_TOP, 0.0, 0.0));
        text.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 0.0));
        for (face, face_name) in FACE_NAMES.iter().enumerate() {
            let (dq, dr) = FACE_DELTA[face];
            let neighbour = Cell::at(q + dq, r + dr, level);
            if cells.contains(&neighbour) {
                text.push_str(&format!("// open seam to sibling cell: {face_name}\n"));
                continue;
            }
            match named
                .iter()
                .find(|port| port.cell == *cell && port.face == face)
            {
                Some(port) => {
                    text.push_str(&format!("// Named threshold: {}\n", port.name));
                    text.push_str(&door_wall_default(face, 0.0, LEVEL));
                }
                None => {
                    text.push_str(&format!("// Solid perimeter wall: {face_name}\n"));
                    text.push_str(&wall(face, 0.0, LEVEL));
                }
            }
        }
        let (ox, oy) = cell_origin(q, r);
        brushes.push_str(&translate(&text, ox, oy, f64::from(level) * LEVEL));
    }
    brushes
}

/// One recessed ceiling practical per footprint cell, so every cell of the room
/// is lit under the Legibility Contract - not just the anchor.
#[must_use]
fn room_fixtures(cells: &[Cell]) -> (String, String) {
    let mut brushes = String::new();
    let mut lights = String::new();
    for cell in cells {
        let (ox, oy) = cell_origin(cell.q, cell.r);
        let (fixture, _) = ceiling_fixture(0.0, 0.0, LEVEL, 18.0, 10.0);
        brushes.push_str(&translate(&fixture, ox, oy, f64::from(cell.level) * LEVEL));
        lights.push_str(&tile_light(
            ox,
            oy,
            f64::from(cell.level) * LEVEL + LEVEL - 12.0,
        ));
    }
    (brushes, lights)
}

/// A whole-room module matching one `RoomBlueprint`.
#[must_use]
fn room_module(
    name: &str,
    room_role: &str,
    cells: &[Cell],
    named: &[NamedPort],
    sockets: &[Socket<'_>],
) -> String {
    let levels = cells.iter().map(|cell| cell.level).max().unwrap_or(0) + 1;
    let mut brushes = room_shell(cells, named);
    let (fixtures, lights) = room_fixtures(cells);
    brushes.push_str(&fixtures);

    let mut out = format!(
        "// Whole-room module: {room_role} \u{2014} {} hexes, one continuous space.\n",
        cells.len()
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    out.push_str(
        &Meta::room(
            &format!("authored/{name}"),
            room_role,
            "sanctuary",
            levels as u8,
            10,
        )
        .with_register_scope("all")
        .with_rotation_policy("none")
        .emit(),
    );
    for cell in cells {
        out.push_str(&tile_cell(cell.q, cell.r, cell.level, 1, "solid"));
    }
    for port in named {
        // The two invariants the importer would otherwise catch at build time,
        // checked here where the room is declared and the message can name the
        // port rather than a coordinate.
        debug_assert!(
            cells.contains(&port.cell),
            "{} is not on a room cell",
            port.name
        );
        let (dq, dr) = FACE_DELTA[port.face];
        debug_assert!(
            !cells.contains(&Cell::at(
                port.cell.q + dq,
                port.cell.r + dr,
                port.cell.level
            )),
            "{} points into the room footprint",
            port.name
        );
        out.push_str(&lateral_port(
            port.face,
            "door",
            port.name,
            port.cell.level,
            port.cell.q,
            port.cell.r,
        ));
    }
    for socket in sockets {
        out.push_str(&tile_socket(socket));
    }
    out.push_str(&lights);
    out
}

/// DualStation: cells (0,0)+(1,0), joined across the east/west seam.
#[must_use]
pub fn room_dual_station() -> String {
    room_module(
        "room_dual_station",
        "DualStation",
        &[Cell::ground(0, 0), Cell::ground(1, 0)],
        &[
            NamedPort {
                cell: Cell::ground(0, 0),
                face: 3,
                name: "port_a",
            },
            NamedPort {
                cell: Cell::ground(1, 0),
                face: 0,
                name: "port_b",
            },
        ],
        &[
            Socket::new("station_a", "station_a")
                .offset(24.0, 0.0)
                .facing(90.0),
            Socket::new("station_b", "station_b")
                .at(1, 0)
                .offset(-24.0, 0.0)
                .facing(-90.0),
        ],
    )
}

/// Decision: cells (0,0)+(1,0)+(0,1) - a triangular three-hex chamber.
#[must_use]
pub fn room_decision() -> String {
    room_module(
        "room_decision",
        "Decision",
        &[Cell::ground(0, 0), Cell::ground(1, 0), Cell::ground(0, 1)],
        &[
            NamedPort {
                cell: Cell::ground(0, 0),
                face: 3,
                name: "port_a",
            },
            NamedPort {
                cell: Cell::ground(1, 0),
                face: 0,
                name: "port_b",
            },
            NamedPort {
                cell: Cell::ground(0, 1),
                face: 1,
                name: "port_c",
            },
        ],
        &[],
    )
}

/// AnchorCheckpoint: cells (0,0)+(0,1), joined across the SE/NW seam.
#[must_use]
pub fn room_anchor_checkpoint() -> String {
    room_module(
        "room_anchor_checkpoint",
        "AnchorCheckpoint",
        &[Cell::ground(0, 0), Cell::ground(0, 1)],
        &[
            NamedPort {
                cell: Cell::ground(0, 0),
                face: 3,
                name: "port_a",
            },
            NamedPort {
                cell: Cell::ground(0, 1),
                face: 0,
                name: "port_b",
            },
        ],
        &[Socket::new("lantern_cache", "lantern_cache").at(0, 1)],
    )
}

/// DecoherenceFork: cells (0,0)+(1,0)+(0,1)+(1,1) - a four-hex rhombus.
#[must_use]
pub fn room_decoherence_fork() -> String {
    room_module(
        "room_decoherence_fork",
        "DecoherenceFork",
        &[
            Cell::ground(0, 0),
            Cell::ground(1, 0),
            Cell::ground(0, 1),
            Cell::ground(1, 1),
        ],
        &[
            NamedPort {
                cell: Cell::ground(0, 0),
                face: 3,
                name: "port_a",
            },
            NamedPort {
                cell: Cell::ground(1, 0),
                face: 0,
                name: "port_b",
            },
            NamedPort {
                cell: Cell::ground(0, 1),
                face: 3,
                name: "port_c",
            },
            NamedPort {
                cell: Cell::ground(1, 1),
                face: 0,
                name: "port_d",
            },
        ],
        &[],
    )
}

/// GuardianControl: a two-**storey** control room.
///
/// The blueprint's internal Up/Down ports are `ShaftOpen`, but a port on an
/// internal face is a hard validator error and the projector never checks
/// internal faces, so each storey is authored as its own clear, walkable room
/// with one named lateral threshold. That is what fixed the observed bot trap:
/// the per-cell `sanctuary` fallback filled these cells with interior furniture
/// a bot could neither step over (1.3 m against a 0.45 m step) nor route around.
#[must_use]
pub fn room_guardian_control() -> String {
    room_module(
        "room_guardian_control",
        "GuardianControl",
        &[Cell::at(0, 0, 0), Cell::at(0, 0, 1)],
        &[
            NamedPort {
                cell: Cell::at(0, 0, 0),
                face: 3,
                name: "lower_port",
            },
            NamedPort {
                cell: Cell::at(0, 0, 1),
                face: 0,
                name: "upper_port",
            },
        ],
        &[Socket::new("guardian_control", "guardian_control")],
    )
}

#[must_use]
pub fn room_start() -> String {
    room_module(
        "room_start",
        "Start",
        &[Cell::ground(0, 0)],
        &[
            NamedPort {
                cell: Cell::ground(0, 0),
                face: 3,
                name: "entrance",
            },
            NamedPort {
                cell: Cell::ground(0, 0),
                face: 0,
                name: "exit",
            },
        ],
        &[],
    )
}

/// Lateral face indices, matching `observed_hex::HexFace`. Named because a bare
/// `3` in a port table is the kind of thing that drifts from the blueprint side
/// silently, and `room_contract_matches` is the only thing that would notice.
const WEST: usize = 3;
const EAST: usize = 0;

/// A one-hex room with its doors on the faces given.
///
/// Takes a list rather than a single port because a room with one door is a
/// leaf in the facility graph, and a facility of leaves has nothing to route
/// between. Which faces a role opens is a content decision; that it *can* open
/// more than one is this function's job.
#[must_use]
fn simple_room(
    name: &str,
    role: &str,
    ports: &[(&'static str, usize)],
    socket: Option<Socket<'_>>,
) -> String {
    let sockets: Vec<Socket<'_>> = socket.into_iter().collect();
    let named: Vec<NamedPort> = ports
        .iter()
        .map(|&(name, face)| NamedPort {
            cell: Cell::ground(0, 0),
            face,
            name,
        })
        .collect();
    room_module(name, role, &[Cell::ground(0, 0)], &named, &sockets)
}

#[must_use]
pub fn room_teleport_relay() -> String {
    simple_room(
        "room_teleport_relay",
        "TeleportRelay",
        &[("port_a", WEST)],
        None,
    )
}

#[must_use]
pub fn room_keystone() -> String {
    simple_room(
        "room_keystone",
        "Keystone",
        &[("port_a", WEST), ("port_b", EAST)],
        Some(Socket::new("keystone", "keystone")),
    )
}

#[must_use]
pub fn room_monitor() -> String {
    simple_room(
        "room_monitor",
        "Monitor",
        &[("port_a", WEST), ("port_b", EAST)],
        Some(Socket::new("monitor", "monitor")),
    )
}

#[must_use]
pub fn room_recovery() -> String {
    simple_room(
        "room_recovery",
        "Recovery",
        &[("port_a", WEST), ("port_b", EAST)],
        Some(Socket::new("recovery", "recovery")),
    )
}

#[must_use]
pub fn room_exit() -> String {
    simple_room(
        "room_exit",
        "Exit",
        &[("entrance", WEST)],
        Some(Socket::new("exit", "exit")),
    )
}

/// Every room builder, paired with the file it must reproduce.
#[must_use]
pub fn builders() -> Vec<Builder> {
    vec![
        ("room_start", room_start as fn() -> String),
        ("room_teleport_relay", room_teleport_relay),
        ("room_dual_station", room_dual_station),
        ("room_decision", room_decision),
        ("room_anchor_checkpoint", room_anchor_checkpoint),
        ("room_decoherence_fork", room_decoherence_fork),
        ("room_guardian_control", room_guardian_control),
        ("room_keystone", room_keystone),
        ("room_monitor", room_monitor),
        ("room_recovery", room_recovery),
        ("room_exit", room_exit),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_room_reproduces_its_committed_file() {
        super::super::assert_reproduces(&builders());
    }

    /// Named ports must sit on the perimeter. A port pointing into the
    /// footprint is `PortOnInternalFace`, which fails the whole build.
    #[test]
    fn no_named_port_points_into_its_own_room() {
        // Driving every builder runs the debug assertions inside `room_module`.
        for (_, build) in builders() {
            let text = build();
            assert!(text.contains("tile_port"), "a room with no threshold");
        }
    }

    /// Multi-cell rooms must leave shared faces open, or the two halves are
    /// walled off from each other inside one "continuous space".
    #[test]
    fn shared_faces_are_open_seams() {
        let text = room_dual_station();
        assert!(
            text.contains("open seam to sibling cell"),
            "the two cells are not joined"
        );
        // (0,0) opens east to (1,0); (1,0) opens west back.
        assert_eq!(
            text.matches("open seam to sibling cell").count(),
            2,
            "a seam must be open from both sides"
        );
    }
}
