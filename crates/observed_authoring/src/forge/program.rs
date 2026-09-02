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

/// A rectangle of wall-parallel mass on `face`: `a0..a1` along it, `in0..in1`
/// inward of the face plane, `z0..z1` up.
///
/// `band` is this with `a0..a1` fixed at the whole face, which is right for a
/// dado and wrong for everything that comes in bays - a servery hatch, a
/// shutter, a run of lockers. Program is mostly things that come in bays.
fn panel(face: usize, a0: f64, a1: f64, in0: f64, in1: f64, z0: f64, z1: f64) -> String {
    let (a, b) = edge(face);
    let (oa, ob) = offset_inward(a, b, in0);
    let (ia, ib) = offset_inward(a, b, in1);
    let at = |p: (f64, f64), q: (f64, f64), t: f64| (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t);
    prism(
        &[
            at(oa, ob, a0),
            at(oa, ob, a1),
            at(ia, ib, a1),
            at(ia, ib, a0),
        ],
        z0,
        z1,
        None,
        0.0,
        0.0,
    )
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

macro_rules! program_tiles {
    (
        $(($afn:ident, $astem:literal, $adialect:expr)),* $(,)? ;
        $(($rfn:ident, $rstem:literal, $rspec:expr)),* $(,)?
    ) => {
        $(
            #[must_use]
            pub fn $afn() -> String {
                ablutions(&$adialect)
            }
        )*
        $(
            #[must_use]
            pub fn $rfn() -> String {
                refectory(&$rspec)
            }
        )*

        #[must_use]
        pub fn builders() -> Vec<Builder> {
            vec![
                $(($astem, $afn as fn() -> String),)*
                $(($rstem, $rfn as fn() -> String),)*
            ]
        }
    };
}

program_tiles![
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
    );
    (
        hall_refectory_shadow_screen,
        "hall_refectory_shadow_screen",
        Refectory { screen: true, shutter: Shutter::Absent, ..canteen("shadow_screen") }
    ),
    (
        hall_refectory_monolith,
        "hall_refectory_monolith",
        Refectory { shutter: Shutter::Absent, dado: 32.0, ..canteen("monolith") }
    ),
    (
        hall_refectory_overlit_grid,
        "hall_refectory_overlit_grid",
        Refectory { shutter: Shutter::Absent, dado: 4.0, ..canteen("overlit_grid") }
    ),
    (
        hall_refectory_institutional,
        "hall_refectory_institutional",
        Refectory { shutter: Shutter::Mixed, ..canteen("institutional") }
    ),
    (
        hall_refectory_facet_monument,
        "hall_refectory_facet_monument",
        Refectory { plinth: true, counter: 56.0, dado: 24.0, ..canteen("facet_monument") }
    ),
    (
        hall_refectory_megastructure,
        "hall_refectory_megastructure",
        Refectory { counter: 72.0, dado: 48.0, ..canteen("megastructure") }
    ),
    (
        hall_refectory_wellshaft,
        "hall_refectory_wellshaft",
        Refectory { pads: Pads::Row, shutter: Shutter::Absent, dado: 20.0, ..canteen("wellshaft") }
    ),
    (
        hall_refectory_infinite_gallery,
        "hall_refectory_infinite_gallery",
        Refectory { shelves: true, dado: 0.0, ..canteen("infinite_gallery") }
    ),
    (
        hall_refectory_thinning,
        "hall_refectory_thinning",
        Refectory { shutter: Shutter::Absent, dado: 0.0, pads: Pads::Row, ..canteen("thinning") }
    ),
    (
        hall_refectory_liminal_grid,
        "hall_refectory_liminal_grid",
        Refectory { shutter: Shutter::Mixed, dado: 40.0, ..canteen("liminal_grid") }
    ),
];

/// What became of the serving hatches.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shutter {
    /// All three down. Closed properly, by someone who expected to come back.
    Down,
    /// Two down and one still up. Nobody closed this room; it was left.
    Mixed,
    /// No shutters. A district that does not do moving parts, or does not do
    /// concealment.
    Absent,
}

/// How the seating was arranged, read from what is left of its fixings.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pads {
    /// Two rows of three: tables, on an institutional grid.
    Grid,
    /// One long line: a refectory bench, everyone facing everyone.
    Row,
}

/// One district's refectory.
struct Refectory {
    register: &'static str,
    counter: f64,
    shutter: Shutter,
    pads: Pads,
    /// Thin verticals across the servery. You queue at a grille.
    screen: bool,
    /// A stepped plinth under the servery.
    plinth: bool,
    /// Shelf courses behind the line instead of a hot cupboard.
    shelves: bool,
    dado: f64,
}

/// The refectory: a servery, a shutter, and the fixings of the furniture.
///
/// # Why this one has two doors
///
/// The module's second rule is that program cells are leaves, and this tile
/// breaks it deliberately. A restroom you can walk out the far side of is a
/// corridor with basins in it; a *canteen* you can walk through is a canteen,
/// because that is what a canteen was. It is the one room in any institution
/// that is both a destination and a route, which is exactly why it is where
/// everyone met, and a version of it you had to double back out of would be a
/// different building with the same fittings in it.
///
/// # What says canteen
///
/// Not the counter — a counter could be anything. Three things together, and
/// none of them is furniture:
///
/// 1. **The tray rail.** A bar on the front of the counter at knee-to-waist
///    height, whose only purpose is that something slides along it. Nothing
///    else in a building has one.
/// 2. **The shutter.** A closed servery is the most complete statement of "this
///    is over" that architecture has, and leaving one of three still raised is
///    the difference between a room that was closed and a room that was left.
/// 3. **The pads.** Six of them, in rows, at table pitch. Three pads is an
///    absence; six in a grid is a *seating plan*, and a seating plan is a
///    number of people.
fn refectory(spec: &Refectory) -> String {
    const SERVERY: usize = 4;
    const COUNTER_DEPTH: f64 = 30.0;
    let doors = [0usize, 3];
    let soffit = LEVEL - FLOOR_TOP - 28.0;

    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));

    brushes.push_str("// Envelope. Two doors: you pass through a canteen\n");
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 8.0, 6.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }

    if spec.dado > 0.0 {
        brushes.push_str("// Dado, on the two walls the seating stood against\n");
        for face in [1usize, 2] {
            brushes.push_str(&band(
                face,
                WALL,
                WALL + 4.0,
                FLOOR_TOP,
                FLOOR_TOP + spec.dado,
            ));
        }
    }

    if spec.plinth {
        brushes.push_str("// The servery on a plinth. Even lunch is approached\n");
        brushes.push_str(&band(
            SERVERY,
            WALL,
            WALL + COUNTER_DEPTH + 8.0,
            FLOOR_TOP,
            FLOOR_TOP + 8.0,
        ));
    }

    brushes.push_str("// The servery: counter, tray rail, and the line behind it\n");
    brushes.push_str(&band(
        SERVERY,
        WALL,
        WALL + COUNTER_DEPTH,
        spec.counter,
        spec.counter + 8.0,
    ));
    brushes.push_str(&band(
        SERVERY,
        WALL + COUNTER_DEPTH,
        WALL + COUNTER_DEPTH + 4.0,
        spec.counter - 12.0,
        spec.counter - 6.0,
    ));
    if spec.shelves {
        for course in 0..2 {
            #[allow(clippy::cast_precision_loss)]
            let z = spec.counter + 20.0 + f64::from(course) * 18.0;
            brushes.push_str(&band(SERVERY, WALL, WALL + 12.0, z, z + 4.0));
        }
    } else {
        // The Unwitnessed's counter is high enough that the hot cupboard behind
        // it would grow through the soffit, and a brush whose top is under its
        // bottom is not a brush. Clamped rather than special-cased: a district
        // is allowed to build this too big, and the room is still a room.
        brushes.push_str(&band(
            SERVERY,
            WALL,
            WALL + 12.0,
            spec.counter + 8.0,
            (spec.counter + 30.0).min(soffit - 8.0),
        ));
    }

    brushes.push_str("// The soffit over the line\n");
    brushes.push_str(&band(
        SERVERY,
        WALL,
        WALL + COUNTER_DEPTH + 10.0,
        soffit,
        LEVEL - FLOOR_TOP,
    ));

    if spec.shutter != Shutter::Absent {
        brushes.push_str("// The shutters. Two were pulled down; one was not\n");
        for (index, (a0, a1)) in [(0.08, 0.32), (0.38, 0.62), (0.68, 0.92)]
            .iter()
            .enumerate()
        {
            let raised = spec.shutter == Shutter::Mixed && index == 1;
            let bottom = if raised {
                soffit - 12.0
            } else {
                (spec.counter + 30.0).min(soffit - 16.0)
            };
            brushes.push_str(&panel(
                SERVERY,
                *a0,
                *a1,
                WALL + 4.0,
                WALL + 10.0,
                bottom,
                soffit,
            ));
        }
    }

    if spec.screen {
        brushes.push_str("// Thin verticals across the line. You queue at a grille\n");
        for index in 0..4 {
            #[allow(clippy::cast_precision_loss)]
            let t = 0.14 + f64::from(index) * 0.24;
            brushes.push_str(&panel(
                SERVERY,
                t,
                t + 0.035,
                WALL + COUNTER_DEPTH - 4.0,
                WALL + COUNTER_DEPTH,
                spec.counter + 8.0,
                soffit,
            ));
        }
    }

    brushes.push_str("// The fixings of the furniture. Six of them is a seating plan\n");
    let pads: Vec<(f64, f64)> = match spec.pads {
        Pads::Grid => vec![
            (-4.0, -46.0),
            (-4.0, 0.0),
            (-4.0, 46.0),
            (52.0, -46.0),
            (52.0, 0.0),
            (52.0, 46.0),
        ],
        Pads::Row => (0..6)
            .map(|index| {
                #[allow(clippy::cast_precision_loss)]
                let t = f64::from(index) - 2.5;
                (24.0, t * 26.0)
            })
            .collect(),
    };
    for (x, y) in pads {
        brushes.push_str(&translate(
            &boxed((-11.0, -11.0, FLOOR_TOP), (11.0, 11.0, FLOOR_TOP + 2.0)),
            x,
            y,
            0.0,
        ));
    }

    let mut lights = String::new();
    let (line, line_source) = ceiling_fixture(-52.0, 0.0, soffit, 26.0, 8.0);
    brushes.push_str(&line);
    lights.push_str(&line_source);
    for y in [-40.0, 40.0] {
        let (fixture, source) = ceiling_fixture(26.0, y, LEVEL - FLOOR_TOP, 22.0, 8.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = format!("// Refectory, {}: the servery is shut.\n", spec.register);
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            &format!("authored/hall_refectory_{}", spec.register),
            "hall_refectory",
            0,
            1,
            8,
        )
        .with_register_scope(spec.register)
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

/// The template: counter at 850 mm, three shutters down, tables on a grid.
const fn canteen(register: &'static str) -> Refectory {
    Refectory {
        register,
        counter: 48.0,
        shutter: Shutter::Down,
        pads: Pads::Grid,
        screen: false,
        plinth: false,
        shelves: false,
        dado: DADO,
    }
}
