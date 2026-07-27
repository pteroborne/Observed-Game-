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

/// A physically continuous switchback stair, with the walkable line through it.
///
/// Thin flights preserve headroom when cells stack; floor piers and
/// wall-connected brackets visibly support every span. The north-west opening is
/// kept clear so the flight below can emerge into this cell instead of meeting
/// the underside of another wedge.
///
/// The returned spine is derived from the same flight constants as the brushes,
/// which is the point: the bot follows the line, so the line cannot fall out of
/// step with the surface the way a hand-written copy of these numbers did.
fn supported_switchback() -> (String, Vec<[f64; 3]>, Vec<[f64; 3]>) {
    let low_flight = [[-72.0, -52.0], [60.0, -52.0], [60.0, -16.0], [-72.0, -16.0]];
    let high_flight = [[-80.0, 16.0], [60.0, 16.0], [60.0, 52.0], [-80.0, 52.0]];
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
            [-80.0, -40.0],
            [96.0, -40.0],
            [96.0, -68.0],
            [88.0, -76.0],
            [64.0, -80.0],
            [-64.0, -80.0],
            [-88.0, -76.0],
            [-80.0, -68.0],
        ],
        0.0,
        FLOOR_TOP,
        [0.0, -60.0],
    );
    for (min, max) in [
        ([48.0, -52.0, 0.0], [96.0, 68.0, FLOOR_TOP]),
        ([-64.0, 56.0, 0.0], [60.0, 68.0, FLOOR_TOP]),
        ([-80.0, 52.0, 0.0], [-64.0, 68.0, FLOOR_TOP]),
        ([-84.0, -52.0, 0.0], [-48.0, 16.0, FLOOR_TOP]),
        ([-48.0, -16.0, 0.0], [48.0, 16.0, FLOOR_TOP]),
    ] {
        out += &box_brush(min, max);
    }

    // The low edge begins inside the grounded south deck and rises through its
    // surface, eliminating a separate collider lip at the ramp entrance.
    out += &sloped_deck_brush(
        &low_flight,
        &[climb_base, landing, landing, climb_base],
        low_thickness,
        [-6.0, -34.0],
    );
    // The turn landing is a thick cantilever keyed into the east structural
    // wall. Ground-level circulation passes underneath its clearance.
    //
    // It spans the full depth of BOTH flights. It used to be only 40 units deep
    // (TB y -20..20) while the flights sit at y -52..-16 and 16..52, so each
    // flight met it across a 0.25 m x 0.25 m corner patch — narrower than the
    // 0.76 m player capsule, which is why the turn was impassable without
    // threading an exact corner, and why the staircase read as broken.
    out += &box_brush([56.0, -52.0, landing - 8.0], [96.0, 52.0, landing]);
    out += &sloped_deck_brush(
        &high_flight,
        &[climb_top, landing, landing, climb_top],
        8.0,
        [-6.0, 34.0],
    );

    // The upper flight itself runs through the north-west opening and a short
    // distance above the next cell's grounded deck. Their surfaces intersect,
    // so there is no exposed collider lip or separate floating landing.

    // Guard the through-opening on each level. The lower flight rises inside
    // these rails and exits at the open west end; lateral traffic cannot cut
    // across the void by accident.
    out += &box_brush([-48.0, 52.0, FLOOR_TOP], [48.0, 56.0, 28.0]);
    out += &box_brush([48.0, 16.0, FLOOR_TOP], [52.0, 52.0, 28.0]);

    // Narrow floor piers support the lower flight inside its footprint; they
    // never protrude into the south circulation bypass. Heights come from the
    // flight itself so a pier always meets the span it carries.
    for x in [-40.0, 4.0, 48.0] {
        out += &box_brush([x - 4.0, -52.0, 0.0], [x + 4.0, -16.0, low_underside(x)]);
    }
    // The upper span keys into the east turn cantilever and the west wall
    // brackets below, leaving its underside free of collider seams.
    for (y0, y1) in [(18.0, 26.0), (42.0, 50.0)] {
        out += &box_brush([-104.0, y0, 120.0], [-72.0, y1, climb_top - 8.0]);
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
        [-64.0, -34.0, low_height(-64.0)],
        // Up the low flight to the turn.
        [60.0, -34.0, landing],
        // Around the turn landing, which spans the depth of both flights.
        [76.0, 0.0, landing],
        [60.0, 34.0, landing],
        // Up the high flight and out through the north-west opening onto the
        // deck of the cell above.
        [-80.0, 34.0, climb_top],
        [-72.0, 60.0, climb_top],
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
        [-72.0, 60.0, FLOOR_TOP],
        [-20.0, 62.0, FLOOR_TOP],
        [72.0, 62.0, FLOOR_TOP],
        [72.0, 0.0, FLOOR_TOP],
        [72.0, -60.0, FLOOR_TOP],
        [0.0, -60.0, FLOOR_TOP],
        [-76.0, -56.0, FLOOR_TOP],
        [-76.0, -20.0, FLOOR_TOP],
    ];
    // The west leg runs at x = -76, clear of the low flight's west end at
    // x = -72. Two units further east it passes under the flight's shallowest
    // treads, where the tread is barely a step above the deck — and a body
    // crossing the tower laterally drifts up onto it, whereupon the descend
    // steering tries to walk it back down while the lateral steering keeps
    // pushing it across. Measured: all four soak bots wedged in that argument.
    debug_assert!(deck[6][0] < -72.0 && deck[7][0] < -72.0);
    (out, spine, deck)
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
    let (mut brushes, spine, deck) = supported_switchback();
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
    for (index, node) in spine.iter().enumerate() {
        out += &tile_stair_node(index as u16, node[0], node[1], node[2]);
    }
    for (index, node) in deck.iter().enumerate() {
        out += &tile_deck_node(index as u16, node[0], node[1], node[2]);
    }
    out
}
