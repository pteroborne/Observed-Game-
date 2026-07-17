//! Room blueprint footprint tiles: the single-hex room, the multi-hex
//! strip / triangle / diamond cells (open internal faces, doors elsewhere),
//! and the two-level atrium pair. Shapes and internal seals follow the
//! Phase 90 blueprint alignment note (`docs/arc_l/phase_90_91_alignment.md`).

use observed_hex::HexFace;

use super::geometry::{
    DOOR_TOP, FLOOR_TOP, band_brush, door_wall, hex_slab_brush, level_units, pillar_brush,
    pylon_brush, tb_edge, tile_meta, tile_port, worldspawn,
};
use super::{face_name, register_style};

/// Corner colonnade for the single room: six pillars on the hex corner
/// directions.
fn corner_pillars(h: f64) -> String {
    let mut out = String::new();
    for face in HexFace::LATERAL {
        let (a, _) = tb_edge(face);
        let len = (a[0] * a[0] + a[1] * a[1]).sqrt();
        let center = [a[0] / len * 72.0, a[1] / len * 72.0];
        out += &pillar_brush(center, 8.0, FLOOR_TOP, h - FLOOR_TOP);
    }
    out
}

/// Threshold pillars flanking an open internal face, marking the transition
/// between the cells of a multi-hex room.
fn threshold_pillars(face: HexFace, h: f64) -> String {
    let (a, b) = tb_edge(face);
    let mut out = String::new();
    for corner in [a, b] {
        let len = (corner[0] * corner[0] + corner[1] * corner[1]).sqrt();
        let center = [corner[0] / len * 92.0, corner[1] / len * 92.0];
        out += &pillar_brush(center, 8.0, FLOOR_TOP, h - FLOOR_TOP);
    }
    out
}

/// Single-hex room: doors on all six faces, corner colonnade. Variant 0.
pub fn room_single_map(register: &str) -> String {
    let style = register_style(register);
    let h = level_units();
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    brushes += &corner_pillars(h);
    for face in HexFace::LATERAL {
        brushes += &door_wall(face, 0.0, h, FLOOR_TOP, DOOR_TOP, style.trim_height);
    }
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let mut out = String::from("// Room single: doors on all six faces.\n");
    out += &worldspawn(&brushes);
    out += &tile_meta("room_single", register, 0, 1);
    for face in HexFace::LATERAL {
        out += &tile_port(face_name(face), "door");
    }
    out
}

/// One cell of a multi-hex room: `open_faces` are fully open to the sibling
/// cells (no wall, no port — sealed in the signature), every other lateral
/// face is a doorway. Used for the 2-hex strips, the 3-hex triangle, and the
/// 4-hex diamond. Variant 0; the archetype names the cell.
pub fn room_wing_map(register: &str, archetype: &str, open_faces: &[HexFace]) -> String {
    let style = register_style(register);
    let h = level_units();
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    for &face in open_faces {
        brushes += &threshold_pillars(face, h);
    }
    for face in HexFace::LATERAL {
        if !open_faces.contains(&face) {
            brushes += &door_wall(face, 0.0, h, FLOOR_TOP, DOOR_TOP, style.trim_height);
        }
    }
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let names: Vec<&str> = open_faces.iter().map(|&face| face_name(face)).collect();
    let mut out = format!("// {archetype}: open to siblings {}.\n", names.join(", "));
    out += &worldspawn(&brushes);
    out += &tile_meta(archetype, register, 0, 1);
    for face in HexFace::LATERAL {
        if !open_faces.contains(&face) {
            out += &tile_port(face_name(face), "door");
        }
    }
    out
}

/// A 2-hex strip cell (`room_double_{position}`), open toward its sibling.
pub fn room_double_map(register: &str, position: &str, open_face: HexFace) -> String {
    room_wing_map(register, &format!("room_double_{position}"), &[open_face])
}

/// Atrium lower cell: doors on all six faces, a central dais, and an open
/// ceiling (`up: shaft_open`) under the upper gallery. Variant 0.
pub fn room_atrium_lower_map(register: &str) -> String {
    let style = register_style(register);
    let h = level_units();
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    brushes += &pylon_brush(40.0, FLOOR_TOP, FLOOR_TOP + 12.0);
    for face in HexFace::LATERAL {
        brushes += &door_wall(face, 0.0, h, FLOOR_TOP, DOOR_TOP, style.trim_height);
    }
    let mut out = String::from("// Room atrium lower: open ceiling, central dais.\n");
    out += &worldspawn(&brushes);
    out += &tile_meta("room_atrium_lower", register, 0, 1);
    out += &tile_port("up", "shaft_open");
    for face in HexFace::LATERAL {
        out += &tile_port(face_name(face), "door");
    }
    out
}

/// Atrium upper cell: a gallery balcony ring with a railing around the open
/// center (`down: shaft_open`), doors on all six faces. Variant 0.
pub fn room_atrium_upper_map(register: &str) -> String {
    let style = register_style(register);
    let h = level_units();
    let mut brushes = String::new();
    for face in HexFace::LATERAL {
        // Balcony ring floor and its inner railing.
        brushes += &band_brush(face, 0.0, 48.0, 0.0, FLOOR_TOP);
        brushes += &band_brush(face, 40.0, 48.0, FLOOR_TOP, FLOOR_TOP + 20.0);
    }
    for face in HexFace::LATERAL {
        brushes += &door_wall(face, 0.0, h, FLOOR_TOP, DOOR_TOP, style.trim_height);
    }
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let mut out = String::from("// Room atrium upper: balcony ring over the open well.\n");
    out += &worldspawn(&brushes);
    out += &tile_meta("room_atrium_upper", register, 0, 1);
    out += &tile_port("down", "shaft_open");
    for face in HexFace::LATERAL {
        out += &tile_port(face_name(face), "door");
    }
    out
}
