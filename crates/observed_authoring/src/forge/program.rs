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

// Human dimensions, in units *above the floor surface*, at sixteen units to
// the metre. Add `FLOOR_TOP` for an absolute z.
//
// # The scale mistake, and what it turned out to mean
//
// The restroom and the refectory were first authored with a counter at 48 and a
// dado at 44 - which is a counter two and a half metres off the floor and a
// dado at two and a quarter. Both were picked by eye against a wall 128 units
// tall, and both were wrong by a factor of about two and a half. The whole
// argument for this module is that *a basin is 850 mm in every building anyone
// has ever built*, so fittings that are not at that height are not doing the
// one job they were added to do.
//
// Correcting it says something the module was groping toward. The facility is
// built at roughly 1.6x a body: four-metre doorways, eight-metre storeys. Put
// true-scale fittings in it and they read as *small* - a locker bank a quarter
// of the way up the wall, a bench you could miss. That mismatch is not an
// awkwardness to design around. **The building is not at the scale of the
// people who used it**, and the fittings are the only evidence of that, because
// they are the only things in the corpus that were made for a body.
const H_COUNTER: f64 = 14.0;
const H_DADO: f64 = 20.0;
const H_MIRROR: f64 = 23.0;
const H_STALL_HEAD: f64 = 29.0;
const H_STALL_FOOT: f64 = 5.0;
const H_SEAT: f64 = 7.0;
const H_LOCKER_HEAD: f64 = 29.0;
const H_LOCKER_PLINTH: f64 = 4.0;

/// Height of a tiled dado, absolute.
const DADO: f64 = H_DADO;

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

const STALL_HEAD: f64 = FLOOR_TOP + H_STALL_HEAD;
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
            let z = FLOOR_TOP + 10.0 + f64::from(course) * 10.0;
            brushes.push_str(&band(2, WALL, WALL + 14.0, z, z + 2.0));
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
                    brushes.push_str(&leaf(0.0, len, 7.0, FLOOR_TOP, STALL_HEAD + 6.0));
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
            let z = FLOOR_TOP + step * 4.0;
            brushes.push_str(&band(3, WALL, WALL + depth, z, z + 4.0));
        }
    }
    brushes.push_str(&band(
        3,
        WALL,
        WALL + 26.0,
        dialect.counter,
        dialect.counter + 4.0,
    ));
    brushes.push_str(&band(
        3,
        WALL,
        WALL + 8.0,
        dialect.counter + 4.0,
        dialect.counter + 12.0,
    ));
    if dialect.mirror {
        brushes.push_str("// The line the mirrors were on\n");
        brushes.push_str(&band(
            3,
            WALL + 2.0,
            WALL + 6.0,
            FLOOR_TOP + H_MIRROR,
            FLOOR_TOP + H_MIRROR + 2.0,
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
        stall_foot: FLOOR_TOP + H_STALL_FOOT,
        dado: DADO,
        shelves: false,
        counter: FLOOR_TOP + H_COUNTER,
        mirror: true,
        soffit: false,
        plinth: false,
    }
}

macro_rules! program_tiles {
    (
        $(($afn:ident, $astem:literal, $adialect:expr)),* $(,)? ;
        $(($rfn:ident, $rstem:literal, $rspec:expr)),* $(,)? ;
        $(($lfn:ident, $lstem:literal, $lspec:expr)),* $(,)? ;
        $(($wfn:ident, $wstem:literal, $wspec:expr)),* $(,)? ;
        $(($cfn:ident, $cstem:literal, $cspec:expr)),* $(,)?
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
        $(
            #[must_use]
            pub fn $lfn() -> String {
                lockers(&$lspec)
            }
        )*
        $(
            #[must_use]
            pub fn $wfn() -> String {
                waiting(&$wspec)
            }
        )*
        $(
            #[must_use]
            pub fn $cfn() -> String {
                classroom(&$cspec)
            }
        )*

        #[must_use]
        pub fn builders() -> Vec<Builder> {
            vec![
                $(($astem, $afn as fn() -> String),)*
                $(($rstem, $rfn as fn() -> String),)*
                $(($lstem, $lfn as fn() -> String),)*
                $(($wstem, $wfn as fn() -> String),)*
                $(($cstem, $cfn as fn() -> String),)*
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
            dado: 16.0,
            mirror: false,
            ..plain("monolith")
        }
    ),
    (
        hall_ablutions_overlit_grid,
        "hall_ablutions_overlit_grid",
        Dialect {
            dado: 2.0,
            mirror: false,
            soffit: true,
            stall_foot: FLOOR_TOP + 7.0,
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
            dado: 12.0,
            plinth: true,
            counter: FLOOR_TOP + H_COUNTER + 8.0,
            ..plain("facet_monument")
        }
    ),
    (
        hall_ablutions_megastructure,
        "hall_ablutions_megastructure",
        Dialect {
            dado: 26.0,
            counter: FLOOR_TOP + H_COUNTER + 8.0,
            stall_foot: FLOOR_TOP + 9.0,
            ..plain("megastructure")
        }
    ),
    (
        hall_ablutions_wellshaft,
        "hall_ablutions_wellshaft",
        Dialect {
            stall: Stall::Absent,
            dado: 10.0,
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
            dado: 18.0,
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
        Refectory { shutter: Shutter::Absent, dado: 16.0, ..canteen("monolith") }
    ),
    (
        hall_refectory_overlit_grid,
        "hall_refectory_overlit_grid",
        Refectory { shutter: Shutter::Absent, dado: 2.0, ..canteen("overlit_grid") }
    ),
    (
        hall_refectory_institutional,
        "hall_refectory_institutional",
        Refectory { shutter: Shutter::Mixed, ..canteen("institutional") }
    ),
    (
        hall_refectory_facet_monument,
        "hall_refectory_facet_monument",
        Refectory { plinth: true, counter: FLOOR_TOP + H_COUNTER + 4.0, dado: 12.0, ..canteen("facet_monument") }
    ),
    (
        hall_refectory_megastructure,
        "hall_refectory_megastructure",
        Refectory { counter: FLOOR_TOP + H_COUNTER + 8.0, dado: 26.0, ..canteen("megastructure") }
    ),
    (
        hall_refectory_wellshaft,
        "hall_refectory_wellshaft",
        Refectory { pads: Pads::Row, shutter: Shutter::Absent, dado: 10.0, ..canteen("wellshaft") }
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
        Refectory { shutter: Shutter::Mixed, dado: 18.0, ..canteen("liminal_grid") }
    );
    (
        hall_lockers_shadow_screen,
        "hall_lockers_shadow_screen",
        Lockers { bays: 10, divider: 0.008, ..bank("shadow_screen") }
    ),
    (
        hall_lockers_monolith,
        "hall_lockers_monolith",
        Lockers { bays: 4, divider: 0.03, depth: 26.0, plinth: false, head: Head::Bare, ..bank("monolith") }
    ),
    (
        hall_lockers_overlit_grid,
        "hall_lockers_overlit_grid",
        Lockers { head: Head::Soffit, shut: false, ..bank("overlit_grid") }
    ),
    (
        hall_lockers_institutional,
        "hall_lockers_institutional",
        bank("institutional")
    ),
    (
        hall_lockers_facet_monument,
        "hall_lockers_facet_monument",
        Lockers { bays: 5, divider: 0.022, dado: 12.0, ..bank("facet_monument") }
    ),
    (
        hall_lockers_megastructure,
        "hall_lockers_megastructure",
        Lockers { bays: 3, divider: 0.03, depth: 26.0, dado: 26.0, ..bank("megastructure") }
    ),
    (
        hall_lockers_wellshaft,
        "hall_lockers_wellshaft",
        Lockers { bays: 0, shut: false, dado: 10.0, ..bank("wellshaft") }
    ),
    (
        hall_lockers_infinite_gallery,
        "hall_lockers_infinite_gallery",
        Lockers { courses: true, shut: false, ..bank("infinite_gallery") }
    ),
    (
        hall_lockers_thinning,
        "hall_lockers_thinning",
        Lockers { bays: 0, plinth: false, shut: false, ..bank("thinning") }
    ),
    (
        hall_lockers_liminal_grid,
        "hall_lockers_liminal_grid",
        Lockers { dado: 18.0, ..bank("liminal_grid") }
    );
    (
        hall_waiting_shadow_screen,
        "hall_waiting_shadow_screen",
        Waiting { screen: Screen::Grille, ..queue("shadow_screen") }
    ),
    (
        hall_waiting_monolith,
        "hall_waiting_monolith",
        Waiting { screen: Screen::Open, rows: 2, queue: false, dado: 16.0, ..queue("monolith") }
    ),
    (
        hall_waiting_overlit_grid,
        "hall_waiting_overlit_grid",
        Waiting { screen: Screen::Open, dado: 2.0, ..queue("overlit_grid") }
    ),
    (
        hall_waiting_institutional,
        "hall_waiting_institutional",
        queue("institutional")
    ),
    (
        hall_waiting_facet_monument,
        "hall_waiting_facet_monument",
        Waiting { plinth: true, counter: FLOOR_TOP + H_COUNTER + 4.0, rows: 2, dado: 12.0, ..queue("facet_monument") }
    ),
    (
        hall_waiting_megastructure,
        "hall_waiting_megastructure",
        Waiting { counter: FLOOR_TOP + H_COUNTER + 8.0, rows: 2, dado: 26.0, ..queue("megastructure") }
    ),
    (
        hall_waiting_wellshaft,
        "hall_waiting_wellshaft",
        Waiting { seating: Seating::Opposed, screen: Screen::Open, queue: false, dado: 10.0, ..queue("wellshaft") }
    ),
    (
        hall_waiting_infinite_gallery,
        "hall_waiting_infinite_gallery",
        Waiting { seating: Seating::Ledge, ..queue("infinite_gallery") }
    ),
    (
        hall_waiting_thinning,
        "hall_waiting_thinning",
        Waiting { seating: Seating::None, screen: Screen::Open, queue: false, ..queue("thinning") }
    ),
    (
        hall_waiting_liminal_grid,
        "hall_waiting_liminal_grid",
        Waiting { dado: 18.0, ..queue("liminal_grid") }
    );
    (
        hall_classroom_shadow_screen,
        "hall_classroom_shadow_screen",
        Classroom { recess: Recess::Slatted, ..taught("shadow_screen") }
    ),
    (
        hall_classroom_monolith,
        "hall_classroom_monolith",
        Classroom { dais: 0, rake: 0, rows: 2, dado: 16.0, ..taught("monolith") }
    ),
    (
        hall_classroom_overlit_grid,
        "hall_classroom_overlit_grid",
        Classroom { recess: Recess::Blank, rake: 0, dado: 2.0, ..taught("overlit_grid") }
    ),
    (
        hall_classroom_institutional,
        "hall_classroom_institutional",
        taught("institutional")
    ),
    (
        hall_classroom_facet_monument,
        "hall_classroom_facet_monument",
        Classroom { dais: 3, rows: 2, dado: 12.0, ..taught("facet_monument") }
    ),
    (
        hall_classroom_megastructure,
        "hall_classroom_megastructure",
        Classroom { rake: 1, rows: 2, dado: 26.0, ..taught("megastructure") }
    ),
    (
        hall_classroom_wellshaft,
        "hall_classroom_wellshaft",
        Classroom { ring: true, dais: 0, rake: 0, recess: Recess::Blank, dado: 10.0, ..taught("wellshaft") }
    ),
    (
        hall_classroom_infinite_gallery,
        "hall_classroom_infinite_gallery",
        Classroom { recess: Recess::Shelved, ..taught("infinite_gallery") }
    ),
    (
        hall_classroom_thinning,
        "hall_classroom_thinning",
        Classroom { rake: 0, rows: 0, recess: Recess::Blank, ..taught("thinning") }
    ),
    (
        hall_classroom_liminal_grid,
        "hall_classroom_liminal_grid",
        Classroom { dado: 18.0, ..taught("liminal_grid") }
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
    // The servery opening is human even though the room is not: counter at
    // 850 mm, an opening a metre and a half above it, and a bulkhead over that.
    // A shutter that came down from an eight-metre ceiling would not be a
    // shutter, it would be a wall.
    let head = spec.counter + 24.0;
    let soffit = head + 24.0;

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
            FLOOR_TOP + 4.0,
        ));
    }

    brushes.push_str("// The servery: counter, tray rail, and the line behind it\n");
    brushes.push_str(&band(
        SERVERY,
        WALL,
        WALL + COUNTER_DEPTH,
        spec.counter,
        spec.counter + 4.0,
    ));
    brushes.push_str(&band(
        SERVERY,
        WALL + COUNTER_DEPTH,
        WALL + COUNTER_DEPTH + 4.0,
        spec.counter - 6.0,
        spec.counter - 3.0,
    ));
    if spec.shelves {
        for course in 0..2 {
            #[allow(clippy::cast_precision_loss)]
            let z = spec.counter + 8.0 + f64::from(course) * 9.0;
            brushes.push_str(&band(SERVERY, WALL, WALL + 12.0, z, z + 2.0));
        }
    } else {
        // The Unwitnessed's counter is high enough that the hot cupboard behind
        // it would grow through the soffit, and a brush whose top is under its
        // bottom is not a brush. Clamped rather than special-cased: a district
        // is allowed to build this too big, and the room is still a room.
        brushes.push_str(&band(SERVERY, WALL, WALL + 12.0, spec.counter + 4.0, head));
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
                spec.counter + 4.0,
                head,
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
    let (line, line_source) = ceiling_fixture(-52.0, 0.0, LEVEL - FLOOR_TOP, 26.0, 8.0);
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
        counter: FLOOR_TOP + H_COUNTER,
        shutter: Shutter::Down,
        pads: Pads::Grid,
        screen: false,
        plinth: false,
        shelves: false,
        dado: DADO,
    }
}

/// What tops a locker bank.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Head {
    /// A capping rail. The bank stops, and above it is wall.
    Rail,
    /// A soffit running up to the ceiling, so the bank has no top edge.
    Soffit,
    /// Nothing. The dividers just end.
    Bare,
}

/// One district's locker run.
struct Lockers {
    register: &'static str,
    /// How many bays. Zero means an open shelf run with no divisions at all.
    bays: usize,
    /// Half-width of a divider, as a fraction of the wall.
    divider: f64,
    depth: f64,
    plinth: bool,
    head: Head,
    /// One bay still has its door on.
    shut: bool,
    /// Shelf courses crossing the bays, turning lockers into pigeonholes.
    courses: bool,
    bench: bool,
    dado: f64,
}

/// The locker corridor: a bank of open bays at body pitch, and a bench.
///
/// The cheapest statement in the whole module, and possibly the loudest. A
/// locker is the only fitting in a building whose entire purpose is that a
/// specific person keeps specific things in it, so a wall of them, all open,
/// is not "storage is absent" — it is *eleven people are absent*, and it can be
/// counted off the wall.
///
/// One bay still has its door on. That is the tile.
///
/// It is also where the building's scale stops being an abstraction. The bank
/// is 1.8 m tall against a wall of eight metres, so it sits in the bottom
/// quarter and the rest of the wall runs up past it to nothing. Everything else
/// in the corpus is sized to the facility. This is sized to a person, and the
/// gap between the two is visible in one glance.
fn lockers(spec: &Lockers) -> String {
    const BANK: usize = 4;
    const BENCH: usize = 1;
    let doors = [0usize, 3];
    let plinth_top = FLOOR_TOP + if spec.plinth { H_LOCKER_PLINTH } else { 0.0 };
    let head_z = FLOOR_TOP + H_LOCKER_HEAD;

    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));

    brushes.push_str("// Envelope. Lockers line a route; you pass them\n");
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 8.0, 6.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }

    if spec.dado > 0.0 {
        brushes.push_str(&band(
            BENCH,
            WALL,
            WALL + 4.0,
            FLOOR_TOP,
            FLOOR_TOP + spec.dado,
        ));
    }

    if spec.plinth {
        brushes.push_str("// The bank stands on a plinth, so the floor sweeps under it\n");
        brushes.push_str(&band(BANK, WALL, WALL + spec.depth, FLOOR_TOP, plinth_top));
    }

    brushes.push_str("// The carcass. The doors are gone; the divisions are not\n");
    for index in 0..=spec.bays {
        #[allow(clippy::cast_precision_loss)]
        let t = f64::from(u16::try_from(index).unwrap_or(0)) / spec.bays.max(1) as f64;
        let a0 = (t - spec.divider).max(0.0);
        let a1 = (t + spec.divider).min(1.0);
        brushes.push_str(&panel(
            BANK,
            a0,
            a1,
            WALL,
            WALL + spec.depth,
            plinth_top,
            head_z,
        ));
    }
    if spec.bays == 0 {
        // No divisions at all: a shelf, not lockers. Nothing here was yours.
        brushes.push_str(&band(BANK, WALL, WALL + spec.depth, head_z - 4.0, head_z));
    }

    if spec.courses {
        brushes.push_str("// Shelf courses across the bays. Pigeonholes, not lockers\n");
        for course in 1..3 {
            #[allow(clippy::cast_precision_loss)]
            let z = plinth_top + f64::from(course) * (head_z - plinth_top) / 3.0;
            brushes.push_str(&band(BANK, WALL, WALL + spec.depth, z, z + 2.0));
        }
    }

    match spec.head {
        Head::Rail => {
            brushes.push_str(&band(
                BANK,
                WALL,
                WALL + spec.depth + 2.0,
                head_z,
                head_z + 4.0,
            ));
        }
        Head::Soffit => {
            brushes.push_str(&band(
                BANK,
                WALL,
                WALL + spec.depth + 2.0,
                LEVEL - FLOOR_TOP - 20.0,
                LEVEL - FLOOR_TOP,
            ));
        }
        Head::Bare => {}
    }

    if spec.shut && spec.bays > 1 {
        brushes.push_str("// One of them still has its door on\n");
        #[allow(clippy::cast_precision_loss)]
        let step = 1.0 / spec.bays as f64;
        let index = (spec.bays / 2) as f64;
        brushes.push_str(&panel(
            BANK,
            index * step + spec.divider,
            (index + 1.0) * step - spec.divider,
            WALL + spec.depth - 3.0,
            WALL + spec.depth,
            plinth_top,
            head_z,
        ));
    }

    if spec.bench {
        brushes.push_str("// The bench. Four hundred and fifty millimetres, as always\n");
        brushes.push_str(&band(
            BENCH,
            WALL,
            WALL + 16.0,
            FLOOR_TOP + H_SEAT,
            FLOOR_TOP + H_SEAT + 3.0,
        ));
    }

    let mut lights = String::new();
    for x in [-40.0, 40.0] {
        let (fixture, source) = ceiling_fixture(x, 0.0, LEVEL - FLOOR_TOP, 22.0, 8.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = format!(
        "// Lockers, {}: one of them is still shut.\n",
        spec.register
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            &format!("authored/hall_lockers_{}", spec.register),
            "hall_lockers",
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

/// The template: eight bays, a plinth, a capping rail, one door left on.
const fn bank(register: &'static str) -> Lockers {
    Lockers {
        register,
        bays: 8,
        divider: 0.012,
        depth: 20.0,
        plinth: true,
        head: Head::Rail,
        shut: true,
        courses: false,
        bench: true,
        dado: 0.0,
    }
}

/// What stands between you and whoever was behind the counter.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    /// A plane above the counter with a gap in it. You talk through a hole.
    Glazed,
    /// Thin verticals. You talk through a gap between bars.
    Grille,
    /// Nothing. The counter is open to the room.
    Open,
}

/// Where the waiting happened.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Seating {
    /// Rows facing the counter. Everyone looks the same way, at the same shut
    /// window.
    Facing,
    /// Two runs down opposite walls, facing each other across the room.
    Opposed,
    /// A continuous ledge. You wait standing, leaning.
    Ledge,
    /// Nowhere. You wait on your feet.
    None,
}

/// One district's waiting area.
struct Waiting {
    register: &'static str,
    counter: f64,
    screen: Screen,
    seating: Seating,
    rows: usize,
    /// A barrier defining where the queue went.
    queue: bool,
    plinth: bool,
    dado: f64,
}

/// The waiting area: seats, a barrier, and a window that is shut.
///
/// # Why this is the most liminal room there is
///
/// Every other room in a building is *for* something — you eat here, you wash
/// here, you keep your coat here. This one is for **delay**. Its entire purpose
/// is the interval between arriving and being dealt with, which means it was
/// already, when full, a room nobody wanted to be in and everybody was in
/// anyway. Emptying it does not change what it is for. It just removes the only
/// thing that ever ended the wait.
///
/// # The seats are still here, and that is the point
///
/// Everywhere else in this module the furniture was taken and the fixings are
/// the evidence. Waiting-room seating is beam-mounted steel bolted to the slab,
/// which is exactly why it is the one kind of furniture that survives an
/// evacuation: nobody could carry it and nobody wanted it. So this room is not
/// the shape of absent seating. It is **rows of seats with nobody in them**,
/// facing a shut window, which is worse.
///
/// Three things say waiting room and none of them is a chair:
///
/// 1. **The window with a gap in it.** A counter you speak through rather than
///    across. It is the only fitting in a building that is explicitly a barrier
///    to the person it serves.
/// 2. **The queue barrier.** Nothing but a queue has one, and it is still
///    standing, still defining a route to a window nobody is behind.
/// 3. **Rows.** All facing one way, at one thing.
fn waiting(spec: &Waiting) -> String {
    const DESK: usize = 4;
    let doors = [0usize, 3];
    let seat = FLOOR_TOP + H_SEAT;
    let screen_head = spec.counter + 26.0;

    let mut brushes = String::from("// Floor and lid\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));

    brushes.push_str("// Envelope. A waiting area is a widening of a route\n");
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 8.0, 6.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }

    if spec.dado > 0.0 {
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
        brushes.push_str("// The desk on a plinth. You are received, not served\n");
        brushes.push_str(&band(DESK, WALL, WALL + 26.0, FLOOR_TOP, FLOOR_TOP + 4.0));
    }

    brushes.push_str("// The counter\n");
    brushes.push_str(&band(
        DESK,
        WALL,
        WALL + 22.0,
        spec.counter,
        spec.counter + 4.0,
    ));

    match spec.screen {
        Screen::Glazed => {
            brushes.push_str("// The screen, and the gap you had to speak through\n");
            for (a0, a1) in [(0.10, 0.44), (0.56, 0.90)] {
                brushes.push_str(&panel(
                    DESK,
                    a0,
                    a1,
                    WALL + 8.0,
                    WALL + 11.0,
                    spec.counter + 4.0,
                    screen_head,
                ));
            }
            brushes.push_str(&panel(
                DESK,
                0.44,
                0.56,
                WALL + 8.0,
                WALL + 11.0,
                spec.counter + 12.0,
                screen_head,
            ));
        }
        Screen::Grille => {
            brushes.push_str("// Bars. You spoke between them\n");
            for index in 0..5 {
                #[allow(clippy::cast_precision_loss)]
                let t = 0.12 + f64::from(index) * 0.19;
                brushes.push_str(&panel(
                    DESK,
                    t,
                    t + 0.03,
                    WALL + 8.0,
                    WALL + 11.0,
                    spec.counter + 4.0,
                    screen_head,
                ));
            }
        }
        Screen::Open => {}
    }

    brushes.push_str("// The bracket the numbers were called on\n");
    brushes.push_str(&panel(
        DESK,
        0.40,
        0.60,
        WALL,
        WALL + 6.0,
        screen_head + 8.0,
        screen_head + 14.0,
    ));

    if spec.queue {
        brushes.push_str("// The barrier. Still defining a route to a shut window\n");
        for (a0, a1, depth) in [(0.04, 0.62, 30.0), (0.38, 0.96, 42.0)] {
            brushes.push_str(&panel(
                DESK,
                a0,
                a1,
                WALL + depth,
                WALL + depth + 3.0,
                FLOOR_TOP + 14.0,
                FLOOR_TOP + 17.0,
            ));
        }
    }

    match spec.seating {
        Seating::Facing => {
            brushes.push_str("// Rows, all looking the same way, at the same shut window\n");
            for row in 0..spec.rows {
                #[allow(clippy::cast_precision_loss)]
                let depth = WALL + 56.0 + f64::from(u16::try_from(row).unwrap_or(0)) * 24.0;
                brushes.push_str(&panel(
                    DESK,
                    0.16,
                    0.84,
                    depth,
                    depth + 8.0,
                    seat,
                    seat + 3.0,
                ));
                brushes.push_str(&panel(
                    DESK,
                    0.26,
                    0.74,
                    depth + 2.0,
                    depth + 6.0,
                    FLOOR_TOP,
                    seat,
                ));
            }
        }
        Seating::Opposed => {
            brushes.push_str("// Two runs, facing each other. Nobody here faced a window\n");
            for face in [1usize, 2] {
                brushes.push_str(&band(face, WALL, WALL + 14.0, seat, seat + 3.0));
                brushes.push_str(&band(face, WALL + 3.0, WALL + 11.0, FLOOR_TOP, seat));
            }
        }
        Seating::Ledge => {
            brushes.push_str("// A ledge. You waited standing, and you read\n");
            brushes.push_str(&band(
                2,
                WALL,
                WALL + 14.0,
                spec.counter,
                spec.counter + 3.0,
            ));
            for course in 0..2 {
                #[allow(clippy::cast_precision_loss)]
                let z = spec.counter + 10.0 + f64::from(course) * 9.0;
                brushes.push_str(&band(2, WALL, WALL + 12.0, z, z + 2.0));
            }
        }
        Seating::None => {}
    }

    let mut lights = String::new();
    let (over, over_source) = ceiling_fixture(-44.0, -30.0, LEVEL - FLOOR_TOP, 20.0, 8.0);
    brushes.push_str(&over);
    lights.push_str(&over_source);
    let (room, room_source) = ceiling_fixture(30.0, 20.0, LEVEL - FLOOR_TOP, 24.0, 8.0);
    brushes.push_str(&room);
    lights.push_str(&room_source);

    let mut out = format!("// Waiting, {}: the window is shut.\n", spec.register);
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            &format!("authored/hall_waiting_{}", spec.register),
            "hall_waiting",
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

/// The template: a glazed screen with a gap, a barrier, three rows facing it.
const fn queue(register: &'static str) -> Waiting {
    Waiting {
        register,
        counter: FLOOR_TOP + H_COUNTER,
        screen: Screen::Glazed,
        seating: Seating::Facing,
        rows: 3,
        queue: true,
        plinth: false,
        dado: 0.0,
    }
}

/// What is left of the surface that was written on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Recess {
    /// A framed opening in the wall. The board was screwed into it and is gone;
    /// the frame is masonry and is not.
    Framed,
    /// Thin verticals across it. Taught from behind bars.
    Slatted,
    /// Shelf courses. The wall you consult rather than the wall you write on.
    Shelved,
    /// Nothing. No surface, and so nothing that could have been said here.
    Blank,
}

/// One district's teaching room.
struct Classroom {
    register: &'static str,
    /// Steps up to the dais. Zero means the floor does not change.
    dais: usize,
    /// Tiers the floor rises in, away from the dais.
    rake: usize,
    /// Rows of desk-and-bench.
    rows: usize,
    recess: Recess,
    /// Desks round the walls facing each other instead of rows facing a front.
    ring: bool,
    dado: f64,
}

/// Height of a writing surface: 730 mm. The number that makes a desk a desk
/// rather than a bench, because it is the one a forearm needs.
const H_DESK: f64 = 12.0;
/// Rise of one dais step or one tier of rake.
const STEP: f64 = 4.0;

/// The classroom: everything in it aimed at a place nobody is standing.
///
/// # The only room here that points
///
/// The other three program rooms are shaped by an activity that is not
/// happening — you cannot wash, you cannot eat, you cannot wait for anything.
/// This one is shaped by a *person* who is not there. The floor rakes toward
/// one end, the desks face that end, the framed recess is centred on it, and on
/// the dais there is a bolt pad where the lectern stood. Every line in the plan
/// converges on a rectangle of empty platform about the size of a body.
///
/// That is a different kind of absence and a sharper one. A canteen with no
/// food in it is disused. A room in which forty seats and a raked floor are all
/// still carefully aimed at one unoccupied square metre is *waiting*, and it is
/// the only thing in the corpus that looks like it expects someone.
///
/// # Two heights, not one
///
/// A waiting room's seating is one height. A classroom's is two: a bench at
/// 450 mm and a writing surface at 730. That pair is what distinguishes this
/// from every other row of fixed seating in a building, at a glance, from the
/// door — and it is the reason the desks are drawn as two members rather than
/// one, which costs a brush a row and is worth it.
fn classroom(spec: &Classroom) -> String {
    const FRONT: usize = 4;
    let doors = [0usize, 3];

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

    if spec.dado > 0.0 {
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

    // The dais, and the pad where the lectern was bolted.
    let dais_top = FLOOR_TOP + STEP * spec.dais as f64;
    if spec.dais > 0 {
        brushes.push_str("// The dais. One step is enough to make a room point\n");
        for step in 0..spec.dais {
            #[allow(clippy::cast_precision_loss)]
            let inset = f64::from(u16::try_from(step).unwrap_or(0)) * 8.0;
            brushes.push_str(&panel(
                FRONT,
                0.06,
                0.94,
                WALL,
                WALL + 36.0 - inset,
                FLOOR_TOP,
                FLOOR_TOP + STEP * (f64::from(u16::try_from(step).unwrap_or(0)) + 1.0),
            ));
        }
        brushes.push_str("// Where the lectern was bolted. A body's worth of nothing\n");
        brushes.push_str(&panel(
            FRONT,
            0.44,
            0.58,
            WALL + 14.0,
            WALL + 26.0,
            dais_top,
            dais_top + 2.0,
        ));
    }

    // The wall that was written on.
    let board_low = dais_top + 16.0;
    let board_high = dais_top + 38.0;
    match spec.recess {
        Recess::Framed => {
            brushes.push_str("// The frame the board was screwed into\n");
            for (a0, a1) in [(0.06, 0.16), (0.84, 0.94)] {
                brushes.push_str(&panel(
                    FRONT,
                    a0,
                    a1,
                    WALL,
                    WALL + 5.0,
                    board_low,
                    board_high,
                ));
            }
            brushes.push_str(&panel(
                FRONT,
                0.06,
                0.94,
                WALL,
                WALL + 5.0,
                board_high,
                board_high + 4.0,
            ));
        }
        Recess::Slatted => {
            for index in 0..6 {
                #[allow(clippy::cast_precision_loss)]
                let t = 0.10 + f64::from(index) * 0.14;
                brushes.push_str(&panel(
                    FRONT,
                    t,
                    t + 0.03,
                    WALL,
                    WALL + 5.0,
                    board_low,
                    board_high,
                ));
            }
        }
        Recess::Shelved => {
            for course in 0..3 {
                #[allow(clippy::cast_precision_loss)]
                let z = board_low + f64::from(course) * 8.0;
                brushes.push_str(&band(FRONT, WALL, WALL + 12.0, z, z + 2.0));
            }
        }
        Recess::Blank => {}
    }

    if spec.ring {
        brushes.push_str("// Desks round the walls. Nobody here was taught from the front\n");
        for face in [1usize, 2, 5] {
            brushes.push_str(&band(
                face,
                WALL,
                WALL + 14.0,
                FLOOR_TOP + H_DESK,
                FLOOR_TOP + H_DESK + 2.0,
            ));
            brushes.push_str(&band(
                face,
                WALL + 16.0,
                WALL + 26.0,
                FLOOR_TOP + H_SEAT,
                FLOOR_TOP + H_SEAT + 3.0,
            ));
        }
    } else {
        for tier in 0..spec.rake {
            #[allow(clippy::cast_precision_loss)]
            let index = f64::from(u16::try_from(tier).unwrap_or(0));
            let near = WALL + 58.0 + index * 34.0;
            brushes.push_str(&panel(
                FRONT,
                0.10 - index * 0.02,
                0.90 + index * 0.02,
                near,
                near + 34.0,
                FLOOR_TOP,
                FLOOR_TOP + STEP * (index + 1.0),
            ));
        }
        brushes.push_str("// Desk and bench: two heights, which is what makes it a desk\n");
        for row in 0..spec.rows {
            #[allow(clippy::cast_precision_loss)]
            let index = f64::from(u16::try_from(row).unwrap_or(0));
            let base = FLOOR_TOP + STEP * index.min(spec.rake as f64);
            let near = WALL + 42.0 + index * 34.0;
            brushes.push_str(&panel(
                FRONT,
                0.14,
                0.86,
                near,
                near + 9.0,
                base + H_DESK,
                base + H_DESK + 2.0,
            ));
            brushes.push_str(&panel(
                FRONT,
                0.18,
                0.82,
                near + 11.0,
                near + 18.0,
                base + H_SEAT,
                base + H_SEAT + 3.0,
            ));
        }
    }

    let mut lights = String::new();
    let (front, front_source) = ceiling_fixture(-46.0, -34.0, LEVEL - FLOOR_TOP, 22.0, 8.0);
    brushes.push_str(&front);
    lights.push_str(&front_source);
    let (back, back_source) = ceiling_fixture(28.0, 22.0, LEVEL - FLOOR_TOP, 24.0, 8.0);
    brushes.push_str(&back);
    lights.push_str(&back_source);

    let mut out = format!("// Classroom, {}: aimed at nobody.\n", spec.register);
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            &format!("authored/hall_classroom_{}", spec.register),
            "hall_classroom",
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

/// The template: one step of dais, two tiers of rake, three rows, a framed
/// recess where the board was.
const fn taught(register: &'static str) -> Classroom {
    Classroom {
        register,
        dais: 1,
        rake: 2,
        rows: 3,
        recess: Recess::Framed,
        ring: false,
        dado: 0.0,
    }
}
