//! Vertical circulation: grounded two-level ramp prefabs in all six exit directions.

use observed_hex::HexFace;

use super::geometry::{
    DOOR_TOP, FLOOR_TOP, door_wall, hex_slab_brush, level_units, sloped_slab_brush, tb_edge,
    tile_light, tile_meta, tile_port, tile_stair_node, wall_brush, worldspawn,
};
use super::{face_name, register_style};

/// The midpoint of a lateral face's edge, in TB plan units.
fn face_mid(face: HexFace) -> (f64, f64) {
    let (a, b) = tb_edge(face);
    ((a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5)
}

/// The climbable line across a generated ramp: entrance sill to exit sill.
///
/// A ramp is one solid slope with no variation across the seam, so the line is
/// the centreline between the two door midpoints and nothing more — no wall to
/// go round, and therefore no `DeckPath` either.
///
/// Five nodes, matching the authored perimeter flight's density over a
/// comparable run. The first sits at the entrance doorway, where the slope is
/// flush with the floor, and the last on the exit sill a full level up, which
/// for this shape is the only plan position where a body stands on the deck
/// above: the mass reaches full height exactly at the cell's far edge.
///
/// Without this the tile shipped **no traversal annotation of any kind**, so
/// the projection recorded no guide for its cell, no graph leg could be
/// acquired, and the objective bot fell back to reading `RampUp`/`RampHead` to
/// pick a heading — the last shape inference in the match layer.
fn ramp_spine(entrance_face: HexFace, exit_face: HexFace, rise: f64) -> String {
    const NODES: usize = 5;
    let (low, high) = (face_mid(entrance_face), face_mid(exit_face));
    let mut out = String::new();
    for index in 0..NODES {
        #[allow(clippy::cast_precision_loss)]
        let t = index as f64 / (NODES - 1) as f64;
        #[allow(clippy::cast_possible_truncation)]
        out.push_str(&tile_stair_node(
            index as u16,
            low.0 + (high.0 - low.0) * t,
            low.1 + (high.1 - low.1) * t,
            FLOOR_TOP + rise * t,
        ));
    }
    out
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
    out += &ramp_spine(entrance_face, exit_face, h);
    for (face, z) in [(entrance_face, 64.0), (exit_face, h + 64.0)] {
        let (a, b) = tb_edge(face);
        out += &tile_light((a[0] + b[0]) * 0.25, (a[1] + b[1]) * 0.25, z);
    }
    out
}

// The switchback and its four entry points are gone; `forge::tower` authors the
// facility's vertical circulation now.
//
// What lived here: `supported_switchback` (two flights, a turn cantilever,
// piers, brackets and rails), `TowerHand`/`register_tower_hand` which mirrored
// it per register, `StairVertical`, and `stair_access_map` wrapping it in walls
// and doors. It shipped 66 variants per register - 726 prototypes - and two
// faults that the authored family exists to not repeat: capped heads whose
// climb ran through their own lid, and an `up` port on an internal face that
// passed only because `authoring_version 1` skips the strict checks.
//
// `ramp_map` stays: it is the straight `hall_ramp`, unrelated to towers.
