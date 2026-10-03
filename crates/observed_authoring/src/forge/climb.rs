//! Climb compositions: a storey climbed across several cells instead of inside one.
//!
//! # Why several
//!
//! Every single-cell climb rises a full 8 m storey in a hexagon 14 m across, and the
//! first-person captures showed what that costs: the switchback ramp and the spiral
//! stair were both too steep and too narrow to read as anything but an obstacle. Laid
//! across three cells instead, the same storey is a broad flight about 17 degrees at
//! its steepest - something a body walks up rather than negotiates
//! (`docs/climb_compositions_plan.md`).
//!
//! # The four cells
//!
//! Authored climbing east, and turned to every heading by the catalogue:
//!
//! | cell | storey | floor | openings |
//! | --- | --- | --- | --- |
//! | foot | L | an entry pad, then rising 2.1 m | door west, span east |
//! | mid | L | rising a further 2.4 m | spans west and east |
//! | high | L | rising the last 3.5 m to the storey line | span west, open above |
//! | landing | L+1 | the flight's last few centimetres, then a pad | door east |
//!
//! Every floor runs wall to wall, so a flight has no edge to fall from and needs no
//! rail. The rise is back-loaded: the foot and mid keep the ordinary ceiling, and a
//! body's head clears it by about 0.7 m only if the flight is still low there, so the
//! high cell - open to the landing above - takes the steepest stretch.
//!
//! # The districts
//!
//! The hexagon's east and west faces are flat, so a straight aisle 7 m wide runs the
//! whole length of every cell, and either side of it the cell bulges into a triangular
//! alcove nearly 4 m deep. A district dresses the alcoves and the aisle's edges; the
//! flight, its spans and its spine are the same in every one, so a body climbs every
//! district's the same way and any district's high cell meets any district's landing:
//!
//! | dressing | the alcoves | the aisle |
//! | --- | --- | --- |
//! | Backrooms | empty | plain walls and ceiling |
//! | Library | stacks standing across them, floor to ceiling | |
//! | Lumen | lit plinths, tiled, too high to step onto | |
//! | Zen | lanterns | paper screens along both edges |
//! | Monument | solid masonry | a pier at every seam |
//! | Reactor | columns | a balustrade along both edges, a girder overhead |
//! | Sky | | parapets for walls, no ceiling |
//!
//! Where the landing is open over the high cell, the Library's stacks and the
//! Monument's masonry carry on up through it, so the high cell reads as one tall
//! stairwell rather than a ceiling missing.

use super::GENERATED_NOTE;
use super::entities::{
    Meta, lateral_port, stair_node, tile_cell, tile_light, vertical_port, wall_fixture, worldspawn,
};
use super::geometry::{
    FLOOR_TOP, LEVEL, P2, P3, WALL, centroid, corners, custom_plane, door_wall_default, edge,
    flat_plane, hex_slab, offset_inward, prism, pylon, side_plane, translate, wall,
};

/// The cell's half-width across its east and west faces, in plan `x`.
const APOTHEM: f64 = 112.0;
/// Where the foot's entry pad ends and its flight begins.
const PAD_END: f64 = -80.0;
/// Where the landing's exit pad begins: the flight arrives at the next floor here.
const ARRIVAL: f64 = 80.0;
/// The storey above's floor top, in the high cell's frame.
const NEXT_FLOOR: f64 = LEVEL + FLOOR_TOP;
/// The ordinary ceiling slab's underside.
const CEILING: f64 = LEVEL - FLOOR_TOP;
/// The flight's height where it crosses from the foot into the mid, and from the mid
/// into the high cell.
const FOOT_TOP: f64 = FLOOR_TOP + LOW_RISE * (APOTHEM - PAD_END) / LOW_RUN;
const MID_TOP: f64 = FLOOR_TOP + LOW_RISE;
/// The low two cells climb `LOW_RISE` over `LOW_RUN`; the high cell climbs the rest.
const LOW_RISE: f64 = 72.0;
const LOW_RUN: f64 = (APOTHEM - PAD_END) + 2.0 * APOTHEM;
/// Where the high cell's flight reaches the storey line, in plan `x`.
const STOREY_LINE: f64 = -APOTHEM + (LEVEL - MID_TOP) * HIGH_RUN / HIGH_RISE;
const HIGH_RISE: f64 = NEXT_FLOOR - MID_TOP;
const HIGH_RUN: f64 = ARRIVAL + APOTHEM;
/// The landing's floor starts this far above its base, where it meets the flight: a
/// lip of six centimetres rather than a wedge thinning to nothing, which is a brush
/// no hull can be made of.
const LIP: f64 = 1.0;
/// Faces, as the forge numbers them: east first, counterclockwise.
const EAST: usize = 0;
const WEST: usize = 3;
/// The faces along the climb's sides.
const SIDES: [usize; 4] = [1, 2, 4, 5];

/// Half the aisle's width: the straight run inside the walls the whole length of a cell.
const AISLE: f64 = 56.0;
/// The inner face of the diagonal walls where they meet on the cell's long axis.
const APEX: f64 = 118.786;
/// Where an alcove's base, at the aisle's edge, meets the diagonal walls.
const ALCOVE_END: f64 = (APEX - AISLE) * APOTHEM / 64.0;
/// Lumen's plinths' height above the flight: 0.8 m, too high to step onto.
const PLINTH: f64 = 13.0;
/// A balustrade's, a parapet's or a screen's height above what it stands on: 1.1 m.
const GUARD: f64 = 18.0;

/// Which cell of the composition a builder is making.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Foot,
    Mid,
    High,
    Landing,
}

impl Part {
    const ALL: [Self; 4] = [Self::Foot, Self::Mid, Self::High, Self::Landing];

    const fn stem(self) -> &'static str {
        match self {
            Self::Foot => "climb_foot",
            Self::Mid => "climb_mid",
            Self::High => "climb_high",
            Self::Landing => "climb_landing",
        }
    }

    /// The walking surface, as straight stretches `(x0, z0, x1, z1)` west to east. The
    /// landing's begins at the storey line: west of it is open over the high cell.
    fn surface(self) -> Vec<(f64, f64, f64, f64)> {
        match self {
            Self::Foot => vec![
                (-APOTHEM, FLOOR_TOP, PAD_END, FLOOR_TOP),
                (PAD_END, FLOOR_TOP, APOTHEM, FOOT_TOP),
            ],
            Self::Mid => vec![(-APOTHEM, FOOT_TOP, APOTHEM, MID_TOP)],
            Self::High => vec![
                (-APOTHEM, MID_TOP, STOREY_LINE, LEVEL),
                (STOREY_LINE, LEVEL, APOTHEM, LEVEL),
            ],
            Self::Landing => vec![
                (STOREY_LINE, LIP, ARRIVAL, FLOOR_TOP),
                (ARRIVAL, FLOOR_TOP, APOTHEM, FLOOR_TOP),
            ],
        }
    }

    /// The walking surface's height at `x`, if there is one there.
    fn height(self, x: f64) -> Option<f64> {
        self.surface().into_iter().find_map(|(x0, z0, x1, z1)| {
            (x0..=x1)
                .contains(&x)
                .then(|| z0 + (z1 - z0) * (x - x0) / (x1 - x0))
        })
    }

    /// How high anything standing in this cell may rise: the ceiling, or in the high
    /// cell, which has none, the storey line.
    const fn top(self, ceiling: bool) -> f64 {
        match self {
            Self::High => LEVEL,
            _ if ceiling => CEILING,
            _ => LEVEL,
        }
    }

    /// Where along the climb this cell's own light hangs.
    const fn light_x(self) -> f64 {
        match self {
            Self::Foot | Self::Mid => 0.0,
            Self::High => -40.0,
            Self::Landing => 80.0,
        }
    }
}

/// One district's climb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dressing {
    Backrooms,
    Library,
    Lumen,
    Zen,
    Monument,
    Reactor,
    Sky,
}

impl Dressing {
    const ALL: [Self; 7] = [
        Self::Backrooms,
        Self::Library,
        Self::Lumen,
        Self::Zen,
        Self::Monument,
        Self::Reactor,
        Self::Sky,
    ];

    /// The suffix on each file's stem: none for the plain one.
    const fn suffix(self) -> &'static str {
        match self {
            Self::Backrooms => "",
            Self::Library => "_library",
            Self::Lumen => "_lumen",
            Self::Zen => "_zen",
            Self::Monument => "_monument",
            Self::Reactor => "_reactor",
            Self::Sky => "_sky",
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::Backrooms => "",
            Self::Library => ", in the Library: between the stacks",
            Self::Lumen => ", in Lumen: past lit plinths",
            Self::Zen => ", in Zen: between paper screens",
            Self::Monument => ", in the Monument: through masonry and piers",
            Self::Reactor => ", in the Reactor: a gantry among columns",
            Self::Sky => ", in the Sky: open to the air",
        }
    }

    /// The registers it serves: every register has exactly one.
    const fn registers(self) -> &'static str {
        match self {
            Self::Backrooms => "liminal_grid,monolith,institutional,wellshaft",
            Self::Library => "infinite_gallery",
            Self::Lumen => "overlit_grid",
            Self::Zen => "shadow_screen",
            Self::Monument => "facet_monument",
            Self::Reactor => "megastructure",
            Self::Sky => "thinning",
        }
    }

    /// The authored variant, the same numbers the ramps used for the same districts.
    const fn variant(self) -> i32 {
        match self {
            Self::Backrooms => 0,
            Self::Library => 21,
            Self::Lumen => 22,
            Self::Zen => 23,
            Self::Monument => 24,
            Self::Reactor => 25,
            Self::Sky => 26,
        }
    }

    /// Whether the cell keeps its ceiling slab and full side walls.
    const fn enclosed(self) -> bool {
        !matches!(self, Self::Sky)
    }
}

/// `poly` clipped to the side of the line `axis = at` that `keep_greater` names, where
/// `axis` 0 is plan `x` and 1 is plan `y`.
fn clip_axis(poly: &[P2], axis: usize, at: f64, keep_greater: bool) -> Vec<P2> {
    let value = |p: P2| if axis == 0 { p.0 } else { p.1 };
    let inside = |p: P2| {
        if keep_greater {
            value(p) >= at
        } else {
            value(p) <= at
        }
    };
    let mut out = Vec::new();
    for index in 0..poly.len() {
        let (a, b) = (poly[index], poly[(index + 1) % poly.len()]);
        if inside(a) {
            out.push(a);
        }
        if inside(a) != inside(b) {
            let t = (at - value(a)) / (value(b) - value(a));
            out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
        }
    }
    out
}

/// The cell's hexagon clipped to the side of `x = at` that `keep_east` names.
fn clip(at: f64, keep_east: bool) -> Vec<P2> {
    clip_axis(&corners(), 0, at, keep_east)
}

/// `poly` clipped to `x0..=x1`, or `None` where nothing of it is left.
fn between(poly: &[P2], x0: f64, x1: f64) -> Option<Vec<P2>> {
    let out = clip_axis(&clip_axis(poly, 0, x0, true), 0, x1, false);
    (out.len() >= 3 && x1 - x0 > 0.5).then_some(out)
}

/// The alcove beside the aisle on the north side (`north`) or the south: from the
/// aisle's edge out to the diagonal walls' inner faces.
fn alcove(north: bool) -> Vec<P2> {
    let side = if north { 1.0 } else { -1.0 };
    vec![
        (-ALCOVE_END, side * AISLE),
        (ALCOVE_END, side * AISLE),
        (0.0, side * APEX),
    ]
}

/// A rectangle in plan.
fn rect(x0: f64, x1: f64, y0: f64, y1: f64) -> Vec<P2> {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

/// A floor brush over `plan` from the cell's base up to the plane rising east through
/// `(x0, z0)` and `(x1, z1)`, capped flat at `cap`.
fn flight(plan: &[P2], (x0, z0): (f64, f64), (x1, z1): (f64, f64), cap: Option<f64>) -> String {
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
    out.push_str(&flat_plane(0.0, false));
    let top: [P3; 3] = [(x0, -100.0, z0), (x0, 100.0, z0), (x1, 0.0, z1)];
    out.push_str(&custom_plane(top[0], top[1], top[2], true));
    if let Some(cap) = cap {
        out.push_str(&flat_plane(cap, true));
    }
    out.push_str("}\n");
    out
}

/// A mass over `plan` from the cell's base to `rise` above the walking surface, one
/// brush for each straight stretch of the flight it stands on, never above `top`.
///
/// A stretch already at `top` is left out: anything there would stand no higher than
/// the floor it is meant to rise from.
fn on_flight(part: Part, plan: &[P2], rise: f64, top: f64) -> String {
    let mut out = String::new();
    for (x0, z0, x1, z1) in part.surface() {
        if z0 >= top && z1 >= top {
            continue;
        }
        let Some(piece) = between(plan, x0, x1) else {
            continue;
        };
        let cap = (z0.max(z1) + rise > top).then_some(top);
        out.push_str(&flight(&piece, (x0, z0 + rise), (x1, z1 + rise), cap));
    }
    out
}

/// The walls round a cell: a door on `door`, nothing on the `open` faces, and a
/// sealed wall everywhere else.
fn walls(door: Option<usize>, open: &[usize]) -> String {
    let mut out = String::new();
    for face in 0..6 {
        if Some(face) == door {
            out.push_str(&door_wall_default(face, 0.0, LEVEL));
        } else if !open.contains(&face) {
            out.push_str(&wall(face, 0.0, LEVEL));
        }
    }
    out
}

/// A parapet on `face` where the wall would stand, `GUARD` above the walking surface,
/// and over the landing's open part `GUARD` above its floor.
fn parapet(part: Part, face: usize) -> String {
    let (a, b) = edge(face);
    let (ia, ib) = offset_inward(a, b, WALL);
    let plan = [a, b, ib, ia];
    let mut out = on_flight(part, &plan, GUARD, LEVEL);
    if part == Part::Landing
        && let Some(piece) = between(&plan, -APOTHEM, STOREY_LINE)
    {
        out.push_str(&prism(&piece, 0.0, FLOOR_TOP + GUARD, None, 0.0, 0.0));
    }
    out
}

/// One sconce, centred on a side wall at height `z`.
fn sconce(face: usize, z: f64) -> (String, String) {
    wall_fixture(face, 0.5, z, 20.0)
}

/// What a district stands in the cell, and the lights it hangs there in place of the
/// plain sconce, if it does.
fn dress(dressing: Dressing, part: Part) -> (String, Option<String>) {
    let top = part.top(dressing.enclosed());
    let light_x = part.light_x();
    let floor_at_light = part.height(light_x).unwrap_or(FLOOR_TOP);
    // A light or fixture this high above the floor, kept inside the cell.
    let hang = |rise: f64| (floor_at_light + rise).min(top - 12.0);
    // Halfway into the alcove, where the light is: it is shallower toward the ends.
    let into_alcove = (AISLE + APEX - light_x.abs() * 64.0 / APOTHEM) * 0.5;
    let mut out = String::new();
    let lights = match dressing {
        Dressing::Backrooms => None,
        Dressing::Library => {
            out.push_str("// Stacks across the alcoves, floor to ceiling\n");
            for north in [true, false] {
                let side = if north { 1.0 } else { -1.0 };
                for centre in [-48.0, 0.0, 48.0] {
                    let plan = clip_axis(&alcove(north), 0, centre - 5.0, true);
                    let plan = clip_axis(&plan, 0, centre + 5.0, false);
                    let plan = clip_axis(&plan, 1, side * (AISLE + 4.0), north);
                    if plan.len() >= 3 {
                        out.push_str(&prism(&plan, 0.0, top, None, 0.0, 0.0));
                    }
                }
            }
            None
        }
        Dressing::Lumen => {
            // Higher than a body can step (0.45 m): at bench height bodies stepped up
            // and wedged in the alcove's narrowing corner.
            out.push_str("// A lit plinth in each alcove, too high to step onto\n");
            let mut lights = String::new();
            for north in [true, false] {
                out.push_str(&on_flight(part, &alcove(north), PLINTH, top));
                let side = if north { 1.0 } else { -1.0 };
                lights.push_str(&tile_light(
                    light_x,
                    side * into_alcove,
                    hang(PLINTH + 24.0),
                ));
            }
            Some(lights)
        }
        Dressing::Zen => {
            out.push_str("// Paper screens along both edges, gaps narrower than a body\n");
            let mut lights = String::new();
            for north in [true, false] {
                let side = if north { 1.0 } else { -1.0 };
                let (y0, y1) = (side * AISLE, side * (AISLE + 3.0));
                for (x0, x1) in [
                    (-APOTHEM, -60.5),
                    (-54.5, -3.0),
                    (3.0, 54.5),
                    (60.5, APOTHEM),
                ] {
                    out.push_str(&on_flight(part, &rect(x0, x1, y0, y1), GUARD + 22.0, top));
                }
                lights.push_str(&tile_light(light_x, side * into_alcove, hang(36.0)));
            }
            Some(lights)
        }
        Dressing::Monument => {
            out.push_str("// Masonry filling the alcoves, and a pier at each seam\n");
            for north in [true, false] {
                out.push_str(&prism(&alcove(north), 0.0, top, None, 0.0, 0.0));
                let side = if north { 1.0 } else { -1.0 };
                let (y0, y1) = (side * (AISLE - 12.0), side * AISLE);
                for (x0, x1) in [(-APOTHEM, -96.0), (96.0, APOTHEM)] {
                    out.push_str(&prism(&rect(x0, x1, y0, y1), 0.0, top, None, 0.0, 0.0));
                }
            }
            let z = hang(48.0);
            let (fixture, light) = (
                prism(
                    &rect(light_x - 10.0, light_x + 10.0, AISLE - 6.0, AISLE),
                    z - 8.0,
                    z + 8.0,
                    None,
                    2.0,
                    0.0,
                ),
                tile_light(light_x, AISLE - 14.0, z),
            );
            out.push_str(&fixture);
            Some(light)
        }
        Dressing::Reactor => {
            out.push_str("// Columns in the alcoves, balustrades along the aisle\n");
            for north in [true, false] {
                let side = if north { 1.0 } else { -1.0 };
                for x in [-40.0, 40.0] {
                    out.push_str(&translate(
                        &pylon(10.0, 0.0, top, 0.0, 0.0, 0.0),
                        x,
                        side * 74.0,
                        0.0,
                    ));
                }
                let (y0, y1) = (side * AISLE, side * (AISLE + 2.0));
                out.push_str(&on_flight(
                    part,
                    &rect(-APOTHEM, APOTHEM, y0, y1),
                    GUARD,
                    top,
                ));
            }
            if part != Part::High {
                out.push_str("// A girder across the aisle, wall to wall\n");
                let reach = APEX - (light_x.abs() + 6.0) * 64.0 / APOTHEM;
                out.push_str(&prism(
                    &rect(light_x - 6.0, light_x + 6.0, -reach, reach),
                    CEILING - 14.0,
                    CEILING,
                    None,
                    0.0,
                    0.0,
                ));
            }
            None
        }
        // A lamp on the north parapet: anything hung over the aisle would float.
        Dressing::Sky => Some(tile_light(
            light_x,
            APEX - light_x.abs() * 64.0 / APOTHEM - 6.0,
            hang(GUARD + 6.0),
        )),
    };
    (out, lights)
}

/// The cell's sides: walls, or in the Sky parapets.
fn sides(dressing: Dressing, part: Part, door: Option<usize>, open: &[usize]) -> String {
    if dressing.enclosed() {
        return walls(door, open);
    }
    let mut out = String::new();
    for face in 0..6 {
        if open.contains(&face) {
            continue;
        }
        if Some(face) == door {
            out.push_str(&door_wall_default(face, 0.0, LEVEL));
        } else if SIDES.contains(&face) {
            out.push_str(&parapet(part, face));
        } else if part != Part::Landing {
            out.push_str(&wall(face, 0.0, LEVEL));
        }
    }
    out
}

/// The climb line through this cell, in its own frame.
fn spine(nodes: &[(f64, f64)]) -> String {
    let mut out = String::new();
    for (index, &(x, z)) in nodes.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        out.push_str(&stair_node(index as u16, x, 0.0, z));
    }
    out
}

/// What every district's cell of one part shares: the flight, the faces, the ports,
/// the spine, and where the plain sconce hangs.
struct Skeleton {
    brushes: String,
    door: Option<usize>,
    open: &'static [usize],
    ports: String,
    nodes: Vec<(f64, f64)>,
    sconce: (usize, f64),
    title: &'static str,
}

impl Skeleton {
    fn of(part: Part) -> Self {
        match part {
            Part::Foot => {
                let mut brushes = String::from("// The entry pad\n");
                brushes.push_str(&prism(
                    &clip(PAD_END, false),
                    0.0,
                    FLOOR_TOP,
                    None,
                    2.0,
                    0.0,
                ));
                brushes.push_str("// The flight, rising east\n");
                brushes.push_str(&flight(
                    &clip(PAD_END, true),
                    (PAD_END, FLOOR_TOP),
                    (APOTHEM, FOOT_TOP),
                    None,
                ));
                let mut ports = lateral_port(WEST, "door", "entry", 0, 0, 0);
                ports.push_str(&lateral_port(EAST, "span", "flight_on", 0, 0, 0));
                Self {
                    brushes,
                    door: Some(WEST),
                    open: &[EAST],
                    ports,
                    nodes: vec![
                        (-100.0, FLOOR_TOP),
                        (PAD_END, FLOOR_TOP),
                        (APOTHEM, FOOT_TOP),
                    ],
                    sconce: (1, 88.0),
                    title: "the foot: in from the west, and the flight begins",
                }
            }
            Part::Mid => {
                let mut brushes = String::from("// The flight, carried across\n");
                brushes.push_str(&flight(
                    &corners(),
                    (-APOTHEM, FOOT_TOP),
                    (APOTHEM, MID_TOP),
                    None,
                ));
                let mut ports = lateral_port(WEST, "span", "flight_back", 0, 0, 0);
                ports.push_str(&lateral_port(EAST, "span", "flight_on", 0, 0, 0));
                Self {
                    brushes,
                    door: None,
                    open: &[EAST, WEST],
                    ports,
                    nodes: vec![(-APOTHEM, FOOT_TOP), (APOTHEM, MID_TOP)],
                    sconce: (4, 100.0),
                    title: "the middle of the flight",
                }
            }
            Part::High => {
                let mut brushes = String::from(
                    "// The flight's steepest stretch, to the storey line, open above\n",
                );
                brushes.push_str(&flight(
                    &corners(),
                    (-APOTHEM, MID_TOP),
                    (ARRIVAL, NEXT_FLOOR),
                    Some(LEVEL),
                ));
                let mut ports = lateral_port(WEST, "span", "flight_back", 0, 0, 0);
                ports.push_str(&vertical_port("up", "ramp_open", "landing", 0));
                Self {
                    brushes,
                    door: None,
                    open: &[WEST],
                    ports,
                    nodes: vec![(-APOTHEM, MID_TOP), (STOREY_LINE, LEVEL)],
                    sconce: (2, 112.0),
                    title: "the flight's last stretch, open to the landing above",
                }
            }
            Part::Landing => {
                let mut brushes =
                    String::from("// The flight's last few centimetres, and the exit pad\n");
                brushes.push_str(&flight(
                    &clip(STOREY_LINE, true),
                    (STOREY_LINE, LIP),
                    (ARRIVAL, FLOOR_TOP),
                    Some(FLOOR_TOP),
                ));
                let mut ports = vertical_port("down", "ramp_open", "flight", 0);
                ports.push_str(&lateral_port(EAST, "door", "exit", 0, 0, 0));
                Self {
                    brushes,
                    door: Some(EAST),
                    open: &[],
                    ports,
                    nodes: vec![(STOREY_LINE, LIP), (ARRIVAL, FLOOR_TOP), (100.0, FLOOR_TOP)],
                    sconce: (2, 40.0),
                    title: "the landing: the flight arrives, and out to the east",
                }
            }
        }
    }
}

/// One cell of one district's climb: its file's stem and text.
fn cell(dressing: Dressing, part: Part) -> (String, String) {
    let stem = format!("{}{}", part.stem(), dressing.suffix());
    let Skeleton {
        brushes: skeleton,
        door,
        open,
        ports,
        nodes,
        sconce: sconce_at,
        title,
    } = Skeleton::of(part);
    let mut brushes = skeleton;
    if dressing.enclosed() && part != Part::High {
        brushes.push_str(&hex_slab(CEILING, LEVEL, 0.0, 3.0));
    }
    brushes.push_str(&sides(dressing, part, door, open));
    let (dressed, lights) = dress(dressing, part);
    brushes.push_str(&dressed);
    let (fixture, source) = match lights {
        Some(lights) => (String::new(), lights),
        None => sconce(sconce_at.0, sconce_at.1),
    };
    let mut out = format!("// Climb composition, {title}{}.\n", dressing.title());
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&(brushes + &fixture)));
    out.push_str(
        &Meta::cell(
            &format!("authored/{stem}"),
            part.stem(),
            dressing.variant(),
            1,
            10,
        )
        .with_register_scope(dressing.registers())
        .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 1, "flight"));
    out.push_str(&ports);
    out.push_str(&spine(&nodes));
    out.push_str(&source);
    (stem, out)
}

/// Every district's four cells, as `(stem, text)`.
#[must_use]
pub fn builders() -> Vec<(String, String)> {
    Dressing::ALL
        .into_iter()
        .flat_map(|dressing| Part::ALL.map(|part| cell(dressing, part)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::FLIGHT_MAX_SLOPE;

    #[test]
    fn every_climb_cell_reproduces_its_committed_file() {
        for (stem, produced) in builders() {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/tiles/authored")
                .join(format!("{stem}.map"));
            let committed = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
                .replace("\r\n", "\n");
            assert!(committed == produced, "{stem} differs: run tilec gen-tiles");
        }
    }

    /// Every cell of every district survives the importer it is written for.
    #[test]
    fn every_climb_cell_parses_and_validates() {
        for (stem, text) in builders() {
            crate::parse_authored_module(&text)
                .unwrap_or_else(|error| panic!("{stem} does not validate: {error:?}"));
        }
    }

    /// Every register has exactly one climb: two for one register would let a draw
    /// mix a district's cells with another's.
    #[test]
    fn every_register_has_one_climb() {
        for &register in crate::tile_source::REGISTERS {
            let owners = Dressing::ALL
                .iter()
                .filter(|dressing| dressing.registers().split(',').any(|slug| slug == register))
                .count();
            assert_eq!(owners, 1, "{register:?} has {owners} climbs");
        }
    }

    /// Nothing a district stands in a cell reaches into the aisle a body climbs, but
    /// what hangs overhead, a Monument pier, which stands only at a seam, and the Sky's
    /// parapets, which are its walls.
    #[test]
    fn the_aisle_is_clear_in_every_district() {
        let hulls = |dressing: Dressing, part: Part| {
            crate::parse_authored_module(&cell(dressing, part).1)
                .expect("a climb cell validates")
                .prototype
                .hulls
        };
        for dressing in Dressing::ALL {
            if matches!(
                dressing,
                Dressing::Backrooms | Dressing::Monument | Dressing::Sky
            ) {
                continue;
            }
            for part in Part::ALL {
                let plain = hulls(Dressing::Backrooms, part);
                let same = |a: &[glam::Vec3], b: &[glam::Vec3]| {
                    a.len() == b.len() && a.iter().all(|p| b.iter().any(|q| p.distance(*q) < 1e-3))
                };
                for hull in hulls(dressing, part)
                    .iter()
                    .filter(|hull| !plain.iter().any(|other| same(hull, other)))
                {
                    for point in hull {
                        // Metres, the importer's frame: plan y is the negated world z.
                        let (x, y, z) = (point.x * 16.0, -point.z * 16.0, point.y * 16.0);
                        let floor = f64::from(x).clamp(-APOTHEM, APOTHEM);
                        let overhead = part
                            .height(floor)
                            .is_none_or(|floor| f64::from(z) > floor + 1.9 * 16.0);
                        assert!(
                            f64::from(y).abs() >= AISLE - 0.01 || overhead || z <= 0.01,
                            "{dressing:?} {part:?}: ({x:.1}, {y:.1}, {z:.1}) stands in the aisle"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_flight_rises_a_storey_across_the_composition() {
        assert!((MID_TOP + HIGH_RISE - NEXT_FLOOR).abs() < 1e-9);
        assert!((NEXT_FLOOR - FLOOR_TOP - LEVEL).abs() < 1e-9);
    }

    #[test]
    fn no_stretch_of_the_flight_is_too_steep() {
        let low = LOW_RISE / LOW_RUN;
        let high = HIGH_RISE / HIGH_RUN;
        let limit = f64::from(FLIGHT_MAX_SLOPE);
        assert!(
            low <= limit && high <= limit,
            "low {low:.3}, high {high:.3}"
        );
    }

    /// The low cells keep the ordinary ceiling, so the flight under it must leave a
    /// body's head clear: 1.8 m tall, with at least 0.5 m to spare.
    #[test]
    fn a_body_clears_the_ceiling_wherever_there_is_one() {
        let body = 1.8 * 16.0;
        let spare = CEILING - (MID_TOP + body);
        assert!(spare >= 0.5 * 16.0, "{:.2} m to spare", spare / 16.0);
    }

    /// The alcove's corners lie on the diagonal walls' inner faces.
    #[test]
    fn an_alcove_meets_the_walls() {
        let (a, b) = edge(5);
        let (ia, ib) = offset_inward(a, b, WALL);
        for (x, y) in [(ALCOVE_END, AISLE), (0.0, APEX)] {
            let cross = (ib.0 - ia.0) * (y - ia.1) - (ib.1 - ia.1) * (x - ia.0);
            let length = (ib.0 - ia.0).hypot(ib.1 - ia.1);
            assert!((cross / length).abs() < 0.05, "({x}, {y}) is off the wall");
        }
    }

    /// Checked at compile time: the storey line falls inside the cell, before the pad.
    const _: () = assert!(STOREY_LINE > -APOTHEM && STOREY_LINE < ARRIVAL);
    /// Checked at compile time: the step from flight to landing is too small to notice.
    const _: () = assert!(LIP < 2.0, "a lip a body would notice");
}
