//! The silo wellshaft family, plus the two remaining originals.
//!
//! A 7-hex composition: a solid centre core, six ring tiles carrying a
//! continuous helical ramp around it - each rises `LEVEL / 6`, so one full loop
//! climbs exactly one level - and a bridge variant whose outer face opens onto
//! a landing.
//!
//! The ring tile is authored once in the E-position frame: core at local west,
//! enter low at south_west, exit high at north_west, sealed outer arc. The
//! composition places rotated and raised copies, so every ramp seam matches by
//! construction. Walk surfaces exceed the 1.5 m floor-probe band, which is why
//! the footprint declares `floor="open"`.

use super::entities::{
    Meta, PORT_SHORT, ceiling_fixture, lateral_port, tile_cell, tile_cell_default, vertical_port,
    wall_fixture, worldspawn,
};
use super::geometry::{
    DOOR_TOP, FLOOR_TOP, LEVEL, P2, corners, door_wall, door_wall_default, hex_slab, prism, pylon,
    sloped_prism, wall,
};
use super::{Builder, GENERATED_NOTE};

/// One ring tile's rise: 21.333 units (1.333 m).
const RING_RISE: f64 = LEVEL / 6.0;

#[must_use]
fn silo_meta(name: &str, archetype: &str, ports: &str) -> String {
    let mut out = Meta::cell(&format!("authored/{name}"), archetype, 0, 1, 1)
        .with_register_scope("all")
        .with_rotation_policy("none")
        .emit();
    out.push_str(&tile_cell(0, 0, 0, 1, "open"));
    out.push_str(ports);
    out
}

/// A discrete helicoid: five triangular facets fanning between the core-side
/// west edge and the outer rim.
///
/// Height is **constant** along the entry (SW) and exit (NW) seam edges, so the
/// seam profile is flat and rotated, raised copies meet exactly no matter how
/// the neighbour is oriented. Heights rise by thirds of `RING_RISE` around the
/// outer rim and halve along the core edge.
#[must_use]
fn ring_ramp() -> String {
    let lo = FLOOR_TOP;
    let hi = FLOOR_TOP + RING_RISE;
    let third = RING_RISE / 3.0;
    // Entry edge, outer corner.
    let c2 = (0.0, -128.0, lo);
    // Entry edge, core corner.
    let c3 = (-112.0, -64.0, lo);
    let c1 = (112.0, -64.0, lo + third);
    let c0 = (112.0, 64.0, lo + 2.0 * third);
    // Core edge midpoint.
    let k1 = (-112.0, 0.0, lo + RING_RISE * 0.5);
    // Exit edge, core corner.
    let c4 = (-112.0, 64.0, hi);
    // Exit edge, outer corner.
    let c5 = (0.0, 128.0, hi);
    let triangles = [
        [c2, c1, c3],
        [c3, c1, k1],
        [k1, c1, c0],
        [k1, c0, c4],
        [c4, c0, c5],
    ];
    let mut out = String::new();
    for tri in triangles {
        let plan: Vec<P2> = tri.iter().map(|p| (p.0, p.1)).collect();
        out.push_str(&sloped_prism(&plan, 0.0, tri, None));
    }
    out
}

#[must_use]
pub fn silo_core() -> String {
    let mut brushes = String::from("// Solid full-height core mass\n");
    brushes.push_str(&prism(&corners(), 0.0, LEVEL, Some((0.0, 0.0)), 0.0, 0.0));
    let mut out = String::from("// Silo core: 100% solid center column of the 7-hex wellshaft.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(&silo_meta("silo_core", "silo_core", ""));
    out
}

#[must_use]
pub fn silo_ring() -> String {
    let mut brushes =
        String::from("// Helical ramp facet (rises RING_RISE from SW edge to NW edge)\n");
    brushes.push_str(&ring_ramp());
    brushes.push_str("// Sealed outer arc: east / south_east / north_east\n");
    for face in [0, 1, 5] {
        brushes.push_str(&wall(face, 0.0, LEVEL));
    }
    let (fixture, lights) = wall_fixture(0, 0.5, 88.0, 20.0);
    brushes.push_str(&fixture);
    let mut out = String::from("// Silo ring segment: one sixth of the helical wellshaft ramp.\n");
    out.push_str("// Core sits beyond the west face; SW/NW faces continue the ramp.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(&silo_meta("silo_ring", "silo_ring", ""));
    out.push_str(&lights);
    out
}

#[must_use]
pub fn silo_ring_bridge() -> String {
    let mut brushes = String::from("// Helical ramp facet\n");
    brushes.push_str(&ring_ramp());
    brushes.push_str("// Sealed outer faces flanking the bridge door\n");
    for face in [1, 5] {
        brushes.push_str(&wall(face, 0.0, LEVEL));
    }
    brushes.push_str("// Bridge landing pad in front of the east door\n");
    brushes.push_str(&prism(
        &[(56.0, -36.0), (104.0, -36.0), (104.0, 36.0), (56.0, 36.0)],
        0.0,
        24.0,
        None,
        2.0,
        0.0,
    ));
    brushes.push_str("// East door raised to the landing height\n");
    brushes.push_str(&door_wall(0, 0.0, LEVEL, 24.0, 96.0, 10.0, 8.0));
    let (fixture, lights) = wall_fixture(1, 0.58, 96.0, 20.0);
    brushes.push_str(&fixture);
    let mut out =
        String::from("// Silo ring segment with the per-level bridge landing and door.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(&silo_meta(
        "silo_ring_bridge",
        "silo_ring_bridge",
        &lateral_port(0, "door", "bridge_door", 0, 0, 0),
    ));
    out.push_str(&lights);
    out
}

/// A fully enclosed decision room with six physical thresholds.
#[must_use]
pub fn room_grounded_hub() -> String {
    let mut brushes =
        String::from("// Fully enclosed decision room with six physical thresholds\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 3.0, 0.0));
    brushes.push_str(&hex_slab(LEVEL - FLOOR_TOP, LEVEL, 0.0, 3.0));
    for face in 0..6 {
        brushes.push_str(&door_wall_default(face, 0.0, LEVEL));
    }
    brushes.push_str("// Full-height central service pier: grounded structure, no mezzanine\n");
    brushes.push_str(&pylon(14.0, FLOOR_TOP, LEVEL - FLOOR_TOP, 30.0, 4.0, 0.0));
    let mut lights = String::new();
    for (x, y) in [(-48.0, -28.0), (48.0, -28.0), (0.0, 54.0)] {
        let (fixture, source) = ceiling_fixture(x, y, LEVEL, 13.0, 8.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }
    let mut out =
        String::from("// Grounded sanctuary hub: six decisions around a supported service pier.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(&Meta::cell("authored/room_grounded_hub", "sanctuary", 0, 1, 10).emit());
    out.push_str(&tile_cell_default());
    for (face, short) in PORT_SHORT.iter().enumerate() {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("{short}_portal"),
            0,
            0,
            0,
        ));
    }
    out.push_str(&lights);
    out
}

/// A ground-supported two-level ramp: enter west, exit east one level up.
#[must_use]
pub fn hall_ramp() -> String {
    let top = 2.0 * LEVEL;
    let mut brushes = String::from("// Ground slab below the supported full-level ramp\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 2.0, 0.0));
    brushes.push_str("// One solid ramp mass: west sill 0.5 m, east sill 8.5 m\n");
    brushes.push_str(&sloped_prism(
        &corners(),
        0.0,
        [
            (-112.0, -64.0, FLOOR_TOP),
            (-112.0, 64.0, FLOOR_TOP),
            (112.0, -64.0, LEVEL + FLOOR_TOP),
        ],
        None,
    ));
    brushes.push_str(&hex_slab(top - FLOOR_TOP, top, 0.0, 3.0));
    for face in 0..6 {
        if face == 3 {
            brushes.push_str(&door_wall_default(face, 0.0, top));
        } else if face == 0 {
            // The east door sits a full level up, on the ramp's high sill.
            brushes.push_str(&door_wall(
                face,
                0.0,
                top,
                LEVEL + FLOOR_TOP,
                LEVEL + DOOR_TOP,
                10.0,
                8.0,
            ));
        } else {
            brushes.push_str(&wall(face, 0.0, top));
        }
    }
    let mut lights = String::new();
    for (face, along, z) in [(2, 0.72, 88.0), (5, 0.28, 184.0)] {
        let (fixture, source) = wall_fixture(face, along, z, 20.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }
    let mut out =
        String::from("// Ground-supported two-level ramp: enter west, exit east one level up.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_ramp", "hall_ramp", 0, 2, 10)
            .with_register_scope("all")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 2, "ramp"));
    out.push_str(&lateral_port(3, "door", "west_entry", 0, 0, 0));
    out.push_str(&vertical_port("up", "ramp_open", "upper_ramp", 0));
    out.push_str(&lights);
    out
}

/// Every builder in this module, paired with the file it must reproduce.
#[must_use]
pub fn builders() -> Vec<Builder> {
    vec![
        ("silo_core", silo_core as fn() -> String),
        ("silo_ring", silo_ring),
        ("silo_ring_bridge", silo_ring_bridge),
        ("room_grounded_hub", room_grounded_hub),
        ("hall_ramp", hall_ramp),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_silo_reproduces_its_committed_file() {
        super::super::assert_reproduces(&builders());
    }

    /// One loop of six ring tiles must climb exactly one level, or the helix
    /// does not close and the wellshaft has a step in it.
    #[test]
    fn six_ring_tiles_climb_exactly_one_level() {
        assert!((RING_RISE * 6.0 - LEVEL).abs() < 1e-9);
    }
}
