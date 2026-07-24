//! Vertical circulation: grounded two-level ramp prefabs in all six exit directions.

use observed_hex::HexFace;

use super::geometry::{
    DOOR_TOP, FLOOR_TOP, box_brush, door_floor_apron, door_wall, general_prism_brush,
    hex_slab_brush, level_units, sloped_deck_brush, sloped_slab_brush, tb_edge, tile_light,
    tile_meta, tile_port, wall_brush, worldspawn,
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

/// A physically continuous switchback stair. Thin flights preserve headroom
/// when cells stack; floor piers and wall-connected brackets visibly support
/// every span. The north-west opening is kept clear so the flight below can
/// emerge into this cell instead of meeting the underside of another wedge.
fn supported_switchback() -> String {
    let low_flight = [[-72.0, -52.0], [60.0, -52.0], [60.0, -16.0], [-72.0, -16.0]];
    let high_flight = [[-80.0, 16.0], [60.0, 16.0], [60.0, 52.0], [-80.0, 52.0]];
    let mut out = String::new();

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
    out += &sloped_deck_brush(&low_flight, &[4.0, 64.0, 64.0, 4.0], 4.0, [-6.0, -34.0]);
    // The turn landing is a thick cantilever keyed into the east structural
    // wall. Ground-level circulation passes underneath its 3.5 m clearance.
    out += &box_brush([56.0, -20.0, 56.0], [96.0, 20.0, 64.0]);
    out += &sloped_deck_brush(&high_flight, &[144.0, 64.0, 64.0, 144.0], 8.0, [-6.0, 34.0]);

    // The upper flight itself runs through the north-west opening and a short
    // distance above the next cell's grounded deck. Their surfaces intersect,
    // so there is no exposed collider lip or separate floating landing.

    // Guard the through-opening on each level. The lower flight rises inside
    // these rails and exits at the open west end; lateral traffic cannot cut
    // across the void by accident.
    out += &box_brush([-48.0, 52.0, FLOOR_TOP], [48.0, 56.0, 28.0]);
    out += &box_brush([48.0, 16.0, FLOOR_TOP], [52.0, 52.0, 28.0]);

    // Narrow floor piers support the lower flight inside its footprint; they
    // never protrude into the south circulation bypass.
    for (x, underside) in [(-40.0, 10.8), (4.0, 29.5), (48.0, 48.2)] {
        out += &box_brush([x - 4.0, -52.0, 0.0], [x + 4.0, -16.0, underside]);
    }
    // The upper span keys into the east turn cantilever and the west wall
    // brackets below, leaving its underside free of collider seams.
    for (y0, y1) in [(18.0, 26.0), (42.0, 50.0)] {
        out += &box_brush([-104.0, y0, 120.0], [-72.0, y1, 128.0]);
    }
    out
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
    let mut brushes = supported_switchback();
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
    out
}
