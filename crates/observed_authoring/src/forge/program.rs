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

/// What a stall partition is, in a given district.
///
/// The stalls are the load-bearing part of the sentence: a row of vertical
/// planes standing off the floor is a restroom and cannot be read as anything
/// else. So this is the parameter that says the most about a district, because
/// it is the one place where a dialect has to answer a question every dialect
/// would rather not: *how much privacy does this civilisation think a body is
/// owed?*
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stall {
    /// A solid panel. The default answer.
    Panel,
    /// Two thin fins with a gap between them. Interruption, not privacy.
    Slatted,
    /// Full-depth mass, floor to head, no gap under. Privacy as masonry.
    Pier,
    /// Two posts and nothing between them: the outline of a stall.
    Posts,
    /// None at all.
    Absent,
}

/// One district's way of saying the same sentence.
struct Dialect {
    register: &'static str,
    stall: Stall,
    /// How far the partitions stand off the floor. The gap is the tell.
    stall_foot: f64,
    /// Tiled dado height, or zero.
    dado: f64,
    /// Shelf courses instead of a dado.
    shelves: bool,
    /// Height of the counter's working surface.
    counter: f64,
    /// Is there a line where the mirrors were?
    mirror: bool,
    /// A dropped soffit over the stall run.
    soffit: bool,
    /// A stepped plinth under the counter.
    plinth: bool,
}

const STALL_HEAD: f64 = 96.0;
const STALL_DEPTH: f64 = 62.0;

/// The same restroom, ten times.
///
/// Everything that says *restroom* is fixed and identical in every one: four
/// stalls at the same pitch on the same wall, a counter, a splashback, three
/// bare pads where something was bolted, one door narrowed inboard. What varies
/// is only how each district builds those things — and the point of holding the
/// sentence still is that the differences stop being moods and become dialects.
///
/// Read them against the fiction and each one is an argument:
///
/// - **Shadow Screen** slats the partitions. You are interrupted, not hidden.
/// - **Monolith** will not make a gap under anything, so its stalls are piers
///   standing on the floor: privacy as masonry, in the district whose answer to
///   everything was mass.
/// - **The Noon** runs a soffit over the stalls so the light comes from above
///   them, and **removes the mirrors**. The district that erased every landmark
///   was not going to leave you the one fitting whose entire purpose is letting
///   you check that you are still there.
/// - **The Welcome** puts the counter on a stepped plinth. Even the restroom is
///   approached.
/// - **The Unwitnessed** builds it all correctly and half a metre too high. The
///   fittings are for bodies, and not for ours.
/// - **The Well**, whose whole design is sightlines between people, has no
///   partitions at all. It did not solve privacy. It abolished it.
/// - **The Index** replaces the dado with shelf courses, because everything
///   gets catalogued, including this.
/// - **The Thin** reduces a stall to two posts: the least structure that still
///   names the activity, which is the district's entire method.
fn ablutions(dialect: &Dialect) -> String {
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

    if dialect.shelves {
        brushes.push_str("// Shelf courses. Everything gets catalogued, including this\n");
        for course in 0..3 {
            #[allow(clippy::cast_precision_loss)]
            let z = FLOOR_TOP + 20.0 + f64::from(course) * 22.0;
            brushes.push_str(&band(2, WALL, WALL + 14.0, z, z + 4.0));
        }
    } else if dialect.dado > 0.0 {
        brushes.push_str("// Tiled dado. Nobody tiles to this height for any reason but people\n");
        for face in [2usize, 3, 4] {
            brushes.push_str(&band(
                face,
                WALL,
                WALL + 4.0,
                FLOOR_TOP,
                FLOOR_TOP + dialect.dado,
            ));
        }
    }

    if dialect.stall != Stall::Absent {
        brushes.push_str("// Stalls. The partitions were bolted down; the doors were not\n");
        let (pa, pb) = edge(4);
        let (ia, ib) = offset_inward(pa, pb, WALL);
        let (ja, jb) = offset_inward(pa, pb, WALL + STALL_DEPTH);
        for index in 0..4 {
            #[allow(clippy::cast_precision_loss)]
            let t = (f64::from(index) + 0.5) / 4.0;
            let lerp2 = |p0: (f64, f64), p1: (f64, f64)| {
                (p0.0 + (p1.0 - p0.0) * t, p0.1 + (p1.1 - p0.1) * t)
            };
            let outer = lerp2(ia, ib);
            let inner = lerp2(ja, jb);
            let axis = (inner.0 - outer.0, inner.1 - outer.1);
            let len = axis.0.hypot(axis.1);
            let unit = (axis.0 / len, axis.1 / len);
            let perp = (-unit.1, unit.0);
            let leaf = |from: f64, to: f64, half: f64, foot: f64, head: f64| {
                let a = (outer.0 + unit.0 * from, outer.1 + unit.1 * from);
                let b = (outer.0 + unit.0 * to, outer.1 + unit.1 * to);
                prism(
                    &[
                        (a.0 + perp.0 * half, a.1 + perp.1 * half),
                        (b.0 + perp.0 * half, b.1 + perp.1 * half),
                        (b.0 - perp.0 * half, b.1 - perp.1 * half),
                        (a.0 - perp.0 * half, a.1 - perp.1 * half),
                    ],
                    foot,
                    head,
                    None,
                    0.0,
                    0.0,
                )
            };
            match dialect.stall {
                Stall::Panel => {
                    brushes.push_str(&leaf(0.0, len, 3.0, dialect.stall_foot, STALL_HEAD));
                }
                Stall::Slatted => {
                    brushes.push_str(&leaf(0.0, len * 0.44, 2.0, dialect.stall_foot, STALL_HEAD));
                    brushes.push_str(&leaf(len * 0.58, len, 2.0, dialect.stall_foot, STALL_HEAD));
                }
                Stall::Pier => {
                    brushes.push_str(&leaf(0.0, len, 7.0, FLOOR_TOP, STALL_HEAD + 12.0));
                }
                Stall::Posts => {
                    brushes.push_str(&leaf(len - 9.0, len, 4.0, FLOOR_TOP, STALL_HEAD));
                }
                Stall::Absent => {}
            }
        }
        if dialect.stall == Stall::Posts {
            brushes.push_str("// A rail across the posts, and that is the whole stall\n");
            brushes.push_str(&band(
                4,
                WALL + STALL_DEPTH - 8.0,
                WALL + STALL_DEPTH,
                STALL_HEAD - 6.0,
                STALL_HEAD,
            ));
        }
    }

    if dialect.soffit {
        brushes.push_str("// A soffit over the stalls, so the light is above them\n");
        brushes.push_str(&band(
            4,
            WALL,
            WALL + STALL_DEPTH + 6.0,
            LEVEL - FLOOR_TOP - 26.0,
            LEVEL - FLOOR_TOP,
        ));
    }

    brushes.push_str("// The counter and its splashback\n");
    if dialect.plinth {
        for (step, depth) in [(0.0, 34.0), (1.0, 30.0)] {
            let z = FLOOR_TOP + step * 8.0;
            brushes.push_str(&band(3, WALL, WALL + depth, z, z + 8.0));
        }
    }
    brushes.push_str(&band(
        3,
        WALL,
        WALL + 26.0,
        dialect.counter,
        dialect.counter + 8.0,
    ));
    brushes.push_str(&band(
        3,
        WALL,
        WALL + 8.0,
        dialect.counter + 8.0,
        dialect.counter + 16.0,
    ));
    if dialect.mirror {
        brushes.push_str("// The line the mirrors were on\n");
        brushes.push_str(&band(
            3,
            WALL + 2.0,
            WALL + 6.0,
            dialect.counter + 28.0,
            dialect.counter + 30.0,
        ));
    }

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

    let mut out = format!(
        "// Ablutions, {}: a room shaped entirely by nobody.\n",
        dialect.register
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            &format!("authored/hall_ablutions_{}", dialect.register),
            "hall_ablutions",
            0,
            1,
            8,
        )
        .with_register_scope(dialect.register)
        .emit(),
    );
    out.push_str(&tile_cell_default());
    out.push_str(&lateral_port(0, "door", FACE_NAMES[0], 0, 0, 0));
    out.push_str(&lights);
    out
}

/// The template every dialect departs from: a solid panel a metre off the
/// floor, a chest-high dado, a counter at 850 mm, mirrors above it.
const fn plain(register: &'static str) -> Dialect {
    Dialect {
        register,
        stall: Stall::Panel,
        stall_foot: 16.0,
        dado: DADO,
        shelves: false,
        counter: 48.0,
        mirror: true,
        soffit: false,
        plinth: false,
    }
}

macro_rules! ablution_tiles {
    ($(($fname:ident, $stem:literal, $dialect:expr)),* $(,)?) => {
        $(
            #[must_use]
            pub fn $fname() -> String {
                ablutions(&$dialect)
            }
        )*

        #[must_use]
        pub fn builders() -> Vec<Builder> {
            vec![$(($stem, $fname as fn() -> String)),*]
        }
    };
}

ablution_tiles![
    (
        hall_ablutions_shadow_screen,
        "hall_ablutions_shadow_screen",
        Dialect {
            stall: Stall::Slatted,
            ..plain("shadow_screen")
        }
    ),
    (
        hall_ablutions_monolith,
        "hall_ablutions_monolith",
        Dialect {
            stall: Stall::Pier,
            dado: 32.0,
            mirror: false,
            ..plain("monolith")
        }
    ),
    (
        hall_ablutions_overlit_grid,
        "hall_ablutions_overlit_grid",
        Dialect {
            dado: 4.0,
            mirror: false,
            soffit: true,
            stall_foot: 20.0,
            ..plain("overlit_grid")
        }
    ),
    (
        hall_ablutions_institutional,
        "hall_ablutions_institutional",
        plain("institutional")
    ),
    (
        hall_ablutions_facet_monument,
        "hall_ablutions_facet_monument",
        Dialect {
            dado: 24.0,
            plinth: true,
            counter: 64.0,
            ..plain("facet_monument")
        }
    ),
    (
        hall_ablutions_megastructure,
        "hall_ablutions_megastructure",
        Dialect {
            dado: 48.0,
            counter: 72.0,
            stall_foot: 26.0,
            ..plain("megastructure")
        }
    ),
    (
        hall_ablutions_wellshaft,
        "hall_ablutions_wellshaft",
        Dialect {
            stall: Stall::Absent,
            dado: 20.0,
            ..plain("wellshaft")
        }
    ),
    (
        hall_ablutions_infinite_gallery,
        "hall_ablutions_infinite_gallery",
        Dialect {
            shelves: true,
            dado: 0.0,
            ..plain("infinite_gallery")
        }
    ),
    (
        hall_ablutions_thinning,
        "hall_ablutions_thinning",
        Dialect {
            stall: Stall::Posts,
            dado: 0.0,
            ..plain("thinning")
        }
    ),
    (
        hall_ablutions_liminal_grid,
        "hall_ablutions_liminal_grid",
        Dialect {
            dado: 40.0,
            ..plain("liminal_grid")
        }
    ),
];
