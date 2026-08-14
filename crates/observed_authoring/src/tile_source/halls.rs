//! Hall tiles: straights (three interior readings), dead-end caps, corners,
//! and 3-/4-way junctions — every door face combination, including the
//! mitered diagonal faces.

use observed_hex::HexFace;

use super::geometry::{
    DOOR_TOP, FLOOR_TOP, ceiling_relief, column_brush, door_wall, face_angle_deg,
    general_prism_brush, hex_slab_brush, open_span, rotate_points, tile_light, tile_meta,
    tile_port, wall_brush, worldspawn,
};
use super::{RegisterStyle, face_name, register_style};

/// How many interior readings each archetype offers.
///
/// Straights have had three since the library was written; nothing else had
/// more than one, which is why two-thirds of a facility was the same three
/// shapes repeated. Junction gets two rather than three because it carries the
/// most signatures (35) and so costs the most tiles per reading — see the
/// library-size measurement in the plan.
pub const CORNER_READINGS: u16 = 3;
pub const JUNCTION_READINGS: u16 = 2;
pub const EXPANSE_READINGS: u16 = 2;

/// Interior readings are packed above the direction bits so that reading 0
/// keeps the exact variant number it had before they existed. The `TileKey`
/// schema gains rows; it does not shift.
pub(crate) const READING_STRIDE: u16 = 64;

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
fn straight_interior(interior: u16, style: RegisterStyle, face: HexFace, h: f64) -> String {
    let angle = face_angle_deg(face);
    let mut out = String::new();
    if interior == 1 {
        // Two facing pillar rows: the Colonnade reading. How many bays it has
        // is the district's `support_rhythm` — two reads as monumental, four as
        // a gallery, and that difference is most of why one colonnade should
        // not look like another district's.
        let bays = style.support_rhythm.max(1);
        #[allow(clippy::cast_precision_loss)]
        let span = 48.0f64;
        for side in [-1.0, 1.0] {
            for bay in 0..bays {
                #[allow(clippy::cast_precision_loss)]
                let t = if bays == 1 {
                    0.0
                } else {
                    (bay as f64 / (bays - 1) as f64) * 2.0 - 1.0
                };
                let x = t * span;
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
    brushes += &straight_interior(interior, style, face, h);
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

/// The corner's interior readings, placed against the elbow's own geometry.
/// 0 = guide pillars (the historical reading), 1 = a buttress mass on the outer
/// corner, 2 = cut — nothing in the floor, so the sightline runs clean through
/// the bend.
fn corner_interior(
    reading: u16,
    style: RegisterStyle,
    dir: [f64; 2],
    perp: [f64; 2],
    h: f64,
) -> String {
    let mut out = String::new();
    match reading {
        // Guide pillars flanking the walk line, on the far side of the turn.
        0 => {
            for side in [-1.0, 1.0] {
                let center = [
                    -dir[0] * 40.0 + perp[0] * side * 40.0,
                    -dir[1] * 40.0 + perp[1] * side * 40.0,
                ];
                out += &column_brush(style.column, center, 7.0, FLOOR_TOP, h - FLOOR_TOP);
            }
        }
        // A single grounded mass filling the outer corner, echoing the
        // committed `hall_turn_60_buttressed`. The turn reads as carved out of
        // a solid rather than routed between two posts.
        1 => {
            let centre = [-dir[0] * 62.0, -dir[1] * 62.0];
            let half = 26.0;
            out += &general_prism_brush(
                &[
                    [centre[0] - perp[0] * half, centre[1] - perp[1] * half],
                    [centre[0] + perp[0] * half, centre[1] + perp[1] * half],
                    [
                        centre[0] + perp[0] * half - dir[0] * 34.0,
                        centre[1] + perp[1] * half - dir[1] * 34.0,
                    ],
                    [
                        centre[0] - perp[0] * half - dir[0] * 34.0,
                        centre[1] - perp[1] * half - dir[1] * 34.0,
                    ],
                ],
                FLOOR_TOP,
                h - FLOOR_TOP,
                [centre[0] - dir[0] * 17.0, centre[1] - dir[1] * 17.0],
            );
        }
        // Cut: no floor mass at all. Every third corner being empty is what
        // stops a route of turns reading as one repeated tile.
        _ => {}
    }
    out
}

/// Corner hall with doors on `f1` and `f2` (any non-opposite pair — opposite
/// pairs are straights). Variant = `f1.index() * 6 + f2.index()`, with the
/// interior reading packed above it at [`READING_STRIDE`].
pub fn hall_corner_map(register: &str, reading: u16, f1: HexFace, f2: HexFace) -> String {
    let style = register_style(register);
    let h = super::geometry::level_units();
    let variant = (f1.index() * 6 + f2.index()) as u16 + reading * READING_STRIDE;
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    // The elbow frame: the negated bisector of the two door directions, and its
    // perpendicular. Every reading is placed against these, so a reading means
    // the same thing on all thirty door pairs.
    let (a1, b1) = super::geometry::tb_edge(f1);
    let (a2, b2) = super::geometry::tb_edge(f2);
    let m1 = [(a1[0] + b1[0]) * 0.5, (a1[1] + b1[1]) * 0.5];
    let m2 = [(a2[0] + b2[0]) * 0.5, (a2[1] + b2[1]) * 0.5];
    let bis = [m1[0] + m2[0], m1[1] + m2[1]];
    let len = (bis[0] * bis[0] + bis[1] * bis[1]).sqrt();
    let dir = [bis[0] / len, bis[1] / len];
    let perp = [-dir[1], dir[0]];
    brushes += &corner_interior(reading, style, dir, perp, h);
    for face in HexFace::LATERAL {
        if face == f1 || face == f2 {
            brushes += &door_wall(face, 0.0, h, FLOOR_TOP, DOOR_TOP, style.trim_height);
        } else {
            brushes += &wall_brush(face, 0.0, h);
            brushes += &trim(face, style.trim_height);
        }
    }
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    brushes += &ceiling_relief(style.ceiling, h - FLOOR_TOP, 72.0);
    let mut out = format!(
        "// Corner hall, doors {} and {} (interior {reading}).\n",
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
/// An expanse: open floor whose only walls are on the faces it does not open
/// through, and with no central pylon at all.
///
/// A junction keeps a pylon in the middle because a junction is a place where
/// ways meet and the eye needs something to read against. An expanse is the
/// opposite: its whole job is that a run of them merges into one uninterrupted
/// volume, and a pylon per cell would turn a vast room back into a colonnade of
/// tiles. Only the perimeter trim survives, so a boundary still reads.
///
/// An open face is [`open_span`], **not** [`door_wall`]. This used to reuse the
/// junction's doorway on the grounds that a junction is "already exactly
/// wall-free where it opens", which is not true of it: a junction opens through
/// a `DOOR_HALF_WIDTH` aperture between two jambs. Tiled, that made the
/// facility's largest archetype a honeycomb of single-cell rooms joined by
/// doors — the exact opposite of the merged volume the archetype exists for,
/// across roughly a third of every production layout.
///
/// The lintel stays. It keeps the face signature identical to a doorway's, so
/// an expanse still mates with an ordinary hall, and a run of expanses reads as
/// a bay structure rather than an unsupported slab.
/// The expanse's two readings — deliberately only two, and deliberately sparse.
///
/// 0 = nothing, the pure volume. 1 = a pair of slim piers set well off the
/// centre line, so a run of expanses reads as a hall that has structure rather
/// than as an absence. They are off-centre and thin on purpose: a support in
/// the middle of every cell is precisely the "colonnade of tiles" this
/// archetype exists to avoid, which is why there is no reading that puts one
/// there.
fn expanse_interior(reading: u16, style: RegisterStyle, h: f64) -> String {
    if reading == 0 {
        return String::new();
    }
    let mut out = String::new();
    for side in [-1.0f64, 1.0] {
        out += &column_brush(
            style.column,
            [side * 54.0, side * 30.0],
            style.pylon_radius * 0.55,
            FLOOR_TOP,
            h - FLOOR_TOP,
        );
    }
    out
}

pub fn expanse_map(register: &str, reading: u16, open_faces: &[HexFace]) -> String {
    let style = register_style(register);
    let h = super::geometry::level_units();
    let variant: u16 = open_faces
        .iter()
        .map(|face| 1u16 << face.index())
        .sum::<u16>()
        + reading * READING_STRIDE;
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    brushes += &expanse_interior(reading, style, h);
    for face in HexFace::LATERAL {
        if open_faces.contains(&face) {
            brushes += &open_span(face, 0.0, h, FLOOR_TOP, DOOR_TOP);
        } else {
            brushes += &wall_brush(face, 0.0, h);
            brushes += &trim(face, style.trim_height);
        }
    }
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    let names: Vec<&str> = open_faces.iter().map(|&face| face_name(face)).collect();
    let mut out = format!(
        "// Expanse, open across {}.
",
        names.join(", ")
    );
    out += &worldspawn(&brushes);
    out += &tile_meta("expanse", register, variant, 1);
    for &face in open_faces {
        out += &tile_port(face_name(face), "door");
    }
    // Two practicals well apart, so a merged run of expanses is lit as one
    // volume rather than as a string of separately-lit cells.
    out += &tile_light(-56.0, 0.0, h - 24.0);
    out += &tile_light(56.0, 0.0, h - 24.0);
    out
}

/// The junction's interior readings. 0 = the historical waypoint pylon with its
/// base collar; 1 = open — the collar alone, and the crossing itself clear.
///
/// A pylon in every junction is what made every junction the same place. It is
/// still the right reading for *a* junction; it was only ever wrong as the
/// reading for all of them.
fn junction_interior(reading: u16, style: RegisterStyle, h: f64) -> String {
    let mut out = String::new();
    if reading == 0 {
        out += &column_brush(
            style.column,
            [0.0, 0.0],
            style.pylon_radius,
            FLOOR_TOP,
            h - FLOOR_TOP,
        );
    }
    // The collar survives both readings: it is what says "this is a crossing"
    // at floor level, and it never blocks a sightline.
    out += &column_brush(
        style.column,
        [0.0, 0.0],
        style.pylon_radius + 8.0,
        FLOOR_TOP,
        FLOOR_TOP + 12.0,
    );
    out
}

pub fn hall_junction_map(register: &str, reading: u16, open_faces: &[HexFace]) -> String {
    let style = register_style(register);
    let h = super::geometry::level_units();
    let variant: u16 = open_faces
        .iter()
        .map(|face| 1u16 << face.index())
        .sum::<u16>()
        + reading * READING_STRIDE;
    let mut brushes = hex_slab_brush(0.0, FLOOR_TOP);
    brushes += &junction_interior(reading, style, h);
    for face in HexFace::LATERAL {
        if open_faces.contains(&face) {
            brushes += &door_wall(face, 0.0, h, FLOOR_TOP, DOOR_TOP, style.trim_height);
        } else {
            brushes += &wall_brush(face, 0.0, h);
            brushes += &trim(face, style.trim_height);
        }
    }
    brushes += &hex_slab_brush(h - FLOOR_TOP, h);
    brushes += &ceiling_relief(style.ceiling, h - FLOOR_TOP, 72.0);
    let names: Vec<&str> = open_faces.iter().map(|&face| face_name(face)).collect();
    let mut out = format!(
        "// Junction hall, doors {} (interior {reading}).\n",
        names.join(", ")
    );
    out += &worldspawn(&brushes);
    out += &tile_meta("hall_junction", register, variant, 1);
    for &face in open_faces {
        out += &tile_port(face_name(face), "door");
    }
    out += &tile_light(-48.0, 0.0, h - 32.0);
    out += &tile_light(48.0, 0.0, h - 32.0);
    out
}
