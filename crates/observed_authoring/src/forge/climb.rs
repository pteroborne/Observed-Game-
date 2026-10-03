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
    face_mid, flat_plane, hex_slab, offset_inward, prism, pylon, regular_polygon, side_plane,
    translate, wall,
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
/// Steps in a turning mid cell's flight: nine risers of 0.27 m, the first up from
/// the foot's flight and the last up to the high cell's.
const TREADS: usize = 8;
/// The post a sharp turn's steps wind round, at the corner they share: the treads
/// narrow to nothing there.
const NEWEL: f64 = 20.0;
/// How far the west pad of a landing left back over its flight reaches: far enough
/// east to meet the gallery where the alcove is wide enough to walk, not so far that
/// the flight below loses its headroom.
const WEST_PAD: f64 = -64.0;

/// A turn from the climb's heading, as faces counted toward its right, the way the
/// forge numbers them: the facility's `ClimbTurn`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Turn {
    Ahead,
    Right,
    SharpRight,
    Back,
    SharpLeft,
    Left,
}

impl Turn {
    /// The face this turn names from a climb heading east.
    const fn face(self) -> usize {
        self as usize
    }

    const fn word(self) -> &'static str {
        match self {
            Self::Ahead => "",
            Self::Right => "_right",
            Self::SharpRight => "_sharp_right",
            Self::Back => "_back",
            Self::SharpLeft => "_sharp_left",
            Self::Left => "_left",
        }
    }

    const fn phrase(self) -> &'static str {
        match self {
            Self::Ahead => "straight on",
            Self::Right => "to the right",
            Self::SharpRight => "sharply right",
            Self::Back => "back over the flight",
            Self::SharpLeft => "sharply left",
            Self::Left => "to the left",
        }
    }
}

/// Which cell of the composition a builder is making. The mid cell may turn the
/// flight, and the landing may be left by a face other than the one ahead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Foot,
    Mid(Turn),
    High,
    Landing(Turn),
}

impl Part {
    const ALL: [Self; 11] = [
        Self::Foot,
        Self::Mid(Turn::Ahead),
        Self::High,
        Self::Landing(Turn::Ahead),
        Self::Mid(Turn::Right),
        Self::Mid(Turn::SharpRight),
        Self::Mid(Turn::SharpLeft),
        Self::Mid(Turn::Left),
        Self::Landing(Turn::Right),
        Self::Landing(Turn::Left),
        Self::Landing(Turn::Back),
    ];

    fn stem(self) -> String {
        match self {
            Self::Foot => "climb_foot".to_owned(),
            Self::Mid(turn) => format!("climb_mid{}", turn.word()),
            Self::High => "climb_high".to_owned(),
            Self::Landing(exit) => format!("climb_landing{}", exit.word()),
        }
    }

    /// The bend a turning mid cell's flight takes, if this is one.
    fn bend(self) -> Option<Bend> {
        match self {
            Self::Mid(turn) if turn != Turn::Ahead => Some(Bend::of(turn)),
            _ => None,
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
            Self::Mid(_) => vec![(-APOTHEM, FOOT_TOP, APOTHEM, MID_TOP)],
            Self::High => vec![
                (-APOTHEM, MID_TOP, STOREY_LINE, LEVEL),
                (STOREY_LINE, LEVEL, APOTHEM, LEVEL),
            ],
            Self::Landing(_) => vec![
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
            Self::Foot | Self::Mid(_) => 0.0,
            Self::High => -40.0,
            Self::Landing(_) => 80.0,
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

/// `poly` clipped to the side of the line through `origin` along `along` that
/// `keep_left` names: left is counterclockwise in plan.
fn clip_line(poly: &[P2], origin: P2, along: P2, keep_left: bool) -> Vec<P2> {
    let side = |p: P2| {
        let cross = along.0 * (p.1 - origin.1) - along.1 * (p.0 - origin.0);
        if keep_left { cross } else { -cross }
    };
    let mut out = Vec::new();
    for index in 0..poly.len() {
        let (a, b) = (poly[index], poly[(index + 1) % poly.len()]);
        let (sa, sb) = (side(a), side(b));
        if sa >= 0.0 {
            out.push(a);
        }
        if (sa >= 0.0) != (sb >= 0.0) {
            let t = sa / (sa - sb);
            out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
        }
    }
    out
}

/// `poly` clipped to the cell's hexagon.
fn in_hex(poly: &[P2]) -> Vec<P2> {
    let corners = corners();
    let mut out = poly.to_vec();
    for index in 0..6 {
        let (a, b) = (corners[index], corners[(index + 1) % 6]);
        let along = (b.0 - a.0, b.1 - a.1);
        // Keep the side the cell's centre is on.
        let centre_left = along.0 * -a.1 - along.1 * -a.0 > 0.0;
        out = clip_line(&out, a, along, centre_left);
        if out.len() < 3 {
            return Vec::new();
        }
    }
    out
}

/// Where the lines along two of the hexagon's edges meet.
fn meet((a, b): (P2, P2), (c, d): (P2, P2)) -> P2 {
    let (r, s) = ((b.0 - a.0, b.1 - a.1), (d.0 - c.0, d.1 - c.1));
    let t = ((c.0 - a.0) * s.1 - (c.1 - a.1) * s.0) / (r.0 * s.1 - r.1 * s.0);
    (a.0 + r.0 * t, a.1 + r.1 * t)
}

/// A turning mid cell's flight, entered by the west face and left by `exit`.
///
/// The west face and the exit face lie on two rays from the point their lines meet:
/// the corner they share for a sharp turn, a point outside the cell for a gentle one.
/// Steps fanned about that point have every edge on such a ray, so the first meets
/// the foot's flight along the whole of the west face and the last meets the high
/// cell's along the whole of the exit face - which no warped slope made of flat
/// brushes can. The climb line follows the pitch of the steps on the arc through
/// both faces' midpoints, where the flights either side carry theirs.
struct Bend {
    exit: usize,
    centre: P2,
    /// The angle of the west face's midpoint about the centre, and how far round,
    /// signed, the exit face's lies from it.
    from: f64,
    sweep: f64,
    /// The arc's radius at the west face and at the exit face.
    radii: (f64, f64),
}

impl Bend {
    fn of(turn: Turn) -> Self {
        let exit = turn.face();
        let centre = meet(edge(WEST), edge(exit));
        let polar = |p: P2| {
            let d = (p.0 - centre.0, p.1 - centre.1);
            (d.1.atan2(d.0), d.0.hypot(d.1))
        };
        let (from, entry_radius) = polar(face_mid(WEST));
        let (to, exit_radius) = polar(face_mid(exit));
        let mut sweep = to - from;
        if sweep > std::f64::consts::PI {
            sweep -= std::f64::consts::TAU;
        } else if sweep <= -std::f64::consts::PI {
            sweep += std::f64::consts::TAU;
        }
        Self {
            exit,
            centre,
            from,
            sweep,
            radii: (entry_radius, exit_radius),
        }
    }

    /// Whether the turn is sharp: the steps wind round a corner of the cell.
    fn sharp(&self) -> bool {
        self.radii.0 < APOTHEM
    }

    /// The direction from the centre at fraction `t` of the way round.
    fn ray(&self, t: f64) -> P2 {
        let angle = self.from + self.sweep * t;
        (angle.cos(), angle.sin())
    }

    /// The point on the climb line at fraction `t` of the way round.
    fn on_arc(&self, t: f64) -> P2 {
        let radius = self.radii.0 + (self.radii.1 - self.radii.0) * t;
        let ray = self.ray(t);
        (
            self.centre.0 + ray.0 * radius,
            self.centre.1 + ray.1 * radius,
        )
    }

    /// `plan` clipped to step `index`'s wedge.
    fn wedge(&self, plan: &[P2], index: usize) -> Vec<P2> {
        #[allow(clippy::cast_precision_loss)]
        let (a, b) = (
            index as f64 / TREADS as f64,
            (index + 1) as f64 / TREADS as f64,
        );
        // Counterclockwise from ray `a` to ray `b` when the sweep is positive.
        let left = self.sweep > 0.0;
        let out = clip_line(plan, self.centre, self.ray(a), left);
        clip_line(&out, self.centre, self.ray(b), !left)
    }

    /// Step `index`'s height: risers of one equal height from the foot's flight to
    /// the high cell's.
    fn tread(index: usize) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let t = (index + 1) as f64 / (TREADS + 1) as f64;
        FOOT_TOP + (MID_TOP - FOOT_TOP) * t
    }

    /// The step under plan point `p`.
    fn step_at(&self, p: P2) -> usize {
        let angle = (p.1 - self.centre.1).atan2(p.0 - self.centre.0);
        let mut round = angle - self.from;
        if round > std::f64::consts::PI {
            round -= std::f64::consts::TAU;
        } else if round <= -std::f64::consts::PI {
            round += std::f64::consts::TAU;
        }
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let index = ((round / self.sweep) * TREADS as f64).floor().max(0.0) as usize;
        index.min(TREADS - 1)
    }

    /// The climb line: in at the west face, a node on the pitch of the steps at
    /// each one, and out at the exit face.
    fn nodes(&self) -> Vec<P3> {
        let mut nodes = Vec::new();
        for index in 0..=TREADS + 1 {
            #[allow(clippy::cast_precision_loss)]
            let t = match index {
                0 => 0.0,
                _ if index == TREADS + 1 => 1.0,
                _ => (index as f64 - 0.5) / TREADS as f64,
            };
            let (x, y) = self.on_arc(t);
            nodes.push((x, y, FOOT_TOP + (MID_TOP - FOOT_TOP) * t));
        }
        nodes
    }

    /// The faces the flight does not use, outermost first: walls round the outside
    /// of the turn, farthest from where it turns.
    fn outer_faces(&self) -> Vec<usize> {
        let mut faces: Vec<usize> = (0..6).filter(|&f| f != WEST && f != self.exit).collect();
        let distance = |f: usize| {
            let m = face_mid(f);
            (m.0 - self.centre.0).hypot(m.1 - self.centre.1)
        };
        faces.sort_by(|&a, &b| distance(b).total_cmp(&distance(a)));
        faces
    }
}

/// A mass over `plan` from the cell's base to `rise` above whichever step it stands
/// on, one brush for each, never above `top`.
fn on_bend(bend: &Bend, plan: &[P2], rise: f64, top: f64) -> String {
    let mut out = String::new();
    for index in 0..TREADS {
        let piece = bend.wedge(plan, index);
        if piece.len() < 3 {
            continue;
        }
        out.push_str(&prism(
            &piece,
            0.0,
            (Bend::tread(index) + rise).min(top),
            None,
            0.0,
            0.0,
        ));
    }
    out
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
    if let Some(bend) = part.bend() {
        return on_bend(&bend, plan, rise, top);
    }
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
    if matches!(part, Part::Landing(_))
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
    if let Some(bend) = part.bend() {
        return dress_bend(dressing, &bend);
    }
    // A turned landing's gallery takes one alcove: only the other is dressed.
    let alcoves: &[bool] = match part {
        Part::Landing(Turn::Right) => &[true],
        Part::Landing(exit) if exit != Turn::Ahead => &[false],
        _ => &[true, false],
    };
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
            for &north in alcoves {
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
            for &north in alcoves {
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
            for &north in alcoves {
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
            for &north in alcoves {
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
            for &north in alcoves {
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

/// What a district stands round a turning flight, and the lights it hangs there in
/// place of the plain sconce. A turning flight has no alcoves, so a district dresses
/// the two walls farthest round the outside of the turn, well clear of the climb
/// line, and the Monument a sharp turn's newel as well.
fn dress_bend(dressing: Dressing, bend: &Bend) -> (String, Option<String>) {
    let top = if dressing.enclosed() { CEILING } else { LEVEL };
    let outer = bend.outer_faces();
    let walls = &outer[..2];
    // A band along a wall's inner face, from `near` to `far` in front of it, kept
    // clear of the corners.
    let band = |face: usize, near: f64, far: f64| {
        let (a, b) = edge(face);
        let (ia, ib) = offset_inward(a, b, WALL + near);
        let (ja, jb) = offset_inward(a, b, WALL + far);
        let at = |p: P2, q: P2, t: f64| (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t);
        vec![
            at(ia, ib, 0.15),
            at(ia, ib, 0.85),
            at(ja, jb, 0.85),
            at(ja, jb, 0.15),
        ]
    };
    // A light `out` in front of a wall's middle, `rise` above the step there.
    let light_by = |face: usize, out: f64, rise: f64| {
        let (mx, my) = face_mid(face);
        let length = mx.hypot(my);
        let reach = WALL + out;
        let p = (mx - mx / length * reach, my - my / length * reach);
        let z = (Bend::tread(bend.step_at(p)) + rise).min(top - 12.0);
        tile_light(p.0, p.1, z)
    };
    let mut out = String::new();
    let lights = match dressing {
        Dressing::Backrooms => None,
        Dressing::Library => {
            out.push_str("// Stacks along the walls round the outside of the turn\n");
            let mut lights = String::new();
            for &face in walls {
                out.push_str(&prism(&band(face, 0.0, 10.0), 0.0, top, None, 0.0, 0.0));
                lights.push_str(&light_by(face, 22.0, 56.0));
            }
            Some(lights)
        }
        Dressing::Lumen => {
            out.push_str("// Lit plinths along the outside of the turn, too high to step onto\n");
            let mut lights = String::new();
            for &face in walls {
                out.push_str(&on_bend(bend, &band(face, 0.0, 18.0), PLINTH, top));
                lights.push_str(&light_by(face, 9.0, PLINTH + 24.0));
            }
            Some(lights)
        }
        Dressing::Zen => {
            out.push_str("// Paper screens along the outside of the turn, lanterns behind\n");
            let mut lights = String::new();
            for &face in walls {
                out.push_str(&on_bend(bend, &band(face, 16.0, 19.0), GUARD + 22.0, top));
                lights.push_str(&light_by(face, 8.0, 36.0));
            }
            Some(lights)
        }
        Dressing::Monument => {
            out.push_str("// Masonry along the outside of the turn\n");
            let mut lights = String::new();
            for &face in walls {
                out.push_str(&prism(&band(face, 0.0, 16.0), 0.0, top, None, 0.0, 0.0));
                lights.push_str(&light_by(face, 22.0, 48.0));
            }
            if bend.sharp() {
                out.push_str("// The newel cased in masonry\n");
                let post: Vec<P2> = regular_polygon(NEWEL + 10.0, 8, 22.5)
                    .into_iter()
                    .map(|(x, y)| (x + bend.centre.0, y + bend.centre.1))
                    .collect();
                out.push_str(&prism(&in_hex(&post), 0.0, top, None, 0.0, 0.0));
            }
            Some(lights)
        }
        Dressing::Reactor => {
            out.push_str("// Columns along the outside of the turn\n");
            for &face in walls {
                let (a, b) = edge(face);
                let (ia, ib) = offset_inward(a, b, WALL + 18.0);
                for t in [0.3, 0.7] {
                    let (x, y) = (ia.0 + (ib.0 - ia.0) * t, ia.1 + (ib.1 - ia.1) * t);
                    out.push_str(&translate(&pylon(10.0, 0.0, top, 0.0, 0.0, 0.0), x, y, 0.0));
                }
            }
            None
        }
        // A lamp on the parapet farthest round the outside.
        Dressing::Sky => Some(light_by(outer[0], 6.0, GUARD + 6.0)),
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
        // A turned landing's pad and gallery reach the east and west faces too.
        let guarded = SIDES.contains(&face)
            || matches!(part, Part::Landing(exit) if exit != Turn::Ahead)
            || part.bend().is_some();
        if Some(face) == door {
            out.push_str(&door_wall_default(face, 0.0, LEVEL));
        } else if guarded {
            out.push_str(&parapet(part, face));
        } else if !matches!(part, Part::Landing(_)) {
            out.push_str(&wall(face, 0.0, LEVEL));
        }
    }
    out
}

/// The climb line through this cell, in its own frame.
fn spine(nodes: &[P3]) -> String {
    let mut out = String::new();
    for (index, &(x, y, z)) in nodes.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        out.push_str(&stair_node(index as u16, x, y, z));
    }
    out
}

/// What every district's cell of one part shares: the flight, the faces, the ports,
/// the spine, and where the plain sconce hangs.
struct Skeleton {
    brushes: String,
    door: Option<usize>,
    open: Vec<usize>,
    ports: String,
    nodes: Vec<P3>,
    sconce: (usize, f64),
    title: String,
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
                    open: vec![EAST],
                    ports,
                    nodes: vec![
                        (-100.0, 0.0, FLOOR_TOP),
                        (PAD_END, 0.0, FLOOR_TOP),
                        (APOTHEM, 0.0, FOOT_TOP),
                    ],
                    sconce: (1, 88.0),
                    title: "the foot: in from the west, and the flight begins".to_owned(),
                }
            }
            Part::Mid(turn) if turn != Turn::Ahead => Self::bend(&Bend::of(turn), turn),
            Part::Mid(_) => {
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
                    open: vec![EAST, WEST],
                    ports,
                    nodes: vec![(-APOTHEM, 0.0, FOOT_TOP), (APOTHEM, 0.0, MID_TOP)],
                    sconce: (4, 100.0),
                    title: "the middle of the flight".to_owned(),
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
                    open: vec![WEST],
                    ports,
                    nodes: vec![(-APOTHEM, 0.0, MID_TOP), (STOREY_LINE, 0.0, LEVEL)],
                    sconce: (2, 112.0),
                    title: "the flight's last stretch, open to the landing above".to_owned(),
                }
            }
            Part::Landing(exit) if exit != Turn::Ahead => Self::turned_landing(exit),
            Part::Landing(_) => {
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
                    open: Vec::new(),
                    ports,
                    nodes: vec![
                        (STOREY_LINE, 0.0, LIP),
                        (ARRIVAL, 0.0, FLOOR_TOP),
                        (100.0, 0.0, FLOOR_TOP),
                    ],
                    sconce: (2, 40.0),
                    title: "the landing: the flight arrives, and out to the east".to_owned(),
                }
            }
        }
    }

    /// A mid cell whose flight turns: steps fanned about the turn, wound round a
    /// newel where the turn is sharp.
    fn bend(bend: &Bend, turn: Turn) -> Self {
        let mut brushes = format!(
            "// The flight, turning {}: steps fanned round the turn\n",
            turn.phrase()
        );
        brushes.push_str(&on_bend(bend, &corners(), 0.0, LEVEL));
        if bend.sharp() {
            brushes.push_str("// The newel the steps wind round\n");
            let post: Vec<P2> = regular_polygon(NEWEL, 8, 22.5)
                .into_iter()
                .map(|(x, y)| (x + bend.centre.0, y + bend.centre.1))
                .collect();
            brushes.push_str(&prism(&in_hex(&post), 0.0, LEVEL, None, 0.0, 0.0));
        }
        let mut ports = lateral_port(WEST, "span", "flight_back", 0, 0, 0);
        ports.push_str(&lateral_port(bend.exit, "span", "flight_on", 0, 0, 0));
        // The plain sconce on the wall farthest round the outside of the turn, a
        // body's height above the step beneath it.
        let face = bend.outer_faces()[0];
        let step = bend.step_at(face_mid(face));
        Self {
            brushes,
            door: None,
            open: vec![WEST, bend.exit],
            ports,
            nodes: bend.nodes(),
            sconce: (face, (Bend::tread(step) + 40.0).min(CEILING - 12.0)),
            title: format!("the middle of the flight, turning {}", turn.phrase()),
        }
    }

    /// A landing left by a side face or back over the flight. The flight arrives on
    /// the east pad as on every landing; a gallery round the alcove on the exit's side
    /// carries the floor to the door, railed where it overlooks the flight, and a
    /// landing left back over the flight has a pad of its own before the west door.
    fn turned_landing(exit: Turn) -> Self {
        let face = exit.face();
        // The gallery runs on the exit's side; back over the flight it runs left.
        let north = !matches!(exit, Turn::Right);
        let side = if north { 1.0 } else { -1.0 };
        let mut brushes =
            String::from("// The flight's last few centimetres, and the arrival pad\n");
        brushes.push_str(&flight(
            &clip(STOREY_LINE, true),
            (STOREY_LINE, LIP),
            (ARRIVAL, FLOOR_TOP),
            Some(FLOOR_TOP),
        ));
        brushes.push_str("// The gallery round the alcove\n");
        brushes.push_str(&prism(
            &clip_axis(&corners(), 1, side * AISLE, north),
            0.0,
            FLOOR_TOP,
            None,
            0.0,
            0.0,
        ));
        // Where the gallery overlooks the flight, from the west pad or the west wall
        // to the storey line, a rail.
        let gallery_from = if exit == Turn::Back {
            WEST_PAD
        } else {
            -APOTHEM
        };
        let rail_band = clip_axis(
            &clip_axis(&corners(), 1, side * AISLE, north),
            1,
            side * (AISLE + 2.0),
            !north,
        );
        brushes.push_str("// A rail along the gallery's edge\n");
        if let Some(rail) = between(&rail_band, gallery_from, STOREY_LINE) {
            brushes.push_str(&prism(&rail, 0.0, FLOOR_TOP + GUARD, None, 0.0, 0.0));
        }
        let mut nodes = vec![(STOREY_LINE, 0.0, LIP), (ARRIVAL, 0.0, FLOOR_TOP)];
        if exit == Turn::Back {
            brushes.push_str("// The west pad, before the door, railed over the flight\n");
            brushes.push_str(&prism(
                &clip(WEST_PAD, false),
                0.0,
                FLOOR_TOP,
                None,
                0.0,
                0.0,
            ));
            let edge_band = clip_axis(&clip(WEST_PAD, true), 0, WEST_PAD + 2.0, false);
            let edge_band = clip_axis(&edge_band, 1, side * (AISLE + 2.0), !north);
            brushes.push_str(&prism(&edge_band, 0.0, FLOOR_TOP + GUARD, None, 0.0, 0.0));
            nodes.extend([
                (ARRIVAL, side * 66.0, FLOOR_TOP),
                (0.0, side * 74.0, FLOOR_TOP),
                (-72.0, side * 66.0, FLOOR_TOP),
                (-88.0, side * 36.0, FLOOR_TOP),
                (-100.0, 0.0, FLOOR_TOP),
            ]);
        } else {
            // In front of the door, a body's half-width inside the wall.
            let (mx, my) = face_mid(face);
            let length = mx.hypot(my);
            let inset = 14.0;
            nodes.extend([
                (84.0, side * 62.0, FLOOR_TOP),
                (
                    mx - mx / length * inset,
                    my - my / length * inset,
                    FLOOR_TOP,
                ),
            ]);
        }
        let mut ports = vertical_port("down", "ramp_open", "flight", 0);
        ports.push_str(&lateral_port(face, "door", "exit", 0, 0, 0));
        // The plain sconce on the gallery's own wall.
        let sconce_face = if north { 4 } else { 2 };
        Self {
            brushes,
            door: Some(face),
            open: Vec::new(),
            ports,
            nodes,
            sconce: (sconce_face, 40.0),
            title: format!("the landing: the flight arrives, and out {}", exit.phrase()),
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
    brushes.push_str(&sides(dressing, part, door, &open));
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
            &part.stem(),
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

/// Every district's cells of every shape, as `(stem, text)`.
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
            for part in Part::ALL.into_iter().filter(|part| part.bend().is_none()) {
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

    /// A turning flight has no aisle: what a district stands round it keeps clear of a
    /// lane 3 m wide along its climb line, in every district, the Monument's newel
    /// casing (2.1 m from the line round a sharp turn) and the Sky's parapets too. Each hull's outline is sampled along every
    /// pair of its corners, so a wall whose ends lie outside the lane cannot cut across
    /// it between them.
    #[test]
    fn the_climb_line_is_clear_round_every_turn() {
        const LANE: f64 = 24.0;
        let hulls = |dressing: Dressing, part: Part| {
            crate::parse_authored_module(&cell(dressing, part).1)
                .expect("a climb cell validates")
                .prototype
                .hulls
        };
        let same = |a: &[glam::Vec3], b: &[glam::Vec3]| {
            a.len() == b.len() && a.iter().all(|p| b.iter().any(|q| p.distance(*q) < 1e-3))
        };
        for part in Part::ALL {
            let Some(bend) = part.bend() else {
                continue;
            };
            let plain = hulls(Dressing::Backrooms, part);
            // Where along the turn, and how far off the climb line, a plan point lies.
            let off_line = |x: f64, y: f64| {
                let radius = (x - bend.centre.0).hypot(y - bend.centre.1);
                let mut round = (y - bend.centre.1).atan2(x - bend.centre.0) - bend.from;
                if round > std::f64::consts::PI {
                    round -= std::f64::consts::TAU;
                } else if round <= -std::f64::consts::PI {
                    round += std::f64::consts::TAU;
                }
                let t = (round / bend.sweep).clamp(0.0, 1.0);
                (radius - (bend.radii.0 + (bend.radii.1 - bend.radii.0) * t)).abs()
            };
            for dressing in Dressing::ALL {
                for hull in hulls(dressing, part)
                    .iter()
                    .filter(|hull| !plain.iter().any(|other| same(hull, other)))
                {
                    // Metres, the importer's frame: plan y is the negated world z.
                    let plan: Vec<(f64, f64, f64)> = hull
                        .iter()
                        .map(|p| {
                            (
                                f64::from(p.x) * 16.0,
                                f64::from(-p.z) * 16.0,
                                f64::from(p.y) * 16.0,
                            )
                        })
                        .collect();
                    // Low enough to be floor: a plinth or screen base under the steps.
                    let low = plan.iter().all(|p| p.2 <= Bend::tread(0) + 0.01);
                    if low {
                        continue;
                    }
                    for a in &plan {
                        for b in &plan {
                            for step in 0..=10 {
                                let t = f64::from(step) / 10.0;
                                let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                                assert!(
                                    off_line(x, y) >= LANE,
                                    "{dressing:?} {part:?}: ({x:.1}, {y:.1}) stands on the climb line"
                                );
                            }
                        }
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
