//! Vertical circulation: two-level ramp prefabs (all six exit directions) and
//! the shaft family (segment, top/bottom caps, and door landings).

use observed_hex::HexFace;

use super::geometry::{
    DOOR_TOP, FLOOR_TOP, box_brush, door_wall, general_prism_brush, hex_slab_brush, level_units,
    offset_inward, sloped_slab_brush, tb_edge, tile_meta, tile_port, wall_brush, worldspawn,
};
use super::{face_name, register_style};

/// Which vertical openings a lateral shaft landing keeps. The solver can
/// enter/leave a shaft at a top cap, a bottom cap, or a through segment, so
/// authored landings cover every one of those exact port signatures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShaftVertical {
    UpOnly,
    DownOnly,
    Through,
}

impl ShaftVertical {
    fn label(self) -> &'static str {
        match self {
            Self::UpOnly => "up",
            Self::DownOnly => "down",
            Self::Through => "through",
        }
    }
}

/// Two-level ramp rising from the door on `exit_face.opposite()` to the
/// upper-level doorway on `exit_face`. Explicit prefab per direction — the
/// frozen schema has no tile rotation. Variant = exit face index.
pub fn ramp_map(register: &str, exit_face: HexFace) -> String {
    let style = register_style(register);
    let h = level_units();
    let top = 2.0 * h;
    let entrance_face = exit_face.opposite();
    let mut brushes = sloped_slab_brush(entrance_face, exit_face, FLOOR_TOP, h);
    for face in HexFace::LATERAL {
        if face == entrance_face {
            brushes += &door_wall(face, 0.0, top, FLOOR_TOP, DOOR_TOP, style.trim_height);
        } else if face == exit_face {
            brushes += &door_wall(
                face,
                0.0,
                top,
                FLOOR_TOP + h,
                DOOR_TOP + h,
                style.trim_height,
            );
        } else {
            brushes += &wall_brush(face, 0.0, top);
        }
    }
    brushes += &hex_slab_brush(top - FLOOR_TOP, top);
    let mut out = format!("// Ramp exit {}.\n", face_name(exit_face));
    out += &worldspawn(&brushes);
    out += &tile_meta("ramp", register, exit_face.index() as u16, 2);
    out += &tile_port(face_name(entrance_face), "door");
    out += &tile_port("up", "ramp_open");
    out
}

/// The climbing ledges shared by the whole shaft family: three staggered
/// jump-up steps per level.
fn shaft_ledges() -> String {
    let mut out = String::new();
    out += &box_brush([-56.0, -32.0, 24.0], [-8.0, 32.0, 32.0]);
    out += &box_brush([8.0, -32.0, 64.0], [56.0, 32.0, 72.0]);
    out += &box_brush([-56.0, -32.0, 104.0], [-8.0, 32.0, 112.0]);
    out
}

/// Open shaft segment: sealed walls, open floor and ceiling. Variant 0.
pub fn shaft_segment_map(register: &str) -> String {
    let h = level_units();
    let mut brushes = shaft_ledges();
    for face in HexFace::LATERAL {
        brushes += &wall_brush(face, 0.0, h);
    }
    let mut out = String::from("// Shaft segment.\n");
    out += &worldspawn(&brushes);
    out += &tile_meta("shaft", register, 0, 1);
    out += &tile_port("up", "shaft_open");
    out += &tile_port("down", "shaft_open");
    out
}

/// Shaft top cap: sealed ceiling, open floor. Variant 0.
pub fn shaft_top_cap_map(register: &str) -> String {
    let h = level_units();
    let mut brushes = shaft_ledges();
    for face in HexFace::LATERAL {
        brushes += &wall_brush(face, 0.0, h);
    }
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let mut out = String::from("// Shaft top cap.\n");
    out += &worldspawn(&brushes);
    out += &tile_meta("shaft_top", register, 0, 1);
    out += &tile_port("down", "shaft_open");
    out
}

/// Shaft bottom cap: sealed floor, open ceiling. Variant 0.
pub fn shaft_bottom_cap_map(register: &str) -> String {
    let h = level_units();
    let mut brushes = shaft_ledges();
    for face in HexFace::LATERAL {
        brushes += &wall_brush(face, 0.0, h);
    }
    brushes += &hex_slab_brush(0.0, FLOOR_TOP);
    let mut out = String::from("// Shaft bottom cap.\n");
    out += &worldspawn(&brushes);
    out += &tile_meta("shaft_bottom", register, 0, 1);
    out += &tile_port("up", "shaft_open");
    out
}

/// Shaft landing: an open segment with a lateral door and a bridge floor
/// reaching it. Variant = door face index.
pub fn shaft_landing_map(register: &str, door_face: HexFace) -> String {
    shaft_access_map(
        register,
        &[door_face],
        ShaftVertical::Through,
        door_face.index() as u16,
    )
}

/// A shaft access tile with one or two lateral doors and an exact vertical
/// opening class. Existing through/one-door variants call this helper too, so
/// their committed source remains byte-identical while the solver's complete
/// shaft alphabet uses the same geometry construction.
pub(crate) fn shaft_access_map(
    register: &str,
    door_faces: &[HexFace],
    vertical: ShaftVertical,
    variant: u16,
) -> String {
    debug_assert!((1..=2).contains(&door_faces.len()));
    debug_assert!(door_faces.iter().all(|face| face.is_lateral()));
    let style = register_style(register);
    let h = level_units();
    let mut brushes = shaft_ledges();
    if vertical != ShaftVertical::UpOnly {
        for &door_face in door_faces {
            let (a, b) = tb_edge(door_face);
            let (ia_48, ib_48) = offset_inward(a, b, 48.0);
            let inward = [ia_48[0] - a[0], ia_48[1] - a[1]];
            let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
            let bridge_hint = [mid[0] + inward[0] * 0.5, mid[1] + inward[1] * 0.5];
            brushes += &general_prism_brush(&[a, b, ib_48, ia_48], 0.0, FLOOR_TOP, bridge_hint);
        }
    }
    for face in HexFace::LATERAL {
        if door_faces.contains(&face) {
            brushes += &door_wall(face, 0.0, h, FLOOR_TOP, DOOR_TOP, style.trim_height);
        } else {
            brushes += &wall_brush(face, 0.0, h);
        }
    }
    match vertical {
        ShaftVertical::UpOnly => brushes += &hex_slab_brush(0.0, FLOOR_TOP),
        ShaftVertical::DownOnly => brushes += &hex_slab_brush(h - FLOOR_TOP, h),
        ShaftVertical::Through => {}
    }
    let door_names = door_faces
        .iter()
        .map(|&face| face_name(face))
        .collect::<Vec<_>>();
    let mut out = if vertical == ShaftVertical::Through && door_faces.len() == 1 {
        format!("// Shaft landing door {}.\n", door_names[0])
    } else {
        format!(
            "// Shaft landing {} doors {}.\n",
            vertical.label(),
            door_names.join(", ")
        )
    };
    out += &worldspawn(&brushes);
    out += &tile_meta("shaft_landing", register, variant, 1);
    if matches!(vertical, ShaftVertical::UpOnly | ShaftVertical::Through) {
        out += &tile_port("up", "shaft_open");
    }
    if matches!(vertical, ShaftVertical::DownOnly | ShaftVertical::Through) {
        out += &tile_port("down", "shaft_open");
    }
    for door in door_names {
        out += &tile_port(door, "door");
    }
    out
}
