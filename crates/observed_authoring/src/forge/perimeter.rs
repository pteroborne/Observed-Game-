//! Perimeter ramps: a climb that hugs the outer wall instead of crossing the cell.
//!
//! `hall_ramp` variant 0 drives a single solid mass straight across the hex,
//! climbing 8 m in 14 m at slope 0.57 - near the 0.65 the validator rejects at.
//! Going round the wall instead buys run: two faces is 0.50, and a body sees the
//! room turn around it rather than a ramp filling it.
//!
//! **A family, not a tile.** This shares the straight ramp's signature - one
//! lateral door plus an `up` `ramp_open` port - so the solver picks freely
//! between them for the same demand. Sixfold rotation covers all six exit
//! directions from one source; compiled variants land at 6..11, clear of the
//! generated family's 0..5.
//!
//! ## What a `hall_ramp` owes the cell above it
//!
//! The prefab spans two levels and **`ramp_head` is geometry-free**: the upper
//! cell is a real cell the facility routes through, and every scrap of floor and
//! wall it has comes from here. Two things follow, both of which this module
//! shipped without, and together they trapped any bot routed over a perimeter
//! ramp:
//!
//! - **A deck across the upper level.** A landing wedge is enough to reach the
//!   `up` port at the centre and nothing else. A body that stepped off it walked
//!   into open air and fell 8 m back onto the spiral, then climbed again - 30,731
//!   ticks of one match spent going round. The stall was a loop, not a snag.
//! - **A door out of the upper level**, on the face opposite the entry, exactly
//!   as `ramp_map` emits. Without it the head cell is sealed and a body that
//!   reaches the top has nowhere to go.
//!
//! ## Why there is only one extent
//!
//! Entry and exit are opposite faces, so the climb has to fit between them: the
//! two free runs are two faces each. 180 and 240 degree members existed here
//! and could not have worked - they swept the face the upper door needs. The
//! gentler slope was never available; it only looked available because nothing
//! checked that a body could leave.

use super::GENERATED_NOTE;
use super::entities::{
    Meta, lateral_port, stair_node, tile_cell, vertical_port, wall_fixture, worldspawn,
};
use super::geometry::{
    DOOR_TOP, FLOOR_TOP, LEVEL, P2, P3, corners, door_wall, hex_slab, prism, sloped_prism, wall,
};

/// Where the sweep begins: the run between the two doors.
const SWEEP_START: usize = 4;

/// The entry face, at level 0. Also the straight ramp's, so the signatures match.
const ENTRY_FACE: usize = 3;

/// The exit face, at level 1. Opposite the entry, as `ramp_map` has it - the
/// head cell has no geometry of its own, so this is its only way out.
const EXIT_FACE: usize = (ENTRY_FACE + 3) % 6;

/// Inner ring as a fraction of the outer hexagon.
///
/// 0.55 leaves a band about 50 units (3.1 m) wide - a corridor a body walks
/// rather than a ledge it edges along - and, because neighbouring facets share
/// their inner corner exactly, adjacent facets meet without mitring.
const INNER: f64 = 0.55;

/// The deck above, which a climb must finish exactly on: one level plus the
/// floor slab. Short of it is a gap; proud of it is a lip that, past the
/// autostep, stops a body walking back onto the flight.
const DECK_ABOVE: f64 = LEVEL + FLOOR_TOP;

/// One member of the family.
#[derive(Clone, Copy, Debug)]
pub struct Extent {
    /// How many 60-degree faces the climb sweeps.
    pub faces: usize,
    /// Its authored variant slot. Sixfold rotation compiles this to
    /// `6n..6n+5`, so 1 lands at 6..11 - clear of the generated 0..5.
    pub variant: i32,
    /// How far out the climb reaches, as a fraction of the hexagon.
    ///
    /// `1.0` hugs the wall, which is what a perimeter ramp wants: it has one
    /// fixed entry and the climb may consume every other face. A tower cannot,
    /// because its doors can be anywhere - so it pulls the climb in and leaves
    /// the rim as a walkable ring the doors open onto. Same fan, smaller radius.
    pub outer_scale: f64,
}

/// The family. One member: see the module docs on why there is only one.
///
/// Two faces is 120 degrees at slope 0.50 - gentler than the straight ramp's
/// 0.57 and well under the controller's 36-degree slide. One face would be
/// 0.99, and anything wider sweeps a face a door needs.
#[must_use]
pub fn extents() -> [Extent; 1] {
    [Extent {
        faces: 2,
        variant: 1,
        // A perimeter ramp is defined by hugging the wall.
        outer_scale: 1.0,
    }]
}

impl Extent {
    /// The face index the sweep begins at. A tower needs it to route its deck
    /// round to the foot of the climb.
    #[must_use]
    pub fn start_face(self) -> usize {
        SWEEP_START
    }

    #[must_use]
    pub fn stem(self) -> String {
        format!("hall_ramp_perimeter_{}", self.faces * 60)
    }

    /// Height at the `step`th corner of the sweep, in TB units.
    ///
    /// Linear in the corner index, so every facet gains the same amount and the
    /// walk has one slope throughout. A body should not meet a change of
    /// gradient halfway up for no reason.
    #[must_use]
    fn height(self, step: usize) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let t = step as f64 / self.faces as f64;
        FLOOR_TOP + (DECK_ABOVE - FLOOR_TOP) * t
    }

    /// Outer corner index `step` along the sweep.
    #[must_use]
    fn outer(self, step: usize) -> P2 {
        let c = corners()[(SWEEP_START + step) % 6];
        (c.0 * self.outer_scale, c.1 * self.outer_scale)
    }

    /// The matching point on the inner ring.
    #[must_use]
    fn inner(self, step: usize) -> P2 {
        let c = self.outer(step);
        (c.0 * INNER, c.1 * INNER)
    }
}

/// The climb, as triangles.
///
/// **Two per face, not one quad.** The radial seams between facets have to be
/// level - outer and inner end at the same height - or a body crossing one
/// walks a sideways tilt. Two level radial edges at different heights do not lie
/// on a plane, so a quad cannot do it. `_ring_ramp` in the silo family splits
/// for exactly this reason.
#[must_use]
pub(super) fn flight(extent: Extent) -> String {
    let mut out = String::new();
    for step in 0..extent.faces {
        let (o0, o1) = (extent.outer(step), extent.outer(step + 1));
        let (i0, i1) = (extent.inner(step), extent.inner(step + 1));
        let (h0, h1) = (extent.height(step), extent.height(step + 1));

        out.push_str(&sloped_prism(
            &[o0, o1, i0],
            0.0,
            [(o0.0, o0.1, h0), (o1.0, o1.1, h1), (i0.0, i0.1, h0)],
            None,
        ));
        out.push_str(&sloped_prism(
            &[i0, o1, i1],
            0.0,
            [(i0.0, i0.1, h0), (o1.0, o1.1, h1), (i1.0, i1.1, h1)],
            None,
        ));
    }
    out
}

/// The deck the climb arrives on.
///
/// Without it the flight tops out at the rim in mid-air while the `up` port
/// sits at the cell centre with nothing joining them. It spans the exit seam -
/// outer and inner, both level at the deck height - across to the centre and
/// one face beyond, so a body steps off and walks to the opening.
#[must_use]
pub(super) fn landing(extent: Extent) -> String {
    let end = extent.faces;
    let plan: Vec<P2> = vec![
        extent.outer(end),
        extent.inner(end),
        (0.0, 0.0),
        extent.inner(end + 1),
        extent.outer(end + 1),
    ];
    prism(&plan, LEVEL, DECK_ABOVE, None, 2.0, 0.0)
}

/// The floor of the upper level: everything the climb does not sweep.
///
/// A `hall_ramp` prefab spans two levels, and the upper one is a real cell the
/// facility routes *through* — a body arrives at the top and then walks on to a
/// lateral neighbour. The wedge `landing` provides is enough to reach the `up`
/// port at the centre and nothing else, so a body that stepped off it toward
/// any other face walked into open air and fell 8 m back onto the spiral. It
/// then climbed again, and again: 30,731 ticks of one match spent going round,
/// which is what "the spectator stalled" turned out to mean. Not stuck —
/// looping.
///
/// The swept band is deliberately left open, because that is the stairwell: a
/// deck over it would be a lid on the climb, which is the same fault the
/// generated capped towers shipped with.
///
/// Emitted as the inner hexagon plus one quad per unswept face rather than as
/// one polygon, because a hexagon with a bite out of its rim is not convex and
/// a brush must be.
#[must_use]
pub(super) fn upper_deck(extent: Extent) -> String {
    let mut out = prism(
        &(0..6).map(|c| extent.inner(c)).collect::<Vec<P2>>(),
        LEVEL,
        DECK_ABOVE,
        None,
        2.0,
        0.0,
    );
    for step in extent.faces..6 {
        let plan = vec![
            extent.outer(step),
            extent.outer(step + 1),
            extent.inner(step + 1),
            extent.inner(step),
        ];
        out.push_str(&prism(&plan, LEVEL, DECK_ABOVE, None, 2.0, 0.0));
    }
    out
}

/// The climbable line, down the middle of the band.
///
/// Geometry does not tell a follower where a climb goes, and this one goes round
/// a wall: a straight line across the cell finds the open middle and stops. The
/// first node sits on the flat deck at the foot and the last on the landing, so
/// the ends join the flat circulation either side without a special case.
#[must_use]
pub(super) fn spine(extent: Extent) -> String {
    let mid = |step: usize| -> P2 {
        let (o, i) = (extent.outer(step), extent.inner(step));
        ((o.0 + i.0) * 0.5, (o.1 + i.1) * 0.5)
    };

    let mut nodes: Vec<P3> = Vec::new();
    // On the deck, short of the ramp, so a body joins the climb from the floor.
    let foot = mid(0);
    nodes.push((foot.0 * 0.5, foot.1 * 0.5, FLOOR_TOP));
    for step in 0..=extent.faces {
        let point = mid(step);
        nodes.push((point.0, point.1, extent.height(step)));
    }
    // Off the flight and onto the landing, toward the opening at the centre.
    let head = mid(extent.faces);
    nodes.push((head.0 * 0.45, head.1 * 0.45, DECK_ABOVE));

    let mut out = String::new();
    for (index, node) in nodes.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        out.push_str(&stair_node(index as u16, node.0, node.1, node.2));
    }
    out
}

/// One perimeter ramp.
#[must_use]
pub fn perimeter_ramp(extent: Extent) -> String {
    let top = 2.0 * LEVEL;
    let mut brushes = String::from("// Ground slab: the flat centre a body crosses to the foot\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 2.0, 0.0));
    brushes.push_str(&format!(
        "// Perimeter flight, {} degrees, two triangles per face\n",
        extent.faces * 60
    ));
    brushes.push_str(&flight(extent));
    brushes.push_str("// Upper deck: the floor of the level above, stairwell left open\n");
    brushes.push_str(&upper_deck(extent));

    brushes.push_str("// Envelope: two levels, so the walls run the full height\n");
    for face in 0..6 {
        if face == ENTRY_FACE {
            brushes.push_str(&door_wall(face, 0.0, top, FLOOR_TOP, DOOR_TOP, 10.0, 8.0));
        } else if face == EXIT_FACE {
            // The way out of the head cell, which has no geometry of its own.
            brushes.push_str(&door_wall(
                face,
                0.0,
                top,
                FLOOR_TOP + LEVEL,
                DOOR_TOP + LEVEL,
                10.0,
                8.0,
            ));
        } else {
            brushes.push_str(&wall(face, 0.0, top));
        }
    }
    brushes.push_str(&hex_slab(top - FLOOR_TOP, top, 0.0, 3.0));

    // One practical at the foot and one at the head, so the route reads as a
    // route rather than a lit room with a dark rim.
    let mut lights = String::new();
    for (face, z) in [
        (SWEEP_START % 6, 88.0),
        ((SWEEP_START + extent.faces) % 6, 184.0),
    ] {
        let (fixture, source) = wall_fixture(face, 0.5, z, 20.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = format!(
        "// Perimeter ramp, {} degrees: enter west, climb the outer wall, exit a level up.\n",
        extent.faces * 60
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            &format!("authored/{}", extent.stem()),
            "hall_ramp",
            extent.variant,
            2,
            7,
        )
        .with_register_scope("all")
        .emit(),
    );
    // "open", not "ramp": the ramp floor policy wants a surface over the cell
    // centre spanning most of a level, which assumes a full-width ramp. A
    // perimeter climb leaves the centre open, the same declaration silo_ring
    // makes.
    out.push_str(&tile_cell(0, 0, 0, 2, "open"));
    out.push_str(&lateral_port(ENTRY_FACE, "door", "west_entry", 0, 0, 0));
    out.push_str(&vertical_port("up", "ramp_open", "upper_ramp", 0));
    out.push_str(&spine(extent));
    out.push_str(&lights);
    out
}

/// Every perimeter ramp, paired with the file it must reproduce.
#[must_use]
pub fn builders() -> Vec<(String, String)> {
    extents()
        .into_iter()
        .map(|extent| (extent.stem(), perimeter_ramp(extent)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `hall_ramp` owes its head cell a floor and a way out, because
    /// `ramp_head` has no geometry of its own.
    ///
    /// Without the deck a body steps off the landing into open air and falls
    /// back onto the spiral; without the upper door it reaches the top of a
    /// sealed room. Both shipped, and together they turned every facility that
    /// placed a perimeter ramp on a route into an unfinishable match.
    #[test]
    fn the_head_cell_gets_a_floor_and_a_way_out() {
        for extent in extents() {
            let text = perimeter_ramp(extent);
            let deck = upper_deck(extent);
            assert!(
                text.contains(&deck) && !deck.is_empty(),
                "{} ships no upper deck",
                extent.stem()
            );
            // The deck must reach every face the climb does not sweep, or the
            // parts of the upper level it does not cover are a fall.
            let brushes = deck.matches("( ").count();
            assert!(
                brushes > 0 && deck.len() > landing(extent).len(),
                "{} upper deck is no larger than the old landing wedge",
                extent.stem()
            );
            // And the door out, on the face opposite the entry.
            let swept: Vec<usize> = (0..extent.faces).map(|s| (SWEEP_START + s) % 6).collect();
            assert!(
                !swept.contains(&EXIT_FACE),
                "{} sweeps {swept:?}, burying the upper door on face {EXIT_FACE}",
                extent.stem()
            );
        }
    }

    /// The climb must leave both doors clear. The entry is at level 0 and the
    /// exit at level 1, but a swept face is solid at every height.
    #[test]
    fn neither_door_opens_into_the_climb() {
        for extent in extents() {
            let swept: Vec<usize> = (0..extent.faces).map(|s| (SWEEP_START + s) % 6).collect();
            for door in [ENTRY_FACE, EXIT_FACE] {
                assert!(
                    !swept.contains(&door),
                    "{} sweeps {swept:?}, which buries the door on face {door}",
                    extent.stem()
                );
            }
        }
    }

    /// Every extent must climb exactly one level and land on the deck above.
    /// Short is a gap, proud is a lip the autostep cannot get back over.
    #[test]
    fn every_extent_lands_flush_on_the_deck_above() {
        for extent in extents() {
            assert!((extent.height(0) - FLOOR_TOP).abs() < 1e-9);
            assert!(
                (extent.height(extent.faces) - DECK_ABOVE).abs() < 1e-9,
                "{} tops out at {}",
                extent.stem(),
                extent.height(extent.faces)
            );
        }
    }

    /// The entry must never be a face the ramp swept, or the door opens into
    /// solid mass. This is what fixes the sweep's start corner.
    #[test]
    fn the_entry_face_is_never_swept() {
        for extent in extents() {
            let swept: Vec<usize> = (0..extent.faces).map(|s| (SWEEP_START + s) % 6).collect();
            assert!(
                !swept.contains(&ENTRY_FACE),
                "{} sweeps {swept:?}, which covers the entry",
                extent.stem()
            );
        }
    }

    /// Every extent must be walkable and, being a family, must stay under the
    /// limits the straight ramp is already close to.
    #[test]
    fn every_extent_is_gentler_than_the_straight_ramp() {
        // hall_ramp variant 0: 8 m over 14 m.
        let straight = 8.0 / 14.0;
        for extent in extents() {
            #[allow(clippy::cast_precision_loss)]
            let run = extent.faces as f64 * 128.0;
            let slope = (DECK_ABOVE - FLOOR_TOP) / run;
            assert!(
                slope < straight,
                "{} at slope {slope:.2} is not gentler than {straight:.2}",
                extent.stem()
            );
            assert!(
                slope.atan().to_degrees() < 36.0,
                "{} exceeds the controller's slide angle",
                extent.stem()
            );
        }
    }

    /// The family shares one signature, or the solver cannot treat them as
    /// alternatives for the same demand - which is the entire point.
    #[test]
    fn the_family_shares_one_port_signature() {
        for extent in extents() {
            let text = perimeter_ramp(extent);
            assert!(
                text.contains("\"name\" \"west_entry\""),
                "{}",
                extent.stem()
            );
            assert!(
                text.contains("\"class\" \"ramp_open\""),
                "{}",
                extent.stem()
            );
            assert_eq!(
                text.matches("\"classname\" \"tile_port\"").count(),
                2,
                "{} should carry exactly the two ports of the family",
                extent.stem()
            );
        }
    }

    /// Variants index the compiled catalog, so a collision would have two
    /// sources claiming one slot - and variant 0 is the straight ramp.
    #[test]
    fn variants_are_distinct_and_leave_the_straight_ramp_alone() {
        let mut seen: Vec<i32> = extents().iter().map(|e| e.variant).collect();
        let total = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(total, seen.len());
        assert!(!seen.contains(&0), "variant 0 belongs to hall_ramp");
    }

    /// A spine is what makes a perimeter climb followable at all.
    #[test]
    fn every_extent_carries_a_rising_spine() {
        for extent in extents() {
            let text = perimeter_ramp(extent);
            let nodes = text.matches("\"classname\" \"tile_stair_node\"").count();
            assert!(
                nodes >= extent.faces + 3,
                "{} has {nodes} spine nodes for {} faces",
                extent.stem(),
                extent.faces
            );
        }
    }

    /// Every member must survive the importer it is written for.
    #[test]
    fn every_extent_parses_and_validates() {
        for (stem, text) in builders() {
            crate::parse_authored_module(&text)
                .unwrap_or_else(|error| panic!("{stem} does not validate: {error:?}"));
        }
    }
}
