//! Point-entity emission: a byte-exact port of `tools/tileforge.py`.
//!
//! Property **order** is part of the output, not a detail. These are emitted as
//! ordered key/value lists rather than maps for that reason: a map would reorder
//! them, every committed `.map` would change, and the byte-identity gate would
//! fail for a reason that has nothing to do with geometry.

use super::geometry::{
    FACE_NAMES, LEVEL, WALL, cell_origin, edge, face_mid, fmt, lerp, offset_inward, prism,
};

/// The register scope every originally-authored module carries.
pub const ORIGINAL_REGISTER_SCOPE: &str = "shadow_screen,monolith,overlit_grid,institutional,\
     facet_monument,megastructure,wellshaft,infinite_gallery,thinning";

/// Short face names, used to build port names.
pub const PORT_SHORT: [&str; 6] = ["east", "se", "sw", "west", "nw", "ne"];

/// One point entity from ordered properties.
#[must_use]
pub fn point_entity(props: &[(&str, String)]) -> String {
    let mut out = String::from("{\n");
    for (key, value) in props {
        out.push_str(&format!("\"{key}\" \"{value}\"\n"));
    }
    out.push_str("}\n");
    out
}

#[must_use]
pub fn worldspawn(brushes: &str) -> String {
    format!("{{\n\"classname\" \"worldspawn\"\n{brushes}}}\n")
}

/// What kind of module `tile_meta` declares.
///
/// `Room` carries its role rather than taking it separately, because the
/// importer hard-errors on a room without one (`SourceError::RoomRoleMissing`)
/// and that fails the whole `tilec build`, not just the one module. Making the
/// pair unrepresentable-apart moves that from a runtime error to a compile one -
/// the Python could only raise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetaKind {
    Cell,
    Room { role: String },
}

/// Everything `tile_meta` needs. A struct rather than eleven positional
/// arguments, with `Default` carrying the same defaults the Python has.
#[derive(Clone, Debug)]
pub struct Meta {
    pub id: String,
    pub archetype: String,
    pub variant: i32,
    pub levels: u8,
    pub weight: u32,
    pub register: String,
    pub register_scope: String,
    pub rotation_policy: String,
    pub kind: MetaKind,
}

impl Meta {
    /// A cell module with the Python's defaults.
    #[must_use]
    pub fn cell(id: &str, archetype: &str, variant: i32, levels: u8, weight: u32) -> Self {
        Self {
            id: id.to_string(),
            archetype: archetype.to_string(),
            variant,
            levels,
            weight,
            register: String::from("generic"),
            register_scope: ORIGINAL_REGISTER_SCOPE.to_string(),
            rotation_policy: String::from("sixfold"),
            kind: MetaKind::Cell,
        }
    }

    #[must_use]
    pub fn room(id: &str, role: &str, archetype: &str, levels: u8, weight: u32) -> Self {
        Self {
            kind: MetaKind::Room {
                role: role.to_string(),
            },
            ..Self::cell(id, archetype, 0, levels, weight)
        }
    }

    #[must_use]
    pub fn with_register(mut self, register: &str) -> Self {
        self.register = register.to_string();
        self
    }

    #[must_use]
    pub fn with_register_scope(mut self, scope: &str) -> Self {
        self.register_scope = scope.to_string();
        self
    }

    #[must_use]
    pub fn with_rotation_policy(mut self, policy: &str) -> Self {
        self.rotation_policy = policy.to_string();
        self
    }

    /// Emit. `room_role` is inserted at index 5, exactly where the Python puts
    /// it - between `archetype` and `register`.
    #[must_use]
    pub fn emit(&self) -> String {
        let mut props: Vec<(&str, String)> = vec![
            ("classname", String::from("tile_meta")),
            ("authoring_version", String::from("2")),
            ("id", self.id.clone()),
            (
                "kind",
                match &self.kind {
                    MetaKind::Cell => String::from("cell"),
                    MetaKind::Room { .. } => String::from("room"),
                },
            ),
            ("archetype", self.archetype.clone()),
            ("register", self.register.clone()),
            ("register_scope", self.register_scope.clone()),
            ("variant", self.variant.to_string()),
            ("levels", self.levels.to_string()),
            ("rotation_policy", self.rotation_policy.clone()),
            ("weight", self.weight.to_string()),
        ];
        if let MetaKind::Room { role } = &self.kind {
            props.insert(5, ("room_role", role.clone()));
        }
        point_entity(&props)
    }
}

#[must_use]
pub fn tile_cell(q: i32, r: i32, level: i32, levels: u8, floor: &str) -> String {
    point_entity(&[
        ("classname", String::from("tile_cell")),
        ("q", q.to_string()),
        ("r", r.to_string()),
        ("level", level.to_string()),
        ("levels", levels.to_string()),
        ("floor", floor.to_string()),
    ])
}

/// The common case: a single solid cell at the origin.
#[must_use]
pub fn tile_cell_default() -> String {
    tile_cell(0, 0, 0, 1, "solid")
}

/// An up/down port at the exact cell-centre origin the validator demands.
#[must_use]
pub fn vertical_port(face_name: &str, class: &str, name: &str, level: i32) -> String {
    let z = if face_name == "up" {
        f64::from(level + 1) * LEVEL
    } else {
        f64::from(level) * LEVEL
    };
    point_entity(&[
        ("classname", String::from("tile_port")),
        ("q", String::from("0")),
        ("r", String::from("0")),
        ("level", level.to_string()),
        ("face", face_name.to_string()),
        ("class", class.to_string()),
        ("name", name.to_string()),
        ("origin", format!("0 0 {}", fmt(z))),
    ])
}

/// A lateral port with the exact origin the validator demands.
///
/// For a multi-cell room the origin is the face-edge midpoint of *that cell*,
/// shifted by the cell's plan origin - see `expected_port_origin`.
#[must_use]
pub fn lateral_port(face: usize, class: &str, name: &str, level: i32, q: i32, r: i32) -> String {
    let mid = face_mid(face);
    let (ox, oy) = cell_origin(q, r);
    let origin = format!(
        "{} {} {}",
        fmt(mid.0 + ox),
        fmt(mid.1 + oy),
        fmt(f64::from(level) * LEVEL + 48.0)
    );
    point_entity(&[
        ("classname", String::from("tile_port")),
        ("q", q.to_string()),
        ("r", r.to_string()),
        ("level", level.to_string()),
        ("face", FACE_NAMES[face % 6].to_string()),
        ("class", class.to_string()),
        ("name", name.to_string()),
        ("origin", origin),
    ])
}

/// A semantic practical source. Colour and energy stay presentation-owned.
#[must_use]
pub fn tile_light(x: f64, y: f64, z: f64) -> String {
    point_entity(&[
        ("classname", String::from("tile_light")),
        ("kind", String::from("practical")),
        ("origin", format!("{} {} {}", fmt(x), fmt(y), fmt(z))),
    ])
}

/// A typed gameplay socket inside one room footprint cell.
///
/// A struct rather than eight positional arguments: `(q, r, level, x, y, yaw)`
/// is six numbers in a row, and transposing any two of them produces a socket
/// that imports cleanly in the wrong place.
#[derive(Clone, Debug)]
pub struct Socket<'a> {
    pub id: &'a str,
    pub kind: &'a str,
    pub q: i32,
    pub r: i32,
    pub level: i32,
    pub x: f64,
    pub y: f64,
    pub yaw: f64,
}

impl<'a> Socket<'a> {
    /// A socket at the centre of cell `(0, 0)`, level 0, facing 0 degrees.
    #[must_use]
    pub fn new(id: &'a str, kind: &'a str) -> Self {
        Self {
            id,
            kind,
            q: 0,
            r: 0,
            level: 0,
            x: 0.0,
            y: 0.0,
            yaw: 0.0,
        }
    }

    #[must_use]
    pub fn at(mut self, q: i32, r: i32) -> Self {
        self.q = q;
        self.r = r;
        self
    }

    #[must_use]
    pub fn offset(mut self, x: f64, y: f64) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    #[must_use]
    pub fn facing(mut self, yaw: f64) -> Self {
        self.yaw = yaw;
        self
    }
}

#[must_use]
pub fn tile_socket(socket: &Socket<'_>) -> String {
    let Socket {
        id: socket_id,
        kind,
        q,
        r,
        level,
        x,
        y,
        yaw,
    } = *socket;
    let (ox, oy) = cell_origin(q, r);
    point_entity(&[
        ("classname", String::from("tile_socket")),
        ("id", socket_id.to_string()),
        ("kind", kind.to_string()),
        ("q", q.to_string()),
        ("r", r.to_string()),
        ("level", level.to_string()),
        (
            "origin",
            format!(
                "{} {} {}",
                fmt(ox + x),
                fmt(oy + y),
                fmt(f64::from(level) * LEVEL + 24.0)
            ),
        ),
        ("yaw", fmt(yaw)),
    ])
}

/// One node of the climbable line through a tile, in TrenchBroom units.
///
/// `index` orders the climb bottom to top; the importer sorts by it and rejects
/// a repeat. The first node belongs on the lower deck at the foot of the climb
/// and the last on the deck above, so the ends join the flat circulation either
/// side without a special case.
#[must_use]
pub fn stair_node(index: u16, x: f64, y: f64, z: f64) -> String {
    point_entity(&[
        ("classname", String::from("tile_stair_node")),
        ("index", index.to_string()),
        ("origin", format!("{} {} {}", fmt(x), fmt(y), fmt(z))),
    ])
}

/// A recessed housing physically attached to a ceiling, plus its source.
///
/// Returns `(brush, light)`: the fixture is geometry and the light is an
/// entity, and they go to different parts of the file.
#[must_use]
pub fn ceiling_fixture(x: f64, y: f64, ceiling: f64, half_x: f64, half_y: f64) -> (String, String) {
    let brush = super::geometry::boxed(
        (x - half_x, y - half_y, ceiling - 8.0),
        (x + half_x, y + half_y, ceiling),
    );
    (brush, tile_light(x, y, ceiling - 12.0))
}

/// A shallow sconce attached to a wall, and a source just inside it.
#[must_use]
pub fn wall_fixture(face: usize, along: f64, z: f64, width: f64) -> (String, String) {
    let (a, b) = edge(face);
    let length = (b.0 - a.0).hypot(b.1 - a.1);
    let half_t = width / (2.0 * length);
    let (oa, ob) = offset_inward(a, b, WALL);
    let (ia, ib) = offset_inward(a, b, WALL + 6.0);
    let (p0, p1) = (lerp(oa, ob, along - half_t), lerp(oa, ob, along + half_t));
    let (q0, q1) = (lerp(ia, ib, along - half_t), lerp(ia, ib, along + half_t));
    let brush = prism(&[p0, p1, q1, q0], z - 8.0, z + 8.0, None, 2.0, 0.0);
    let (la, lb) = offset_inward(a, b, WALL + 14.0);
    let source = lerp(la, lb, along);
    (brush, tile_light(source.0, source.1, z))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Property order is output. A map would reorder these and change every
    /// committed file.
    #[test]
    fn meta_emits_its_properties_in_the_committed_order() {
        let text = Meta::cell("hall/cap", "hall_cap", 0, 1, 1).emit();
        let keys: Vec<&str> = text
            .lines()
            .filter_map(|line| line.split('"').nth(1))
            .collect();
        assert_eq!(
            keys,
            [
                "classname",
                "authoring_version",
                "id",
                "kind",
                "archetype",
                "register",
                "register_scope",
                "variant",
                "levels",
                "rotation_policy",
                "weight",
            ]
        );
    }

    /// `room_role` goes between `archetype` and `register`, which is where the
    /// Python inserts it. Anywhere else is a byte difference.
    #[test]
    fn a_room_role_lands_between_archetype_and_register() {
        let text = Meta::room("room/keystone", "keystone", "sanctuary", 1, 10).emit();
        let keys: Vec<&str> = text
            .lines()
            .filter_map(|line| line.split('"').nth(1))
            .collect();
        let role = keys.iter().position(|k| *k == "room_role").expect("role");
        assert_eq!(keys[role - 1], "archetype");
        assert_eq!(keys[role + 1], "register");
    }

    /// A room without a role fails the whole `tilec build`, so the type makes
    /// the pair inseparable rather than leaving it to a runtime check.
    #[test]
    fn a_room_always_carries_its_role() {
        let meta = Meta::room("room/exit", "exit", "sanctuary", 1, 10);
        assert!(matches!(meta.kind, MetaKind::Room { .. }));
        assert!(meta.emit().contains("\"room_role\" \"exit\""));
        assert!(
            !Meta::cell("h/c", "hall_cap", 0, 1, 1)
                .emit()
                .contains("room_role")
        );
    }

    /// Port origins are what the validator checks to the unit, so the east
    /// face-edge midpoint has to be exactly the documented value.
    #[test]
    fn a_lateral_port_lands_on_its_face_edge_midpoint() {
        let text = lateral_port(0, "door", "east_threshold", 0, 0, 0);
        assert!(
            text.contains("\"origin\" \"112 0 48\""),
            "east port origin moved: {text}"
        );
    }

    /// The same port on a neighbouring cell shifts by that cell's plan origin.
    #[test]
    fn a_port_on_another_cell_shifts_by_that_cells_origin() {
        let text = lateral_port(0, "door", "east_threshold", 0, 1, 0);
        assert!(text.contains("\"origin\" \"336 0 48\""), "{text}");
    }

    /// Up and down sit at the cell centre, at their own level's boundary.
    #[test]
    fn vertical_ports_sit_at_the_level_boundary() {
        assert!(vertical_port("up", "shaft_open", "up_shaft", 0).contains("\"0 0 128\""));
        assert!(vertical_port("down", "shaft_open", "down_shaft", 0).contains("\"0 0 0\""));
        assert!(vertical_port("up", "shaft_open", "up_shaft", 1).contains("\"0 0 256\""));
    }
}
