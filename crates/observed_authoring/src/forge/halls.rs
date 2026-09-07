//! The hall family: eight single-cell corridor modules.
//!
//! A byte-exact port of the corresponding builders in `tools/tileforge.py`.
//! Same stable IDs, archetypes, variants, and ports, so every seam signature is
//! unchanged - these regenerate the committed files rather than replacing them.

use super::entities::{
    Meta, PORT_SHORT, ceiling_fixture, lateral_port, tile_cell, tile_cell_default, wall_fixture,
    worldspawn,
};
use super::geometry::{
    DOOR_HALF_WIDTH, DOOR_TOP, FACE_NAMES, FLOOR_TOP, LEVEL, P2, WALL, band, boxed, centroid,
    corners, door_wall, edge, face_mid, hex_slab, offset_inward, prism, pylon, regular_polygon,
    sloped_prism, translate, wall,
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

/// One hand-cut face ring, as radii at sixty-degree steps from `phase`.
///
/// Every other mass in this corpus is a call to `pylon`, `square` or `band` -
/// a shape a function chose. These radii were chosen by hand, tier by tier, and
/// that is the whole point of the tile they build: the kit needs something that
/// is not a regular solid, and the only way to get one is to write the numbers.
///
/// Six radii rather than an arbitrary polygon because a brush must be convex
/// and a hand-written vertex list is one typo away from not being. Evenly
/// spaced angles with radii inside a modest band are convex by construction, so
/// the shape can be irregular without being invalid.
#[must_use]
fn hewn_ring(center: P2, radii: [f64; 6], phase_deg: f64) -> Vec<P2> {
    (0..6)
        .map(|index| {
            #[allow(clippy::cast_precision_loss)]
            let angle = (phase_deg + index as f64 * 60.0).to_radians();
            (
                center.0 + radii[index] * angle.cos(),
                center.1 + radii[index] * angle.sin(),
            )
        })
        .collect()
}

/// A three-way junction with a hewn monolith standing in it.
///
/// # Why this one is written rather than generated
///
/// Arc T's T-6 note says the kit will need hand-made tiles and that everything
/// in the corpus is forge-generated. This is the first that is not. Its mass is
/// three tiers of hand-chosen radii, canted against each other, rather than a
/// primitive with a radius argument - so it is the one shape in the facility
/// that does not read as a solid of revolution.
///
/// A junction is where it belongs. Junctions are decision points, and the
/// measured corpus places about 117 three-way junctions a facility on the
/// pylon reading - all of them identical. A landmark is only a landmark if
/// there is one of it.
///
/// # Where it may stand, which is not the middle
///
/// **The cell centre stays clear**, and that is a bot constraint rather than an
/// aesthetic one. `lateral_waypoint` steers a body at the shared doorway and
/// then at the neighbour's centre, so the centre of a cell is on the path
/// through it; the existing waypoint pylon is thin enough to be walked round and
/// a mass this size is not. It stands in the solid quarter between the two
/// undoored faces, found from their own mid-points rather than from a bearing
/// written down here, so it follows the door pattern if this is ever asked for
/// another one.
///
/// **Nothing reaches a door channel.** `hall_shell` has already cut the arms to
/// `DOOR_HALF_WIDTH`; the monolith sits outside them by construction, and the
/// canted beam overhead starts above `DOOR_TOP` so a sightline down any arm is
/// unobstructed.
#[must_use]
pub fn hall_junction_3way_hewn() -> String {
    const DOORS: [usize; 3] = [0, 3, 5];
    let mut brushes = hall_shell(&DOORS);

    // The solid quarter, from the two faces that carry no door.
    let (a, b) = (face_mid(1), face_mid(2));
    let bisector = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
    let length = bisector.0.hypot(bisector.1);
    let seat = (bisector.0 / length * 54.0, bisector.1 / length * 54.0);

    brushes.push_str("// Hewn monolith: three canted tiers, radii chosen by hand\n");
    for (radii, phase, z0, z1, chamfer) in [
        (
            [38.0, 31.0, 35.0, 29.0, 36.0, 33.0],
            0.0,
            FLOOR_TOP,
            44.0,
            3.0,
        ),
        ([33.0, 36.0, 28.0, 34.0, 30.0, 37.0], 22.0, 44.0, 82.0, 2.0),
        (
            [27.0, 24.0, 29.0, 23.0, 28.0, 25.0],
            41.0,
            82.0,
            LEVEL - FLOOR_TOP,
            4.0,
        ),
    ] {
        brushes.push_str(&prism(
            &hewn_ring(seat, radii, phase),
            z0,
            z1,
            Some(seat),
            chamfer,
            0.0,
        ));
    }

    // A canted beam from the monolith's head across the crossing. It starts
    // above `DOOR_TOP`, so it darkens the ceiling over the junction without
    // taking anything off a sightline down an arm.
    brushes.push_str("// Canted head beam, clear of every doorway\n");
    let across = (-seat.0 / 54.0 * 96.0, -seat.1 / 54.0 * 96.0);
    let perp = (-(across.1 - seat.1), across.0 - seat.0);
    let span = perp.0.hypot(perp.1).max(1.0);
    let half = (perp.0 / span * 11.0, perp.1 / span * 11.0);
    brushes.push_str(&prism(
        &[
            (seat.0 + half.0, seat.1 + half.1),
            (across.0 + half.0, across.1 + half.1),
            (across.0 - half.0, across.1 - half.1),
            (seat.0 - half.0, seat.1 - half.1),
        ],
        88.0,
        104.0,
        None,
        2.0,
        2.0,
    ));

    let mut lights = String::new();
    // One practical washing the monolith and one out over the crossing, so the
    // mass is what is lit rather than the empty middle.
    for (x, y, size, reach) in [
        (seat.0 * 1.5, seat.1 * 1.5, 10.0, 7.0),
        (0.0, 0.0, 16.0, 10.0),
    ] {
        let (fixture, source) = ceiling_fixture(x, y, LEVEL, size, reach);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = String::from(
        "// Three-way junction: a hand-hewn monolith in the solid quarter, centre kept clear.\n",
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            "authored/hall_junction_3way_hewn",
            "hall_junction_3way",
            1,
            1,
            6,
        )
        .emit(),
    );
    out.push_str(&tile_cell_default());
    for &face in &DOORS {
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
    out.push_str(&lights);
    out
}

/// Where a hand-cut mass stands in a **turn**, in plan.
///
/// Directly opposite the elbow: the two door faces are 60 or 120 degrees apart,
/// so their summed mid-point is a bearing into the bend and the negation of it
/// is the deepest part of the cell away from the walk. A body rounding the
/// corner cuts the inside; this sits on the outside of that arc.
///
/// A junction cannot use this rule and does not - three doors roughly cancel,
/// so `hall_junction_3way_hewn` seats from the *undoored* faces instead. Two
/// door patterns, two rules, and neither generalises to the other.
#[must_use]
fn hewn_seat_opposite(doors: [usize; 2], radius: f64) -> P2 {
    let (a, b) = (face_mid(doors[0]), face_mid(doors[1]));
    let sum = (a.0 + b.0, a.1 + b.1);
    let length = sum.0.hypot(sum.1).max(1.0);
    (-sum.0 / length * radius, -sum.1 / length * radius)
}

/// The shared body of the two hand-cut turns.
///
/// The tiers are the argument rather than the code, because the whole point of
/// these two tiles is that they are *different landmarks*. A shared shape with
/// a shared silhouette would put the same object at both kinds of corner and
/// leave the corpus exactly as legible as it was.
#[must_use]
fn hall_turn_hewn(
    name: &str,
    archetype: &str,
    doors: [usize; 2],
    seat_radius: f64,
    variant: i32,
    tiers: &[([f64; 6], f64, f64, f64, f64)],
    note: &str,
) -> String {
    let mut brushes = hall_shell(&doors);
    let seat = hewn_seat_opposite(doors, seat_radius);
    brushes.push_str("// Hand-cut mass, seated opposite the elbow\n");
    for &(radii, phase, z0, z1, chamfer) in tiers {
        brushes.push_str(&prism(
            &hewn_ring(seat, radii, phase),
            z0,
            z1,
            Some(seat),
            chamfer,
            0.0,
        ));
    }
    let mut lights = String::new();
    for (x, y, size, reach) in [
        (seat.0 * 1.4, seat.1 * 1.4, 11.0, 7.0),
        (-seat.0 * 0.5, -seat.1 * 0.5, 15.0, 9.0),
    ] {
        let (fixture, source) = ceiling_fixture(x, y, LEVEL, size, reach);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }
    let mut out = format!("// {note}\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(&Meta::cell(&format!("authored/{name}"), archetype, variant, 1, 6).emit());
    out.push_str(&tile_cell_default());
    for &face in &doors {
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
    out.push_str(&lights);
    out
}

/// A sixty-degree turn with a splinter standing in it.
///
/// Tall and thin: it reaches the ceiling and reads in silhouette from either
/// arm, which is what a tight corner wants. You know which turn this is before
/// you have rounded it.
#[must_use]
pub fn hall_turn_60_hewn() -> String {
    hall_turn_hewn(
        "hall_turn_60_hewn",
        "hall_turn_60",
        [0, 5],
        50.0,
        2,
        &[
            (
                [25.0, 20.0, 24.0, 19.0, 23.0, 21.0],
                0.0,
                FLOOR_TOP,
                52.0,
                3.0,
            ),
            ([21.0, 24.0, 18.0, 22.0, 19.0, 23.0], 27.0, 52.0, 96.0, 2.0),
            (
                [17.0, 14.0, 18.0, 13.0, 16.0, 15.0],
                49.0,
                96.0,
                LEVEL - FLOOR_TOP,
                4.0,
            ),
        ],
        "Sixty-degree turn: a hand-cut splinter opposite the elbow, full height.",
    )
}

/// A hundred-and-twenty-degree turn with a boulder in it.
///
/// Squat and broad, and deliberately **not** full height - it stops at 70 units
/// so a body sees over it and the shallow bend keeps its long sightline. A
/// splinter here would block the one thing this corner has that the sharp one
/// does not.
#[must_use]
pub fn hall_turn_120_hewn() -> String {
    hall_turn_hewn(
        "hall_turn_120_hewn",
        "hall_turn_120",
        [0, 4],
        46.0,
        1,
        &[
            (
                [43.0, 36.0, 41.0, 34.0, 39.0, 37.0],
                0.0,
                FLOOR_TOP,
                40.0,
                4.0,
            ),
            ([34.0, 30.0, 33.0, 28.0, 31.0, 29.0], 31.0, 40.0, 70.0, 6.0),
        ],
        "Hundred-and-twenty-degree turn: a low hand-cut boulder, sightline kept.",
    )
}

/// A ring of `N` radii at evenly spaced angles, for masses that want more
/// facets than [`hewn_ring`]'s six.
///
/// Same convexity guarantee and for the same reason: evenly spaced angles with
/// radii inside a modest band cannot fold back on themselves, so the shape can
/// be irregular without being an invalid brush.
#[must_use]
fn hewn_ring_n(center: P2, radii: &[f64], phase_deg: f64) -> Vec<P2> {
    let count = radii.len();
    #[allow(clippy::cast_precision_loss)]
    let step = 360.0 / count as f64;
    radii
        .iter()
        .enumerate()
        .map(|(index, &radius)| {
            #[allow(clippy::cast_precision_loss)]
            let angle = (phase_deg + index as f64 * step).to_radians();
            (
                center.0 + radius * angle.cos(),
                center.1 + radius * angle.sin(),
            )
        })
        .collect()
}

/// A three-way junction whose mass is cut for one district's own vocabulary.
///
/// The hand-cut masses that came before this are register-agnostic: the same
/// splinter and the same monolith stand in every district, re-skinned. That is
/// a landmark you can navigate by and it is *not* a place you can tell apart
/// from the next district's version of it, which is the other half of what Arc
/// T asks for.
///
/// `register_style` in the generated kit already states each district's
/// vocabulary in as many words - "one mass, undivided", "faceted by name:
/// flutes that catch the key light", "almost nothing, by name" - and until now
/// only the *generated* library read it. These are hand-cut answers to the same
/// three sentences, scoped so each is reachable only in the district it was cut
/// for.
#[must_use]
fn hall_junction_3way_themed(
    name: &str,
    register: &str,
    variant: i32,
    tiers: &[(Vec<f64>, f64, f64, f64, f64)],
    note: &str,
) -> String {
    const DOORS: [usize; 3] = [0, 3, 5];
    let mut brushes = hall_shell(&DOORS);
    let (a, b) = (face_mid(1), face_mid(2));
    let bisector = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
    let length = bisector.0.hypot(bisector.1);
    let seat = (bisector.0 / length * 54.0, bisector.1 / length * 54.0);

    brushes.push_str("// District mass, cut to this register's own vocabulary\n");
    for (radii, phase, z0, z1, chamfer) in tiers {
        brushes.push_str(&prism(
            &hewn_ring_n(seat, radii, *phase),
            *z0,
            *z1,
            Some(seat),
            *chamfer,
            0.0,
        ));
    }

    let mut lights = String::new();
    for (x, y, size, reach) in [
        (seat.0 * 1.4, seat.1 * 1.4, 11.0, 7.0),
        (0.0, 0.0, 15.0, 9.0),
    ] {
        let (fixture, source) = ceiling_fixture(x, y, LEVEL, size, reach);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = format!("// {note}\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            &format!("authored/{name}"),
            "hall_junction_3way",
            variant,
            1,
            8,
        )
        .with_register_scope(register)
        .emit(),
    );
    out.push_str(&tile_cell_default());
    for &face in &DOORS {
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
    out.push_str(&lights);
    out
}

/// Monolith: "one mass, undivided", the fewest supports of any district and the
/// heaviest. So one block, floor to ceiling, four-sided and barely tapered - no
/// tiers, because a tier is a division and this district's whole statement is
/// that there are none.
#[must_use]
pub fn hall_junction_3way_monolith() -> String {
    hall_junction_3way_themed(
        "hall_junction_3way_monolith",
        "monolith",
        2,
        &[(
            vec![41.0, 38.0, 41.0, 38.0],
            12.0,
            FLOOR_TOP,
            LEVEL - FLOOR_TOP,
            6.0,
        )],
        "Junction, Monolith: one undivided block, floor to ceiling.",
    )
}

/// Facet Monument: "flutes that catch the key light on every stop". Twelve
/// narrow faces on a tall shaft, alternating in and out by three units, so a
/// single moving key rakes across a dozen highlights instead of one. Tall and
/// slim rather than heavy - a monument is read at distance.
#[must_use]
pub fn hall_junction_3way_facet() -> String {
    let flutes: Vec<f64> = (0..12)
        .map(|index| if index % 2 == 0 { 30.0 } else { 27.0 })
        .collect();
    let upper: Vec<f64> = (0..12)
        .map(|index| if index % 2 == 0 { 24.0 } else { 21.5 })
        .collect();
    hall_junction_3way_themed(
        "hall_junction_3way_facet",
        "facet_monument",
        3,
        &[
            (flutes, 0.0, FLOOR_TOP, 84.0, 3.0),
            (upper, 15.0, 84.0, LEVEL - FLOOR_TOP, 5.0),
        ],
        "Junction, Facet Monument: a twelve-flute shaft that rakes the key light.",
    )
}

/// Thinning: "almost nothing, by name". A stump - what is left of a mass rather
/// than a mass, knee-high on one side and shin-high on the other, so the
/// district reads as the one where the architecture has gone. Deliberately the
/// least of the three: this is a landmark by absence, and making it handsome
/// would be answering a different brief.
#[must_use]
pub fn hall_junction_3way_thinning() -> String {
    hall_junction_3way_themed(
        "hall_junction_3way_thinning",
        "thinning",
        4,
        &[
            (
                vec![34.0, 21.0, 29.0, 17.0, 31.0, 24.0],
                0.0,
                FLOOR_TOP,
                26.0,
                5.0,
            ),
            (
                vec![19.0, 12.0, 16.0, 10.0, 17.0, 13.0],
                37.0,
                26.0,
                41.0,
                4.0,
            ),
        ],
        "Junction, Thinning: a stump where the district's mass used to be.",
    )
}

/// How low a dropped soffit may hang, in editor units.
///
/// 56 is 3.5 m, against the 7.5 m every other cell in the corpus has. The floor
/// is `validate_floor_and_headroom`: a body needs 2.2 m and this leaves 3.0 m,
/// so it is compressed and not a crawl.
///
/// It sits **below** `DOOR_TOP`, which is the point. A 4.5 m doorway under a
/// 3.5 m ceiling is a wrong proportion, and wrongness is the whole feeling this
/// exists to produce - a place built to a plan that no longer matches what is
/// standing in it.
const SOFFIT_UNDERSIDE: f64 = 56.0;

/// How far from each door the soffit stops.
///
/// Not decoration. The aperture is a frozen contract and the soffit hangs below
/// `DOOR_TOP`, so a soffit run to the face plane would put mass across the top
/// of a doorway and break every neighbour that meets it. Stopping at 60 leaves
/// three metres of full-height reveal at each end, and a body walks from tall,
/// through low, back to tall - which reads far more strongly than a uniformly
/// low cell would.
const SOFFIT_HALF_RUN: f64 = 60.0;

/// A corridor with a dropped soffit: the first cell in the corpus that is not
/// 7.5 m tall.
///
/// # The gap this is aimed at
///
/// Ten districts, and every hall tile in all of them - generated and authored -
/// puts its ceiling at `LEVEL - FLOOR_TOP`. Ten call sites, one value. What a
/// district varies today is skirting height, column cross-section, how many
/// columns, whether the ceiling carries ribs, and hue. The **envelope** is a
/// constant: the same 14 m hexagon, 7.5 m to the ceiling, a 4.5 m doorway.
///
/// So a district cannot currently read as a *kind of space*. "Somewhere
/// endless and too low" and "somewhere vertical and vast" are differences in
/// section, and the corpus has no way to say either. This is the first tile
/// that uses the one dimension the seam contract leaves free.
///
/// # Why the ceiling is free and the rest is not
///
/// The aperture is fixed at `FLOOR_TOP..DOOR_TOP`, 8 to 72, and the ceiling
/// sits at 120 - forty-eight units of headroom above the tallest thing any
/// neighbour can see through. Nothing outside the cell can observe where the
/// ceiling is. Floor height, doorway and footprint are all load-bearing across
/// seams; ceiling height alone is a district's to spend.
///
/// Scoped to Infinite Gallery, whose stated vocabulary is "a gallery is a
/// rhythm: the most supports, the slimmest, repeating". A rhythm needs
/// something to beat against, and a low lid is what turns a run of pilasters
/// into a corridor you are travelling *along* rather than a room you are
/// standing in.
#[must_use]
pub fn hall_straight_soffit() -> String {
    let mut brushes = hall_shell(&[0, 3]);
    brushes.push_str("// Dropped soffit: 3.5 m over the middle, full reveal at both doors\n");
    brushes.push_str(&prism(
        &[
            (-SOFFIT_HALF_RUN, -DOOR_HALF_WIDTH),
            (SOFFIT_HALF_RUN, -DOOR_HALF_WIDTH),
            (SOFFIT_HALF_RUN, DOOR_HALF_WIDTH),
            (-SOFFIT_HALF_RUN, DOOR_HALF_WIDTH),
        ],
        SOFFIT_UNDERSIDE,
        LEVEL - FLOOR_TOP,
        None,
        0.0,
        4.0,
    ));

    brushes.push_str("// The rhythm the soffit beats against: slim pilasters, both walls\n");
    for step in 0..4 {
        #[allow(clippy::cast_precision_loss)]
        let x = -66.0 + f64::from(step) * 44.0;
        for side in [-1.0, 1.0] {
            let (near, far) = (DOOR_HALF_WIDTH * side, (DOOR_HALF_WIDTH - 7.0) * side);
            let plan: Vec<P2> = if side > 0.0 {
                vec![
                    (x - 5.0, far),
                    (x + 5.0, far),
                    (x + 5.0, near),
                    (x - 5.0, near),
                ]
            } else {
                vec![
                    (x - 5.0, near),
                    (x + 5.0, near),
                    (x + 5.0, far),
                    (x - 5.0, far),
                ]
            };
            brushes.push_str(&prism(&plan, FLOOR_TOP, SOFFIT_UNDERSIDE, None, 2.0, 0.0));
        }
    }

    let mut lights = String::new();
    // Under the soffit, not above it: a low lid you can see the fittings on is
    // lower than one you cannot.
    for x in [-30.0, 30.0] {
        let (fixture, source) = ceiling_fixture(x, 0.0, SOFFIT_UNDERSIDE, 12.0, 7.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = String::from(
        "// Straight hall, Infinite Gallery: a dropped soffit and a rhythm of pilasters.\n",
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_straight_soffit", "hall_straight", 3, 1, 9)
            .with_register_scope("infinite_gallery")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    out.push_str(&lateral_port(0, "door", "east_port", 0, 0, 0));
    out.push_str(&lateral_port(3, "door", "west_port", 0, 0, 0));
    out.push_str(&lights);
    out
}

/// How far in from the rim a gallery's walkway reaches.
///
/// The annulus between the wall and this radius is floor; inside it is a hole.
/// 66 units is 4.1 m of walk against a 7 m apothem - wide enough to pass on,
/// narrow enough that the void is the subject.
const GALLERY_INNER: f64 = 66.0;

/// A gallery: a walkway round an open middle, with no ceiling over the void.
///
/// # What composing by hand revealed
///
/// Stacking four storeys of `expanse` by hand produced four separate plates.
/// Every cell in the corpus carries a floor slab *and* a ceiling slab, so a
/// stack of them is layered, not continuous - you cannot see up through it, and
/// a vertical space you cannot see through is just several rooms.
///
/// A silo, a Babel gallery and a BLAME! shaft are all the same claim: **one
/// volume that several storeys look into**. That needs a cell whose floor is a
/// ring and whose middle is absent, top and bottom. This is the first one.
///
/// # The seam is untouched, which is what makes it composable
///
/// Doors sit on the walkway at exactly the authored aperture, so a gallery
/// mates with an ordinary hall on the usual terms and a body walks in at floor
/// level. Only the *middle* of the cell is missing, and the middle is not
/// something a neighbour can see.
///
/// The footprint declares `floor="open"` for the reason `silo_ring` does: the
/// walkable surface is a ring rather than a slab under the cell origin, and the
/// floor probe would otherwise look straight down the hole and call the cell
/// bottomless.
///
/// A parapet stands at the lip. It is not decoration - without it the first
/// thing that happens on a gallery is a body walking off the inside edge.
#[must_use]
/// Everything one gallery differs from another by.
///
/// Four tiles come out of one function because the differences between them
/// are data, not code: the Index's rail is low, the Unwitnessed's walkway is
/// missing two of its wedges, the Well's has a landing over the drop. Writing
/// four builders would have made those three facts look unrelated.
struct GallerySpec<'a> {
    name: &'a str,
    register: &'a str,
    /// Distinguishes galleries that share an archetype *and* a register. The
    /// Unwitnessed has two, and so does the Well; without this they collide at
    /// variant 0 and a composition gets whichever one resolution happens to
    /// find first.
    variant: i32,
    doors: &'a [usize],
    /// Which of the six floor wedges exist. A missing wedge leaves the walkway
    /// ending in a raw edge over the void.
    wedges: &'a [usize],
    /// Height of the parapet above the walkway. Borges' "very low railing" is
    /// the whole of the Index's vertigo, and it is one number.
    parapet: f64,
    /// Faces whose wedge extends inward as a landing over the void.
    cantilever: &'a [usize],
}

impl<'a> GallerySpec<'a> {
    const fn plain(name: &'a str, register: &'a str, doors: &'a [usize]) -> Self {
        Self {
            name,
            register,
            variant: 0,
            doors,
            wedges: &[0, 1, 2, 3, 4, 5],
            parapet: 20.0,
            cantilever: &[],
        }
    }
}

fn hall_gallery(spec: &GallerySpec<'_>) -> String {
    let hex: Vec<P2> = corners().to_vec();
    let inner: Vec<P2> = (0..6)
        .map(|index| {
            #[allow(clippy::cast_precision_loss)]
            let angle = (30.0 + f64::from(index) * 60.0).to_radians();
            (GALLERY_INNER * angle.cos(), GALLERY_INNER * angle.sin())
        })
        .collect();

    let mut brushes = String::from("// Walkway ring: floor everywhere but the middle\n");
    for &index in spec.wedges {
        let (a, b) = (hex[index], hex[(index + 1) % 6]);
        let (c, d) = (inner[(index + 1) % 6], inner[index]);
        brushes.push_str(&prism(&[a, b, c, d], 0.0, FLOOR_TOP, None, 2.0, 0.0));
    }
    brushes.push_str("// Landings out over the drop - the good addresses\n");
    for &face in spec.cantilever {
        let (a, b) = (inner[face], inner[(face + 1) % 6]);
        let reach = |p: P2| (p.0 * 0.32, p.1 * 0.32);
        brushes.push_str(&prism(
            &[a, b, reach(b), reach(a)],
            0.0,
            FLOOR_TOP,
            None,
            2.0,
            0.0,
        ));
    }
    if spec.parapet > 0.0 {
        brushes.push_str("// Parapet at the lip - the reason a body stays on the walkway\n");
        for index in 0..6 {
            // A missing wedge leaves nothing to stand on, so it gets no rail:
            // the edge is meant to be raw.
            if !spec.wedges.contains(&index) {
                continue;
            }
            let (a, b) = (inner[index], inner[(index + 1) % 6]);
            let shrink = |p: P2| (p.0 * 0.90, p.1 * 0.90);
            brushes.push_str(&prism(
                &[a, b, shrink(b), shrink(a)],
                FLOOR_TOP,
                FLOOR_TOP + spec.parapet,
                None,
                2.0,
                0.0,
            ));
        }
    }
    brushes.push_str("// Envelope: doors where the gallery is entered, wall elsewhere\n");
    for face in 0..6 {
        if spec.doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 10.0, 8.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }

    let mut lights = String::new();
    for (face, along, z) in [(1usize, 0.5, 96.0), (4, 0.5, 96.0)] {
        let (fixture, source) = wall_fixture(face, along, z, 18.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = String::from(
        "// Gallery: a walkway round an open middle, so a stack of them is one volume.\n",
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            &format!("authored/{}", spec.name),
            "hall_gallery",
            spec.variant,
            1,
            8,
        )
        .with_register_scope(spec.register)
        .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 1, "open"));
    for &face in spec.doors {
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
    out.push_str(&lights);
    out
}

/// The Megastructure gallery: two opposed doors, so a run of them rings a shaft.
#[must_use]
pub fn hall_gallery_megastructure() -> String {
    hall_gallery(&GallerySpec::plain(
        "hall_gallery_megastructure",
        "megastructure",
        &[0, 3],
    ))
}

/// The Wellshaft gallery: the same walkway, in the register that is *about* a
/// well. Two doors, because a silo is entered on one side and left on the other.
#[must_use]
pub fn hall_gallery_wellshaft() -> String {
    hall_gallery(&GallerySpec::plain(
        "hall_gallery_wellshaft",
        "wellshaft",
        &[0, 3],
    ))
}

/// The Infinite Gallery gallery: doors on every other face, and a rail so low
/// it is more of an insult than a precaution.
///
/// Babel's cell is not a corridor with a hole in it - it is a landing that
/// every neighbouring landing can be reached from, and the identical landing
/// above and below. Three doors is what makes a storey read as a *floor of the
/// library* rather than a link in a route.
///
/// The rail was 20 units and is now 8. Borges specifies "very low", and low is
/// the word doing the work: the Index is a district that believed writing
/// things down was safety, and it guarded its shafts accordingly.
#[must_use]
pub fn hall_gallery_infinite() -> String {
    hall_gallery(&GallerySpec {
        parapet: 8.0,
        ..GallerySpec::plain("hall_gallery_infinite", "infinite_gallery", &[0, 2, 4])
    })
}

/// The Unwitnessed gallery: two wedges of the walkway are simply not there.
///
/// Not broken - *finished*, the way a sentence stops when the speaker loses
/// interest. The composition that staggers shaft heights already reads as
/// incoherent; this is what makes it read as having been left rather than as
/// having gone wrong. The missing wedges get no parapet, because a rail on an
/// absent floor is a different and much sadder building.
#[must_use]
pub fn hall_gallery_broken() -> String {
    hall_gallery(&GallerySpec {
        variant: 1,
        wedges: &[0, 1, 2, 3],
        ..GallerySpec::plain("hall_gallery_broken", "megastructure", &[0, 3])
    })
}

/// The Well's good address: a landing projecting out over the drop.
///
/// Every design decision in the Well is about sightlines between people, and
/// this is the one that says so out loud - a platform whose only purpose is to
/// put a body where the most levels can see it, and where it can see the most
/// levels.
#[must_use]
pub fn hall_gallery_cantilever() -> String {
    hall_gallery(&GallerySpec {
        variant: 1,
        cantilever: &[1],
        ..GallerySpec::plain("hall_gallery_cantilever", "wellshaft", &[0, 3])
    })
}

/// How wide the Noon's dropped lid is, either side of the corridor axis.
///
/// Eight units narrower than the aperture, so a slot of light runs the whole
/// length of both walls. The first attempt made the lid a centred hexagon on
/// the theory that a turn has no single run - but `hall_shell` fills every
/// undoored sector solid, so there is no "round" for a slot to go round, and
/// the lid was buried in mass. A corridor's soffit follows the corridor.
const SOFFIT_HALF_WIDTH: f64 = 28.0;

/// How far short of the face the lid stops, so the aperture keeps full height.
const SOFFIT_REVEAL: f64 = 18.0;

fn soffit_arm(face: usize) -> String {
    let mid = face_mid(face);
    let length = mid.0.hypot(mid.1);
    let u = (mid.0 / length, mid.1 / length);
    let perp = (-u.1, u.0);
    let reach = length - SOFFIT_REVEAL;
    let point = |along: f64, side: f64| {
        (
            u.0 * along + perp.0 * side * SOFFIT_HALF_WIDTH,
            u.1 * along + perp.1 * side * SOFFIT_HALF_WIDTH,
        )
    };
    prism(
        &[
            point(-12.0, 1.0),
            point(reach, 1.0),
            point(reach, -1.0),
            point(-12.0, -1.0),
        ],
        SOFFIT_UNDERSIDE,
        LEVEL - FLOOR_TOP,
        None,
        0.0,
        4.0,
    )
}

/// The archetype a two-door turn *is*, so the solver can ask for it.
///
/// A district's dialect of an existing shape is a **variant** of that shape,
/// not a new one. `placement_tile_archetype` is a closed match on eight names;
/// anything outside it can be authored, validated, committed and shipped and
/// will still never be placed. `hall_straight_soffit` got this right by
/// accident - it is `hall_straight` variant 3 and has been placeable all along
/// - and these two got it wrong, which is most of what the archetype sweep is.
fn turn_archetype(second_face: usize) -> &'static str {
    if second_face == 5 {
        "hall_turn_60"
    } else {
        "hall_turn_120"
    }
}

fn hall_turn_soffit(name: &str, second_face: usize) -> String {
    let mut brushes = hall_shell(&[0, second_face]);
    brushes.push_str("// The lid, dropped and narrower than the hall it covers\n");
    for face in [0usize, second_face] {
        brushes.push_str(&soffit_arm(face));
    }

    let mut lights = String::new();
    // Under the lid rather than above it. A Noon cell has no visible fitting
    // from the middle of the floor; you find them by looking up into the slot,
    // and by then you have already stopped being able to say which way you
    // came in.
    for face in [0usize, second_face] {
        let mid = face_mid(face);
        let (fixture, source) =
            ceiling_fixture(mid.0 * 0.52, mid.1 * 0.52, SOFFIT_UNDERSIDE, 11.0, 11.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out =
        String::from("// Turn, Overlit Grid: a dropped lid with a slot of light either side.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            &format!("authored/{name}"),
            turn_archetype(second_face),
            1,
            1,
            8,
        )
        .with_register_scope("overlit_grid")
        .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in [0usize, second_face] {
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
    out.push_str(&lights);
    out
}

/// The Noon's 60-degree turn.
#[must_use]
pub fn hall_turn_60_soffit() -> String {
    hall_turn_soffit("hall_turn_60_soffit", 5)
}

/// The Noon's 120-degree turn.
#[must_use]
pub fn hall_turn_120_soffit() -> String {
    hall_turn_soffit("hall_turn_120_soffit", 4)
}

/// The Back's opening: the wall simply stops.
///
/// `door_wall` with zero splay and zero lintel bevel. Every other tile in the
/// corpus reveals its apertures - a splayed jamb, a chamfered head - because a
/// framed opening reads as *intended*. Service space was never drawn by
/// anybody, so nothing about it was intended, and the absence of a frame is the
/// only thing that says so. It costs nothing: two numbers set to nought.
#[must_use]
pub fn hall_straight_threshold() -> String {
    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));
    brushes.push_str("// Envelope: unframed apertures east and west\n");
    for face in 0..6 {
        if face == 0 || face == 3 {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 0.0, 0.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }
    let mut lights = String::new();
    for x in [-46.0, 46.0] {
        let (fixture, source) = ceiling_fixture(x, 0.0, LEVEL - FLOOR_TOP, 16.0, 9.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = String::from("// Straight hall, Liminal Grid: openings with no frame at all.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            "authored/hall_straight_threshold",
            "hall_straight",
            4,
            1,
            10,
        )
        .with_register_scope("liminal_grid")
        .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in [0usize, 3] {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out.push_str(&lights);
    out
}

/// How far the Thin's floor rises across a threshold: one step, half a metre.
///
/// A hand's height is what the fiction asks for and half a metre is what a body
/// can climb. Eight units is also exactly the floor slab's own thickness, so
/// the raised half reads as *one more floor laid on top* rather than as a
/// plinth - which is what a raised timber deck actually is.
const STEP_RISE: f64 = 8.0;

/// The Thin's divider: a change of floor level, and nothing else.
///
/// Every other tile separates space with a wall. This one separates it with
/// half a metre, and the separation is entirely in the body: you cannot cross
/// without your knee knowing about it. The Thin's builders had watched walls
/// fail - a wall is mostly a thing that can be moved - and this is what they
/// used instead.
#[must_use]
pub fn hall_step_platform() -> String {
    let mut brushes = hall_shell(&[0, 3]);
    brushes.push_str("// The raised half. The threshold is the step, not a door\n");
    let hex = corners();
    brushes.push_str(&prism(
        &[(0.0, hex[5].1), hex[0], hex[1], (0.0, hex[2].1)],
        FLOOR_TOP,
        FLOOR_TOP + STEP_RISE,
        None,
        2.0,
        0.0,
    ));

    let mut lights = String::new();
    for face in [0usize, 3] {
        let mid = face_mid(face);
        let (fixture, source) =
            ceiling_fixture(mid.0 * 0.5, mid.1 * 0.5, LEVEL - FLOOR_TOP, 12.0, 8.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out =
        String::from("// Straight hall, Thinning: the floor steps, and that is the wall.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_step_platform", "hall_straight", 5, 1, 6)
            .with_register_scope("thinning")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in [0usize, 3] {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out.push_str(&lights);
    out
}

/// How far from the cell centre the dais stands, toward the sealed end.
const DAIS_OFFSET: f64 = -46.0;

/// The Welcome's terminus: a stepped dais with nothing on it.
///
/// One door, because the Welcome was never built to be travelled - it was built
/// to be arrived at. Three courses rising to a flat top, and the top is empty:
/// not "nothing left on it", nothing was ever on it. The dais is a place to be
/// seen standing, by a guest who did not come.
#[must_use]
pub fn hall_dais_monument() -> String {
    // An envelope, not a channel. `hall_shell` fills every undoored sector
    // solid, which makes a one-door cell a corridor stub - and the Welcome's
    // terminus is not a stub, it is a chamber. A perimeter wall with one
    // aperture leaves the whole hex open, which is what you have come into.
    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));
    brushes.push_str("// Envelope: one aperture, five faces of wall\n");
    for face in 0..6 {
        if face == 0 {
            brushes.push_str(&door_wall(
                face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 14.0, 10.0,
            ));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }
    // Three courses at the far end, not in the middle. A dais over the cell
    // centre reads to the contract as a low ceiling above the floor - which is
    // exactly what it is, from the doorway - and it is the wrong building
    // anyway: you enter at one end and the thing you have come to see stands at
    // the other. Being made to cross the room is most of the ceremony.
    brushes.push_str("// Three courses, each stepped back from the one below\n");
    for (step, radius) in [(0.0, 56.0), (1.0, 42.0), (2.0, 28.0)] {
        let z0 = FLOOR_TOP + step * STEP_RISE;
        brushes.push_str(&translate(
            &prism(
                &regular_polygon(radius, 6, 30.0),
                z0,
                z0 + STEP_RISE,
                Some((0.0, 0.0)),
                2.0,
                0.0,
            ),
            DAIS_OFFSET,
            0.0,
            0.0,
        ));
    }

    let mut lights = String::new();
    // Four fittings low on the walls, aimed across the dais rather than at it.
    // The Welcome does not light its objects; it lights the space an object
    // would occupy.
    for face in [1usize, 2, 4, 5] {
        let (fixture, source) = wall_fixture(face, 0.5, 84.0, 20.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out =
        String::from("// Cap, Facet Monument: the axis arrives at a dais with nothing on it.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_dais_monument", "hall_dais", 0, 1, 8)
            .with_register_scope("facet_monument")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    out.push_str(&lateral_port(0, "door", "east_port", 0, 0, 0));
    out.push_str(&lights);
    out
}

// ---------------------------------------------------------------------------
// The Vestry - one room, built the way a Quake map is built.
// ---------------------------------------------------------------------------
//
// Every other tile in this file solves a district. This one solves a *room*,
// and it follows the rules you learn making deathmatch maps rather than the
// ones in the seam contract:
//
//   1. Everything lands on the grid. Eight units, no exceptions.
//   2. Trim every transition, and never let two planes meet flush - set the
//      course proud by four so the light catches an edge and drops a shadow
//      line under it. Rooms are legible because of their shadow lines.
//   3. **One idea, at full size.** The first cut of this room had a ledge on
//      one wall, a short ramp and a crate, and it was correct and boring: trim
//      everywhere and nothing happening. A room with three small gestures is
//      worse than a room with one large one.
//   4. Light where the geometry already is. Three sources, all of them tucked
//      under something. No lamp floats.
//   5. Cover breaks the diagonal. Two doors facing each other down the long
//      axis is a free shot from threshold to threshold, so a block stands
//      *off* the axis to spoil it - off, not centred, because centred cover
//      makes a symmetric room and symmetric rooms are dull to fight in.
//
// The idea, at full size: **the room is a spiral.** A ramp climbs the
// south-east wall, a second ramp continues from it up the south-west wall, and
// what they arrive at is a gallery that runs the remaining three walls at five
// metres - passing clean over the west doorway on the way. You end up standing
// above the door you came in by, having walked the entire perimeter to get
// there, with the whole floor under you and one way down.

/// Where the gallery runs, and where the second ramp has to reach.
///
/// Above `DOOR_TOP`, which is the entire reason the number is what it is: a
/// gallery below the door head has to stop at every doorway, and a gallery
/// that stops at every doorway is three shelves rather than one circuit.
const VESTRY_GALLERY: f64 = 80.0;
/// Height of the ramps' shared landing, halfway up.
const VESTRY_HALF: f64 = 40.0;
/// How far the gallery cuts into the room.
const VESTRY_DEPTH: f64 = 44.0;
/// Thickness of every proud course. Four units is one shadow line.
const VESTRY_PROUD: f64 = 4.0;

/// One ramp of the spiral, climbing the inside of `face` from `rise0` to
/// `rise1` in the direction the faces are numbered.
fn vestry_ramp(face: usize, rise0: f64, rise1: f64) -> String {
    let (a, b) = edge(face);
    let (foot, head) = offset_inward(a, b, WALL);
    let (inner_foot, _) = offset_inward(a, b, WALL + VESTRY_DEPTH);
    let (_, inner_head) = offset_inward(a, b, WALL + VESTRY_DEPTH);
    sloped_prism(
        &[foot, head, inner_head, inner_foot],
        0.0,
        [
            (foot.0, foot.1, rise0),
            (head.0, head.1, rise1),
            (inner_foot.0, inner_foot.1, rise0),
        ],
        None,
    )
}

/// A room that is a spiral.
///
/// Monolith, because the register is "one mass, undivided, the fewest supports
/// of any district and the heaviest" - which is the brief a pier-and-lintel
/// room was going to be built to anyway.
///
/// # The walk
///
/// You come in the east door onto open floor, with a block off to one side
/// spoiling the shot straight through to the west door. The first ramp is
/// immediately on your right and climbing it turns your back on the way you
/// came in - that is what the height costs, charged in the only currency a room
/// has. It lands halfway up and hands you to a second ramp, which puts you on
/// the gallery. The gallery crosses *over* the west doorway, so anyone leaving
/// underneath you never looks up, and it dead-ends five metres above the door
/// you entered by. There is no second way down. Everything about the room is
/// arranged so that going up is a decision.
#[must_use]
pub fn hall_arena_monolith() -> String {
    let doors = [0usize, 3];
    let gallery_faces = [3usize, 4, 5];

    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));

    brushes.push_str("// Envelope\n");
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 12.0, 8.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }

    brushes.push_str("// The spiral: two ramps, south-east then south-west\n");
    brushes.push_str(&vestry_ramp(1, FLOOR_TOP, VESTRY_HALF + FLOOR_TOP));
    brushes.push_str(&vestry_ramp(
        2,
        VESTRY_HALF + FLOOR_TOP,
        VESTRY_GALLERY + FLOOR_TOP,
    ));

    brushes.push_str("// The gallery, over the west door and round to the east\n");
    for &face in &gallery_faces {
        // Deck.
        brushes.push_str(&band(
            face,
            WALL,
            WALL + VESTRY_DEPTH,
            VESTRY_GALLERY,
            VESTRY_GALLERY + FLOOR_TOP,
        ));
        // Nosing. This is the difference between a gallery and a slab stuck to
        // a wall: four units of shadow under the lip, and the deck reads as
        // carried rather than glued.
        brushes.push_str(&band(
            face,
            WALL + VESTRY_DEPTH - VESTRY_PROUD,
            WALL + VESTRY_DEPTH,
            VESTRY_GALLERY - VESTRY_PROUD,
            VESTRY_GALLERY,
        ));
        // Parapet. Waist high from up there, a lip that hides a crouched body
        // from down here.
        brushes.push_str(&band(
            face,
            WALL + VESTRY_DEPTH - WALL,
            WALL + VESTRY_DEPTH,
            VESTRY_GALLERY + FLOOR_TOP,
            VESTRY_GALLERY + FLOOR_TOP + 20.0,
        ));
    }

    brushes.push_str("// Base course, on the two walls that have a base to course\n");
    for face in [4usize, 5] {
        brushes.push_str(&band(
            face,
            WALL,
            WALL + VESTRY_PROUD,
            FLOOR_TOP,
            FLOOR_TOP + 24.0,
        ));
    }

    brushes.push_str("// Two piers under the gallery's far side. Fewest, heaviest\n");
    let mid = face_mid(4);
    let reach = mid.0.hypot(mid.1);
    let normal = (mid.0 / reach, mid.1 / reach);
    let tangent = (-normal.1, normal.0);
    let stand = reach - VESTRY_DEPTH + 8.0;
    for side in [-1.0, 1.0] {
        brushes.push_str(&translate(
            &pylon(16.0, FLOOR_TOP, VESTRY_GALLERY, 0.0, 3.0, 0.0),
            normal.0 * stand + tangent.0 * side * 44.0,
            normal.1 * stand + tangent.1 * side * 44.0,
            0.0,
        ));
    }

    brushes.push_str("// Cover, off the axis between the two doors\n");
    brushes.push_str(&translate(
        &pylon(32.0, FLOOR_TOP, FLOOR_TOP + 24.0, 0.0, 3.0, 0.0),
        -16.0,
        -56.0,
        0.0,
    ));

    let mut lights = String::new();
    for face in [4usize, 5] {
        let m = face_mid(face);
        let r = m.0.hypot(m.1);
        let (fixture, source) = ceiling_fixture(
            m.0 / r * (r - 24.0),
            m.1 / r * (r - 24.0),
            VESTRY_GALLERY,
            14.0,
            14.0,
        );
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }
    let (high, high_source) = ceiling_fixture(0.0, 0.0, LEVEL - FLOOR_TOP, 22.0, 14.0);
    brushes.push_str(&high);
    lights.push_str(&high_source);

    let mut out = String::from("// The Vestry, Monolith: a room that is a spiral.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_arena_monolith", "hall_straight", 9, 1, 2)
            .with_register_scope("monolith")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out.push_str(&lights);
    out
}

// ---------------------------------------------------------------------------
// The Squint - a doorway that is only there from one place.
// ---------------------------------------------------------------------------
//
// Three blocks hang in this room at three different distances, at three
// different sizes, apparently unrelated. Stand on the plate just inside the
// east door and they line up into the outline of a doorway: two posts and a
// lintel, framing the far wall.
//
// Take one step off the plate and it falls apart again.
//
// Anamorphosis is a four-hundred-year-old trick - Holbein's skull, Borromini's
// colonnade at the Palazzo Spada - and it has never had a better home than a
// facility that only holds its shape where somebody is looking. The room
// contains a door that exists exactly as long as it is observed from the one
// place it can be observed from, which is the game's whole premise stated as
// masonry.
//
// The maths is one line, applied three times. A piece meant to read at image
// rectangle `(y0..y1, z0..z1)` at reference distance `D` from eye `E`, placed
// at depth `d`, is that rectangle scaled by `d / D` about the eye. Nearer
// pieces are proportionally smaller, so all three subtend the same angles and
// the silhouette closes. Everything else about the room is ordinary.

/// Where a body must stand for the doorway to be there. Just inside the east
/// aperture, on the plate.
const SQUINT_EYE: (f64, f64, f64) = (84.0, 0.0, 34.0);
/// Reference distance the portal is described at.
const SQUINT_REFERENCE: f64 = 190.0;

/// One block of the illusion.
///
/// `y0..y1` and `z0..z1` describe the piece as it should *appear*, in the plane
/// at `SQUINT_REFERENCE`; `depth` is how far from the eye it actually sits.
fn squint_piece(depth: f64, y0: f64, y1: f64, z0: f64, z1: f64, thick: f64) -> String {
    let (ex, ey, ez) = SQUINT_EYE;
    let scale = depth / SQUINT_REFERENCE;
    let x = ex - depth;
    boxed(
        (x - thick * 0.5, ey + y0 * scale, ez + (z0 - ez) * scale),
        (x + thick * 0.5, ey + y1 * scale, ez + (z1 - ez) * scale),
    )
}

/// A room with a doorway in it that is not there.
///
/// Shadow Screen, because the register is thin members and what is between you
/// and the light, and this room is entirely about what happens to be in front
/// of what.
///
/// # The walk
///
/// Almost nobody will ever see this. It is a through-hall: east door, west
/// door, three odd blocks and a floor plate, and a body crossing it at speed
/// registers a slightly cluttered room and keeps going. The plate is the only
/// invitation, and it is the sort of invitation you have to already be the kind
/// of person who accepts.
///
/// That is deliberate. A secret that announces itself is a landmark; this one
/// is a reward for the specific act the facility runs on, which is stopping and
/// looking properly at something.
#[must_use]
/// The Welcome's gate: the axis passes under something, not merely through it.
///
/// The geometry plan has been asking for this one since the district was
/// written - "ceremony is entirely about how you pass through a threshold" -
/// and it is buildable without touching the seam, because everything it adds
/// sits *inboard* of the face plane. The doorway the solver sees is unchanged;
/// what changes is that you walk under a lintel to reach it.
///
/// # Why the piers step instead of tapering
///
/// The first sketch of this leaned the piers, which is what Forerunner
/// architecture looks like from memory. Reference frames say otherwise: the
/// legs batter outward at the floor and step *in courses* as they rise, three
/// to a leg, each narrower and shallower than the one below. A smooth taper
/// reads as a buttress; a stepped one reads as a thing assembled by somebody
/// with an opinion about permanence. It is also the only version a brush can
/// build, which is a happy coincidence rather than the reason.
///
/// Scale reads from the number of steps and from nothing else - there is no
/// object in here of a known size, which is the district's whole claim.
pub fn hall_gate_monument() -> String {
    let doors = [0usize, 3];
    // Courses, base upward: (z0, z1, |y| inner, |y| outer, depth inboard from
    // the face plane at its shallowest and deepest).
    const COURSES: [(f64, f64, f64, f64, f64, f64); 3] = [
        (FLOOR_TOP, 46.0, 40.0, 70.0, 16.0, 36.0),
        (46.0, 84.0, 43.0, 66.0, 16.0, 30.0),
        (84.0, 112.0, 46.0, 62.0, 16.0, 25.0),
    ];
    /// The face plane. Everything here is measured back from it so the seam
    /// itself is never touched.
    const FACE: f64 = 112.0;

    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));

    brushes.push_str("// Envelope\n");
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 12.0, 8.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }

    // Faces 0 and 3 are the +x and -x faces, so a gate is built once in x and
    // mirrored. No rotation, and no chance of the two ends disagreeing.
    for sign in [1.0_f64, -1.0] {
        brushes.push_str("// A gate, inboard of the face plane\n");
        for (z0, z1, y_in, y_out, deep, shallow) in COURSES {
            let (x0, x1) = ((FACE - shallow) * sign, (FACE - deep) * sign);
            for side in [-1.0_f64, 1.0] {
                brushes.push_str(&boxed(
                    (x0.min(x1), (y_in * side).min(y_out * side), z0),
                    (x0.max(x1), (y_in * side).max(y_out * side), z1),
                ));
            }
        }
        // The lintel, clear of the door aperture by twenty-four units. A gate
        // whose head is inside the doorway is a doorway; a gate whose head is
        // above it is a gate.
        let (x0, x1) = ((FACE - 28.0) * sign, (FACE - 16.0) * sign);
        brushes.push_str(&boxed(
            (x0.min(x1), -58.0, DOOR_TOP + 24.0),
            (x0.max(x1), 58.0, DOOR_TOP + 44.0),
        ));
    }

    // One reveal course on the sealed faces, not the two the district would
    // like. With both, this cell lands on exactly thirty-six hulls - the budget
    // to the unit, with no room for anyone to ever add anything. The stepped
    // gate is the move the district is actually named for; a second dado is
    // not, so the dado is what gets cut.
    brushes.push_str("// Stepped reveal on the sealed faces\n");
    for face in [1usize, 2, 4, 5] {
        brushes.push_str(&band(face, WALL, WALL + 7.0, FLOOR_TOP, 44.0));
    }

    let mut out = String::from(
        "// Straight, Facet Monument: the axis passes under a gate rather than through a hole.\n",
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_gate_monument", "hall_straight", 0, 1, 6)
            .with_register_scope("facet_monument")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out
}

/// The Noon's pocket: a hall you cannot see the end of, without a single door.
///
/// The Overlit Grid's whole claim is that it is neither open nor closed. Every
/// sightline ends on a stub with a gap beside it, and the gap leads somewhere
/// identical - so a room that is entirely traversable still refuses to tell you
/// where you are. That is a *plan*, not a surface, which is why this one ports
/// out of the lab without needing anything from the material system.
///
/// # Why the stubs are offset rather than paired
///
/// Two baffles facing each other across the axis make a gate, and a gate is a
/// thing you can see through the middle of. Two offset from opposite walls make
/// a dogleg: the eye is stopped at both ends and the body still walks straight
/// past. The capsule never turns more than a few degrees; only the view does.
///
/// # The skirting is doing real work
///
/// `horizontal_surface` classifies each hull by its height extent - under
/// 0.75 m is floor, over 7.25 m is ceiling, everything between is wall - so a
/// four-unit plinth at the foot of each stub is the one part of it that renders
/// in the floor's material rather than the wall's. That dark line where a
/// partition meets the carpet is most of what makes a space read as an office
/// rather than as a box, and it is the only trim this register gets.
pub fn hall_pocket_overlit() -> String {
    let doors = [0usize, 3];
    /// Where the skirting stops. Anything below 0.75 m renders as floor, and
    /// twelve units is 0.75 m exactly.
    const SKIRT_TOP: f64 = 12.0;
    // (x0, x1, y0, y1) for each stub, in cell units. Two from one wall, two
    // from the other, none of them opposite anything.
    // A hexagon narrows as you go out along the axis: at |x| the furthest a
    // corner may sit is 64 + (112 - |x|) * 64 / 112, and the skirting adds two
    // units to every side of that. The outermost stub is the short one for
    // exactly that reason, not for composition.
    const STUBS: [(f64, f64, f64, f64); 4] = [
        (-52.0, -36.0, -88.0, -16.0),
        (-8.0, 8.0, 26.0, 100.0),
        (36.0, 52.0, -88.0, -20.0),
        (76.0, 92.0, 28.0, 68.0),
    ];

    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));

    brushes.push_str("// Envelope\n");
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 8.0, 4.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }

    brushes.push_str("// Stubs: full height, so they stop the eye rather than the body\n");
    for (x0, x1, y0, y1) in STUBS {
        brushes.push_str(&boxed((x0, y0, FLOOR_TOP), (x1, y1, LEVEL - FLOOR_TOP)));
    }
    brushes.push_str("// Skirting, which is the only thing here that reads as floor\n");
    for (x0, x1, y0, y1) in STUBS {
        brushes.push_str(&boxed(
            (x0 - 2.0, y0 - 2.0, FLOOR_TOP),
            (x1 + 2.0, y1 + 2.0, SKIRT_TOP),
        ));
    }

    let mut out = String::from(
        "// Straight, Overlit Grid: every sightline ends on a stub with a gap beside it.\n",
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_pocket_overlit", "hall_straight", 0, 1, 6)
            .with_register_scope("overlit_grid")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out
}

/// The Shadow Screen's wall: timber rails in front of lit paper.
///
/// This tile could not be authored a day ago and the reason is worth keeping,
/// because it was never an authoring problem. The district's identity is a
/// dark frame standing in front of a bright surface - but the shell picks a
/// material per hull by height, everything at mid height took the *wall*, and
/// Shadow Screen's wall is the lit one. Every rail would have glowed like the
/// paper it is supposed to divide.
///
/// Two changes elsewhere make it buildable. A thin, wide hull at mid height now
/// renders as floor rather than wall, so a rail across a screen comes out in
/// the district's near-black timber. And the register carries its own weave, so
/// the fine kumiko lattice is drawn into the wall's image instead of costing
/// forty brushes it does not have.
///
/// That split - horizontals modelled, the grid drawn - is the one the daydream
/// lab arrived at independently, where a hundred cells to a panel was far past
/// the point of modelling each bar and only the stiles that *cast* were built.
///
/// # Why there are no posts
///
/// A post is thin and tall, which is a wall by any reading of the classifier,
/// so it would come out lit. Vertical structure here is the weave's job. The
/// tile contributes only what runs horizontally, which is also - conveniently -
/// what a shoji screen's heavy members actually are.
pub fn hall_screen_shoji() -> String {
    let doors = [0usize, 3];
    /// Rails as (z0, z1). Every one is half a metre deep against an eight-metre
    /// run, and that ratio is exactly what earns it the floor's timber: a deck
    /// is thin and wide, a pier is not.
    const RAILS: [(f64, f64); 4] = [
        // The sill, at the foot of the paper.
        (FLOOR_TOP, FLOOR_TOP + 7.0),
        // The waist rail, where a hand goes.
        (44.0, 52.0),
        // The kamoi: the head of the screen, and the heaviest line in the room.
        (DOOR_TOP + 5.0, DOOR_TOP + 14.0),
        // The top of the transom, just under the lid.
        (106.0, 114.0),
    ];

    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));

    brushes.push_str("// Envelope\n");
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 10.0, 6.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }

    // Only the sealed faces are screened. A rail across a doorway is a barrier,
    // and the seam is not this tile's to argue with.
    brushes.push_str("// Rails, proud of the paper\n");
    for face in [1usize, 2, 4, 5] {
        for (z0, z1) in RAILS {
            brushes.push_str(&band(face, WALL, WALL + 6.0, z0, z1));
        }
    }

    let mut out = String::from(
        "// Straight, Shadow Screen: horizontal timber, and the grid is in the surface.\n",
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_screen_shoji", "hall_straight", 0, 1, 6)
            .with_register_scope("shadow_screen")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out
}

/// The Welcome between its gates: a bay, so a run of cells reads as one axis.
///
/// This tile exists because of what a multi-cell composition turned out to
/// cost. Rooms that span several hexes go through `RoomBlueprint`, keyed by a
/// closed `RoomRole` enum in the solver - Start, Exit, Keystone, Monitor,
/// Recovery - which is a vocabulary of *gameplay* roles. "An eleven-cell
/// ceremonial axis" is not one of those and putting it there would be a
/// category error, quite apart from moving the simulation hash and needing
/// placement logic of its own.
///
/// The wave function collapse already does this, and does it better. Register
/// scope confines a tile to its district, so a straight run through a Facet
/// Monument district draws only Facet Monument tiles - and a held axis is what
/// you get when consecutive cells continue each other rather than each
/// announcing itself. [`hall_gate_monument`] announces itself, which is right
/// once and wrong four times in a row. This is what belongs between them.
///
/// # What continues, and what does not
///
/// The reveals run at the same two heights as the gate's, so a wall crosses a
/// seam without a step in it. The coffers overhead are the only thing this bay
/// adds, and they are deliberately at three spacings rather than one: an axis
/// that repeats exactly reads as a corridor tiled by a machine, which is what
/// it is, and the whole business of the district is to hide that.
pub fn hall_bay_monument() -> String {
    let doors = [0usize, 3];
    // Coffers across the axis, at (x0, x1, |y|). The hexagon narrows as you go
    // out along x, so the outer pair are shorter - which also happens to be the
    // spacing irregularity the bay wants.
    const COFFERS: [(f64, f64, f64); 3] =
        [(-58.0, -42.0, 88.0), (-8.0, 8.0, 104.0), (44.0, 60.0, 86.0)];

    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));

    brushes.push_str("// Envelope\n");
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 12.0, 8.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }

    // The same two courses the gate carries, at the same heights, so the wall
    // does not step where two cells meet.
    brushes.push_str("// Stepped reveals, continuous with the gate's\n");
    for face in [1usize, 2, 4, 5] {
        brushes.push_str(&band(face, WALL, WALL + 7.0, FLOOR_TOP, 44.0));
        brushes.push_str(&band(face, WALL, WALL + 4.0, 56.0, 82.0));
    }

    brushes.push_str("// Coffers, clear of the doorway by a level's quarter\n");
    for (x0, x1, reach) in COFFERS {
        brushes.push_str(&boxed(
            (x0, -reach, DOOR_TOP + 28.0),
            (x1, reach, DOOR_TOP + 44.0),
        ));
    }

    let mut out = String::from(
        "// Straight, Facet Monument: the bay that lets an axis be longer than a cell.\n",
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_bay_monument", "hall_straight", 0, 1, 9)
            .with_register_scope("facet_monument")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out
}

/// The Welcome's elbow: an axis that turns without breaking.
///
/// A family is only a family if its members agree at the seam. The gate and
/// the bay carry two reveal courses at the same two heights for exactly that
/// reason, and a turn that dropped them would put a step in the wall at the
/// one cell where the eye is already being asked to follow a corner.
pub fn hall_turn_monument() -> String {
    let mut brushes = hall_shell(&[0, 5]);
    brushes.push_str("// The family's two courses, carried round the elbow\n");
    for face in [1usize, 2, 3, 4] {
        brushes.push_str(&band(face, WALL, WALL + 7.0, FLOOR_TOP, 44.0));
        brushes.push_str(&band(face, WALL, WALL + 4.0, 56.0, 82.0));
    }
    // One coffer, set across the corner rather than along either leg, so the
    // turn is a place rather than a join between two corridors.
    brushes.push_str("// A coffer over the elbow\n");
    brushes.push_str(&boxed(
        (-46.0, -46.0, DOOR_TOP + 30.0),
        (46.0, 46.0, DOOR_TOP + 44.0),
    ));

    let mut out =
        String::from("// Turn, Facet Monument: the axis changes direction and nothing else.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_turn_monument", "hall_turn_60", 0, 1, 7)
            .with_register_scope("facet_monument")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in [0usize, 5] {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out
}

/// The Shadow Screen's elbow, with its rails carried round.
///
/// Same argument as [`hall_turn_monument`] and a sharper consequence: this
/// district's rails are the *only* horizontal it has, because its vertical
/// structure lives in the register's weave rather than in brushes. Drop them
/// at a corner and the corner is bare paper.
pub fn hall_turn_shoji() -> String {
    let mut brushes = hall_shell(&[0, 5]);
    brushes.push_str("// The screen's rails, carried round the elbow\n");
    for face in [1usize, 2, 3, 4] {
        for (z0, z1) in [
            (FLOOR_TOP, FLOOR_TOP + 7.0),
            (44.0, 52.0),
            (DOOR_TOP + 5.0, DOOR_TOP + 14.0),
            (106.0, 114.0),
        ] {
            brushes.push_str(&band(face, WALL, WALL + 6.0, z0, z1));
        }
    }

    let mut out =
        String::from("// Turn, Shadow Screen: the rails continue, because nothing else would.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_turn_shoji", "hall_turn_60", 0, 1, 7)
            .with_register_scope("shadow_screen")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in [0usize, 5] {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out
}

/// A second arrangement of the Noon's stubs.
///
/// One pocket tile is a pocket; a run of the same pocket tile is wallpaper,
/// and this district's whole claim is that you cannot tell where you are.
/// Repeating an identical plan is the one thing that would let you.
pub fn hall_pocket_overlit_b() -> String {
    let doors = [0usize, 3];
    const SKIRT_TOP: f64 = 12.0;
    // Mirrored and re-spaced against the first pocket rather than reshuffled,
    // so the two read as the same building rather than two ideas about it.
    const STUBS: [(f64, f64, f64, f64); 4] = [
        (-88.0, -72.0, -30.0, 34.0),
        (-30.0, -14.0, -96.0, -22.0),
        (18.0, 34.0, 24.0, 98.0),
        (66.0, 82.0, -70.0, -24.0),
    ];

    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));
    brushes.push_str("// Envelope\n");
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 8.0, 4.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }
    brushes.push_str("// Stubs\n");
    for (x0, x1, y0, y1) in STUBS {
        brushes.push_str(&boxed((x0, y0, FLOOR_TOP), (x1, y1, LEVEL - FLOOR_TOP)));
    }
    brushes.push_str("// Skirting\n");
    for (x0, x1, y0, y1) in STUBS {
        brushes.push_str(&boxed(
            (x0 - 2.0, y0 - 2.0, FLOOR_TOP),
            (x1 + 2.0, y1 + 2.0, SKIRT_TOP),
        ));
    }

    let mut out =
        String::from("// Straight, Overlit Grid: the same room again, arranged differently.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_pocket_overlit_b", "hall_straight", 0, 1, 6)
            .with_register_scope("overlit_grid")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out
}

/// The Well's landing: the first thing in this facility that is a deck.
///
/// Nothing here had a walkway, and the reason was never authoring. A hull took
/// its material from how high it sat, so anything at waist height rendered as
/// wall - which in a lit district would have been a glowing floor. A thin, wide
/// hull now reads as a deck, and this is the first tile to spend that.
///
/// It is a gallery down one side, not a bridge across the middle. The Well is a
/// place you go around the edge of, and a plate through the centre of a cell
/// would also be a plate through the middle of the only route.
pub fn hall_landing_wellshaft() -> String {
    let doors = [0usize, 3];
    let mut brushes = hall_shell(&doors);

    brushes.push_str("// Plate courses, one to a level of the shaft\n");
    for face in [1usize, 2, 4, 5] {
        brushes.push_str(&band(face, WALL, WALL + 5.0, 34.0, 42.0));
        brushes.push_str(&band(face, WALL, WALL + 5.0, 92.0, 100.0));
    }

    // Thin against its own run by twenty to one, which is what earns it the
    // floor's oxide rather than the wall's.
    brushes.push_str("// The landing, cantilevered off one side\n");
    brushes.push_str(&boxed((-80.0, 30.0, 60.0), (80.0, 80.0, 68.0)));
    brushes.push_str("// Brackets under it\n");
    for x in [-62.0, -20.0, 22.0, 62.0] {
        brushes.push_str(&boxed((x - 5.0, 54.0, 44.0), (x + 5.0, 78.0, 60.0)));
    }

    let mut out =
        String::from("// Straight, Wellshaft: a gallery down one side, at waist height.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_landing_wellshaft", "hall_straight", 0, 1, 6)
            .with_register_scope("wellshaft")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out
}

pub fn hall_squint_screen() -> String {
    let doors = [0usize, 3];

    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));

    brushes.push_str("// Envelope\n");
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 10.0, 6.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }

    brushes.push_str("// Base course on the sealed walls\n");
    for face in [1usize, 2, 4, 5] {
        brushes.push_str(&band(face, WALL, WALL + 4.0, FLOOR_TOP, FLOOR_TOP + 20.0));
    }

    // The three pieces. Read the numbers as the portal they describe: posts
    // twenty wide down each side of a hundred-and-twelve-unit opening, a lintel
    // across the top of it. Then read the depths, which are what makes them
    // look like nothing at all from anywhere else in the room.
    brushes.push_str("// The near post, a stub column two paces in\n");
    brushes.push_str(&squint_piece(66.0, -56.0, -36.0, FLOOR_TOP, 104.0, 12.0));
    brushes.push_str("// The far post, a pilaster against the west wall\n");
    brushes.push_str(&squint_piece(178.0, 36.0, 56.0, FLOOR_TOP, 104.0, 20.0));
    brushes.push_str("// The lintel, hung in the middle of the room on two rods\n");
    brushes.push_str(&squint_piece(122.0, -56.0, 56.0, 84.0, 104.0, 16.0));
    for side in [-1.0, 1.0] {
        let scale = 122.0 / SQUINT_REFERENCE;
        let y = 40.0 * side * scale;
        let top = SQUINT_EYE.2 + (104.0 - SQUINT_EYE.2) * scale;
        brushes.push_str(&boxed(
            (SQUINT_EYE.0 - 124.0, y - 3.0, top - 2.0),
            (SQUINT_EYE.0 - 120.0, y + 3.0, LEVEL - FLOOR_TOP),
        ));
    }

    // The plate. Without it the room is three lumps of nothing; with it, it is
    // a question. Twenty-four units across, two proud of the floor, exactly
    // where a body's feet go.
    brushes.push_str("// The plate: stand here\n");
    brushes.push_str(&translate(
        &pylon(18.0, FLOOR_TOP, FLOOR_TOP + 2.0, 0.0, 1.0, 0.0),
        SQUINT_EYE.0,
        SQUINT_EYE.1,
        0.0,
    ));

    // Light from behind the lintel, so the pieces are silhouettes rather than
    // objects - which is both the register's whole name and the only way an
    // outline made of three separate blocks reads as one outline.
    let mut lights = String::new();
    let (far, far_source) = wall_fixture(3, 0.5, 100.0, 28.0);
    brushes.push_str(&far);
    lights.push_str(&far_source);
    let (near, near_source) = ceiling_fixture(60.0, 0.0, LEVEL - FLOOR_TOP, 14.0, 10.0);
    brushes.push_str(&near);
    lights.push_str(&near_source);

    let mut out = String::from(
        "// The Squint, Shadow Screen: a doorway that is only there from one place.\n",
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_squint_screen", "hall_straight", 7, 1, 1)
            .with_register_scope("shadow_screen")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out.push_str(&lights);
    out
}

// ---------------------------------------------------------------------------
// Five tiles that are ideas rather than decorations.
// ---------------------------------------------------------------------------

/// Sill and head of a transom, above `DOOR_TOP` and below the lid.
const TRANSOM_SILL: f64 = 84.0;
const TRANSOM_HEAD: f64 = 108.0;

/// A hall with a window into a room it has no way into.
///
/// Sight and traversal come apart here, and in this facility that is not a
/// flourish - observation freezes a threshold's connection, so a body standing
/// in this hall can *hold* the cell beyond the transom without ever being able
/// to reach it. Every other tile in the corpus makes those two things the same
/// thing; a doorway is both a look and a walk. This one separates them.
///
/// Wellshaft, because the Well is the district whose entire design is
/// sightlines between people, and a window you cannot climb through is the
/// purest statement of that it is possible to build.
///
/// # What it is not, yet
///
/// The aperture goes through this cell's wall to the seam plane and stops
/// there. The neighbour's own wall is another eight units on their side, so
/// unless they happen to carry a transom on the matching face, what you get is
/// a deep reveal that looks like a window onto masonry. The contract permits
/// it - a sealed face is only checked for the canonical door band - which is
/// how this validated first time, and is also exactly why it does not yet do
/// what it is for.
///
/// Making it real needs a port class: something like `PortClass::Sight`, which
/// matches only other Sight ports, carries no traversal, and does carry
/// observation. That is a solver change and a catalogue-wide hash move, and it
/// is worth it, because "hold a room you cannot enter" is a verb the game does
/// not have. Until then this tile is a very good-looking alcove.
#[must_use]
pub fn hall_transom_wellshaft() -> String {
    let doors = [0usize, 3];
    let transom = 1usize;

    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));

    brushes.push_str("// Envelope, with one face opened high\n");
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 10.0, 8.0));
        } else if face == transom {
            brushes.push_str(&door_wall(
                face,
                0.0,
                LEVEL,
                TRANSOM_SILL,
                TRANSOM_HEAD,
                4.0,
                6.0,
            ));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }

    // A shelf under the transom, at the height a forearm goes. Nothing makes a
    // window read as a window like something to lean on.
    brushes.push_str("// The cill\n");
    brushes.push_str(&band(
        transom,
        WALL,
        WALL + 14.0,
        TRANSOM_SILL - 10.0,
        TRANSOM_SILL,
    ));

    let mut lights = String::new();
    for x in [-40.0, 40.0] {
        let (fixture, source) = ceiling_fixture(x, 0.0, LEVEL - FLOOR_TOP, 12.0, 8.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out =
        String::from("// The Transom, Wellshaft: a window into a room you cannot enter.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_transom_wellshaft", "hall_straight", 8, 1, 2)
            .with_register_scope("wellshaft")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out.push_str(&lights);
    out
}

/// How far in from each aperture a body has floor to stand on.
const DROP_SILL: f64 = 26.0;

/// A cell with walls and nothing else.
///
/// No floor and no lid: six faces, two apertures, and a lip of floor inside
/// each so there is somewhere to stand while you decide. Everything past the
/// lip is the storey below.
///
/// It is the gallery's opposite and its necessary partner. The gallery made a
/// stack into one volume you can see through; this makes the same volume
/// something you can fall down. A shaft with a walkway at every level is tall.
/// A shaft with a walkway at *some* levels is dangerous, and the difference is
/// entirely this tile.
///
/// Megastructure, where storeys are already missing out of the middle of
/// stacks. In the Unwitnessed a floor is not a thing you are entitled to.
#[must_use]
pub fn hall_drop_megastructure() -> String {
    let doors = [0usize, 3];

    let mut brushes = String::from("// No floor and no lid. That is the tile\n");
    brushes.push_str("// Envelope\n");
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 8.0, 6.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }

    brushes.push_str("// A lip inside each aperture: somewhere to stand and think\n");
    for &face in &doors {
        brushes.push_str(&band(face, WALL, WALL + DROP_SILL, 0.0, FLOOR_TOP));
    }

    let mut lights = String::new();
    for &face in &doors {
        let (fixture, source) = wall_fixture(face, 0.5, 96.0, 16.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = String::from("// The Drop, Megastructure: walls, and no floor between them.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_drop_megastructure", "hall_drop", 0, 1, 8)
            .with_register_scope("megastructure")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 1, "open"));
    for face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out.push_str(&lights);
    out
}

/// A dead end that reads as a corridor running away from you.
///
/// Four arch frames step back into a recess barely fifty units deep, each
/// noticeably smaller and closer to the one before it than real perspective
/// would make it, with the floor rising and the soffit falling to meet them.
/// Scamozzi built this at the Teatro Olimpico in 1585 and Borromini built it
/// again at the Palazzo Spada; both of them were selling a street that was not
/// there.
///
/// Liminal Grid, and the choice is the cruel one. The Back is the district that
/// grows while nobody watches, so its occupants already cannot tell whether a
/// corridor was always there. A hall you are certain you walked down, that
/// turns out to be a wall with four holes in it, is that doubt made permanent -
/// and the tile is *not* lying about anything a player could have checked. It
/// is a wall. It was always a wall.
#[must_use]
pub fn hall_false_depth_liminal() -> String {
    const MOUTH: f64 = -40.0;
    const STEP: f64 = 13.0;

    let mut brushes = hall_shell(&[0]);
    brushes.push_str("// Four frames, receding faster than they have any right to\n");
    for index in 0..4 {
        #[allow(clippy::cast_precision_loss)]
        let t = f64::from(index);
        let x = MOUTH - t * STEP;
        let half = 38.0 - t * 6.0;
        let head = 76.0 - t * 9.0;
        let sill = FLOOR_TOP + t * 3.0;
        // Jambs.
        for side in [-1.0, 1.0] {
            brushes.push_str(&boxed(
                (x - 6.0, side * half, sill),
                (x, side * (half + 14.0), head),
            ));
        }
        // Head.
        brushes.push_str(&boxed(
            (x - 6.0, -(half + 14.0), head),
            (x, half + 14.0, head + 12.0),
        ));
    }

    // The floor climbs and the soffit drops. Neither move is large; both are
    // in the direction distance would take them, and the eye supplies the rest.
    brushes.push_str("// The rake\n");
    brushes.push_str(&sloped_prism(
        &[
            (MOUTH, -52.0),
            (MOUTH - 3.0 * STEP - 8.0, -52.0),
            (MOUTH - 3.0 * STEP - 8.0, 52.0),
            (MOUTH, 52.0),
        ],
        0.0,
        [
            (MOUTH, -52.0, FLOOR_TOP),
            (MOUTH - 3.0 * STEP - 8.0, -52.0, FLOOR_TOP + 14.0),
            (MOUTH, 52.0, FLOOR_TOP),
        ],
        None,
    ));

    let mut lights = String::new();
    // One source, deep in the throat, so the last frame is the brightest thing
    // in the room and the eye goes to it.
    let (fixture, source) = ceiling_fixture(MOUTH - 3.0 * STEP, 0.0, 64.0, 8.0, 8.0);
    brushes.push_str(&fixture);
    lights.push_str(&source);

    let mut out = String::from("// False Depth, Liminal Grid: a hall that is a wall.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            "authored/hall_false_depth_liminal",
            "hall_false_depth",
            0,
            1,
            8,
        )
        .with_register_scope("liminal_grid")
        .emit(),
    );
    out.push_str(&tile_cell_default());
    out.push_str(&lateral_port(0, "door", "east_port", 0, 0, 0));
    out.push_str(&lights);
    out
}

/// A cell with no room in it.
///
/// `hall_shell` already fills every undoored sector solid, so a two-door cell
/// is a channel through mass; this narrows that channel to a little over two
/// metres and drops its ceiling to five, and what you get is not a corridor but
/// a hole bored through something.
///
/// It is the cheapest way to make the facility feel like it has mass. Every
/// other tile is a surface with space behind it; between two large rooms, this
/// one is eight metres of *material*, and the walk through it is long enough
/// that you arrive somewhere having been nowhere.
///
/// Monolith, obviously. One mass, undivided.
#[must_use]
pub fn hall_bore_monolith() -> String {
    let doors = [0usize, 3];
    let mut brushes = hall_shell(&doors);

    brushes.push_str("// Narrow the channel. It stays canonical at the seam and closes inboard\n");
    for &face in &doors {
        let (a, b) = edge(face);
        let (ia, ib) = offset_inward(a, b, WALL);
        let u = ((ib.0 - ia.0), (ib.1 - ia.1));
        let length = u.0.hypot(u.1);
        let u = (u.0 / length, u.1 / length);
        let mid = ((ia.0 + ib.0) * 0.5, (ia.1 + ib.1) * 0.5);
        let inward = (-mid.0 / mid.0.hypot(mid.1), -mid.1 / mid.0.hypot(mid.1));
        for side in [-1.0, 1.0] {
            let near = (
                mid.0 + u.0 * side * DOOR_HALF_WIDTH,
                mid.1 + u.1 * side * DOOR_HALF_WIDTH,
            );
            let far = (
                mid.0 + u.0 * side * 34.0 + inward.0 * 56.0,
                mid.1 + u.1 * side * 34.0 + inward.1 * 56.0,
            );
            let far_in = (
                mid.0 + u.0 * side * 60.0 + inward.0 * 56.0,
                mid.1 + u.1 * side * 60.0 + inward.1 * 56.0,
            );
            let near_out = (mid.0 + u.0 * side * 60.0, mid.1 + u.1 * side * 60.0);
            brushes.push_str(&prism(
                &[near, far, far_in, near_out],
                0.0,
                LEVEL,
                None,
                0.0,
                0.0,
            ));
        }
    }

    brushes.push_str("// Drop the ceiling over the middle to five metres\n");
    brushes.push_str(&prism(
        &regular_polygon(58.0, 6, 0.0),
        80.0,
        LEVEL - FLOOR_TOP,
        Some((0.0, 0.0)),
        0.0,
        6.0,
    ));

    let mut lights = String::new();
    let (fixture, source) = ceiling_fixture(0.0, 0.0, 80.0, 10.0, 10.0);
    brushes.push_str(&fixture);
    lights.push_str(&source);

    let mut out = String::from("// The Bore, Monolith: a hole through something, not a room.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_bore_monolith", "hall_straight", 6, 1, 4)
            .with_register_scope("monolith")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out.push_str(&lights);
    out
}

/// The shelf that holds a model of the room the shelf is in.
///
/// Five courses of shelving on three walls, and on the middle course of the
/// middle wall, a plinth carrying a hexagonal cell at one-sixteenth: the same
/// apothem, the same storey height, the same open middle, small enough to pick
/// up.
///
/// The Index believed a described room is an observed room and that an observed
/// room holds. This is that belief taken all the way down - the surveyors got
/// as far as modelling the room they were standing in, which means the model
/// contains a shelf, which means the shelf on the model contains a model. They
/// stopped there because the next one would have been too small to carve, not
/// because they saw the problem.
///
/// The model is the only geometry in the corpus deliberately off the eight-unit
/// grid. It is at a different scale; the grid is a property of the building,
/// and the model is not the building.
#[must_use]
pub fn hall_reliquary_infinite() -> String {
    let doors = [0usize, 3];
    let shelved = [1usize, 2, 4];

    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));

    brushes.push_str("// Envelope\n");
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 8.0, 6.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }

    brushes.push_str("// Twenty shelves: five to a side, on the three sealed walls\n");
    for &face in &shelved {
        for course in 0..5 {
            #[allow(clippy::cast_precision_loss)]
            let z = FLOOR_TOP + 16.0 + f64::from(course) * 20.0;
            brushes.push_str(&band(face, WALL, WALL + 16.0, z, z + 4.0));
        }
    }

    // The plinth and the model. Face 2's middle course.
    let mid = face_mid(2);
    let reach = mid.0.hypot(mid.1);
    let stand = (
        mid.0 / reach * (reach - 26.0),
        mid.1 / reach * (reach - 26.0),
    );
    let base = FLOOR_TOP + 16.0 + 2.0 * 20.0 + 4.0;
    brushes.push_str("// The plinth\n");
    brushes.push_str(&translate(
        &pylon(13.0, base, base + 3.0, 0.0, 1.0, 0.0),
        stand.0,
        stand.1,
        0.0,
    ));
    brushes.push_str("// The cell, at one sixteenth. Apothem seven, storey eight\n");
    for (radius, z0, z1) in [(8.1, 0.0, 0.5), (8.1, 0.5, 8.0), (4.1, 0.5, 8.0)] {
        let phase = if radius > 6.0 { 30.0 } else { 0.0 };
        brushes.push_str(&translate(
            &pylon(radius, base + 3.0 + z0, base + 3.0 + z1, phase, 0.2, 0.0),
            stand.0,
            stand.1,
            0.0,
        ));
    }

    let mut lights = String::new();
    let (fixture, source) = wall_fixture(2, 0.5, 108.0, 22.0);
    brushes.push_str(&fixture);
    lights.push_str(&source);
    let (high, high_source) = ceiling_fixture(30.0, 0.0, LEVEL - FLOOR_TOP, 12.0, 8.0);
    brushes.push_str(&high);
    lights.push_str(&high_source);

    let mut out = String::from("// The Reliquary, Infinite Gallery: a shelf holding this room.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            "authored/hall_reliquary_infinite",
            "hall_straight",
            10,
            1,
            2,
        )
        .with_register_scope("infinite_gallery")
        .emit(),
    );
    out.push_str(&tile_cell_default());
    for face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{}_port", FACE_NAMES[face]),
            0,
            0,
            0,
        ));
    }
    out.push_str(&lights);
    out
}

#[must_use]
pub fn builders() -> Vec<Builder> {
    vec![
        ("hall_straight", hall_straight as fn() -> String),
        ("hall_straight_buttressed", hall_straight_buttressed),
        ("hall_straight_datum", hall_straight_datum),
        ("hall_straight_soffit", hall_straight_soffit),
        ("hall_gallery_megastructure", hall_gallery_megastructure),
        ("hall_gallery_wellshaft", hall_gallery_wellshaft),
        ("hall_gallery_broken", hall_gallery_broken),
        ("hall_gallery_cantilever", hall_gallery_cantilever),
        ("hall_turn_60_soffit", hall_turn_60_soffit),
        ("hall_turn_120_soffit", hall_turn_120_soffit),
        ("hall_straight_threshold", hall_straight_threshold),
        ("hall_step_platform", hall_step_platform),
        ("hall_dais_monument", hall_dais_monument),
        ("hall_arena_monolith", hall_arena_monolith),
        ("hall_squint_screen", hall_squint_screen),
        ("hall_gate_monument", hall_gate_monument),
        ("hall_pocket_overlit", hall_pocket_overlit),
        ("hall_screen_shoji", hall_screen_shoji),
        ("hall_bay_monument", hall_bay_monument),
        ("hall_turn_monument", hall_turn_monument),
        ("hall_turn_shoji", hall_turn_shoji),
        ("hall_pocket_overlit_b", hall_pocket_overlit_b),
        ("hall_landing_wellshaft", hall_landing_wellshaft),
        ("hall_transom_wellshaft", hall_transom_wellshaft),
        ("hall_drop_megastructure", hall_drop_megastructure),
        ("hall_false_depth_liminal", hall_false_depth_liminal),
        ("hall_bore_monolith", hall_bore_monolith),
        ("hall_reliquary_infinite", hall_reliquary_infinite),
        ("hall_gallery_infinite", hall_gallery_infinite),
        ("hall_cap", hall_cap),
        ("hall_turn_60", hall_turn_60),
        ("hall_turn_60_buttressed", hall_turn_60_buttressed),
        ("hall_turn_120", hall_turn_120),
        ("hall_turn_60_hewn", hall_turn_60_hewn),
        ("hall_turn_120_hewn", hall_turn_120_hewn),
        ("hall_junction_3way", hall_junction_3way),
        ("hall_junction_3way_hewn", hall_junction_3way_hewn),
        ("hall_junction_3way_monolith", hall_junction_3way_monolith),
        ("hall_junction_3way_facet", hall_junction_3way_facet),
        ("hall_junction_3way_thinning", hall_junction_3way_thinning),
        ("hall_junction_4way", hall_junction_4way),
    ]
}

// ---------------------------------------------------------------------------
// Open districts: the same route, through a room instead of a channel.
// ---------------------------------------------------------------------------
//
// `hall_shell` fills every undoored sector solid, so a two-door cell built with
// it is a *channel* two and a quarter metres wide through fourteen metres of
// mass. That is the right reading for a district whose answer was weight, and
// the wrong one for four of the ten:
//
// - the Back is a **field**, not a route, and a corridor contradicts it;
// - the Unwitnessed is vast by definition and cannot be narrow;
// - the Noon erased every landmark, and a channel is a landmark - it tells you
//   which way you came in;
// - the Thin has no load-bearing walls at all, only posts.
//
// The alternative recipe has been in this file all session: an eight-unit
// perimeter wall with apertures, leaving the whole hex open. Same ports, same
// seam, same signature - the solver cannot tell them apart and does not need
// to. What changes is that you walk *through a room* rather than along a slot,
// and at fourteen metres across that is the largest single spatial difference
// available inside the contract.

/// The registers whose halls are rooms rather than channels, and the variant
/// block they occupy.
const OPEN_REGISTERS: [&str; 4] = ["overlit_grid", "megastructure", "thinning", "liminal_grid"];
/// Variant base for open halls, clear of every other family in this corpus.
const OPEN_BASE: i32 = 90;

fn open_variant(register: &str) -> i32 {
    OPEN_BASE
        + i32::try_from(
            OPEN_REGISTERS
                .iter()
                .position(|slug| *slug == register)
                .unwrap_or(0),
        )
        .unwrap_or(0)
}

/// A hall that is a room: perimeter wall, apertures where the ports are, and
/// nothing in the middle.
fn hall_open(name: &str, archetype: &str, register: &str, doors: &[usize]) -> String {
    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));
    brushes.push_str("// Perimeter only. The middle of the cell is the cell\n");
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 8.0, 6.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }

    let mut lights = String::new();
    // Two practicals well apart: an open cell lit from one point reads as a
    // pool in a void rather than as a room.
    for face in [doors[0], doors[doors.len() - 1]] {
        let mid = face_mid(face);
        let reach = mid.0.hypot(mid.1);
        let (fixture, source) = ceiling_fixture(
            mid.0 / reach * 44.0,
            mid.1 / reach * 44.0,
            LEVEL - FLOOR_TOP,
            18.0,
            10.0,
        );
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = format!("// {archetype}, {register}: a room on the route, not a channel.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            &format!("authored/{name}"),
            archetype,
            open_variant(register),
            1,
            6,
        )
        .with_register_scope(register)
        .emit(),
    );
    out.push_str(&tile_cell_default());
    for &face in doors {
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
    out.push_str(&lights);
    out
}

/// The five shapes an open district needs, so a whole route can run through
/// rooms rather than switching to channels at every corner.
const OPEN_SHAPES: [(&str, &[usize]); 5] = [
    ("hall_straight", &[0, 3]),
    ("hall_turn_60", &[0, 5]),
    ("hall_turn_120", &[0, 4]),
    ("hall_junction_3way", &[0, 3, 5]),
    ("hall_junction_4way", &[0, 2, 3, 5]),
];

#[must_use]
pub fn open_builders() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for register in OPEN_REGISTERS {
        for (archetype, doors) in OPEN_SHAPES {
            let name = format!("{archetype}_open_{register}");
            out.push((name.clone(), hall_open(&name, archetype, register, doors)));
        }
    }
    out
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
