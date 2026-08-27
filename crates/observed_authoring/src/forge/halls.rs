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
