//! Stair towers: the first authored geometry for the facility's vertical
//! circulation.
//!
//! `stair_tower` is the most-walked element in the facility - about 64 demands
//! in a production solve - and until now it had **no authored coverage at all**.
//! Every shaft rendered from `tile_source::verticals`, the procedural switchback
//! whose spine "stalled all four soak bots on the first run of this code".
//!
//! A switchback doubles back, so two flights pass overhead each other and a body
//! on the deck comes out nearer the flight above than the floor it is standing
//! on. `StairSpine::self_crossing` exists to police exactly that. A helix cannot
//! produce it: the climb never returns over itself.
//!
//! ## Why the archetype is `stair_tower` and not `stair_segment`
//!
//! The generated library names its towers `stair_segment` / `stair_top` /
//! `stair_bottom` / `stair_landing` to keep manifest keys unique, and
//! `compatibility_archetype` flattens all four to `stair_tower` on the way in.
//! **That rewrite happens only in `compatibility_cells`.** Authored `.map`
//! modules never pass through it and keep the archetype they declare, so an
//! authored tower declares the runtime name directly. The four source names are
//! not the authoring interface.
//!
//! ## Why this is an addition, not a replacement
//!
//! `tile_for` picks a tower per *column*, from the base cell's register, because
//! mixing tower shapes within a column tops a lower flight out under the upper
//! cell's solid deck and stops a body climbing. That makes partial coverage the
//! dangerous case - and it does not arise here. `Catalogue::new` keys
//! `(archetype, register, signature)` to a `Vec`, and `select` runs
//! `weighted_select` over it, so this tower joins the same bucket as the
//! generated one and is chosen by weight. Every signature keeps a complete
//! fallback; no column can be stranded.

use super::GENERATED_NOTE;
use super::entities::{Meta, deck_node, tile_cell, vertical_port, wall_fixture, worldspawn};
use super::geometry::{FLOOR_TOP, LEVEL, corners, hex_slab, wall};
use super::liminal::hex_opening_slab;
use super::perimeter::{Extent, flight, landing, spine};

/// How far the climb sweeps. Fixed, and that is the point.
///
/// A tower's climb geometry may depend on the register and on nothing else.
/// `tile_for` pins a column's *register* but leaves the signature and the
/// variation key per cell, so two cells in one column can differ by their
/// doors. If the sweep were chosen from the doors their flights would not line
/// up: a previous attempt did exactly that and stalled 23 of 24 seeds, and
/// forcing the sweep to ignore the doors instead put doors into the mass of the
/// flight and stalled 24 of 24.
///
/// Fixing the sweep is half the answer. The other half is [`OUTER_SCALE`].
const SWEEP: usize = 4;

/// How far out the climb reaches, as a fraction of the hexagon.
///
/// **This is what lets the sweep be fixed.** Pulled in off the wall, the climb
/// leaves a walkable ring at the rim that every door opens onto, so a door
/// never meets the flight and the flight never has to know where the doors are.
/// It is the switchback's own answer: its flights sit at x -80..60, well clear
/// of the rim, ringed by a grounded circulation deck, which is why it tolerates
/// any door pattern with one geometry.
///
/// At 0.75 the ring is 1.88 m wide and the band 2.53 m, against a body 0.76 m
/// across. The climb runs 298 units for its 128 of rise: a slope of 0.43 at 23
/// degrees, inside the validator's 0.65 and the controller's 36.
const OUTER_SCALE: f64 = 0.75;

/// Weight against the generated switchback in the same bucket.
///
/// Deliberately not overwhelming. This is the first authored tower and it has
/// never been played; giving it a modest share means a facility mixes the two
/// rather than betting every shaft on geometry no human has walked. Raise it
/// once it has been.
const WEIGHT: u32 = 6;

/// How a tower connects vertically. This is what decides its port signature,
/// and with it which generated tower it stands beside.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vertical {
    /// Open above and below - the through-tower, the common case.
    Through,
    /// The foot of a shaft: open above, solid floor below.
    Bottom,
    /// The head of a shaft: arrives from below, capped above.
    Top,
}

impl Vertical {
    #[must_use]
    fn stem(self) -> &'static str {
        match self {
            Self::Through => "stair_tower_helix",
            Self::Bottom => "stair_tower_helix_bottom",
            Self::Top => "stair_tower_helix_top",
        }
    }

    /// Whether a flight arrives through the floor.
    #[must_use]
    fn open_below(self) -> bool {
        matches!(self, Self::Through | Self::Top)
    }

    /// Whether the climb continues out through the ceiling.
    #[must_use]
    fn open_above(self) -> bool {
        matches!(self, Self::Through | Self::Bottom)
    }
}

/// Every tower, one per vertical connectivity.
#[must_use]
pub fn verticals() -> [Vertical; 3] {
    [Vertical::Through, Vertical::Bottom, Vertical::Top]
}

/// One helical stair tower.
#[must_use]
pub fn stair_tower(vertical: Vertical) -> String {
    let extent = Extent {
        faces: SWEEP,
        variant: 0,
        outer_scale: OUTER_SCALE,
    };
    let top = 2.0 * LEVEL;

    let mut brushes = String::new();
    // The floor is an aperture where a flight arrives through it - the hole
    // `tile_for` already assumes ("a tower's stairwell opening is the hole the
    // flight below arrives through") - and solid where the shaft bottoms out.
    if vertical.open_below() {
        brushes.push_str("// Aperture floor: the flight below arrives through it\n");
        brushes.push_str(&hex_opening_slab(0.0, FLOOR_TOP));
    } else {
        brushes.push_str("// Solid floor: the shaft bottoms out here\n");
        brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 2.0, 0.0));
    }

    brushes.push_str("// Helical flight, 240 degrees, two triangles per face\n");
    brushes.push_str(&flight(extent));
    brushes.push_str("// Landing at the head of the climb\n");
    brushes.push_str(&landing(extent));

    brushes.push_str("// Sealed envelope: a tower connects vertically only\n");
    for face in 0..6 {
        brushes.push_str(&wall(face, 0.0, top));
    }

    // No cap when the climb continues. A perimeter helix cannot run under a
    // solid deck at all: with 2.2 m of headroom a body reaches 5.3 m of the 8 m
    // it needs, so a capped through-tower would be unclimbable rather than
    // merely enclosed.
    if !vertical.open_above() {
        brushes.push_str("// Capped: nothing climbs out of the top of this one\n");
        brushes.push_str(&hex_slab(top - FLOOR_TOP, top, 0.0, 3.0));
    }

    let mut lights = String::new();
    for (face, z) in [(4usize, 88.0), (2usize, 184.0)] {
        let (fixture, source) = wall_fixture(face, 0.5, z, 20.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = format!(
        "// Helical stair tower ({:?}): a climb that never doubles back.\n",
        vertical
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    // `stair_tower` directly: the compatibility rewrite that turns
    // `stair_segment` into this name runs only over the generated library.
    out.push_str(
        &Meta::cell(
            &format!("authored/{}", vertical.stem()),
            "stair_tower",
            0,
            2,
            WEIGHT,
        )
        .with_register_scope("all")
        // A tower has no lateral door, so every rotation of it is the same
        // tower. Sixfold would compile five identical variants.
        .with_rotation_policy("none")
        .emit(),
    );
    // "open": the walk surface is a helix round the rim, not a floor over the
    // centre, and the centre is a shaft. The same declaration silo_ring makes.
    out.push_str(&tile_cell(0, 0, 0, 2, "open"));
    if vertical.open_above() {
        // Level 1, not 0. The cell spans two levels, so its level-0 Up face is
        // *internal* - it faces the tile's own upper half - and a port there is
        // PortOnInternalFace. The generated towers declare level 0 and get away
        // with it only because they are authoring_version 1, which skips the
        // strict port checks entirely.
        out.push_str(&vertical_port("up", "shaft_open", "up_shaft", 1));
    }
    if vertical.open_below() {
        out.push_str(&vertical_port("down", "shaft_open", "down_shaft", 0));
    }
    out.push_str(&spine(extent));
    out.push_str(&ring_deck(extent));
    out.push_str(&lights);
    out
}

/// The flat route round the ring, from the rim to the foot of the climb.
///
/// The bot measures its distance to the spine and, when it is off the climb,
/// walks this path toward the spine's first node. **With no deck path it is
/// steered straight at the spine instead**, through whatever stands between.
/// The last authored tower family shipped no deck at all;
/// `every_generated_stair_tower_ships_a_walkable_deck` never caught it because
/// it filters on the generated archetype names.
///
/// Nodes sit on the ring, outside the climb, at floor height - which is what
/// that test requires of every node. Three of them, because the importer
/// rejects two as a straight line needing no path, and a route round a ring
/// genuinely bends.
#[must_use]
fn ring_deck(extent: Extent) -> String {
    // Midway between the climb's outer edge and the wall: the walkable part of
    // the ring, clear of both.
    let ring = (OUTER_SCALE + 1.0) * 0.5;
    let at = |face: usize| -> (f64, f64) {
        let (a, b) = (corners()[face % 6], corners()[(face + 1) % 6]);
        ((a.0 + b.0) * 0.5 * ring, (a.1 + b.1) * 0.5 * ring)
    };

    // Round the rim to the face the climb starts on, then in to its foot.
    let start = extent.start_face();
    let mut out = String::new();
    for (index, face) in [start + 3, start + 4, start + 5, start].iter().enumerate() {
        let (x, y) = at(*face);
        #[allow(clippy::cast_possible_truncation)]
        out.push_str(&deck_node(index as u16, x, y, FLOOR_TOP));
    }
    out
}

/// Every tower, paired with the file it must reproduce.
#[must_use]
pub fn builders() -> Vec<(String, String)> {
    verticals()
        .into_iter()
        .map(|vertical| (vertical.stem().to_string(), stair_tower(vertical)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The signatures must match the generated towers they stand beside, or
    /// they land in a different bucket and are never selected.
    #[test]
    fn each_vertical_declares_the_ports_its_connectivity_implies() {
        for vertical in verticals() {
            let text = stair_tower(vertical);
            assert_eq!(
                text.contains("\"name\" \"up_shaft\""),
                vertical.open_above(),
                "{:?} up port disagrees with its connectivity",
                vertical
            );
            assert_eq!(
                text.contains("\"name\" \"down_shaft\""),
                vertical.open_below(),
                "{:?} down port disagrees with its connectivity",
                vertical
            );
            assert!(
                !text.contains("\"class\" \"door\""),
                "{:?} is a tower, not a landing",
                vertical
            );
        }
    }

    /// A climb that continues upward must not be capped. With a solid deck a
    /// body reaches 5.3 m of the 8 m it needs and the tower is unclimbable -
    /// and nothing in the importer objects, because a low ceiling over a ramp
    /// is not what `Headroom` measures.
    #[test]
    fn a_tower_that_climbs_out_is_not_capped() {
        let through = stair_tower(Vertical::Through);
        assert!(
            !through.contains("Capped"),
            "a through tower must stay open"
        );
        let top = stair_tower(Vertical::Top);
        assert!(top.contains("Capped"), "a shaft head must close");
    }

    /// The floor is a hole exactly when a flight arrives through it.
    #[test]
    fn the_floor_opens_only_where_a_flight_arrives() {
        assert!(stair_tower(Vertical::Bottom).contains("Solid floor"));
        assert!(stair_tower(Vertical::Through).contains("Aperture floor"));
        assert!(stair_tower(Vertical::Top).contains("Aperture floor"));
    }

    /// Every tower carries a spine. Without one a follower has no line to walk
    /// and the tower is exactly the trap the switchback was.
    #[test]
    fn every_tower_carries_a_spine() {
        for vertical in verticals() {
            let nodes = stair_tower(vertical)
                .matches("\"classname\" \"tile_stair_node\"")
                .count();
            assert!(
                nodes >= SWEEP + 3,
                "{:?} has only {nodes} spine nodes",
                vertical
            );
        }
    }

    /// The authored tower must not crowd out the generated one before anyone
    /// has walked it.
    #[test]
    fn the_authored_tower_takes_a_modest_share() {
        assert!(
            (1..=10).contains(&WEIGHT),
            "an unplayed shape should not dominate the bucket"
        );
        assert!(stair_tower(Vertical::Through).contains(&format!("\"weight\" \"{WEIGHT}\"")));
    }

    /// Every tower must survive the importer it is written for.
    #[test]
    fn every_tower_parses_and_validates() {
        for (stem, text) in builders() {
            crate::parse_authored_module(&text)
                .unwrap_or_else(|error| panic!("{stem} does not validate: {error:?}"));
        }
    }
}
