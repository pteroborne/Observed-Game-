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
//! ## Why the replacement was atomic
//!
//! `Catalogue::new` keys candidates by `(archetype, register, signature)`, and
//! a shaft column's cells have different signatures as their doors and end caps
//! change. The generated switchback and this helix therefore could not coexist:
//! even a column-constant variation key could select one family below and the
//! other above, putting the lower landing under solid floor. The authored
//! family covers all 66 signatures, so removing the switchback in the same
//! change leaves every demand served by one column-compatible shape.

use super::GENERATED_NOTE;
use super::entities::{
    Meta, deck_node, lateral_port, tile_cell, vertical_port, wall_fixture, worldspawn,
};
use super::geometry::{DOOR_TOP, FLOOR_TOP, LEVEL, WALL, corners, door_wall, hex_slab, wall};
use super::perimeter::{Extent, landing, pierced_floor, spine, thin_flight};

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

/// Where the ring path runs, as a fraction of the hexagon: midway between the
/// climb's outer edge and the inside face of the wall.
///
/// Derived rather than picked, because the band is narrow. `WALL` is measured
/// against the apothem, which is 112 units, so the wall's inner surface is that
/// fraction in from the rim - and the same fraction serves the corners to
/// within a third of a percent, the hexagon being close enough to regular.
///
/// It matters that a node sits in the *middle* of the band. At 0.875 - halfway
/// between the climb and the rim, ignoring the wall - a node stands 0.025 m
/// nearer the wall than a 0.4 m body can reach, which the controller quietly
/// absorbed by pushing the body off its own path.
const RING: f64 = (OUTER_SCALE + (1.0 - WALL / 112.0)) * 0.5;

/// Positive catalogue weight carried by every member of the family.
///
/// Each signature currently has one tower candidate, so the exact value is
/// inert. Keeping the original authored value avoids changing selection if a
/// second column-compatible treatment is introduced later.
const WEIGHT: u32 = 6;

/// Every door pattern a tower can be asked for: none, and each unordered
/// subset of one to four faces. 1 + 6 + 15 + 20 + 15 = 57, and 57 times three
/// connectivities is the 171-source family.
///
/// **The last two sizes are the branching landing**, and they were added for a
/// reason outside this file. A corridor router that routes every named port
/// makes corridors meet, and where two meet on a cell that also climbs, the
/// cell is a three- or four-way junction *and* a staircase. The solver had no
/// such variant because this family had no such tower: the alphabet was capped
/// at two doors to match the corpus, and the corpus was capped at two doors
/// because nothing had ever asked for more. Neither cap was a geometric limit.
///
/// It costs nothing in geometry, which is the part worth stating plainly. A
/// door opens onto the ring and never onto the flight - that is [`OUTER_SCALE`]
/// and it is why the sweep can be fixed - so the envelope loop below already
/// draws any subset of the six faces correctly. Three doors is the same tower
/// with a third opening cut in it.
///
/// Five and six are left out to match the solver's `Junction`, which stops at
/// four for its own reasons. Adding them here without a variant to demand them
/// would author thirty sources nothing can select.
///
/// Order is append-only: the pairs keep the slots they had, so the sixty-six
/// committed towers stay byte-identical and only new files appear. Variant
/// numbers run from [`FIRST_VARIANT`] in this order, and a reordering would
/// rewrite the whole corpus for nothing.
///
/// Enumerated rather than turned. See the `rotation_policy` in [`stair_tower`]
/// for why the compiler's sixfold expansion cannot stand in for this.
#[must_use]
fn door_patterns() -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new()];
    for size in 1..=4usize {
        let mut sized: Vec<Vec<usize>> = Vec::new();
        for mask in 0u8..64 {
            if usize::try_from(mask.count_ones()).expect("six faces fit a usize") != size {
                continue;
            }
            sized.push((0..6).filter(|face| mask & (1 << face) != 0).collect());
        }
        // Lexicographic by face, which for one and two doors is exactly the
        // order the nested loops produced before this generalised.
        sized.sort();
        out.extend(sized);
    }
    out
}

/// First variant slot. Nothing shares this archetype now, but keeping the
/// authored family off 0..62 keeps a `TileKey` in a diagnostic unambiguous
/// against any catalog still carrying the old numbers.
const FIRST_VARIANT: i32 = 11;

/// One authored tower: a connectivity and a door pattern.
///
/// The climb is in neither field, which is the design. See [`OUTER_SCALE`].
#[derive(Clone, Debug)]
pub struct Tower {
    pub vertical: Vertical,
    pub doors: Vec<usize>,
}

impl Tower {
    #[must_use]
    fn stem(&self) -> String {
        let doors = if self.doors.is_empty() {
            String::from("solid")
        } else {
            self.doors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("")
        };
        format!("stair_tower_helix_{doors}{}", self.vertical.label())
    }
}

/// Every tower: each connectivity in each door pattern.
#[must_use]
pub fn towers() -> Vec<Tower> {
    let mut out = Vec::new();
    for doors in door_patterns() {
        for vertical in verticals() {
            out.push(Tower {
                vertical,
                doors: doors.clone(),
            });
        }
    }
    out
}

/// How a tower connects vertically. This decides the vertical part of its port
/// signature and therefore which solver demand it satisfies.
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
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Through => "",
            Self::Bottom => "_bottom",
            Self::Top => "_top",
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
pub fn stair_tower(tower: &Tower, variant: i32) -> String {
    let vertical = tower.vertical;
    let extent = Extent {
        faces: SWEEP,
        variant: 0,
        outer_scale: OUTER_SCALE,
    };
    // **One level, not two.** `levels: 2` is a reservation letting the flight
    // poke half a metre into the cell above to land flush; it is not a claim on
    // that cell's space. A shaft column places a tower at *every* level, so an
    // envelope two levels tall puts each tower's upper half inside its
    // neighbour - and a wall there seals the doorway the tower above just cut.
    // Measured: all 108 exits stopped 0.4 m short of their own doors, pressed
    // against the wall of the tower below. The generated family used one level
    // here and was right to.
    let top = LEVEL;

    let mut brushes = String::new();
    // The floor is an aperture where a flight arrives through it - the hole
    // `tile_for` already assumes ("a tower's stairwell opening is the hole the
    // flight below arrives through") - and solid where the shaft bottoms out.
    if vertical.open_below() {
        brushes.push_str("// Aperture floor: the flight below arrives through it\n");
        brushes.push_str(&pierced_floor(extent, 0.0, FLOOR_TOP));
    } else {
        brushes.push_str("// Solid floor: the shaft bottoms out here\n");
        brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 2.0, 0.0));
    }

    // **A shaft head has no staircase.** Nothing climbs out of one - it
    // declares no `up` port - so a flight inside it only ever ended at the lid.
    // That is the generated family's worst fault stated as geometry rather than
    // as a margin: all 242 of its capped towers ran a switchback into a ceiling
    // with 1.70 m of clearance for a 1.8 m body. The body that matters in a
    // shaft head arrives from *below*, onto this floor, and leaves by a door.
    if vertical.open_above() {
        brushes.push_str("// Helical flight, 240 degrees, two triangles per face\n");
        brushes.push_str(&thin_flight(extent));
        brushes.push_str("// Landing at the head of the climb\n");
        brushes.push_str(&landing(extent));
    }

    brushes.push_str(
        "// Envelope: doors where the shaft is entered, wall elsewhere
",
    );
    for face in 0..6 {
        if tower.doors.contains(&face) {
            // A door opens onto the ring, never onto the flight - which is the
            // whole reason the climb can be fixed. See `OUTER_SCALE`.
            brushes.push_str(&door_wall(face, 0.0, top, FLOOR_TOP, DOOR_TOP, 10.0, 8.0));
        } else {
            brushes.push_str(&wall(face, 0.0, top));
        }
    }

    // No cap when the climb continues: a lid over a helix is a lid on the
    // stairwell, and the climb needs the whole level to reach the deck above.
    if !vertical.open_above() {
        brushes.push_str("// Capped: nothing climbs out of the top of this one\n");
        brushes.push_str(&hex_slab(top - FLOOR_TOP, top, 0.0, 3.0));
    }

    let mut lights = String::new();
    // Both practicals inside this cell. The upper one used to sit at 184 units
    // - 11.5 m, a level and a half up - which was only ever legal because the
    // envelope reached into the cell above. `wall_fixture` spans `z` either
    // side by 8, so a ceiling at 128 puts the highest legal centre at 120.
    for (face, z) in [(4usize, 48.0), (2usize, 104.0)] {
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
    out.push_str(&{
        let meta = Meta::cell(
            &format!("authored/{}", tower.stem()),
            "stair_tower",
            variant,
            2,
            WEIGHT,
        )
        .with_register_scope("all")
        // **Never turned**, and this is the load-bearing decision in the
        // whole family.
        //
        // Authoring one door orbit and letting the compiler turn it into
        // the other five is the obvious economy, and it cannot work here,
        // because the rotation takes the *climb* with it - the doors and
        // the flight are the same brushes. `tile_for` pins a column's
        // register but takes the variation key per cell, so nothing stops
        // one cell of a shaft drawing turn 1 and the next turn 4, and then
        // the lower flight tops out under the upper cell's solid deck.
        // That is word for word the failure `tile_for`'s own comment
        // warns about; what the comment gets wrong is assuming the
        // register is enough to prevent it.
        //
        // Measured, with the orbits turned: 4 of 6 rotations of the tile
        // above stop the climb below at 6.19 m of 8, and one seed of the
        // spectator sweep stalled on `variant 127` - base 21, **turn 1**.
        //
        // So every door pattern is authored outright and the climb faces
        // one way everywhere. It costs 66 sources instead of 15, which is
        // the price of the invariant rather than a failure to be clever.
        .with_rotation_policy("none");
        meta.emit()
    });
    // "open": the walk surface is a helix round the rim, not a floor over the
    // centre, and the centre is a shaft. The same declaration silo_ring makes.
    // The footprint occupies **one** level; `Meta`'s `levels: 2` reserves the
    // space above it for the flight to poke into.
    //
    // These are different claims and conflating them is what put the `up` port
    // in the wrong place. A footprint spanning two levels makes the cell's
    // level-0 Up face *internal* - it faces the tile's own upper half - so the
    // strict checks reject a port there and it was moved to level 1, at 16 m,
    // which the climb never reaches. But a shaft column places a tower at every
    // level (543 adjacent placements against 23 gapped, in a 10-level column),
    // so each tower climbs exactly one level and connects at 8 m.
    out.push_str(&tile_cell(0, 0, 0, 2, "open"));
    if vertical.open_above() {
        // Level 0: the top of the single level this cell occupies, 8 m, which
        // is where the climb tops out and where the tower above begins. The
        // generated towers connect here too.
        out.push_str(&vertical_port("up", "shaft_open", "up_shaft", 0));
    }
    if vertical.open_below() {
        out.push_str(&vertical_port("down", "shaft_open", "down_shaft", 0));
    }
    for &face in &tower.doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("door_{face}"),
            0,
            0,
            0,
        ));
    }
    // A spine is the line through a climb, so a tower with no climb ships none
    // and `climb_command` declines it. Its deck still serves, which is the
    // whole of what a shaft head needs: arrive, cross, leave.
    if vertical.open_above() {
        out.push_str(&spine(extent));
    }
    out.push_str(&ring_deck(extent));
    out.push_str(&lights);
    out
}

/// The flat route across a tower's floor: the two ends of the climb, and the
/// ring that joins every door to them.
///
/// The bot measures its distance to the spine and, when it is off the climb,
/// walks this path toward the spine's first node; crossing a tower to a lateral
/// door it walks the same path toward the doorway. **With no deck path it is
/// steered straight at its goal**, through whatever stands between.
///
/// Four things this has to get right, every one of them learned from a body
/// that stopped rather than from reading the geometry.
///
/// - **It ends at [`Extent::foot`]**, the point the spine starts at, so it
///   reaches the climb instead of stopping a ring's width short of it.
///   `DeckPath::step_toward` hands back the goal itself once body and goal are
///   nearest the same leg, so wherever the path runs out the rest of the way is
///   a straight line - and a straight line from the rim to the foot crosses the
///   flight. Two of six door approaches walked into it.
/// - **It starts at [`Extent::head`]**, where the tower *below* sets a body
///   down. Nothing in a single tile stands there, so no single-tile test could
///   want it, and a body arriving from below was handed a leg at whichever ring
///   node happened to be nearest - across the stairwell. That was the spectator
///   stalling at the head of a shaft with all three levels climbed.
/// - **Head and foot are adjacent**, both inside the inner hexagon, which is
///   solid on every tower. Continuing up a shaft is the commonest move there
///   is, and putting the ring between them would send a body all the way round
///   the tower at every level.
/// - **The ring turns at its corners.** A chord between two face midpoints
///   passes nearer the centre than either end - at this radius, 0.87 m inside
///   the climb's outer edge - so a ring drawn corner to corner cuts through the
///   stairwell twice per lap. With the floor solid that was survivable; with
///   [`pierced_floor`](super::perimeter::pierced_floor) opening the arrival band
///   it is a fall.
///
/// The ring is a polyline and not a loop, so one face is at the far end and
/// walks the long way round. That is the price of a single ordered path, and it
/// is paid in walking rather than in stalling.
#[must_use]
fn ring_deck(extent: Extent) -> String {
    let start = extent.start_face();
    let at_face = |face: usize| -> (f64, f64) {
        let (a, b) = (corners()[face % 6], corners()[(face + 1) % 6]);
        ((a.0 + b.0) * 0.5 * RING, (a.1 + b.1) * 0.5 * RING)
    };
    let at_corner = |corner: usize| -> (f64, f64) {
        let c = corners()[corner % 6];
        (c.0 * RING, c.1 * RING)
    };

    // Where the climb leaves a body, then where it takes one on, then out
    // through the one face the sweep never covers and round the whole rim.
    let mut plan = vec![extent.head(), extent.foot()];
    for step in 0..6 {
        plan.push(at_face(start + 5 + step));
        if step < 5 {
            // The corner shared by this face and the next.
            plan.push(at_corner(start + step));
        }
    }

    let mut out = String::new();
    for (index, (x, y)) in plan.into_iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        out.push_str(&deck_node(index as u16, x, y, FLOOR_TOP));
    }
    out
}

/// Every tower, paired with the file it must reproduce.
#[must_use]
pub fn builders() -> Vec<(String, String)> {
    towers()
        .into_iter()
        .enumerate()
        .map(|(index, tower)| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let variant = FIRST_VARIANT + index as i32;
            (tower.stem(), stair_tower(&tower, variant))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The signatures must match the solver demand or the tower lands in a
    /// different bucket and is never selected.
    #[test]
    fn each_vertical_declares_the_ports_its_connectivity_implies() {
        for vertical in verticals() {
            let text = stair_tower(
                &Tower {
                    vertical,
                    doors: Vec::new(),
                },
                FIRST_VARIANT,
            );
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
        let through = stair_tower(
            &Tower {
                vertical: Vertical::Through,
                doors: Vec::new(),
            },
            FIRST_VARIANT,
        );
        assert!(
            !through.contains("Capped"),
            "a through tower must stay open"
        );
        let top = stair_tower(
            &Tower {
                vertical: Vertical::Top,
                doors: Vec::new(),
            },
            FIRST_VARIANT,
        );
        assert!(top.contains("Capped"), "a shaft head must close");
    }

    /// The floor is a hole exactly when a flight arrives through it.
    #[test]
    fn the_floor_opens_only_where_a_flight_arrives() {
        assert!(
            stair_tower(
                &Tower {
                    vertical: Vertical::Bottom,
                    doors: Vec::new()
                },
                FIRST_VARIANT
            )
            .contains("Solid floor")
        );
        assert!(
            stair_tower(
                &Tower {
                    vertical: Vertical::Through,
                    doors: Vec::new()
                },
                FIRST_VARIANT
            )
            .contains("Aperture floor")
        );
        assert!(
            stair_tower(
                &Tower {
                    vertical: Vertical::Top,
                    doors: Vec::new()
                },
                FIRST_VARIANT
            )
            .contains("Aperture floor")
        );
    }

    /// Every tower that climbs carries a spine. A shaft head deliberately has
    /// neither an up port nor a staircase to describe.
    #[test]
    fn only_the_towers_that_climb_carry_a_spine() {
        for vertical in verticals() {
            let nodes = stair_tower(
                &Tower {
                    vertical,
                    doors: Vec::new(),
                },
                FIRST_VARIANT,
            )
            .matches("\"classname\" \"tile_stair_node\"")
            .count();
            if vertical.open_above() {
                assert!(
                    nodes >= SWEEP + 3,
                    "{vertical:?} has only {nodes} spine nodes"
                );
            } else {
                // A spine is the line through a climb. A shaft head has no
                // climb, so a spine in one is a line to nowhere - and the bot
                // follows spines.
                assert_eq!(
                    nodes, 0,
                    "{vertical:?} is a shaft head and should not climb"
                );
            }
        }
    }

    /// A climb must reach the port it advertises.
    ///
    /// This is the test whose absence let the tower ship with its `up` port at
    /// 16 m and its climb topping out at 8.5 m. Everything else was green: it
    /// validated, the production controller walked its spine, the whole gate
    /// passed. Nothing asked whether the two ends met, so a body would have
    /// climbed to the top and found the connection seven metres overhead.
    #[test]
    fn the_climb_reaches_the_port_it_advertises() {
        for vertical in verticals() {
            let text = stair_tower(
                &Tower {
                    vertical,
                    doors: Vec::new(),
                },
                FIRST_VARIANT,
            );
            if !vertical.open_above() {
                continue;
            }
            let module = crate::parse_authored_module(&text).unwrap_or_else(|error| {
                panic!(
                    "{}: {error:?}",
                    Tower {
                        vertical,
                        doors: Vec::new()
                    }
                    .stem()
                )
            });
            let top = module
                .prototype
                .spine
                .nodes
                .last()
                .copied()
                .expect("a tower ships a spine");
            let port = module
                .ports
                .iter()
                .find(|port| port.face == observed_hex::HexFace::Up)
                .expect("an open-above tower declares an up port");
            let origin = port.origin.expect("a vertical port carries an origin");
            let port_y = origin[2] / 16.0;
            // The port marks the lattice boundary; the climb lands on the
            // *deck* above it, one floor slab higher. That is the flush
            // contract - short of it is a gap, proud of it is a lip the
            // autostep cannot get back over - so the two are a slab apart by
            // design, and any other gap is the bug this test exists for.
            let deck = port_y + f64::from(observed_hex::FLOOR_SLAB_TOP);
            let step = f64::from(observed_traversal::FpsConfig::default().step_height);
            assert!(
                (f64::from(top.y) - deck).abs() <= step,
                "{}: climb tops at {:.2} m but its up port puts the deck above at {deck:.2} m",
                Tower {
                    vertical,
                    doors: Vec::new()
                }
                .stem(),
                top.y
            );
        }
    }

    /// A shaft head ships no staircase at all.
    ///
    /// Stronger than the contract this replaces, and simpler. The generated
    /// family capped its heads at `h - FLOOR_TOP` (7.5 m) while the flight kept
    /// climbing to 8.5, straight through the lid: 1.70 m of clearance for a
    /// 1.8 m body, on all 242 capped towers in the library. The first authored
    /// answer was to hold the lid a body's height above the climb, which worked
    /// only by giving the tower a two-level envelope - and that envelope turned
    /// out to seal the doors of the tower standing on it.
    ///
    /// Nothing climbs out of a shaft head; it declares no `up` port. A flight
    /// inside one is a route that can only end at the ceiling, so the honest
    /// fix is not to build it. What a body does in a shaft head is arrive from
    /// below, cross the floor and leave by a door, and the deck serves that.
    #[test]
    fn a_shaft_head_ships_no_staircase() {
        for tower in towers() {
            if tower.vertical.open_above() {
                continue;
            }
            let text = stair_tower(&tower, FIRST_VARIANT);
            let module = crate::parse_authored_module(&text)
                .unwrap_or_else(|error| panic!("{}: {error:?}", tower.stem()));
            assert!(
                module.prototype.spine.nodes.is_empty(),
                "{}: a shaft head carries a spine to nowhere",
                tower.stem()
            );
            assert!(
                text.contains("Capped"),
                "{}: a shaft head must close",
                tower.stem()
            );
            // And every hull stays inside this cell: the lid is its own
            // ceiling, not a floor hanging in the one above.
            let ceiling = f64::from(observed_hex::TILE_LEVEL_HEIGHT);
            for hull in &module.prototype.hulls {
                for point in hull {
                    assert!(
                        f64::from(point.y) <= ceiling + 1e-3,
                        "{}: a hull reaches {:.2} m, past this cell's {ceiling:.2} m",
                        tower.stem(),
                        point.y
                    );
                }
            }
        }
    }

    /// The deck must join both ends of the climb and touch every face.
    ///
    /// `a_tower_climbs_from_every_face_a_door_could_be_on` and
    /// `a_tower_can_be_left_by_every_door_it_carries` are what found each of
    /// these, but they find them by walking bodies through Rapier and reporting
    /// where they stopped. This states the properties as shape, so breaking one
    /// fails here first and says which it was.
    #[test]
    fn the_deck_joins_both_ends_of_the_climb_and_every_face() {
        let extent = Extent {
            faces: SWEEP,
            variant: 0,
            outer_scale: OUTER_SCALE,
        };
        for tower in towers() {
            let module = crate::parse_authored_module(&stair_tower(&tower, FIRST_VARIANT))
                .unwrap_or_else(|error| panic!("{}: {error:?}", tower.stem()));
            let deck = &module.prototype.deck.nodes;
            assert_eq!(
                deck.len(),
                13,
                "{}: head, foot, and a ring of six faces turning at five corners",
                tower.stem()
            );

            // Both ends of the climb, adjacent and in order: a body continuing
            // up a shaft crosses between them in one leg, not a lap.
            let plan = |point: glam::Vec3| (f64::from(point.x), f64::from(point.z));
            let (hx, hy) = extent.head();
            let (fx, fy) = extent.foot();
            // A tenth of a millimetre, not an epsilon: these have been through
            // the `.map`, which emits four decimal places of TB units.
            let near =
                |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4;
            assert!(
                near(plan(deck[0]), (hx / 16.0, -hy / 16.0)),
                "{}: the deck does not start where the climb sets a body down",
                tower.stem()
            );
            assert!(
                near(plan(deck[1]), (fx / 16.0, -fy / 16.0)),
                "{}: the deck does not reach the foot of the climb",
                tower.stem()
            );
            // A shaft head has no spine to agree with; it is where a climb
            // ends, not where one starts.
            if let Some(&first) = module.prototype.spine.nodes.first() {
                assert!(
                    near(plan(deck[1]), plan(first)),
                    "{}: the deck's foot and the spine's have drifted apart",
                    tower.stem()
                );
            }

            // One node per face, classified by which face's sector it stands
            // in - the bearing it points along, not a distance to a position
            // this test recomputed. A skipped face is a leg across the
            // stairwell.
            let sector = |node: glam::Vec3| {
                (0..6)
                    .max_by(|&a, &b| {
                        let toward = |face: usize| {
                            let (u, v) = (corners()[face], corners()[(face + 1) % 6]);
                            // TB y is the negated world z, as everywhere.
                            let (mx, mz) = ((u.0 + v.0) * 0.5, -(u.1 + v.1) * 0.5);
                            f64::from(node.x) * mx + f64::from(node.z) * mz
                        };
                        toward(a).total_cmp(&toward(b))
                    })
                    .expect("six faces")
            };
            let mut sectors: Vec<usize> = deck[2..].iter().copied().map(sector).collect();
            sectors.sort_unstable();
            sectors.dedup();
            assert_eq!(
                sectors.len(),
                6,
                "{}: the ring misses a face - its nodes lie in sectors {sectors:?}",
                tower.stem()
            );

            // And every leg of the ring stays clear of the climb. A chord
            // between two face midpoints dips nearer the centre than either
            // end, which is how a ring drawn corner to corner cuts through the
            // stairwell twice a lap.
            //
            // Against the climb's *hexagon*, not a circle around it: the outer
            // edge runs from 5.25 m at a face to 6.05 m at a corner, and
            // comparing a leg against either number alone is wrong at the other
            // end. A point is outside a convex hexagon exactly when it is
            // beyond one of its six edges.
            let apothem = f64::from(observed_hex::ACROSS_FLATS) * 0.5 * OUTER_SCALE;
            let outside = |p: (f64, f64)| {
                (0..6).any(|face| {
                    let (u, v) = (corners()[face], corners()[(face + 1) % 6]);
                    let (mx, mz) = ((u.0 + v.0) * 0.5, -(u.1 + v.1) * 0.5);
                    let len = mx.hypot(mz);
                    (p.0 * mx + p.1 * mz) / len > apothem
                })
            };
            for pair in deck[2..].windows(2) {
                let (a, b) = (plan(pair[0]), plan(pair[1]));
                for sample in 0..=32 {
                    let t = f64::from(sample) / 32.0;
                    let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                    assert!(
                        outside(p),
                        "{}: a ring leg from {a:?} to {b:?} passes through the climb at {p:?}",
                        tower.stem()
                    );
                }
            }
        }
    }

    /// Nothing a body is sent to may stand over the hole in the floor.
    ///
    /// The tower's floor is an aperture where a flight arrives through it, and
    /// an inset climb drags the foot of its spine in toward that aperture. The
    /// foot landed 7.6 cm inside the lip. A 0.4 m capsule is carried by the lip
    /// so nothing fell and nothing went red - the same "large by accident"
    /// margin `a_capped_tower_clears_its_climb_by_a_body` was written for, in a
    /// different place.
    ///
    /// Held against `FpsConfig`, which owns the number that decides it. The
    /// forge states a distance and stays out of the controller's business; this
    /// is where the two have to agree.
    #[test]
    fn interior_waypoints_stand_clear_of_the_stairwell() {
        let radius = f64::from(observed_traversal::FpsConfig::default().radius);
        // The aperture is concentric and hexagonal, so its nearest point to
        // anything outside it is a corner.
        let lip = super::super::geometry::SHAFT_APERTURE_SCALE * {
            let (x, y) = corners()[0];
            x.hypot(y)
        } / 16.0;
        for tower in towers() {
            if !tower.vertical.open_below() {
                // A solid floor has no hole to clear.
                continue;
            }
            let module = crate::parse_authored_module(&stair_tower(&tower, FIRST_VARIANT))
                .unwrap_or_else(|error| panic!("{}: {error:?}", tower.stem()));
            // Both interior points the deck sends a body to, taken from the
            // deck itself: a shaft head has no spine to ask, and still stands a
            // body on this floor.
            for (what, node) in [
                ("the climb is joined", module.prototype.deck.nodes[1]),
                ("a body is set down", module.prototype.deck.nodes[0]),
            ] {
                let stood_at = f64::from(node.x).hypot(f64::from(node.z));
                assert!(
                    stood_at - lip >= radius,
                    "{}: {what} {stood_at:.2} m out against a {lip:.2} m aperture, \
                     leaving {:.2} m for a {radius:.2} m body",
                    tower.stem(),
                    stood_at - lip
                );
            }
        }
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
