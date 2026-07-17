//! Vertical circulation: two-level ramp prefabs (all six exit directions) and
//! the shaft family (segment, top/bottom caps, and door landings).

use observed_hex::HexFace;

use super::geometry::{
    DOOR_TOP, FLOOR_TOP, box_brush, door_wall, general_prism_brush, hex_slab_brush, level_units,
    offset_inward, sloped_slab_brush, tb_edge, tile_meta, tile_port, wall_brush, worldspawn,
};
use super::{face_name, register_style};

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
    let style = register_style(register);
    let h = level_units();
    let mut brushes = shaft_ledges();
    let (a, b) = tb_edge(door_face);
    let (ia_48, ib_48) = offset_inward(a, b, 48.0);
    let inward = [ia_48[0] - a[0], ia_48[1] - a[1]];
    let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    let bridge_hint = [mid[0] + inward[0] * 0.5, mid[1] + inward[1] * 0.5];
    brushes += &general_prism_brush(&[a, b, ib_48, ia_48], 0.0, FLOOR_TOP, bridge_hint);
    for face in HexFace::LATERAL {
        if face == door_face {
            brushes += &door_wall(face, 0.0, h, FLOOR_TOP, DOOR_TOP, style.trim_height);
        } else {
            brushes += &wall_brush(face, 0.0, h);
        }
    }
    let mut out = format!("// Shaft landing door {}.\n", face_name(door_face));
    out += &worldspawn(&brushes);
    out += &tile_meta("shaft_landing", register, door_face.index() as u16, 1);
    out += &tile_port("up", "shaft_open");
    out += &tile_port("down", "shaft_open");
    out += &tile_port(face_name(door_face), "door");
    out
}
