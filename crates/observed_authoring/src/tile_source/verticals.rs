//! Vertical circulation: grounded two-level ramp prefabs in all six exit directions.

use observed_hex::HexFace;

use super::geometry::{
    DOOR_TOP, FLOOR_TOP, box_brush, door_floor_apron, door_wall, general_prism_brush,
    hex_slab_brush, level_units, sloped_deck_brush, sloped_slab_brush, tb_edge, tile_deck_node,
    tile_light, tile_meta, tile_port, tile_stair_node, wall_brush, worldspawn,
};
use super::{face_name, register_style};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StairVertical {
    UpOnly,
    DownOnly,
    Through,
}

impl StairVertical {
    fn label(self) -> &'static str {
        match self {
            Self::UpOnly => "up",
            Self::DownOnly => "down",
            Self::Through => "through",
        }
    }
}

/// Two-level ramp rising from the door on `exit_face.opposite()` to the
/// upper-level doorway on `exit_face`. Explicit prefab per direction — the
/// frozen schema has no tile rotation. Variant = exit face index.
pub fn ramp_map(register: &str, exit_face: HexFace) -> String {
    let style = register_style(register);
    let h = level_units();
    let top = 2.0 * h;
    let entrance_face = exit_face.opposite();
    let mut brushes = sloped_slab_brush(entrance_face, exit_face, FLOOR_TOP, h);
    for face in HexFace::LATERAL {
        if face == entrance_face {
            brushes += &door_wall(face, 0.0, top, FLOOR_TOP, DOOR_TOP, style.trim_height);
        } else if face == exit_face {
            brushes += &door_wall(
                face,
                0.0,
                top,
                FLOOR_TOP + h,
                DOOR_TOP + h,
                style.trim_height,
            );
        } else {
            brushes += &wall_brush(face, 0.0, top);
        }
    }
    brushes += &hex_slab_brush(top - FLOOR_TOP, top);
    let mut out = format!("// Ramp exit {}.\n", face_name(exit_face));
    out += &worldspawn(&brushes);
    out += &tile_meta("ramp", register, exit_face.index() as u16, 2);
    out += &tile_port(face_name(entrance_face), "door");
    out += &tile_port("up", "ramp_open");
    for (face, z) in [(entrance_face, 64.0), (exit_face, h + 64.0)] {
        let (a, b) = tb_edge(face);
        out += &tile_light((a[0] + b[0]) * 0.25, (a[1] + b[1]) * 0.25, z);
    }
    out
}

/// Which way a tower turns, and so which side its stairwell opening falls on.
///
/// The quantized hexagon is symmetric about `x = 0`, so a tower can be
/// reflected exactly — brushes, climb, and floor path together. A handed pair is
/// not cosmetic: you turn the other way, the well is on the other side, and the
/// opening you climb through faces north-east instead of north-west. That is
/// what a player reads when two districts stack towers differently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TowerHand {
    /// Turn landing east, stairwell opening north-west.
    Left,
    /// The mirror image: turn landing west, opening north-east.
    Right,
}

impl TowerHand {
    /// Reflect an x coordinate. Applied to every input and never to the output
    /// text, so there is one geometry and two readings of it.
    fn x(self, x: f64) -> f64 {
        match self {
            Self::Left => x,
            Self::Right => -x,
        }
    }

    /// A reflected pair, re-sorted. `box_brush` needs min before max, and
    /// reflection swaps them.
    fn pair(self, min: f64, max: f64) -> (f64, f64) {
        let (a, b) = (self.x(min), self.x(max));
        if a <= b { (a, b) } else { (b, a) }
    }
}

/// A physically continuous switchback stair, with the walkable line through it
/// and the floor path around it.
///
/// Thin flights preserve headroom when cells stack; floor piers and
/// wall-connected brackets visibly support every span. The opening beyond the
/// upper flight is kept clear so the flight below can emerge into this cell
/// instead of meeting the underside of another wedge.
///
/// The spine and deck are derived from the same flight constants as the
/// brushes, which is the point: the bot follows those lines, so they cannot
/// fall out of step with the surface the way a hand-written copy did.
fn supported_switchback(hand: TowerHand) -> (String, Vec<[f64; 3]>, Vec<[f64; 3]>) {
    let mx = |x: f64| hand.x(x);
    let low_flight = [
        [mx(-72.0), -52.0],
        [mx(60.0), -52.0],
        [mx(60.0), -16.0],
        [mx(-72.0), -16.0],
    ];
    let high_flight = [
        [mx(-80.0), 16.0],
        [mx(60.0), 16.0],
        [mx(60.0), 52.0],
        [mx(-80.0), 52.0],
    ];
    let mut out = String::new();

    // Flush with the deck of the cell above (`level_units() + FLOOR_TOP`). It
    // used to top out at 144 — half a metre proud — which read as a lip at every
    // level junction and, because autostep only lifts `step_height` (0.45 m),
    // stopped a body stepping from that deck back down onto the flight. The fix
    // was blocked for an arc on the bot's hardcoded rise thresholds, which broke
    // whenever these numbers moved; the bot now reads the spine below instead.
    let climb_top = level_units() + FLOOR_TOP;
    // The low flight starts buried in the grounded deck and rises through it.
    let climb_base = 4.0;
    // Runs are measured along x, the only axis either flight's height varies on.
    let low_run = 132.0;
    let landing = 64.0;
    // Flight undersides, so the supports below can be derived rather than
    // hand-tuned. The old pier and bracket constants had drifted out of step
    // with the flights and stopped short of them by 0.21 m to 0.40 m, leaving
    // the spans visibly unsupported.
    let low_thickness = 4.0;
    let low_underside =
        |x: f64| climb_base + (x + 72.0) / low_run * (landing - climb_base) - low_thickness;

    // Grounded circulation deck. Its missing north-west quadrant is the open
    // stairwell through which the preceding cell's high flight arrives.
    out += &general_prism_brush(
        &[
            [mx(-80.0), -40.0],
            [mx(96.0), -40.0],
            [mx(96.0), -68.0],
            [mx(88.0), -76.0],
            [mx(64.0), -80.0],
            [mx(-64.0), -80.0],
            [mx(-88.0), -76.0],
            [mx(-80.0), -68.0],
        ],
        0.0,
        FLOOR_TOP,
        [0.0, -60.0],
    );
    for (x0, x1, y0, y1) in [
        (48.0, 96.0, -52.0, 68.0),
        (-64.0, 60.0, 56.0, 68.0),
        (-80.0, -64.0, 52.0, 68.0),
        (-84.0, -48.0, -52.0, 16.0),
        (-48.0, 48.0, -16.0, 16.0),
    ] {
        let (lo, hi) = hand.pair(x0, x1);
        out += &box_brush([lo, y0, 0.0], [hi, y1, FLOOR_TOP]);
    }

    // The low edge begins inside the grounded south deck and rises through its
    // surface, eliminating a separate collider lip at the ramp entrance.
    out += &sloped_deck_brush(
        &low_flight,
        &[climb_base, landing, landing, climb_base],
        low_thickness,
        // Mirrored with the corners. An interior hint is a point inside the
        // solid, and `sloped_deck_brush` derives the height it tests at from
        // the mean of the corner heights — which is only the true mid-height at
        // the flight's plan midpoint. Leave the hint unreflected and it lands
        // outside the reflected wedge, `oriented_plane` reads the normal
        // backwards, and the flight comes out a quarter-metre low along its
        // whole run. That is what a mirrored tower failing where its mirror
        // image passed actually meant.
        [mx(-6.0), -34.0],
    );
    // The turn landing is a thick cantilever keyed into the east structural
    // wall. Ground-level circulation passes underneath its clearance.
    //
    // It spans the full depth of BOTH flights. It used to be only 40 units deep
    // (TB y -20..20) while the flights sit at y -52..-16 and 16..52, so each
    // flight met it across a 0.25 m x 0.25 m corner patch — narrower than the
    // 0.76 m player capsule, which is why the turn was impassable without
    // threading an exact corner, and why the staircase read as broken.
    // The landing stops at x = 56, clear of the rising flight beneath it.
    // Reaching it back to 48 to soften the step onto it does the opposite of
    // what it looks like: the slab's underside is at 3.5 m and the flight is
    // already at 3.66 m by then, so the extra landing takes the headroom a body
    // climbing the flight needs and stops it dead. Measured as a fresh soak
    // stall with the tower unmirrored, which is how it was told apart from the
    // mirroring work landing at the same time.
    let (turn_lo, turn_hi) = hand.pair(56.0, 96.0);
    out += &box_brush([turn_lo, -52.0, landing - 8.0], [turn_hi, 52.0, landing]);
    out += &sloped_deck_brush(
        &high_flight,
        &[climb_top, landing, landing, climb_top],
        8.0,
        [mx(-6.0), 34.0],
    );

    // The upper flight itself runs through the north-west opening and a short
    // distance above the next cell's grounded deck. Their surfaces intersect,
    // so there is no exposed collider lip or separate floating landing.

    // Guard the through-opening on each level. The lower flight rises inside
    // these rails and exits at the open west end; lateral traffic cannot cut
    // across the void by accident.
    let (rail_lo, rail_hi) = hand.pair(-48.0, 48.0);
    out += &box_brush([rail_lo, 52.0, FLOOR_TOP], [rail_hi, 56.0, 28.0]);
    let (stub_lo, stub_hi) = hand.pair(48.0, 52.0);
    out += &box_brush([stub_lo, 16.0, FLOOR_TOP], [stub_hi, 52.0, 28.0]);

    // Narrow floor piers support the lower flight inside its footprint; they
    // never protrude into the south circulation bypass. Heights come from the
    // flight itself so a pier always meets the span it carries.
    for x in [-40.0, 4.0, 48.0] {
        let (lo, hi) = hand.pair(x - 4.0, x + 4.0);
        out += &box_brush([lo, -52.0, 0.0], [hi, -16.0, low_underside(x)]);
    }
    // The upper span keys into the east turn cantilever and the west wall
    // brackets below, leaving its underside free of collider seams.
    let (bracket_lo, bracket_hi) = hand.pair(-104.0, -72.0);
    for (y0, y1) in [(18.0, 26.0), (42.0, 50.0)] {
        out += &box_brush([bracket_lo, y0, 120.0], [bracket_hi, y1, climb_top - 8.0]);
    }

    // The line a body walks, bottom to top. It starts at the foot of the low
    // flight and ends clear of the opening on the deck above — both ends at
    // deck height, so it joins the flat circulation without a special case.
    //
    // It deliberately does *not* include the run across the grounded deck to
    // reach that foot. The deck path below already describes the floor, and two
    // descriptions of the same floor is one too many: a body crossing the deck
    // came within a capture radius of the spine's flat lead-in, was taken to be
    // on the climb, and was steered straight down the lead-in — through a floor
    // pier. Getting to the climb is the floor's business; the climb starts where
    // the floor stops being enough.
    let low_height = |x: f64| climb_base + (x + 72.0) / low_run * (landing - climb_base);
    let spine = vec![
        [mx(-64.0), -34.0, low_height(-64.0)],
        // Up the low flight and *onto* the turn landing, not merely to the top
        // edge of the flight. A node on the edge is a node on the slope: a body
        // that stops a little short of it stands below the landing, and once
        // that gap exceeds one autostep it cannot get up and falls back down
        // the flight. Measured at 0.50 m against a 0.45 m step. The landing
        // spans x 56..96, so 72 is comfortably inside it.
        [mx(72.0), -34.0, landing],
        // Around the turn landing, which spans the depth of both flights.
        [mx(76.0), 0.0, landing],
        [mx(72.0), 34.0, landing],
        // Up the high flight and out through the opening onto the deck of the
        // cell above.
        [mx(-80.0), 34.0, climb_top],
        [mx(-72.0), 60.0, climb_top],
    ];

    // The walkable floor, as an open path rather than a ring. The grounded deck
    // above is a C: the stairwell and the flights above it cut the west side
    // between the north strip and the west strip, so there is no way round that
    // way. Running the path north-east-south instead is longer but it is the
    // only route that exists, and every lateral door apron is reachable from
    // some node on it.
    //
    // The corner nodes are not decoration. Stepping straight from the north
    // strip to the east strip clips the corner where the upper flight is, so
    // the path turns at the corner instead of cutting it.
    let deck = vec![
        [mx(-72.0), 60.0, FLOOR_TOP],
        [mx(-20.0), 62.0, FLOOR_TOP],
        [mx(72.0), 62.0, FLOOR_TOP],
        [mx(72.0), 0.0, FLOOR_TOP],
        [mx(72.0), -60.0, FLOOR_TOP],
        [mx(0.0), -60.0, FLOOR_TOP],
        [mx(-76.0), -56.0, FLOOR_TOP],
        [mx(-76.0), -20.0, FLOOR_TOP],
    ];
    // The west leg runs at x = -76, clear of the low flight's west end at
    // x = -72. Two units further east it passes under the flight's shallowest
    // treads, where the tread is barely a step above the deck — and a body
    // crossing the tower laterally drifts up onto it, whereupon the descend
    // steering tries to walk it back down while the lateral steering keeps
    // pushing it across. Measured: all four soak bots wedged in that argument.
    debug_assert!(deck[6][0].abs() > 72.0 && deck[7][0].abs() > 72.0);
    (out, spine, deck)
}

/// Which way a register's towers turn.
///
/// This is the whole of backlog #13 in one function today: before it, every
/// `Shaft` cell in every district resolved to one procedural switchback through
/// the `generic` fallback, so a facility that is a quarter stairs was a quarter
/// of *the same* stair. The vertical districts now turn the other way from
/// everywhere else, which is the difference a player descending through
/// Wellshaft actually notices.
///
/// It is a small vocabulary on purpose. A handed pair is exact — the hexagon is
/// symmetric, so the mirror is the same geometry with the same guarantees — and
/// nothing here can drift, because the climb and the floor path are reflected
/// with the brushes. Genuinely new skeletons belong in authored `.map` modules
/// under `register_scope`, which is Phase 110.
#[must_use]
pub(crate) fn register_tower_hand(register: &str) -> TowerHand {
    match register {
        "wellshaft" | "megastructure" => TowerHand::Right,
        _ => TowerHand::Left,
    }
}

pub fn stair_segment_map(register: &str) -> String {
    stair_access_map(register, &[], StairVertical::Through, 0)
}

pub fn stair_top_cap_map(register: &str) -> String {
    stair_access_map(register, &[], StairVertical::DownOnly, 0)
}

pub fn stair_bottom_cap_map(register: &str) -> String {
    stair_access_map(register, &[], StairVertical::UpOnly, 0)
}

pub fn stair_landing_map(register: &str, door_face: HexFace) -> String {
    stair_access_map(
        register,
        &[door_face],
        StairVertical::Through,
        door_face.index() as u16,
    )
}

/// Spine nodes up to and including the turn landing.
///
/// The first is the foot of the low flight, then the three that carry the walk
/// around the landing; everything after climbs the high flight and leaves
/// through the ceiling.
const TURN_LANDING_NODES: usize = 4;

/// Ground-supported stair tower with zero to two lateral access doors and an
/// exact vertical opening signature for the legacy logical well state.
pub(crate) fn stair_access_map(
    register: &str,
    door_faces: &[HexFace],
    vertical: StairVertical,
    variant: u16,
) -> String {
    debug_assert!(door_faces.len() <= 2);
    let style = register_style(register);
    let h = level_units();
    let (mut brushes, spine, deck) = supported_switchback(register_tower_hand(register));
    for &door_face in door_faces {
        brushes += &door_floor_apron(door_face, 28.0, 0.0, FLOOR_TOP);
    }
    for face in HexFace::LATERAL {
        if door_faces.contains(&face) {
            brushes += &door_wall(face, 0.0, h, FLOOR_TOP, DOOR_TOP, style.trim_height);
        } else {
            brushes += &wall_brush(face, 0.0, h);
        }
    }
    match vertical {
        StairVertical::UpOnly => brushes += &hex_slab_brush(0.0, FLOOR_TOP),
        StairVertical::DownOnly => brushes += &hex_slab_brush(h - FLOOR_TOP, h),
        StairVertical::Through => {}
    }
    let door_names = door_faces
        .iter()
        .map(|&face| face_name(face))
        .collect::<Vec<_>>();
    let mut out = format!(
        "// Ground-supported stair tower {} doors {}.\n",
        vertical.label(),
        door_names.join(", ")
    );
    out += &worldspawn(&brushes);
    let archetype = if door_faces.is_empty() {
        match vertical {
            StairVertical::UpOnly => "stair_bottom",
            StairVertical::DownOnly => "stair_top",
            StairVertical::Through => "stair_segment",
        }
    } else {
        "stair_landing"
    };
    // The upper flight intersects the first metre of the cell above. This
    // closes the standard floor-slab offset without any runtime pose rewrite.
    out += &tile_meta(archetype, register, variant, 2);
    if matches!(vertical, StairVertical::UpOnly | StairVertical::Through) {
        out += &tile_port("up", "shaft_open");
    }
    if matches!(vertical, StairVertical::DownOnly | StairVertical::Through) {
        out += &tile_port("down", "shaft_open");
    }
    for door in door_names {
        out += &tile_port(door, "door");
    }
    out += &tile_light(-24.0, -30.0, 72.0);
    out += &tile_light(24.0, 30.0, 112.0);
    // A shaft head is capped at `h - FLOOR_TOP` (7.5 m) but the high flight
    // climbs to `climb_top` (8.5 m), straight through that lid. A body
    // following the spine up it meets the underside with 1.70 m of clearance
    // and cannot pass - it is 1.8 m tall. Every one of the 242 capped towers
    // in the library did this, which is what "stalled all four soak bots"
    // was: not a subtle geometry fault, a staircase into a ceiling.
    //
    // The climb ends at the turn landing instead. There is nowhere above it
    // to reach in a capped cell, so leading anything higher only ever ended
    // in the lid. The flight above the landing stays as structure; it is no
    // longer offered as a route.
    let spine: &[[f64; 3]] = match vertical {
        StairVertical::DownOnly => &spine[..TURN_LANDING_NODES],
        _ => &spine,
    };
    for (index, node) in spine.iter().enumerate() {
        out += &tile_stair_node(index as u16, node[0], node[1], node[2]);
    }
    for (index, node) in deck.iter().enumerate() {
        out += &tile_deck_node(index as u16, node[0], node[1], node[2]);
    }
    out
}
