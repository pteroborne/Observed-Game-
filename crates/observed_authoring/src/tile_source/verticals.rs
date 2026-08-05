//! Vertical circulation: grounded two-level ramp prefabs in all six exit directions.

use observed_hex::HexFace;

use super::geometry::{
    DOOR_TOP, FLOOR_TOP, door_wall, hex_slab_brush, level_units, sloped_slab_brush, tb_edge,
    tile_light, tile_meta, tile_port, wall_brush, worldspawn,
};
use super::{face_name, register_style};

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
