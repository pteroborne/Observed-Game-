//! Ramps: a storey climbed as a switchback, dressed for each place on the climb.
//!
//! # Why it folds
//!
//! The ramp used to be one wedge across the whole hexagon, rising a full storey in
//! the cell's 14 m: a slope of 0.57, the floor itself tilted, and no landing at
//! either end, so the top met the next cell's floor at a step a body sometimes failed
//! to take. A storey does not fit comfortably in one straight run across a cell. Two
//! half-storey flights side by side do: each climbs 4 m over 9 m (0.44, the stair
//! tower's own gradient), and the fold leaves room for a landing at every end.
//!
//! # The skeleton every district shares
//!
//! Through the west door, the first flight climbs east up the south half of the cell
//! to a turning landing across the east end; the second climbs back west up the
//! north half to a landing over the west door; a balcony runs east from there, over
//! the first flight, to the east door one storey up. Every joint is flush. The plan
//! and the spine are identical in every district, so a body and a bot climb every
//! one of them the same way; what changes is what the climb is made of.
//!
//! # The districts
//!
//! One dressing a place on the climb (`ArchitectureRegister::CLIMB`), and the plain
//! one for the Backrooms and every register off the climb:
//!
//! | dressing | flights | between the flights | edges |
//! | --- | --- | --- | --- |
//! | Backrooms | solid | a plain wall | parapets |
//! | Library | solid, lined with stacks | a full-height stack | the stack |
//! | Lumen | slabs over open floor | thin guards | parapets |
//! | Zen | solid | a row of screens | parapets |
//! | Monument | solid, chamfered | a plain wall, and four piers | parapets |
//! | Reactor | slabs on struts | thin guards | parapets |
//! | Sky | slabs | nothing | nothing |
//!
//! The Sky's has no rails on purpose: the top of the climb is tower tops and the
//! walkways between them, railless.

use super::Builder;
use super::GENERATED_NOTE;
use super::entities::{
    Meta, lateral_port, stair_node, tile_cell, vertical_port, wall_fixture, worldspawn,
};
use super::geometry::{
    DOOR_TOP, FLOOR_TOP, LEVEL, P2, P3, centroid, custom_plane, door_wall, door_wall_default,
    hex_slab, prism, pylon, side_plane, sloped_prism, translate, wall,
};

/// Where the two flights begin and end, in plan `x`: each runs the middle 9 m of the
/// cell, leaving a landing at either end.
const FLIGHT_WEST: f64 = -72.0;
const FLIGHT_EAST: f64 = 72.0;
/// The turning landing's height: the half-level the first flight climbs to.
const TURN: f64 = FLOOR_TOP + LEVEL * 0.5;
/// The upper floor's walking height, where the second flight arrives.
const UPPER: f64 = FLOOR_TOP + LEVEL;
/// The ramp's full height: two storeys.
const TOP: f64 = 2.0 * LEVEL;
/// Half the gap between the flights, which the centre wall or its stand-in fills.
const SPLIT: f64 = 4.0;
/// How far a parapet, guard or the centre wall stands above what it guards: 1.1 m.
const GUARD: f64 = 18.0;
/// The hexagon pulled in to its walls' inner faces, near enough: a plan polygon
/// against this touches the wall without leaving the footprint.
const INNER_X: f64 = 103.0;
const INNER_Y: f64 = 58.0;
/// Where the diagonal walls' inner faces cross a flight's ends, at `x = ±72`, and
/// the corner they meet in on the cell's long axis.
const FLIGHT_EDGE_Y: f64 = 76.0;
const CORNER_Y: f64 = 117.0;

/// What a flight is made of.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Flights {
    /// A mass down to the floor: the space under a flight is solid.
    Solid,
    /// A half-metre slab: the floor runs on under it.
    Slab,
}

/// What stands in the gap between the two flights.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Divide {
    /// A wall rising with the second flight, a balustrade's height above it.
    Wall,
    /// A wall the full height of the room: a stack of shelves.
    Stack,
    /// A row of screens, each a little taller than a body, with gaps between them
    /// narrower than one.
    Screens,
    /// A thin guard along each flight's open edge.
    Guards,
    /// Nothing.
    Open,
}

/// One district's ramp.
struct Dressing {
    stem: &'static str,
    title: &'static str,
    registers: &'static str,
    variant: i32,
    flights: Flights,
    divide: Divide,
    /// Parapets on the balcony where it overlooks the stairwell.
    parapets: bool,
    /// Bookcases along the outer walls of both flights.
    stacks: bool,
    /// Struts under the second flight's slab.
    struts: bool,
    /// Piers at the corners of both landings.
    piers: bool,
    /// Chamfer on the landings' and balcony's top edges.
    chamfer: f64,
    lights: &'static [(usize, f64, f64)],
}

/// Light where the climb turns: under the balcony on the first flight, over the
/// turning landing, above the second flight, and over the balcony.
const LIGHTS: &[(usize, f64, f64)] = &[
    (1, 0.5, 104.0),
    (5, 0.2, 112.0),
    (4, 0.55, 200.0),
    (2, 0.5, 200.0),
];

const DRESSINGS: [Dressing; 7] = [
    Dressing {
        stem: "hall_ramp",
        title: "the Backrooms, and every register off the climb: the plain one",
        registers: "liminal_grid,monolith,institutional,wellshaft",
        variant: 0,
        flights: Flights::Solid,
        divide: Divide::Wall,
        parapets: true,
        stacks: false,
        struts: false,
        piers: false,
        chamfer: 2.0,
        lights: LIGHTS,
    },
    Dressing {
        stem: "hall_ramp_library",
        title: "the Library: two climbs between the stacks",
        registers: "infinite_gallery",
        variant: 21,
        flights: Flights::Solid,
        divide: Divide::Stack,
        parapets: false,
        stacks: true,
        struts: false,
        piers: false,
        chamfer: 2.0,
        lights: &[(1, 0.5, 104.0), (5, 0.2, 112.0), (4, 0.55, 200.0)],
    },
    Dressing {
        stem: "hall_ramp_lumen",
        title: "Lumen: slabs floating over a lit floor",
        registers: "overlit_grid",
        variant: 22,
        flights: Flights::Slab,
        divide: Divide::Guards,
        parapets: true,
        stacks: false,
        struts: false,
        piers: false,
        chamfer: 0.0,
        lights: &[
            (1, 0.5, 104.0),
            (5, 0.2, 112.0),
            (4, 0.55, 200.0),
            (2, 0.5, 200.0),
            (3, 0.3, 56.0),
        ],
    },
    Dressing {
        stem: "hall_ramp_zen",
        title: "Zen: a climb behind screens",
        registers: "shadow_screen",
        variant: 23,
        flights: Flights::Solid,
        divide: Divide::Screens,
        parapets: true,
        stacks: false,
        struts: false,
        piers: false,
        chamfer: 2.0,
        lights: &[(5, 0.2, 112.0), (4, 0.55, 200.0), (2, 0.5, 200.0)],
    },
    Dressing {
        stem: "hall_ramp_monument",
        title: "the Monument: heavy masses and piers",
        registers: "facet_monument",
        variant: 24,
        flights: Flights::Solid,
        divide: Divide::Wall,
        parapets: true,
        stacks: false,
        struts: false,
        piers: true,
        chamfer: 6.0,
        lights: LIGHTS,
    },
    Dressing {
        stem: "hall_ramp_reactor",
        title: "the Reactor: a catwalk on struts",
        registers: "megastructure",
        variant: 25,
        flights: Flights::Slab,
        divide: Divide::Guards,
        parapets: true,
        stacks: false,
        struts: true,
        piers: false,
        chamfer: 0.0,
        lights: LIGHTS,
    },
    Dressing {
        stem: "hall_ramp_sky",
        title: "the Sky: open slabs, no rails",
        registers: "thinning",
        variant: 26,
        flights: Flights::Slab,
        divide: Divide::Open,
        parapets: false,
        stacks: false,
        struts: false,
        piers: false,
        chamfer: 0.0,
        lights: &[(5, 0.2, 112.0), (4, 0.55, 200.0)],
    },
];

/// The height of the first flight's walking surface at plan `x`.
fn first_flight(x: f64) -> f64 {
    FLOOR_TOP + (x - FLIGHT_WEST) / (FLIGHT_EAST - FLIGHT_WEST) * (TURN - FLOOR_TOP)
}

/// The height of the second flight's walking surface at plan `x`.
fn second_flight(x: f64) -> f64 {
    TURN + (FLIGHT_EAST - x) / (FLIGHT_EAST - FLIGHT_WEST) * (UPPER - TURN)
}

/// Three points on the plane `lift` above the first flight's surface.
fn first_plane(lift: f64) -> [P3; 3] {
    [
        (FLIGHT_WEST, 0.0, first_flight(FLIGHT_WEST) + lift),
        (FLIGHT_WEST, -100.0, first_flight(FLIGHT_WEST) + lift),
        (FLIGHT_EAST, 0.0, first_flight(FLIGHT_EAST) + lift),
    ]
}

/// Three points on the plane `lift` above the second flight's surface.
fn second_plane(lift: f64) -> [P3; 3] {
    [
        (FLIGHT_EAST, 0.0, second_flight(FLIGHT_EAST) + lift),
        (FLIGHT_EAST, 100.0, second_flight(FLIGHT_EAST) + lift),
        (FLIGHT_WEST, 0.0, second_flight(FLIGHT_WEST) + lift),
    ]
}

/// The south half of the cell between the flights' ends: the first flight's plan, and
/// the balcony's above it.
fn south_band() -> [P2; 5] {
    [
        (FLIGHT_WEST, -SPLIT),
        (FLIGHT_EAST, -SPLIT),
        (FLIGHT_EAST, -FLIGHT_EDGE_Y),
        (0.0, -CORNER_Y),
        (FLIGHT_WEST, -FLIGHT_EDGE_Y),
    ]
}

/// The north half: the second flight's plan.
fn north_band() -> [P2; 5] {
    [
        (FLIGHT_WEST, SPLIT),
        (FLIGHT_WEST, FLIGHT_EDGE_Y),
        (0.0, CORNER_Y),
        (FLIGHT_EAST, FLIGHT_EDGE_Y),
        (FLIGHT_EAST, SPLIT),
    ]
}

/// The cell's east or west end, beyond the flights: a landing's plan.
fn end_band(east: bool) -> [P2; 4] {
    let sign = if east { 1.0 } else { -1.0 };
    let (inner, outer) = (FLIGHT_EAST * sign, INNER_X * sign);
    [
        (inner, -FLIGHT_EDGE_Y),
        (inner, FLIGHT_EDGE_Y),
        (outer, INNER_Y),
        (outer, -INNER_Y),
    ]
}

/// The strip of the gap between the flights from `x0` to `x1`, `half` either side of
/// the centreline.
fn gap(x0: f64, x1: f64, half: f64) -> [P2; 4] {
    [(x0, -half), (x1, -half), (x1, half), (x0, half)]
}

/// A slab whose top is the plane through `top` and whose underside lies `thickness`
/// below it, over the plan polygon `corners`.
fn slab(corners: &[P2], top: [P3; 3], thickness: f64) -> String {
    let hint = centroid(corners);
    let mut out = String::from("{\n");
    for index in 0..corners.len() {
        out.push_str(&side_plane(
            corners[index],
            corners[(index + 1) % corners.len()],
            0.0,
            hint,
        ));
    }
    let lowered = top.map(|(x, y, z)| (x, y, z - thickness));
    out.push_str(&custom_plane(top[0], top[1], top[2], true));
    out.push_str(&custom_plane(lowered[0], lowered[1], lowered[2], false));
    out.push_str("}\n");
    out
}

/// One flight over `band`, its surface the plane `top`.
fn flight(dressing: &Dressing, band: &[P2], top: [P3; 3]) -> String {
    match dressing.flights {
        Flights::Solid => sloped_prism(band, 0.0, top, None),
        Flights::Slab => slab(band, top, FLOOR_TOP),
    }
}

/// A bookcase along the wall from `a` to `b`, `depth` deep toward the cell's centre,
/// standing 3 m above the flight surface `top`.
fn bookcase(a: P2, b: P2, top: [P3; 3]) -> String {
    let depth = 12.0;
    let length = (b.0 - a.0).hypot(b.1 - a.1);
    // The inward normal: toward the origin.
    let mut normal = (-(b.1 - a.1) / length, (b.0 - a.0) / length);
    if normal.0 * -a.0 + normal.1 * -a.1 < 0.0 {
        normal = (-normal.0, -normal.1);
    }
    let inset = |p: P2| (p.0 + normal.0 * depth, p.1 + normal.1 * depth);
    let raised = top.map(|(x, y, z)| (x, y, z + 48.0));
    sloped_prism(&[a, b, inset(b), inset(a)], 0.0, raised, None)
}

/// The climb through a ramp, west door to east: up the first flight, across the
/// turning landing, up the second, round the top landing and out along the balcony.
/// It folds back over itself in plan, the balcony above the first flight, but never
/// within a storey of it - four metres at the closest - so a body on one stretch is
/// never nearer another.
fn spine() -> String {
    let nodes = [
        (-100.0, 0.0, FLOOR_TOP),
        (FLIGHT_WEST, -40.0, FLOOR_TOP),
        (FLIGHT_EAST, -40.0, TURN),
        (88.0, 0.0, TURN),
        (FLIGHT_EAST, 40.0, TURN),
        (FLIGHT_WEST, 40.0, UPPER),
        (-88.0, 0.0, UPPER),
        (FLIGHT_WEST, -40.0, UPPER),
        (FLIGHT_EAST, -40.0, UPPER),
        (100.0, 0.0, UPPER),
    ];
    let mut out = String::new();
    for (index, &(x, y, z)) in nodes.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        out.push_str(&stair_node(index as u16, x, y, z));
    }
    out
}

/// What stands between the flights, for `dressing`.
fn divide(dressing: &Dressing) -> String {
    let mut out = String::new();
    match dressing.divide {
        Divide::Wall => {
            out.push_str("// Centre wall: the second flight's balustrade, rising with it\n");
            out.push_str(&sloped_prism(
                &gap(FLIGHT_WEST, FLIGHT_EAST, SPLIT),
                0.0,
                second_plane(GUARD),
                None,
            ));
        }
        Divide::Stack => {
            out.push_str(
                "// Centre stack: shelving the height of the room, the flights either side\n",
            );
            out.push_str(&prism(
                &gap(FLIGHT_WEST, FLIGHT_EAST, SPLIT),
                0.0,
                TOP - 3.0 * FLOOR_TOP - 24.0,
                None,
                2.0,
                0.0,
            ));
        }
        Divide::Screens => {
            out.push_str(
                "// A row of screens: each taller than a body, the gaps narrower than one\n",
            );
            const PANELS: usize = 6;
            const OPENING: f64 = 8.0;
            #[allow(clippy::cast_precision_loss)]
            let width =
                ((FLIGHT_EAST - FLIGHT_WEST) - OPENING * (PANELS - 1) as f64) / PANELS as f64;
            for panel in 0..PANELS {
                #[allow(clippy::cast_precision_loss)]
                let x0 = FLIGHT_WEST + panel as f64 * (width + OPENING);
                out.push_str(&sloped_prism(
                    &gap(x0, x0 + width, 2.0),
                    0.0,
                    second_plane(GUARD + 22.0),
                    None,
                ));
            }
        }
        Divide::Guards => {
            out.push_str("// Thin guards along each flight's open edge\n");
            out.push_str(&slab(
                &gap(FLIGHT_WEST, FLIGHT_EAST, 1.0).map(|(x, y)| (x, y + SPLIT - 1.0)),
                second_plane(GUARD),
                GUARD + FLOOR_TOP,
            ));
            out.push_str(&slab(
                &gap(FLIGHT_WEST, FLIGHT_EAST, 1.0).map(|(x, y)| (x, y - SPLIT + 1.0)),
                first_plane(GUARD),
                GUARD + FLOOR_TOP,
            ));
        }
        Divide::Open => {}
    }
    out
}

/// One district's ramp.
fn ramp(dressing: &Dressing) -> String {
    let landing = |plan: &[P2], z0: f64, z1: f64| prism(plan, z0, z1, None, dressing.chamfer, 0.0);
    let mut brushes = String::from("// Ground slab: the entry landing\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 2.0, 0.0));
    brushes.push_str("// First flight, up the south side to the turning landing\n");
    brushes.push_str(&flight(dressing, &south_band(), first_plane(0.0)));
    brushes.push_str("// Turning landing across the east end\n");
    let turn_base = match dressing.flights {
        Flights::Solid => 0.0,
        Flights::Slab => TURN - FLOOR_TOP,
    };
    brushes.push_str(&landing(&end_band(true), turn_base, TURN));
    brushes.push_str("// Second flight, back up the north side to the upper floor\n");
    brushes.push_str(&flight(dressing, &north_band(), second_plane(0.0)));
    brushes.push_str(&divide(dressing));
    brushes.push_str("// Top landing over the west door, and the balcony out to the east door\n");
    brushes.push_str(&landing(&end_band(false), UPPER - FLOOR_TOP, UPPER));
    brushes.push_str(&landing(&south_band(), UPPER - FLOOR_TOP, UPPER));
    brushes.push_str(&landing(&end_band(true), UPPER - FLOOR_TOP, UPPER));
    if dressing.parapets {
        brushes.push_str("// Parapets where the balcony overlooks the stairwell\n");
        if dressing.divide != Divide::Stack {
            brushes.push_str(&prism(
                &[
                    (FLIGHT_WEST, -SPLIT - 4.0),
                    (FLIGHT_EAST + 4.0, -SPLIT - 4.0),
                    (FLIGHT_EAST + 4.0, -SPLIT),
                    (FLIGHT_WEST, -SPLIT),
                ],
                UPPER,
                UPPER + GUARD,
                None,
                2.0,
                0.0,
            ));
        }
        brushes.push_str(&prism(
            &[
                (FLIGHT_EAST, -SPLIT - 4.0),
                (FLIGHT_EAST + 4.0, -SPLIT - 4.0),
                (FLIGHT_EAST + 4.0, FLIGHT_EDGE_Y - 2.0),
                (FLIGHT_EAST, FLIGHT_EDGE_Y - 2.0),
            ],
            UPPER,
            UPPER + GUARD,
            None,
            2.0,
            0.0,
        ));
    } else if dressing.divide == Divide::Stack {
        brushes.push_str("// Parapet where the balcony overlooks the second flight's foot\n");
        brushes.push_str(&prism(
            &[
                (FLIGHT_EAST, SPLIT),
                (FLIGHT_EAST + 4.0, SPLIT),
                (FLIGHT_EAST + 4.0, FLIGHT_EDGE_Y - 2.0),
                (FLIGHT_EAST, FLIGHT_EDGE_Y - 2.0),
            ],
            UPPER,
            UPPER + GUARD,
            None,
            2.0,
            0.0,
        ));
    }
    if dressing.stacks {
        brushes.push_str("// Stacks along the walls of both flights\n");
        let (sw, s, se) = (
            (FLIGHT_WEST, -FLIGHT_EDGE_Y),
            (0.0, -CORNER_Y),
            (FLIGHT_EAST, -FLIGHT_EDGE_Y),
        );
        brushes.push_str(&bookcase(sw, s, first_plane(0.0)));
        brushes.push_str(&bookcase(s, se, first_plane(0.0)));
        let (nw, n, ne) = (
            (FLIGHT_WEST, FLIGHT_EDGE_Y),
            (0.0, CORNER_Y),
            (FLIGHT_EAST, FLIGHT_EDGE_Y),
        );
        brushes.push_str(&bookcase(nw, n, second_plane(0.0)));
        brushes.push_str(&bookcase(n, ne, second_plane(0.0)));
    }
    if dressing.struts {
        brushes.push_str("// Struts under the second flight's slab\n");
        for x in [-36.0, 24.0] {
            brushes.push_str(&translate(
                &pylon(5.0, 0.0, second_flight(x) - FLOOR_TOP, 30.0, 0.0, 0.0),
                x,
                40.0,
                0.0,
            ));
        }
        brushes.push_str(&translate(
            &pylon(5.0, 0.0, first_flight(48.0) - FLOOR_TOP, 30.0, 0.0, 0.0),
            48.0,
            -40.0,
            0.0,
        ));
    }
    if dressing.piers {
        brushes.push_str("// Piers at the landings' corners\n");
        for (x, y, z) in [
            (90.0, 56.0, TOP),
            (90.0, -56.0, TOP),
            (-90.0, 56.0, TOP),
            (-90.0, -56.0, TOP),
        ] {
            brushes.push_str(&translate(
                &pylon(7.0, 0.0, z - FLOOR_TOP, 0.0, 3.0, 3.0),
                x,
                y,
                0.0,
            ));
        }
    }
    brushes.push_str(&hex_slab(TOP - FLOOR_TOP, TOP, 0.0, 3.0));
    for face in 0..6 {
        if face == 3 {
            brushes.push_str(&door_wall_default(face, 0.0, TOP));
        } else if face == 0 {
            // The east door sits a full level up, on the balcony.
            brushes.push_str(&door_wall(
                face,
                0.0,
                TOP,
                LEVEL + FLOOR_TOP,
                LEVEL + DOOR_TOP,
                10.0,
                8.0,
            ));
        } else {
            brushes.push_str(&wall(face, 0.0, TOP));
        }
    }
    let mut lights = String::new();
    for &(face, along, z) in dressing.lights {
        let (fixture, source) = wall_fixture(face, along, z, 20.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }
    let mut out = format!(
        "// Two-level switchback ramp, {}: enter west, turn at mid-height, leave east one level up.\n",
        dressing.title
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(
            &format!("authored/{}", dressing.stem),
            "hall_ramp",
            dressing.variant,
            2,
            10,
        )
        .with_register_scope(dressing.registers)
        .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 2, "ramp"));
    out.push_str(&lateral_port(3, "door", "west_entry", 0, 0, 0));
    out.push_str(&vertical_port("up", "ramp_open", "upper_ramp", 0));
    out.push_str(&spine());
    out.push_str(&lights);
    out
}

fn dressed(stem: &str) -> String {
    ramp(
        DRESSINGS
            .iter()
            .find(|dressing| dressing.stem == stem)
            .expect("a ramp dressing"),
    )
}

#[must_use]
pub fn hall_ramp() -> String {
    dressed("hall_ramp")
}
#[must_use]
pub fn hall_ramp_library() -> String {
    dressed("hall_ramp_library")
}
#[must_use]
pub fn hall_ramp_lumen() -> String {
    dressed("hall_ramp_lumen")
}
#[must_use]
pub fn hall_ramp_zen() -> String {
    dressed("hall_ramp_zen")
}
#[must_use]
pub fn hall_ramp_monument() -> String {
    dressed("hall_ramp_monument")
}
#[must_use]
pub fn hall_ramp_reactor() -> String {
    dressed("hall_ramp_reactor")
}
#[must_use]
pub fn hall_ramp_sky() -> String {
    dressed("hall_ramp_sky")
}

#[must_use]
pub fn builders() -> Vec<Builder> {
    vec![
        ("hall_ramp", hall_ramp as fn() -> String),
        ("hall_ramp_library", hall_ramp_library),
        ("hall_ramp_lumen", hall_ramp_lumen),
        ("hall_ramp_zen", hall_ramp_zen),
        ("hall_ramp_monument", hall_ramp_monument),
        ("hall_ramp_reactor", hall_ramp_reactor),
        ("hall_ramp_sky", hall_ramp_sky),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ramp_reproduces_its_committed_file() {
        super::super::assert_reproduces(&builders());
    }

    /// Every register has exactly one ramp: the districts on the climb their own, the
    /// rest the plain one. Two ramps for one register would let a draw mix them.
    #[test]
    fn every_register_has_one_ramp() {
        for &register in crate::tile_source::REGISTERS {
            let owners = DRESSINGS
                .iter()
                .filter(|dressing| dressing.registers.split(',').any(|slug| slug == register))
                .count();
            assert_eq!(owners, 1, "{register:?} has {owners} ramps");
        }
    }
}
