//! Stair towers: the facility's vertical circulation, as a spiral stair.
//!
//! `stair_tower` is the most-walked element in the facility - about 130 towers in a
//! production solve. Each cell of a shaft column is one storey of one continuous
//! spiral: a full turn round a solid core, so every storey ends exactly where the
//! next begins, and climbing a column is climbing one staircase.
//!
//! ## What a storey is
//!
//! - **The core**, a hexagonal pier at the centre, rising through every storey.
//! - **The flight**, a full turn round it in six sectors, each two planar
//!   triangles: two level radial edges at different heights do not lie on one
//!   plane, so a sector cannot be one facet. 2.7 m wide, 0.37 along its middle, and
//!   a storey of headroom everywhere, because the turn above is a storey higher.
//! - **The guard wall**, along the flight's outside, rising with it a balustrade's
//!   height above the walking surface and stopping at the storey line, where the
//!   next storey's floor takes over. It is open at the foot of the flight only.
//! - **The gallery**, a ring between the guard wall and the tower's own walls,
//!   onto which every door opens. A body comes in by a door, walks the gallery to
//!   the foot, and climbs; or arrives from the storey below and walks out.
//!
//! ## Why the climb never depends on the doors
//!
//! A tower's climb geometry may depend on the register and on nothing else.
//! `tile_for` pins a column's register but leaves the signature and the variation
//! key per cell, so two cells in one column can differ by their doors; if the
//! climb moved with the doors, one storey's flight would top out under the next
//! storey's floor. Doors are cut in the outer wall and open onto the gallery, which
//! goes all the way round, so the flight never has to know where they are.
//!
//! It replaces the helix, a 240-degree flight of thin facets with a landing at the
//! head of each storey, which ended and began at different bearings: climbing a
//! column meant walking round to the next flight at every floor, under the
//! underside of the one above.
//!
//! ## Why the archetype is `stair_tower`
//!
//! The generated library names its towers `stair_segment` / `stair_top` /
//! `stair_bottom` / `stair_landing`, and `compatibility_archetype` flattens them to
//! `stair_tower` on the way in - in `compatibility_cells` only. Authored modules
//! never pass through it, so an authored tower declares the runtime name directly.

use super::GENERATED_NOTE;
use super::entities::{
    Meta, deck_node, lateral_port, stair_node, tile_cell, vertical_port, wall_fixture, worldspawn,
};
use super::geometry::{
    DOOR_TOP, FLOOR_TOP, LEVEL, P2, P3, centroid, corners, custom_plane, door_wall, flat_plane,
    hex_slab, prism, side_plane, wall,
};

/// The core's size, as a fraction of the hexagon: an apothem of 1.75 m.
const CORE: f64 = 0.25;
/// The flight's outer edge, as a fraction of the hexagon: an apothem of 4.4 m, so
/// the flight is 2.7 m wide.
const FLIGHT: f64 = 0.634;
/// The guard wall's outer face: half a metre thick... near enough a third of one.
const GUARD_WALL: f64 = 0.665;
/// The gallery's walking line, midway across it: between the guard wall and the
/// tower's walls, whose inner faces stand at 104 of 112 units.
const GALLERY: f64 = (GUARD_WALL + 104.0 / 112.0) * 0.5;
/// The flight's walking line, midway across it.
const MIDDLE: f64 = (CORE + FLIGHT) * 0.5;
/// How far the guard wall stands above the flight: 1.1 m.
const GUARD: f64 = 18.0;
/// How much of the first sector's side the guard wall leaves open: the way onto
/// the flight, two metres of it at the foot.
const OPENING: f64 = 0.4;
/// The corner the flight starts from, and every storey's flight with it.
const START: usize = 0;

/// Positive catalogue weight carried by every member of the family.
///
/// Each signature currently has one tower candidate, so the exact value is inert.
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
/// door opens onto the gallery and never onto the flight, which is why the climb
/// can be fixed - so the envelope loop below already
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
/// The climb is in neither field, which is the design.
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

/// Corner `index` of the hexagon scaled by `scale`, in TB units.
fn corner(index: usize, scale: f64) -> P2 {
    let (x, y) = corners()[index % 6];
    (x * scale, y * scale)
}

/// The point `along` the way from corner `index` to the next, on the hexagon scaled
/// by `scale`.
fn along_side(index: usize, scale: f64, along: f64) -> P2 {
    let (a, b) = (corner(index, scale), corner(index + 1, scale));
    (a.0 + (b.0 - a.0) * along, a.1 + (b.1 - a.1) * along)
}

/// The flight's walking height at the start of sector `step` (0 to 6) of its turn.
fn height(step: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let turn = step as f64 / 6.0;
    FLOOR_TOP + LEVEL * turn
}

/// A convex brush over plan polygon `plan`, its top the plane through `top`, its
/// underside the plane through `bottom`, and optionally capped flat at `cap`.
fn sheet(plan: &[P2], top: [P3; 3], bottom: [P3; 3], cap: Option<f64>) -> String {
    let hint = centroid(plan);
    let mut out = String::from("{\n");
    for index in 0..plan.len() {
        out.push_str(&side_plane(
            plan[index],
            plan[(index + 1) % plan.len()],
            0.0,
            hint,
        ));
    }
    out.push_str(&custom_plane(top[0], top[1], top[2], true));
    out.push_str(&custom_plane(bottom[0], bottom[1], bottom[2], false));
    if let Some(cap) = cap {
        out.push_str(&flat_plane(cap, true));
    }
    out.push_str("}\n");
    out
}

/// A triangle of the flight: three plan points at their walking heights, half a
/// metre thick.
fn facet(points: [P3; 3]) -> String {
    let plan = [
        (points[0].0, points[0].1),
        (points[1].0, points[1].1),
        (points[2].0, points[2].1),
    ];
    let lowered = points.map(|(x, y, z)| (x, y, z - FLOOR_TOP));
    sheet(&plan, points, lowered, None)
}

/// The flight: one full turn, six sectors of two facets each, starting at `START`.
fn flight() -> String {
    let mut out = String::new();
    for step in 0..6 {
        let (k, h0, h1) = (START + step, height(step), height(step + 1));
        let (ia, oa) = (corner(k, CORE), corner(k, FLIGHT));
        let (ib, ob) = (corner(k + 1, CORE), corner(k + 1, FLIGHT));
        out.push_str(&facet([
            (ia.0, ia.1, h0),
            (oa.0, oa.1, h0),
            (ob.0, ob.1, h1),
        ]));
        out.push_str(&facet([
            (ia.0, ia.1, h0),
            (ob.0, ob.1, h1),
            (ib.0, ib.1, h1),
        ]));
    }
    out
}

/// The guard wall along the flight's outside: from the gallery floor to a
/// balustrade's height above the flight, stopping at the storey line. The first
/// sector's side is open for [`OPENING`] of its length - the way on.
fn guard_wall() -> String {
    let mut out = String::new();
    for step in 0..6 {
        let k = START + step;
        let from = if step == 0 { OPENING } else { 0.0 };
        let h_at = |along: f64| height(step) + (height(step + 1) - height(step)) * along + GUARD;
        let (ia, ib) = (along_side(k, FLIGHT, from), along_side(k, FLIGHT, 1.0));
        let (oa, ob) = (
            along_side(k, GUARD_WALL, from),
            along_side(k, GUARD_WALL, 1.0),
        );
        let top = [
            (ia.0, ia.1, h_at(from)),
            (ib.0, ib.1, h_at(1.0)),
            (oa.0, oa.1, h_at(from)),
        ];
        let bottom = [(ia.0, ia.1, 0.0), (ib.0, ib.1, 0.0), (oa.0, oa.1, 0.0)];
        out.push_str(&sheet(&[ia, ib, ob, oa], top, bottom, Some(LEVEL)));
    }
    out
}

/// The floor between the hexagon scaled by `inner` and the cell's edge: six
/// trapezoids, the gallery's floor round an open stairwell.
fn ring_floor(inner: f64) -> String {
    let mut out = String::new();
    for k in 0..6 {
        let plan = [
            corner(k, inner),
            corner(k, 1.0),
            corner(k + 1, 1.0),
            corner(k + 1, inner),
        ];
        out.push_str(&prism(&plan, 0.0, FLOOR_TOP, None, 0.0, 0.0));
    }
    out
}

/// One storey of a spiral stair tower.
#[must_use]
pub fn stair_tower(tower: &Tower, variant: i32) -> String {
    let vertical = tower.vertical;
    let mut brushes = String::new();
    match (vertical.open_below(), vertical.open_above()) {
        (true, true) => {
            // The flight below arrives through the stairwell and becomes this one.
            brushes.push_str("// Aperture floor: the gallery, round the open stairwell\n");
            brushes.push_str(&ring_floor(GUARD_WALL));
        }
        (true, false) => {
            // The flight below arrives through the last third of the turn; the rest
            // of the band is floor, railed where it meets the opening.
            brushes.push_str("// Aperture floor: solid but where the flight below arrives\n");
            brushes.push_str(&ring_floor(FLIGHT));
            for step in 0..4 {
                let k = START + step;
                let plan = [
                    corner(k, CORE),
                    corner(k, FLIGHT),
                    corner(k + 1, FLIGHT),
                    corner(k + 1, CORE),
                ];
                brushes.push_str(&prism(&plan, 0.0, FLOOR_TOP, None, 0.0, 0.0));
            }
            brushes.push_str("// Rail round the opening\n");
            let edge = START + 4;
            let (a, b) = (corner(edge, CORE), corner(edge, FLIGHT));
            let across = ((b.1 - a.1) * 0.03, (a.0 - b.0) * 0.03);
            brushes.push_str(&prism(
                &[
                    a,
                    b,
                    (b.0 + across.0, b.1 + across.1),
                    (a.0 + across.0, a.1 + across.1),
                ],
                FLOOR_TOP,
                FLOOR_TOP + GUARD,
                None,
                2.0,
                0.0,
            ));
            for step in 4..6 {
                let k = START + step;
                let (ia, ib) = (corner(k, FLIGHT), corner(k + 1, FLIGHT));
                let (oa, ob) = (corner(k, GUARD_WALL), corner(k + 1, GUARD_WALL));
                brushes.push_str(&prism(
                    &[ia, ib, ob, oa],
                    FLOOR_TOP,
                    FLOOR_TOP + GUARD,
                    None,
                    2.0,
                    0.0,
                ));
            }
        }
        _ => {
            brushes.push_str("// Solid floor: the shaft bottoms out here\n");
            brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 2.0, 0.0));
        }
    }

    brushes.push_str("// Core: a pier at the centre of every storey\n");
    let core: Vec<P2> = (0..6).map(|k| corner(k, CORE)).collect();
    brushes.push_str(&prism(&core, 0.0, LEVEL, None, 0.0, 0.0));

    // A shaft head has no staircase: nothing climbs out of one. The body that
    // matters in a shaft head arrives from below, onto this floor, and leaves by a
    // door.
    if vertical.open_above() {
        brushes.push_str("// Spiral flight: one full turn, two facets a sector\n");
        brushes.push_str(&flight());
        brushes.push_str("// Guard wall along the flight, open at its foot\n");
        brushes.push_str(&guard_wall());
    }

    brushes.push_str("// Envelope: doors where the shaft is entered, wall elsewhere\n");
    for face in 0..6 {
        if tower.doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, FLOOR_TOP, DOOR_TOP, 10.0, 8.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
        }
    }
    if !vertical.open_above() {
        brushes.push_str("// Capped: nothing climbs out of the top of this one\n");
        brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));
    }

    let mut lights = String::new();
    for (face, z) in [(4usize, 48.0), (1usize, 104.0)] {
        let (fixture, source) = wall_fixture(face, 0.5, z, 20.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = format!(
        "// Spiral stair tower ({:?}): one storey of a continuous spiral.\n",
        vertical
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            &format!("authored/{}", tower.stem()),
            "stair_tower",
            variant,
            2,
            WEIGHT,
        )
        .with_register_scope("all")
        // **Never turned.** A rotation takes the climb with the doors, and a
        // column's cells draw their variation per cell: one storey turned 1 and
        // the next turned 4 put one flight's head under the other's floor. Every
        // door pattern is authored outright and the climb faces one way
        // everywhere.
        .with_rotation_policy("none")
        .emit(),
    );
    // "open": the walk surface is a spiral round a pier, not a floor over the
    // centre. One level of footprint; `levels: 2` reserves the half-metre the flight
    // reaches into the cell above to land flush on its floor.
    out.push_str(&tile_cell(0, 0, 0, 2, "open"));
    if vertical.open_above() {
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
    if vertical.open_above() {
        out.push_str(&spine());
    }
    out.push_str(&gallery_deck());
    out.push_str(&lights);
    out
}

/// The climb's line: from the foot, round the middle of the flight, a full turn to
/// the same bearing a storey up - which is the next storey's foot.
fn spine() -> String {
    let mut out = String::new();
    for step in 0..=6 {
        let (x, y) = corner(START + step, MIDDLE);
        #[allow(clippy::cast_possible_truncation)]
        out.push_str(&stair_node(step as u16, x, y, height(step)));
    }
    out
}

/// Where the climb begins and ends on this storey's floor: the middle of the
/// flight at the starting corner. The storey below sets a body down here, and the
/// flight here begins.
fn ends() -> P2 {
    corner(START, MIDDLE)
}

/// The gallery's path: from the climb's ends, out through the opening in the guard
/// wall, and once round the gallery, turning at every corner and passing every
/// face's door.
///
/// It turns at the corners because a chord between two face midpoints passes
/// nearer the centre than either end; round a stairwell that is a leg across the
/// guard wall.
fn gallery_deck() -> String {
    let mut plan = vec![ends(), along_side(START, GALLERY, OPENING * 0.5)];
    for step in 0..6 {
        plan.push(along_side(START + step, GALLERY, 0.5));
        plan.push(corner(START + step + 1, GALLERY));
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
                "{vertical:?}"
            );
            assert_eq!(
                text.contains("\"name\" \"down_shaft\""),
                vertical.open_below(),
                "{vertical:?}"
            );
            assert!(
                !text.contains("\"class\" \"door\""),
                "{vertical:?} is a tower, not a landing"
            );
        }
    }

    /// A climb that continues upward is not capped, and a shaft head is.
    #[test]
    fn a_tower_that_climbs_out_is_not_capped() {
        let text = |vertical| {
            stair_tower(
                &Tower {
                    vertical,
                    doors: Vec::new(),
                },
                FIRST_VARIANT,
            )
        };
        assert!(
            !text(Vertical::Through).contains("Capped"),
            "a through tower must stay open"
        );
        assert!(
            text(Vertical::Top).contains("Capped"),
            "a shaft head must close"
        );
    }

    /// The floor opens exactly where a flight arrives through it.
    #[test]
    fn the_floor_opens_only_where_a_flight_arrives() {
        let text = |vertical| {
            stair_tower(
                &Tower {
                    vertical,
                    doors: Vec::new(),
                },
                FIRST_VARIANT,
            )
        };
        assert!(text(Vertical::Bottom).contains("Solid floor"));
        assert!(text(Vertical::Through).contains("Aperture floor"));
        assert!(text(Vertical::Top).contains("Aperture floor"));
    }

    /// Every tower that climbs carries a spine of a full turn; a shaft head, which
    /// has no climb, carries none - and the bot follows spines.
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
            assert_eq!(
                nodes,
                if vertical.open_above() { 7 } else { 0 },
                "{vertical:?}"
            );
        }
    }

    /// A climb reaches the port it advertises: it tops out on the deck above the up
    /// port, a floor slab over the lattice boundary, and nowhere else.
    #[test]
    fn the_climb_reaches_the_port_it_advertises() {
        for vertical in verticals()
            .into_iter()
            .filter(|vertical| vertical.open_above())
        {
            let tower = Tower {
                vertical,
                doors: Vec::new(),
            };
            let module = crate::parse_authored_module(&stair_tower(&tower, FIRST_VARIANT))
                .unwrap_or_else(|error| panic!("{}: {error:?}", tower.stem()));
            let top = module
                .prototype
                .spine
                .nodes
                .last()
                .copied()
                .expect("a spine");
            let port = module
                .ports
                .iter()
                .find(|port| port.face == observed_hex::HexFace::Up)
                .expect("an up port");
            let deck =
                port.origin.expect("an origin")[2] / 16.0 + f64::from(observed_hex::FLOOR_SLAB_TOP);
            assert!(
                (f64::from(top.y) - deck).abs() < 1e-3,
                "{}: tops at {} for {deck}",
                tower.stem(),
                top.y
            );
        }
    }

    /// One storey's climb ends where the next one's begins, in plan: a column is
    /// one spiral, with no walk between its flights.
    #[test]
    fn each_storey_ends_where_the_next_begins() {
        let module = crate::parse_authored_module(&stair_tower(
            &Tower {
                vertical: Vertical::Through,
                doors: Vec::new(),
            },
            FIRST_VARIANT,
        ))
        .expect("a through tower validates");
        let nodes = &module.prototype.spine.nodes;
        let (first, last) = (nodes[0], nodes[nodes.len() - 1]);
        assert!(first.x.mul_add(1.0, -last.x).abs() < 1e-3 && (first.z - last.z).abs() < 1e-3);
        assert!((last.y - first.y - observed_hex::TILE_LEVEL_HEIGHT).abs() < 1e-3);
    }

    /// A shaft head ships no staircase, and stays inside its own cell.
    #[test]
    fn a_shaft_head_ships_no_staircase() {
        for tower in towers()
            .into_iter()
            .filter(|tower| !tower.vertical.open_above())
        {
            let text = stair_tower(&tower, FIRST_VARIANT);
            let module = crate::parse_authored_module(&text)
                .unwrap_or_else(|error| panic!("{}: {error:?}", tower.stem()));
            assert!(module.prototype.spine.nodes.is_empty(), "{}", tower.stem());
            let ceiling = f64::from(observed_hex::TILE_LEVEL_HEIGHT);
            for point in module.prototype.hulls.iter().flatten() {
                assert!(
                    f64::from(point.y) <= ceiling + 1e-3,
                    "{}: a hull at {}",
                    tower.stem(),
                    point.y
                );
            }
        }
    }

    /// The gallery's path starts at the climb's ends, leaves through the opening,
    /// and passes every face, never crossing the guard wall.
    #[test]
    fn the_deck_joins_the_climb_to_every_face() {
        for tower in towers() {
            let module = crate::parse_authored_module(&stair_tower(&tower, FIRST_VARIANT))
                .unwrap_or_else(|error| panic!("{}: {error:?}", tower.stem()));
            let deck = &module.prototype.deck.nodes;
            assert_eq!(deck.len(), 14, "{}", tower.stem());
            if let Some(&foot) = module.prototype.spine.nodes.first() {
                assert!((deck[0].x - foot.x).abs() < 1e-3 && (deck[0].z - foot.z).abs() < 1e-3);
            }
            // Every leg after the opening stays outside the guard wall's hexagon.
            let apothem = f64::from(observed_hex::ACROSS_FLATS) * 0.5 * GUARD_WALL;
            let outside = |x: f64, z: f64| {
                (0..6).any(|face| {
                    let (u, v) = (corners()[face], corners()[(face + 1) % 6]);
                    let (mx, mz) = ((u.0 + v.0) * 0.5, -(u.1 + v.1) * 0.5);
                    (x * mx + z * mz) / mx.hypot(mz) > apothem
                })
            };
            for pair in deck[1..].windows(2) {
                for sample in 0..=16u8 {
                    let t = f32::from(sample) / 16.0;
                    let p = pair[0].lerp(pair[1], t);
                    assert!(
                        outside(f64::from(p.x), f64::from(p.z)),
                        "{}: a gallery leg crosses the guard wall at {p}",
                        tower.stem()
                    );
                }
            }
        }
    }

    /// Every tower survives the importer it is written for.
    #[test]
    fn every_tower_parses_and_validates() {
        for (stem, text) in builders() {
            crate::parse_authored_module(&text)
                .unwrap_or_else(|error| panic!("{stem} does not validate: {error:?}"));
        }
    }
}
