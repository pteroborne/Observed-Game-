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
use super::entities::{
    Meta, deck_node, lateral_port, tile_cell, vertical_port, wall_fixture, worldspawn,
};
use super::geometry::{DOOR_TOP, FLOOR_TOP, LEVEL, corners, door_wall, hex_slab, wall};
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

/// The door patterns, one per orbit under sixfold rotation.
///
/// Rotating these through six turns reaches every arrangement of up to two
/// doors the solver asks for: none; one; and two doors adjacent, one apart, or
/// opposite. Five orbits times three connectivities is the whole 66-signature
/// demand, not a sample of it.
const DOOR_ORBITS: [&[usize]; 5] = [&[], &[0], &[0, 1], &[0, 2], &[0, 3]];

/// First variant slot. Nothing shares this archetype now, but keeping the
/// authored family off 0..62 keeps a `TileKey` in a diagnostic unambiguous
/// against any catalog still carrying the old numbers.
const FIRST_VARIANT: i32 = 11;

/// One authored tower: a connectivity and a door pattern.
///
/// The climb is in neither field, which is the design. See [`OUTER_SCALE`].
#[derive(Clone, Copy, Debug)]
pub struct Tower {
    pub vertical: Vertical,
    pub doors: &'static [usize],
}

impl Tower {
    #[must_use]
    fn stem(self) -> String {
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

/// Every tower: each connectivity in each door orbit.
#[must_use]
pub fn towers() -> Vec<Tower> {
    let mut out = Vec::new();
    for doors in DOOR_ORBITS {
        for vertical in verticals() {
            out.push(Tower { vertical, doors });
        }
    }
    out
}

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
pub fn stair_tower(tower: Tower, variant: i32) -> String {
    let vertical = tower.vertical;
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
    out.push_str(&{
        let meta = Meta::cell(
            &format!("authored/{}", tower.stem()),
            "stair_tower",
            variant,
            2,
            WEIGHT,
        )
        .with_register_scope("all");
        // A door has to be able to face any way, so a tower carrying one is
        // authored once and turned. A bare column has no feature to orient.
        if tower.doors.is_empty() {
            meta.with_rotation_policy("none")
        } else {
            meta.with_rotation_policy("sixfold")
        }
        .emit()
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
    for &face in tower.doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("door_{face}"),
            0,
            0,
            0,
        ));
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
/// ## A node on every face, and the last one at the foot
///
/// Both of those were learned from bodies rather than argued, by
/// `a_tower_climbs_from_every_face_a_door_could_be_on`. The first version
/// covered four faces and ended on the rim, and two of the six starts walked
/// into the side of the flight and stayed there. `DeckPath::step_toward` hands
/// back the goal itself once the body and the goal are nearest the same leg, so
/// wherever the path runs out, the rest of the way is a straight line - and a
/// straight line from the rim to the foot crosses the climb.
///
/// - **The last node is [`Extent::foot`]**, the same point the spine starts at,
///   so the path reaches the climb instead of stopping a ring's width short of
///   it. That leg comes off the face at `start + 5`, which the sweep does not
///   cover, so it runs over open floor.
/// - **Every face carries a node**, or a body entering on an uncovered one is
///   handed a leg across the cell and cuts the chord through the stairwell.
///
/// The ring is a polyline and not a loop, so one face - the climb's own,
/// `start` - is at the far end and walks the long way round. That is the price
/// of a single ordered path, and it is paid in walking rather than in stalling.
#[must_use]
fn ring_deck(extent: Extent) -> String {
    // Midway between the climb's outer edge and the wall: the walkable part of
    // the ring, clear of both.
    let ring = (OUTER_SCALE + 1.0) * 0.5;
    let at = |face: usize| -> (f64, f64) {
        let (a, b) = (corners()[face % 6], corners()[(face + 1) % 6]);
        ((a.0 + b.0) * 0.5 * ring, (a.1 + b.1) * 0.5 * ring)
    };

    // Round the whole rim, away from the climb's own face, then in to its foot.
    let start = extent.start_face();
    let mut plan: Vec<(f64, f64)> = (0..6).map(|step| at(start + step)).collect();
    plan.push(extent.foot());

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
            (tower.stem(), stair_tower(tower, variant))
        })
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
            let text = stair_tower(
                Tower {
                    vertical,
                    doors: &[],
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
            Tower {
                vertical: Vertical::Through,
                doors: &[],
            },
            FIRST_VARIANT,
        );
        assert!(
            !through.contains("Capped"),
            "a through tower must stay open"
        );
        let top = stair_tower(
            Tower {
                vertical: Vertical::Top,
                doors: &[],
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
                Tower {
                    vertical: Vertical::Bottom,
                    doors: &[]
                },
                FIRST_VARIANT
            )
            .contains("Solid floor")
        );
        assert!(
            stair_tower(
                Tower {
                    vertical: Vertical::Through,
                    doors: &[]
                },
                FIRST_VARIANT
            )
            .contains("Aperture floor")
        );
        assert!(
            stair_tower(
                Tower {
                    vertical: Vertical::Top,
                    doors: &[]
                },
                FIRST_VARIANT
            )
            .contains("Aperture floor")
        );
    }

    /// Every tower carries a spine. Without one a follower has no line to walk
    /// and the tower is exactly the trap the switchback was.
    #[test]
    fn every_tower_carries_a_spine() {
        for vertical in verticals() {
            let nodes = stair_tower(
                Tower {
                    vertical,
                    doors: &[],
                },
                FIRST_VARIANT,
            )
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
        assert!(
            stair_tower(
                Tower {
                    vertical: Vertical::Through,
                    doors: &[]
                },
                FIRST_VARIANT
            )
            .contains(&format!("\"weight\" \"{WEIGHT}\""))
        );
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
                Tower {
                    vertical,
                    doors: &[],
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
                        doors: &[]
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
                    doors: &[]
                }
                .stem(),
                top.y
            );
        }
    }

    /// A capped tower's lid must clear its climb by a body's height.
    ///
    /// This is the generated family's fault, stated as a test. Its shaft heads
    /// capped at `h - FLOOR_TOP` (7.5 m) while the flight kept climbing to 8.5,
    /// straight through the lid: 1.70 m of clearance for a 1.8 m body, on all
    /// 242 capped towers in the library. Nothing caught it, because the
    /// importer does not measure headroom over a flight and those tiles are
    /// authoring version 1.
    ///
    /// Here the cap sits at the top of the reserved two-level envelope and the
    /// climb ends one level below it, so the margin is large - but "large by
    /// accident" is what the generated family had until someone moved a
    /// constant.
    #[test]
    fn a_capped_tower_clears_its_climb_by_a_body() {
        let body = f64::from(observed_traversal::FpsConfig::default().half_height) * 2.0;
        for vertical in verticals() {
            if vertical.open_above() {
                continue;
            }
            let module = crate::parse_authored_module(&stair_tower(
                Tower {
                    vertical,
                    doors: &[],
                },
                FIRST_VARIANT,
            ))
            .unwrap_or_else(|error| {
                panic!(
                    "{}: {error:?}",
                    Tower {
                        vertical,
                        doors: &[]
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
            // The lid's underside, in metres: the envelope's top less the slab.
            let lid = (2.0 * LEVEL - FLOOR_TOP) / 16.0;
            assert!(
                lid - f64::from(top.y) >= body,
                "{}: climb ends at {:.2} m under a lid at {lid:.2} m, leaving {:.2} m for a                  {body:.2} m body",
                Tower {
                    vertical,
                    doors: &[]
                }
                .stem(),
                top.y,
                lid - f64::from(top.y)
            );
        }
    }

    /// The ring must touch every face and end where the climb starts.
    ///
    /// `a_tower_climbs_from_every_face_a_door_could_be_on` is the test that
    /// found both of these, but it finds them by walking eighteen bodies
    /// through Rapier and reporting where they stopped. This states the same
    /// two properties as shape, so breaking one fails here first and says which
    /// it was.
    #[test]
    fn the_ring_deck_touches_every_face_and_ends_at_the_foot() {
        let extent = Extent {
            faces: SWEEP,
            variant: 0,
            outer_scale: OUTER_SCALE,
        };
        for vertical in verticals() {
            let module = crate::parse_authored_module(&stair_tower(vertical))
                .unwrap_or_else(|error| panic!("{}: {error:?}", vertical.stem()));
            let deck = &module.prototype.deck.nodes;
            assert_eq!(
                deck.len(),
                7,
                "{}: a ring is six faces and the run in to the foot",
                vertical.stem()
            );
            // One node per face, classified by which face's sector it stands
            // in - the bearing it points along, not a distance to a position
            // this test recomputed. Six distinct sectors means no face is
            // skipped, and a skipped face is a leg across the stairwell.
            let mut sectors: Vec<usize> = deck[..6]
                .iter()
                .map(|node| {
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
                })
                .collect();
            sectors.sort_unstable();
            sectors.dedup();
            assert_eq!(
                sectors.len(),
                6,
                "{}: the ring misses a face - its nodes lie in sectors {sectors:?}",
                vertical.stem()
            );
            // And the last node is the spine's first, so the path reaches the
            // climb rather than stopping a ring's width short of it.
            let foot = module
                .prototype
                .spine
                .nodes
                .first()
                .copied()
                .expect("a tower ships a spine");
            let last = *deck.last().expect("checked non-empty above");
            assert!(
                last.distance(foot) < 1e-3,
                "{}: the deck ends at {last:?}, but the climb starts at {foot:?}",
                vertical.stem()
            );
            // Stated against the source of both, so the two cannot be brought
            // back into agreement by editing one of them to a literal.
            //
            // A tenth of a millimetre, not an epsilon: these coordinates have
            // been through the `.map`, which emits four decimal places of TB
            // units, so a foot that is not a round number comes back up to
            // 3 micrometres off.
            let (fx, fy) = extent.foot();
            assert!(
                (f64::from(foot.x) - fx / 16.0).abs() < 1e-4
                    && (f64::from(foot.z) + fy / 16.0).abs() < 1e-4,
                "{}: the climb does not start at Extent::foot",
                vertical.stem()
            );
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
    fn a_foot_stands_clear_of_the_stairwell() {
        let radius = f64::from(observed_traversal::FpsConfig::default().radius);
        // The aperture is concentric and hexagonal, so its nearest point to
        // anything outside it is a corner.
        let lip = super::super::geometry::SHAFT_APERTURE_SCALE * {
            let (x, y) = corners()[0];
            x.hypot(y)
        } / 16.0;
        for vertical in verticals() {
            if !vertical.open_below() {
                // A solid floor has no hole to clear.
                continue;
            }
            let module = crate::parse_authored_module(&stair_tower(vertical))
                .unwrap_or_else(|error| panic!("{}: {error:?}", vertical.stem()));
            let foot = module
                .prototype
                .spine
                .nodes
                .first()
                .copied()
                .expect("a tower ships a spine");
            let stood_at = f64::from(foot.x).hypot(f64::from(foot.z));
            assert!(
                stood_at - lip >= radius,
                "{}: the climb is joined {stood_at:.2} m out against a {lip:.2} m aperture, \
                 leaving {:.2} m for a {radius:.2} m body",
                vertical.stem(),
                stood_at - lip
            );
            // And the deck ends there, so the same clearance carries the body
            // that walks in as well as the one that starts on the climb.
            let last = *module
                .prototype
                .deck
                .nodes
                .last()
                .expect("a tower ships a deck");
            assert!(
                last.distance(foot) < 1e-3,
                "{}: the deck no longer ends at the foot",
                vertical.stem()
            );
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
