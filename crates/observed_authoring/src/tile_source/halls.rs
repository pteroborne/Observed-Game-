//! Hall tiles: straights (three interior readings), dead-end caps, corners,
//! and 3-/4-way junctions — every door face combination, including the
//! mitered diagonal faces.

use observed_hex::HexFace;

use super::geometry::{
    DOOR_TOP, FLOOR_TOP, door_wall, face_angle_deg, general_prism_brush, hex_slab_brush,
    pillar_brush, pylon_brush, rotate_points, tile_light, tile_meta, tile_port, wall_brush,
    worldspawn,
};
use super::{face_name, register_style};

/// Rotated interior prism helper: rotates `corners` (authored in the East-door
/// frame) by the door face angle. Keep authored geometry inside
/// [`super::geometry::SAFE_INTERIOR_RADIUS`].
fn rotated_prism(corners: &[[f64; 2]], hint: [f64; 2], angle: f64, z0: f64, z1: f64) -> String {
    for corner in corners {
        let radius = (corner[0] * corner[0] + corner[1] * corner[1]).sqrt();
        debug_assert!(
            radius <= super::geometry::SAFE_INTERIOR_RADIUS,
            "rotated interior geometry escapes the safe disk: {corner:?}"
        );
    }
    general_prism_brush(
        &rotate_points(corners, angle),
        z0,
        z1,
        rotate_points(&[hint], angle)[0],
    )
}

/// The straight hall's interior readings, authored in the East-door frame and
/// rotated to the hall axis. 0 = plain, 1 = colonnade pillar pairs,
/// 2 = pressure narrowing.
fn straight_interior(interior: u16, face: HexFace, h: f64) -> String {
    let angle = face_angle_deg(face);
    let mut out = String::new();
    if interior == 1 {
        // Two facing pillar rows: the Colonnade reading.
        for side in [-1.0, 1.0] {
            for x in [-48.0, 0.0, 48.0] {
                out += &rotated_prism(
                    &[
                        [x - 6.0, side * 30.0],
                        [x + 6.0, side * 30.0],
                        [x + 6.0, side * 42.0],
                        [x - 6.0, side * 42.0],
                    ],
                    [x, side * 36.0],
                    angle,
                    FLOOR_TOP,
                    h - FLOOR_TOP,
                );
            }
        }
    }
    if interior == 2 {
        // Pressure reading: opposing wall masses squeeze the middle of the
        // hall to a ~3 m throat.
        for side in [-1.0, 1.0] {
            out += &rotated_prism(
                &[
                    [-12.0, side * 24.0],
                    [12.0, side * 24.0],
                    [12.0, side * 96.0],
                    [-12.0, side * 96.0],
                ],
                [0.0, side * 44.0],
                angle,
                FLOOR_TOP,
                h - FLOOR_TOP,
            );
        }
    }
    out
}

/// Straight hall along `face`..`face.opposite()`. Variant is
/// `axis * 3 + interior` (axis = face index 0..2).
pub fn hall_straight_map(register: &str, interior: u16, face: HexFace) -> String {
    let style = register_style(register);
    let h = super::geometry::level_units();
    let opposite = face.opposite();
    let variant = face.index() as u16 * 3 + interior;
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    brushes += &straight_interior(interior, face, h);
    for f in HexFace::LATERAL {
        if f == face || f == opposite {
            brushes += &door_wall(f, 0.0, h, FLOOR_TOP, DOOR_TOP, style.trim_height);
        } else {
            brushes += &wall_brush(f, 0.0, h);
            brushes += &trim(f, style.trim_height);
        }
    }
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let mut out = format!(
        "// Straight hall, doors {} and {} (interior {interior}).\n",
        face_name(face),
        face_name(opposite)
    );
    out += &worldspawn(&brushes);
    out += &tile_meta("hall_straight", register, variant, 1);
    out += &tile_port(face_name(face), "door");
    out += &tile_port(face_name(opposite), "door");
    for [x, y] in rotate_points(&[[-48.0, 0.0], [48.0, 0.0]], face_angle_deg(face)) {
        out += &tile_light(x, y, h - 32.0);
    }
    out
}

/// Register trim band along a fully sealed wall face (skipped when 0).
fn trim(face: HexFace, height: f64) -> String {
    if height <= 0.0 {
        return String::new();
    }
    super::geometry::band_brush(
        face,
        super::geometry::WALL,
        super::geometry::WALL + 8.0,
        FLOOR_TOP,
        FLOOR_TOP + height,
    )
}

/// Dead-end cap with its door on `door_face`; a plinth-and-stele alcove marks
/// the sealed back so caps read as destinations, not missing geometry.
/// Variant = door face index.
pub fn hall_cap_map(register: &str, door_face: HexFace) -> String {
    let style = register_style(register);
    let h = super::geometry::level_units();
    let angle = face_angle_deg(door_face);
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    // Back-wall alcove: low platform plus a tall stele, opposite the door.
    brushes += &rotated_prism(
        &[[-98.0, -34.0], [-72.0, -34.0], [-72.0, 34.0], [-98.0, 34.0]],
        [-86.0, 0.0],
        angle,
        FLOOR_TOP,
        FLOOR_TOP + 16.0,
    );
    brushes += &rotated_prism(
        &[[-96.0, -10.0], [-84.0, -10.0], [-84.0, 10.0], [-96.0, 10.0]],
        [-90.0, 0.0],
        angle,
        FLOOR_TOP + 16.0,
        FLOOR_TOP + 96.0,
    );
    for face in HexFace::LATERAL {
        if face == door_face {
            brushes += &door_wall(face, 0.0, h, FLOOR_TOP, DOOR_TOP, style.trim_height);
        } else {
            brushes += &wall_brush(face, 0.0, h);
            brushes += &trim(face, style.trim_height);
        }
    }
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let mut out = format!("// Dead-end cap, door {}.\n", face_name(door_face));
    out += &worldspawn(&brushes);
    out += &tile_meta("hall_cap", register, door_face.index() as u16, 1);
    out += &tile_port(face_name(door_face), "door");
    let [x, y] = rotate_points(&[[-48.0, 0.0]], angle)[0];
    out += &tile_light(x, y, h - 32.0);
    out
}

/// Corner hall with doors on `f1` and `f2` (any non-opposite pair — opposite
/// pairs are straights). A pillar pair on the far side of the turn guides the
/// eye through it. Variant = `f1.index() * 6 + f2.index()` with f1 < f2.
pub fn hall_corner_map(register: &str, f1: HexFace, f2: HexFace) -> String {
    let style = register_style(register);
    let h = super::geometry::level_units();
    let variant = (f1.index() * 6 + f2.index()) as u16;
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    // Pillars opposite the elbow: along the negated bisector of the two door
    // directions, flanking the walk line between the doors.
    let (a1, b1) = super::geometry::tb_edge(f1);
    let (a2, b2) = super::geometry::tb_edge(f2);
    let m1 = [(a1[0] + b1[0]) * 0.5, (a1[1] + b1[1]) * 0.5];
    let m2 = [(a2[0] + b2[0]) * 0.5, (a2[1] + b2[1]) * 0.5];
    let bis = [m1[0] + m2[0], m1[1] + m2[1]];
    let len = (bis[0] * bis[0] + bis[1] * bis[1]).sqrt();
    let dir = [bis[0] / len, bis[1] / len];
    let perp = [-dir[1], dir[0]];
    for side in [-1.0, 1.0] {
        let center = [
            -dir[0] * 40.0 + perp[0] * side * 40.0,
            -dir[1] * 40.0 + perp[1] * side * 40.0,
        ];
        brushes += &pillar_brush(center, 7.0, FLOOR_TOP, h - FLOOR_TOP);
    }
    for face in HexFace::LATERAL {
        if face == f1 || face == f2 {
            brushes += &door_wall(face, 0.0, h, FLOOR_TOP, DOOR_TOP, style.trim_height);
        } else {
            brushes += &wall_brush(face, 0.0, h);
            brushes += &trim(face, style.trim_height);
        }
    }
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let mut out = format!(
        "// Corner hall, doors {} and {}.\n",
        face_name(f1),
        face_name(f2)
    );
    out += &worldspawn(&brushes);
    out += &tile_meta("hall_corner", register, variant, 1);
    out += &tile_port(face_name(f1), "door");
    out += &tile_port(face_name(f2), "door");
    out += &tile_light(0.0, 0.0, h - 32.0);
    out
}

/// Junction hall with doors on `open_faces` (3- and 4-way). A central
/// register-sized waypoint pylon with a base collar makes every junction a
/// landmark. Variant = bitmask of open faces.
pub fn hall_junction_map(register: &str, open_faces: &[HexFace]) -> String {
    let style = register_style(register);
    let h = super::geometry::level_units();
    let variant: u16 = open_faces
        .iter()
        .map(|face| 1u16 << face.index())
        .sum::<u16>();
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    brushes += &pylon_brush(style.pylon_radius, FLOOR_TOP, h - FLOOR_TOP);
    brushes += &pylon_brush(style.pylon_radius + 8.0, FLOOR_TOP, FLOOR_TOP + 12.0);
    for face in HexFace::LATERAL {
        if open_faces.contains(&face) {
            brushes += &door_wall(face, 0.0, h, FLOOR_TOP, DOOR_TOP, style.trim_height);
        } else {
            brushes += &wall_brush(face, 0.0, h);
            brushes += &trim(face, style.trim_height);
        }
    }
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let names: Vec<&str> = open_faces.iter().map(|&face| face_name(face)).collect();
    let mut out = format!("// Junction hall, doors {}.\n", names.join(", "));
    out += &worldspawn(&brushes);
    out += &tile_meta("hall_junction", register, variant, 1);
    for &face in open_faces {
        out += &tile_port(face_name(face), "door");
    }
    out += &tile_light(-48.0, 0.0, h - 32.0);
    out += &tile_light(48.0, 0.0, h - 32.0);
    out
}
