//! Program: the mundane fittings a facility is emptied *of*.
//!
//! Every district in the corpus is architecture — shafts, monuments, voids,
//! galleries. None of it has ever been used for anything. That is the gap this
//! module exists to close, and the reason it matters is not decoration:
//!
//! **A liminal space is a lived-in space missing its living.** Not an abstract
//! void — an abstract void is just a void. What makes an empty corridor
//! frightening is that it is unmistakably shaped *for* something that is not
//! happening. So the rule for everything in here is: **name the missing
//! activity in the geometry**. A room that could have been anything is not
//! liminal; it is unfinished.
//!
//! There is a second reason, and it may be the more important one. Every one of
//! the seven districts removes human scale on purpose — the Unwitnessed has no
//! object in it that tells you how big anything is, the Noon erases every cue,
//! the monument is sized for a guest who never came. That is a good effect and
//! it is currently *unopposed*, which makes it inert. A basin is 850 mm off the
//! floor in every building anyone has ever built. Put one run of basins in a
//! facility and the whole thing acquires a scale, and every space that refuses
//! to have one starts refusing it *against* something.
//!
//! # Grammar
//!
//! Program is **not** a district. It cuts across all of them: the same
//! restroom, in seven dialects, is exactly the "one building, several
//! civilisations" claim the compositions already make, and it is the cheapest
//! way to make a boundary between districts read as a boundary.
//!
//! Three rules hold everywhere in this module:
//!
//! 1. **A room narrows its own doorway, inboard.** The aperture at the seam
//!    stays canonical because it has to; a program cell adds jamb blocks
//!    *behind* it so the opening you actually walk through is 3 m rather than
//!    4.5. That single move is the whole difference between "circulation" and
//!    "somewhere", and it costs two brushes.
//! 2. **Program cells are leaves.** One door, no through route. A restroom you
//!    can walk out the other side of is a corridor with basins in it.
//! 3. **Only what was bolted down is still here.** Anything a person could
//!    carry, or unscrew, is gone — that is what emptied means, and in this
//!    facility it is also literally true, because loose things are exactly what
//!    the building rearranges away. Stalls keep their partitions and lose their
//!    doors. Floors keep the pads the seating was bolted to and lose the
//!    seating. The absence is the subject, so it has to be *specific*: not
//!    "there is nothing here" but "there were eleven of something here".

use super::entities::{Meta, ceiling_fixture, lateral_port, tile_cell_default, worldspawn};
use super::geometry::{
    DOOR_HALF_WIDTH, DOOR_TOP, FACE_NAMES, FLOOR_TOP, LEVEL, WALL, band, boxed, door_wall, edge,
    hex_slab, offset_inward, prism, translate, wall,
};
use super::{Builder, GENERATED_NOTE};

/// Half-width of the opening a program cell actually presents, inboard of the
/// canonical aperture. Three metres against the corridor's four and a half.
const ROOM_HALF_WIDTH: f64 = 24.0;

/// Height of a tiled dado. Chest high, and the single most institutional line
/// available: nobody tiles a wall to this height for any reason but cleaning up
/// after people.
const DADO: f64 = 44.0;

/// Two jamb blocks that narrow `face`'s aperture behind the seam plane.
fn narrowed(face: usize) -> String {
    let (a, b) = edge(face);
    let (ia, ib) = offset_inward(a, b, WALL);
    let (ja, jb) = offset_inward(a, b, WALL + 14.0);
    let u = (ib.0 - ia.0, ib.1 - ia.1);
    let length = u.0.hypot(u.1);
    let u = (u.0 / length, u.1 / length);
    let mid = ((ia.0 + ib.0) * 0.5, (ia.1 + ib.1) * 0.5);
    let jmid = ((ja.0 + jb.0) * 0.5, (ja.1 + jb.1) * 0.5);
    let mut out = String::new();
    for side in [-1.0, 1.0] {
        let p0 = (
            mid.0 + u.0 * side * DOOR_HALF_WIDTH,
            mid.1 + u.1 * side * DOOR_HALF_WIDTH,
        );
        let p1 = (
            mid.0 + u.0 * side * ROOM_HALF_WIDTH,
            mid.1 + u.1 * side * ROOM_HALF_WIDTH,
        );
        let p2 = (
            jmid.0 + u.0 * side * ROOM_HALF_WIDTH,
            jmid.1 + u.1 * side * ROOM_HALF_WIDTH,
        );
        let p3 = (
            jmid.0 + u.0 * side * DOOR_HALF_WIDTH,
            jmid.1 + u.1 * side * DOOR_HALF_WIDTH,
        );
        out.push_str(&prism(
            &[p0, p1, p2, p3],
            0.0,
            DOOR_TOP + 10.0,
            None,
            0.0,
            0.0,
        ));
    }
    out
}

/// The restroom.
///
/// The most photographed liminal space there is, and it earns that: no other
/// room in a building is so completely determined by bodies, so a restroom with
/// no bodies in it is the shape of an absence rather than an empty box.
///
/// Four stall partitions, standing off the floor by a metre and off the ceiling
/// by two — the gap top and bottom is the whole tell, and it is the reason a
/// row of partitions cannot be mistaken for a row of anything else. The stall
/// *doors* are gone. So is whatever was bolted to the three pads on the floor
/// by the far wall. What is left is a counter, a splashback, a mirror line, and
/// a dado at the height institutions have tiled walls to since tiling was
/// invented.
///
/// One door, narrowed inboard, because you do not pass through a restroom.
#[must_use]
pub fn hall_ablutions_liminal() -> String {
    const STALL_FOOT: f64 = 16.0;
    const STALL_HEAD: f64 = 96.0;
    const STALL_DEPTH: f64 = 62.0;

    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));

    brushes.push_str("// Envelope: one aperture, narrowed inboard\n");
    for face in 0..6 {
        if face == 0 {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 6.0, 4.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }
    brushes.push_str(&narrowed(0));

    brushes.push_str("// Tiled dado, three walls. Chest high, for hosing down\n");
    for face in [2usize, 3, 4] {
        brushes.push_str(&band(face, WALL, WALL + 4.0, FLOOR_TOP, FLOOR_TOP + DADO));
    }

    // Four partitions on the north-west wall, off the floor and off the lid.
    brushes.push_str("// Stalls. The partitions are bolted down; the doors were not\n");
    let (pa, pb) = edge(4);
    let (ia, ib) = offset_inward(pa, pb, WALL);
    let (ja, jb) = offset_inward(pa, pb, WALL + STALL_DEPTH);
    for index in 0..4 {
        #[allow(clippy::cast_precision_loss)]
        let t = (f64::from(index) + 0.5) / 4.0;
        let along =
            |p0: (f64, f64), p1: (f64, f64)| (p0.0 + (p1.0 - p0.0) * t, p0.1 + (p1.1 - p0.1) * t);
        let outer = along(ia, ib);
        let inner = along(ja, jb);
        let axis = (inner.0 - outer.0, inner.1 - outer.1);
        let len = axis.0.hypot(axis.1);
        let perp = (-axis.1 / len * 3.0, axis.0 / len * 3.0);
        brushes.push_str(&prism(
            &[
                (outer.0 + perp.0, outer.1 + perp.1),
                (inner.0 + perp.0, inner.1 + perp.1),
                (inner.0 - perp.0, inner.1 - perp.1),
                (outer.0 - perp.0, outer.1 - perp.1),
            ],
            STALL_FOOT,
            STALL_HEAD,
            None,
            0.0,
            0.0,
        ));
    }

    brushes.push_str("// The counter, its splashback, and the line the mirrors were on\n");
    brushes.push_str(&band(3, WALL, WALL + 26.0, 48.0, 56.0));
    brushes.push_str(&band(3, WALL, WALL + 8.0, 56.0, 64.0));
    brushes.push_str(&band(3, WALL + 2.0, WALL + 6.0, 76.0, 78.0));

    brushes.push_str("// Three pads where something was bolted, and taken\n");
    for offset in [-30.0, 0.0, 30.0] {
        brushes.push_str(&translate(
            &boxed((-9.0, -7.0, FLOOR_TOP), (9.0, 7.0, FLOOR_TOP + 2.0)),
            34.0,
            offset,
            0.0,
        ));
    }

    let mut lights = String::new();
    for (x, y) in [(-34.0, 0.0), (26.0, 0.0)] {
        let (fixture, source) = ceiling_fixture(x, y, LEVEL - FLOOR_TOP, 20.0, 7.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = String::from("// Ablutions, Liminal Grid: a room shaped entirely by nobody.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_ablutions_liminal", "hall_ablutions", 0, 1, 8)
            .with_register_scope("liminal_grid")
            .emit(),
    );
    out.push_str(&tile_cell_default());
    out.push_str(&lateral_port(0, "door", FACE_NAMES[0], 0, 0, 0));
    out.push_str(&lights);
    out
}

#[must_use]
pub fn builders() -> Vec<Builder> {
    vec![(
        "hall_ablutions_liminal",
        hall_ablutions_liminal as fn() -> String,
    )]
}
