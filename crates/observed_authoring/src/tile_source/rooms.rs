//! Room blueprint footprint tiles: the single-hex room and the multi-hex
//! strip / triangle / diamond cells. Sibling faces are completely open,
//! named exterior faces are framed thresholds, and every other perimeter face
//! is solid. Shapes and port contracts follow the
//! Phase 90 blueprint alignment note (`docs/arc_l/phase_90_91_alignment.md`).

use observed_hex::HexFace;

use super::geometry::{
    DOOR_TOP, FLOOR_TOP, band_brush, door_wall, hex_slab_brush, level_units, pillar_brush,
    pylon_brush, tb_edge, tile_light, tile_meta, tile_port, wall_brush, worldspawn,
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

fn lateral_shell(
    register: &str,
    h: f64,
    internal_faces: &[HexFace],
    threshold_faces: &[HexFace],
) -> String {
    let style = register_style(register);
    let mut brushes = String::new();
    for face in HexFace::LATERAL {
        if internal_faces.contains(&face) {
            continue;
        }
        if threshold_faces.contains(&face) {
            brushes += &door_wall(face, 0.0, h, FLOOR_TOP, DOOR_TOP, style.trim_height);
        } else {
            brushes += &wall_brush(face, 0.0, h);
        }
    }
    brushes
}

/// Single-hex room with only its role's named exterior thresholds.
pub fn room_single_map(register: &str, variant: u16, threshold_faces: &[HexFace]) -> String {
    let h = level_units();
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    brushes += &corner_pillars(h);
    brushes += &lateral_shell(register, h, &[], threshold_faces);
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let names = threshold_faces
        .iter()
        .map(|&face| face_name(face))
        .collect::<Vec<_>>();
    let mut out = format!("// Room single: named thresholds {}.\n", names.join(", "));
    out += &worldspawn(&brushes);
    out += &tile_meta("room_single", register, variant, 1);
    for &face in threshold_faces {
        out += &tile_port(face_name(face), "door");
    }
    for [x, y] in [[-48.0, -24.0], [48.0, -24.0], [0.0, 48.0]] {
        out += &tile_light(x, y, h - 32.0);
    }
    out
}

/// One cell of a multi-hex room. `internal_faces` are completely open to
/// sibling cells, `threshold_faces` carry the room's framed exterior doors,
/// and all remaining faces are solid walls.
pub fn room_wing_map(
    register: &str,
    archetype: &str,
    internal_faces: &[HexFace],
    threshold_faces: &[HexFace],
) -> String {
    let h = level_units();
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    brushes += &lateral_shell(register, h, internal_faces, threshold_faces);
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let internal_names = internal_faces
        .iter()
        .map(|&face| face_name(face))
        .collect::<Vec<_>>();
    let threshold_names = threshold_faces
        .iter()
        .map(|&face| face_name(face))
        .collect::<Vec<_>>();
    let mut out = format!(
        "// {archetype}: open to siblings {}; thresholds {}.\n",
        internal_names.join(", "),
        threshold_names.join(", ")
    );
    out += &worldspawn(&brushes);
    out += &tile_meta(archetype, register, 0, 1);
    for &face in internal_faces.iter().chain(threshold_faces) {
        out += &tile_port(face_name(face), "door");
    }
    out += &tile_light(0.0, 0.0, h - 32.0);
    out
}

/// A 2-hex strip cell (`room_double_{position}`), open toward its sibling.
pub fn room_double_map(register: &str, position: &str, open_face: HexFace) -> String {
    room_wing_map(
        register,
        &format!("room_double_{position}"),
        &[open_face],
        &[open_face.opposite()],
    )
}

/// Guardian-control atrium lower cell: a grounded dais and an open ceiling
/// below the upper wall-supported gallery.
pub fn room_atrium_lower_map(register: &str) -> String {
    let h = level_units();
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    brushes += &pylon_brush(40.0, FLOOR_TOP, FLOOR_TOP + 12.0);
    brushes += &lateral_shell(register, h, &[], &[HexFace::West]);
    let mut out = String::from("// Guardian atrium lower: grounded dais, open ceiling.\n");
    out += &worldspawn(&brushes);
    out += &tile_meta("room_atrium_lower", register, 0, 1);
    out += &tile_port("up", "shaft_open");
    out += &tile_port(face_name(HexFace::West), "door");
    out += &tile_light(-56.0, 0.0, h - 32.0);
    out += &tile_light(56.0, 0.0, h - 32.0);
    out
}

/// Guardian-control atrium upper cell: a gallery ring continuously supported
/// by the perimeter walls, with an open center over the lower room.
pub fn room_atrium_upper_map(register: &str) -> String {
    let h = level_units();
    let mut brushes = String::new();
    for face in HexFace::LATERAL {
        brushes += &band_brush(face, 0.0, 48.0, 0.0, FLOOR_TOP);
        brushes += &band_brush(face, 40.0, 48.0, FLOOR_TOP, FLOOR_TOP + 20.0);
    }
    brushes += &lateral_shell(register, h, &[], &[HexFace::East]);
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let mut out = String::from("// Guardian atrium upper: wall-supported gallery ring.\n");
    out += &worldspawn(&brushes);
    out += &tile_meta("room_atrium_upper", register, 0, 1);
    out += &tile_port("down", "shaft_open");
    out += &tile_port(face_name(HexFace::East), "door");
    for [x, y] in [[-64.0, 0.0], [32.0, -56.0], [32.0, 56.0]] {
        out += &tile_light(x, y, h - 32.0);
    }
    out
}
