//! The hall family: eight single-cell corridor modules.
//!
//! A byte-exact port of the corresponding builders in `tools/tileforge.py`.
//! Same stable IDs, archetypes, variants, and ports, so every seam signature is
//! unchanged - these regenerate the committed files rather than replacing them.

use super::entities::{
    Meta, PORT_SHORT, ceiling_fixture, lateral_port, tile_cell_default, worldspawn,
};
use super::geometry::{
    DOOR_HALF_WIDTH, FACE_NAMES, FLOOR_TOP, LEVEL, P2, WALL, band, centroid, edge, face_mid,
    hex_slab, prism, pylon,
};
use super::{Builder, GENERATED_NOTE};

#[must_use]
fn square(center: P2, half: f64) -> Vec<P2> {
    vec![
        (center.0 - half, center.1 - half),
        (center.0 + half, center.1 - half),
        (center.0 + half, center.1 + half),
        (center.0 - half, center.1 + half),
    ]
}

/// Unit vector from the cell centre toward a face's midpoint.
#[must_use]
fn axis(face: usize) -> P2 {
    let mid = face_mid(face);
    let length = mid.0.hypot(mid.1);
    (mid.0 / length, mid.1 / length)
}

/// The left-hand normal of `u`, which is the direction a channel is measured in.
#[must_use]
const fn left_normal(u: P2) -> P2 {
    (-u.1, u.0)
}

/// The triangle from the cell centre out to one face.
#[must_use]
fn sector(face: usize) -> Vec<P2> {
    let (a, b) = edge(face);
    vec![(0.0, 0.0), a, b]
}

/// Clip a convex polygon to the half-plane `n . p <= d` (Sutherland-Hodgman).
#[must_use]
pub(super) fn clip(poly: &[P2], n: P2, d: f64) -> Vec<P2> {
    let inside = |p: &P2| n.0 * p.0 + n.1 * p.1 <= d + 1e-9;
    let mut out: Vec<P2> = Vec::new();
    for index in 0..poly.len() {
        let current = poly[index];
        let next = poly[(index + 1) % poly.len()];
        let (cin, nin) = (inside(&current), inside(&next));
        if cin {
            out.push(current);
        }
        if cin != nin {
            let cd = n.0 * current.0 + n.1 * current.1 - d;
            let nd = n.0 * next.0 + n.1 * next.1 - d;
            let t = cd / (cd - nd);
            out.push((
                current.0 + (next.0 - current.0) * t,
                current.1 + (next.1 - current.1) * t,
            ));
        }
    }
    out
}

/// Floor and ceiling slabs, then solid flanks either side of the walk channel.
///
/// A hall used to be a hex-wide chamber whose connected faces carried a wall
/// with a doorway punched through it. Both halves of that read wrongly. The
/// width was the cell's own, so length could never exceed width and a run of
/// cells was a string of chambers; and because a doorway also marks where a
/// *room* begins, crossing into one looked exactly like carrying on.
///
/// The channel is now axial rather than radial: an arm of the canonical door
/// width runs from the centre to each door face, and everything else in the
/// cell is filled solid. The passage is therefore the same width at a seam as
/// in the middle of a cell, and a run of them is one continuous corridor.
///
/// Nothing is emitted across a door face at all. A hall meeting a hall wants no
/// wall between them, and a hall meeting a room already has one: `room_shell`
/// emits `door_wall_default` at its own named ports, so the threshold belongs
/// to the room and is the only doorway on that seam. That is what makes the
/// boundary legible without the tile needing to know what it abuts.
#[must_use]
fn hall_shell(door_faces: &[usize]) -> String {
    let h = LEVEL;
    let mut brushes = String::from("// Floor and ceiling slabs (bevelled rims)\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(h - FLOOR_TOP, h, 0.0, 3.0));
    for (face, name) in FACE_NAMES.iter().enumerate() {
        let piece = sector(face);
        if door_faces.contains(&face) {
            // The arm runs down this sector; fill what it leaves either side.
            let n = left_normal(axis(face));
            brushes.push_str(&format!("// Channel flanks: {name}\n"));
            for side in [1.0, -1.0] {
                let normal = (n.0 * side, n.1 * side);
                let flank = clip(&piece, (-normal.0, -normal.1), -DOOR_HALF_WIDTH);
                if flank.len() >= 3 {
                    brushes.push_str(&prism(&flank, 0.0, h, None, 0.0, 0.0));
                }
            }
        } else {
            // Solid to the face, cut back by any arm that reaches into it.
            brushes.push_str(&format!("// Solid flank: {name}\n"));
            let centre = centroid(&piece);
            let mut solid = piece;
            for &door in door_faces {
                let n = left_normal(axis(door));
                let side = if n.0 * centre.0 + n.1 * centre.1 >= 0.0 {
                    1.0
                } else {
                    -1.0
                };
                let normal = (n.0 * side, n.1 * side);
                solid = clip(&solid, (-normal.0, -normal.1), -DOOR_HALF_WIDTH);
                if solid.len() < 3 {
                    break;
                }
            }
            if solid.len() >= 3 {
                brushes.push_str(&prism(&solid, 0.0, h, None, 0.0, 0.0));
                brushes.push_str(&band(face, WALL, WALL + 8.0, FLOOR_TOP, FLOOR_TOP + 12.0));
            }
        }
    }
    brushes
}

/// Meta, footprint cell, and one door port per face.
///
/// East and west spell their names out while the others use the short form.
/// That asymmetry is in the committed files, so it is preserved rather than
/// tidied: the port name is part of the threshold identity.
#[must_use]
fn hall_meta_and_ports(name: &str, archetype: &str, door_faces: &[usize]) -> String {
    let mut out = Meta::cell(&format!("authored/{name}"), archetype, 0, 1, 10).emit();
    out.push_str(&tile_cell_default());
    for &face in door_faces {
        let short = if face == 0 || face == 3 {
            FACE_NAMES[face]
        } else {
            PORT_SHORT[face]
        };
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{short}_port"),
            0,
            0,
            0,
        ));
    }
    out
}

#[must_use]
pub fn hall_straight() -> String {
    let mut brushes = hall_shell(&[0, 3]);
    brushes.push_str("// Colonnade: two pillar pairs flanking the walk axis\n");
    for x in [-44.0, 44.0] {
        for y in [-34.0, 34.0] {
            brushes.push_str(&prism(
                &square((x, y), 6.0),
                FLOOR_TOP,
                LEVEL - FLOOR_TOP,
                None,
                3.0,
                0.0,
            ));
        }
    }
    let mut lights = String::new();
    for x in [-48.0, 48.0] {
        let (fixture, source) = ceiling_fixture(x, 0.0, LEVEL, 18.0, 10.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }
    let mut out = String::from("// Straight hall, doors east/west, colonnade interior.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(&hall_meta_and_ports(
        "hall_straight",
        "hall_straight",
        &[0, 3],
    ));
    out.push_str(&lights);
    out
}

#[must_use]
pub fn hall_cap() -> String {
    let mut brushes = hall_shell(&[0]);
    brushes.push_str("// Back-wall alcove: plinth and stele opposite the door\n");
    brushes.push_str(&prism(
        &[(-98.0, -34.0), (-72.0, -34.0), (-72.0, 34.0), (-98.0, 34.0)],
        FLOOR_TOP,
        24.0,
        None,
        3.0,
        0.0,
    ));
    brushes.push_str(&prism(
        &[(-96.0, -10.0), (-84.0, -10.0), (-84.0, 10.0), (-96.0, 10.0)],
        24.0,
        104.0,
        None,
        4.0,
        0.0,
    ));
    let (fixture, lights) = ceiling_fixture(-48.0, 0.0, LEVEL, 18.0, 10.0);
    brushes.push_str(&fixture);
    let mut out = String::from("// Dead-end cap, door east; alcove stele marks the sealed back.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(&hall_meta_and_ports("hall_cap", "hall_cap", &[0]));
    out.push_str(&lights);
    out
}

#[must_use]
fn hall_turn(name: &str, archetype: &str, second_face: usize) -> String {
    let mut brushes = hall_shell(&[0, second_face]);
    brushes.push_str("// Guide pillars opposite the elbow\n");
    let (m0, m1) = (face_mid(0), face_mid(second_face));
    let bis = (m0.0 + m1.0, m0.1 + m1.1);
    let length = bis.0.hypot(bis.1);
    let d = (bis.0 / length, bis.1 / length);
    let perp = (-d.1, d.0);
    for side in [-1.0, 1.0] {
        let center = (
            -d.0 * 40.0 + perp.0 * side * 40.0,
            -d.1 * 40.0 + perp.1 * side * 40.0,
        );
        brushes.push_str(&prism(
            &square(center, 6.5),
            FLOOR_TOP,
            LEVEL - FLOOR_TOP,
            None,
            3.0,
            0.0,
        ));
    }
    let (fixture, lights) = ceiling_fixture(0.0, 0.0, LEVEL, 18.0, 10.0);
    brushes.push_str(&fixture);
    let mut out = format!("// Corner hall, doors east/{}.\n", FACE_NAMES[second_face]);
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(&hall_meta_and_ports(name, archetype, &[0, second_face]));
    out.push_str(&lights);
    out
}

#[must_use]
pub fn hall_turn_60() -> String {
    hall_turn("hall_turn_60", "hall_turn_60", 5)
}

#[must_use]
pub fn hall_turn_120() -> String {
    hall_turn("hall_turn_120", "hall_turn_120", 4)
}

#[must_use]
fn hall_junction(name: &str, archetype: &str, door_faces: &[usize]) -> String {
    let mut brushes = hall_shell(door_faces);
    brushes.push_str("// Waypoint pylon with base collar\n");
    brushes.push_str(&pylon(14.0, FLOOR_TOP, LEVEL - FLOOR_TOP, 0.0, 5.0, 0.0));
    brushes.push_str(&pylon(24.0, FLOOR_TOP, 22.0, 0.0, 5.0, 0.0));
    let mut lights = String::new();
    for x in [-44.0, 44.0] {
        let (fixture, source) = ceiling_fixture(x, 0.0, LEVEL, 14.0, 8.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }
    let names: Vec<&str> = door_faces.iter().map(|&f| FACE_NAMES[f]).collect();
    let mut out = format!("// Junction hall, doors {}.\n", names.join(", "));
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(&hall_meta_and_ports(name, archetype, door_faces));
    out.push_str(&lights);
    out
}

#[must_use]
pub fn hall_junction_3way() -> String {
    hall_junction("hall_junction_3way", "hall_junction_3way", &[0, 3, 5])
}

#[must_use]
pub fn hall_junction_4way() -> String {
    hall_junction("hall_junction_4way", "hall_junction_4way", &[0, 2, 3, 5])
}

#[must_use]
pub fn hall_straight_buttressed() -> String {
    let mut brushes = hall_shell(&[0, 3]);
    brushes.push_str("// Grounded side buttresses keep the long axis open\n");
    for face in [1, 2, 4, 5] {
        brushes.push_str(&band(face, WALL, WALL + 18.0, FLOOR_TOP, 72.0));
    }
    let mut lights = String::new();
    for x in [-48.0, 48.0] {
        let (fixture, source) = ceiling_fixture(x, 0.0, LEVEL, 16.0, 9.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }
    let mut out =
        String::from("// Straight hall variant: structural side buttresses, clear E/W route.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            "authored/hall_straight_buttressed",
            "hall_straight",
            1,
            1,
            7,
        )
        .emit(),
    );
    out.push_str(&tile_cell_default());
    out.push_str(&lateral_port(0, "door", "east_port", 0, 0, 0));
    out.push_str(&lateral_port(3, "door", "west_port", 0, 0, 0));
    out.push_str(&lights);
    out
}

#[must_use]
pub fn hall_turn_60_buttressed() -> String {
    let mut brushes = hall_shell(&[0, 5]);
    brushes.push_str("// Grounded outer-corner masses frame the bend\n");
    for face in [2, 3] {
        brushes.push_str(&band(face, WALL, WALL + 22.0, FLOOR_TOP, 88.0));
    }
    brushes.push_str(&pylon(10.0, FLOOR_TOP, LEVEL - FLOOR_TOP, 30.0, 3.0, 0.0));
    let (fixture, lights) = ceiling_fixture(26.0, 28.0, LEVEL, 15.0, 9.0);
    brushes.push_str(&fixture);
    let mut out =
        String::from("// 60-degree turn variant: supported cove around a full-height pier.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(&Meta::cell("authored/hall_turn_60_buttressed", "hall_turn_60", 1, 1, 7).emit());
    out.push_str(&tile_cell_default());
    out.push_str(&lateral_port(0, "door", "east_port", 0, 0, 0));
    out.push_str(&lateral_port(5, "door", "ne_port", 0, 0, 0));
    out.push_str(&lights);
    out
}

/// Every hall builder, paired with the file it must reproduce.
///
/// The pairing lives here rather than in a central registry so a new hall
/// cannot be added without naming its output - which is what the byte-identity
/// gate iterates.
/// How far the datum shelf steps up between bays, in editor units.
///
/// Eleven, which is 0.69 m: large enough to read as a step from the far end of
/// the cell and small enough that four of them stay under [`DOOR_TOP`]. The
/// last thing a directional cue may do is foul the doorway it points at.
const DATUM_RISE: f64 = 11.0;

/// Where the lowest bay's shelf sits. 28 units is 1.75 m - just above a
/// standing eye at 1.6 m, so the run is read against the wall rather than
/// walked into.
const DATUM_BASE: f64 = 28.0;

/// A straight hall that tells you which way you are facing.
///
/// # Why this tile exists
///
/// Arc T's first playtest reported that players could not say where they were
/// or which way they had come from (backlog #30, #35). For a corridor that is
/// not a lighting problem or a corpus-size problem, it is a **symmetry**
/// problem: `hall_straight` and `hall_straight_buttressed` are both invariant
/// under the half-turn that swaps their two doors, so the view east and the
/// view west are the same picture. No amount of authoring more symmetric
/// corridors fixes that.
///
/// So the identity here is a **datum**: a shelf on both channel walls that
/// climbs in four discrete bays from one door to the other. Walking one way the
/// run rises, the other way it falls, and the bay you are beside says roughly
/// how far along you are. It is deliberately the same on the left and the right,
/// because a left/right difference would make the tile read differently
/// depending on which way you entered, which is a second ambiguity rather than
/// an answer to the first.
///
/// # What it may not do
///
/// **Stop short of both seams.** The aperture at a face plane is a frozen
/// contract - 72 units wide, `FLOOR_TOP..DOOR_TOP` - and the shelf spans
/// `|y| = 28..36`, which is inside that width. Running it to the face would put
/// mass in the doorway and break every neighbour. It ends at `+/-88`, a metre
/// and a half short of the 112-unit apothem, and reads as a run that stops at
/// the threshold.
///
/// **Leave the walk clear.** The channel is 72 units across; the shelf takes 8
/// from each side, leaving 3.5 m between the runs for a body 0.76 m wide. The
/// colonnade the plain straight carries is dropped rather than kept, because
/// four pillars plus eight shelf bays is a busier cell than a corridor wants
/// and this variant's identity is the datum, not the pillars.
#[must_use]
pub fn hall_straight_datum() -> String {
    let mut brushes = hall_shell(&[0, 3]);
    brushes.push_str("// Datum run: four bays climbing west to east, both channel walls\n");
    for bay in 0..4 {
        #[allow(clippy::cast_precision_loss)]
        let step = f64::from(bay);
        let x0 = -88.0 + step * 44.0;
        let x1 = x0 + 44.0;
        let z0 = DATUM_BASE + step * DATUM_RISE;
        let z1 = z0 + 8.0;
        for side in [-1.0, 1.0] {
            let (near, far) = (36.0 * side, 28.0 * side);
            let plan: Vec<P2> = if side > 0.0 {
                vec![(x0, far), (x1, far), (x1, near), (x0, near)]
            } else {
                vec![(x0, near), (x1, near), (x1, far), (x0, far)]
            };
            brushes.push_str(&prism(&plan, z0, z1, None, 2.0, 2.0));
        }
    }
    // Lit from the high end only, so the cue survives a dark corridor: the
    // bright end is the end the datum climbs toward.
    let mut lights = String::new();
    for (x, size, reach) in [(-56.0, 12.0, 7.0), (56.0, 20.0, 12.0)] {
        let (fixture, source) = ceiling_fixture(x, 0.0, LEVEL, size, reach);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }
    let mut out = String::from(
        "// Straight hall variant: a stepped datum run that makes the corridor handed.\n",
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(&Meta::cell("authored/hall_straight_datum", "hall_straight", 2, 1, 7).emit());
    out.push_str(&tile_cell_default());
    out.push_str(&lateral_port(0, "door", "east_port", 0, 0, 0));
    out.push_str(&lateral_port(3, "door", "west_port", 0, 0, 0));
    out.push_str(&lights);
    out
}

#[must_use]
pub fn builders() -> Vec<Builder> {
    vec![
        ("hall_straight", hall_straight as fn() -> String),
        ("hall_straight_buttressed", hall_straight_buttressed),
        ("hall_straight_datum", hall_straight_datum),
        ("hall_cap", hall_cap),
        ("hall_turn_60", hall_turn_60),
        ("hall_turn_60_buttressed", hall_turn_60_buttressed),
        ("hall_turn_120", hall_turn_120),
        ("hall_junction_3way", hall_junction_3way),
        ("hall_junction_4way", hall_junction_4way),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_hall_reproduces_its_committed_file() {
        super::super::assert_reproduces(&builders());
    }

    /// The builders must actually be wired to distinct outputs. A registry that
    /// pointed two names at one builder would pass the gate above for one of
    /// them and quietly never test the other.
    #[test]
    fn each_hall_builder_produces_a_distinct_file() {
        let built: Vec<String> = builders().into_iter().map(|(_, build)| build()).collect();
        for (index, text) in built.iter().enumerate() {
            for other in built.iter().skip(index + 1) {
                assert_ne!(text, other, "two hall builders produce identical output");
            }
        }
    }
}
