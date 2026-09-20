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
    Meta, PORT_SHORT, ceiling_fixture, lateral_port, stair_node, tile_cell, tile_cell_default,
    vertical_port, wall_fixture, worldspawn,
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

/// The ramp's west sill, at the west doorway, flush with the floor slab.
const RAMP_WEST: f64 = -112.0;
/// The ramp's east sill, one full level up at the east door's high sill.
const RAMP_EAST: f64 = 112.0;

/// The height of the ramp's walk surface at plan `x`.
///
/// One expression, used by both the mass and the climbable line through it. A
/// spine measured separately from the geometry it describes is how a follower
/// comes to walk through a wall — the whole reason `stair_tower`'s piers were
/// re-derived from their flight's gradient rather than written down beside it.
#[must_use]
fn ramp_height(x: f64) -> f64 {
    FLOOR_TOP + (x - RAMP_WEST) / (RAMP_EAST - RAMP_WEST) * LEVEL
}

/// The climbable line across a `hall_ramp`, west sill to east.
///
/// Variant 0 is one solid slope along x with no variation across y, so the line
/// is the centreline and nothing more; there is no wall to go round and no deck
/// to cross first, which is why this ships a spine and no `DeckPath`.
///
/// Five nodes, matching the perimeter flight's density over a comparable run.
/// The first sits at the west doorway, where the slope is flush with the floor
/// slab, and the last on the east sill — which for this shape is the *only*
/// plan position where a body stands on the deck above, because the mass
/// reaches full height exactly at the cell's east edge.
///
/// Until this existed the ramp projected no traversal annotation at all, so it
/// recorded no guide, could not be executed as a graph leg, and the objective
/// bot walked it by inferring a heading from the `RampUp` archetype — the last
/// piece of shotgun surgery in the match layer.
#[must_use]
fn ramp_spine() -> String {
    const NODES: usize = 5;
    let mut out = String::new();
    for index in 0..NODES {
        #[allow(clippy::cast_precision_loss)]
        let t = index as f64 / (NODES - 1) as f64;
        let x = RAMP_WEST + (RAMP_EAST - RAMP_WEST) * t;
        #[allow(clippy::cast_possible_truncation)]
        out.push_str(&stair_node(index as u16, x, 0.0, ramp_height(x)));
    }
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
            (RAMP_WEST, -64.0, ramp_height(RAMP_WEST)),
            (RAMP_WEST, 64.0, ramp_height(RAMP_WEST)),
            (RAMP_EAST, -64.0, ramp_height(RAMP_EAST)),
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
    out.push_str(&ramp_spine());
    out.push_str(&lights);
    out
}

/// Where the gallery's parapet runs, in plan `y`.
///
/// The walk stays south of this line and the well opens north of it. 44 units
/// is 2.75 m from the centreline, so the flight is still wider than the 72-unit
/// doorway it has to receive at either end.
const WELL_EDGE: f64 = 44.0;

/// Half the well's length along the climb. The mass is left solid outside it,
/// so both doors keep a full-width landing and the opening is a bite out of the
/// middle rather than a channel down one side.
const WELL_HALF: f64 = 60.0;

/// How far the parapet stands above the walk surface. 20 units is 1.25 m -
/// over a stride and under a sightline, so it stops a body and not the view.
const PARAPET: f64 = 20.0;

/// A two-level ramp with a gallery: the same climb, beside an open well.
///
/// # Why a second reading exists at all
///
/// Measured across six production facilities, `hall_ramp` places about 220
/// cells and every one of them was the same tile. It was the largest archetype
/// in the corpus with exactly one reading - expanse has two and runs them 50/50,
/// junctions have two, turns have three. A ramp was the one climb in the game
/// that always looked the same, which is a direct contribution to not being
/// able to say where you are.
///
/// # What the contract allowed and what it did not
///
/// **The walk had to stay one mass.** `validate_ramps` finds the ramp surface
/// by taking the tallest hull covering the cell origin and requires its own
/// height to be within 0.6 m of a full level - it is checking that the thing
/// under your feet really does climb a storey. Cutting the mass into a
/// west half and an east half fails that on both halves, so the well cannot
/// cross the centreline. It is a bite out of the north flank instead, and the
/// south band runs the whole length and carries both the walk and the contract.
///
/// **Both doors keep a full-width landing.** The well stops [`WELL_HALF`] short
/// of each end, so the aperture at either seam meets solid floor.
///
/// **The spine is unchanged and that is deliberate.** The climb line is the
/// centreline at `y = 0`, which is inside the south band at every `x`, so the
/// follower walks exactly what it walked before and this variant cannot be the
/// reason a bot stalls.
#[must_use]
pub fn hall_ramp_gallery() -> String {
    let top = 2.0 * LEVEL;
    let plane = [
        (RAMP_WEST, -64.0, ramp_height(RAMP_WEST)),
        (RAMP_WEST, 64.0, ramp_height(RAMP_WEST)),
        (RAMP_EAST, -64.0, ramp_height(RAMP_EAST)),
    ];
    let hex: Vec<P2> = corners().to_vec();

    let mut brushes =
        String::from("// Ground slab: the well's floor, four metres under the walk\n");
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 2.0, 0.0));
    brushes.push_str("// The walk: one mass the whole length, south of the parapet\n");
    brushes.push_str(&sloped_prism(
        &super::halls::clip(&hex, (0.0, 1.0), WELL_EDGE),
        0.0,
        plane,
        None,
    ));
    brushes.push_str("// North shoulders: solid landing at both doors, well between them\n");
    let north = super::halls::clip(&hex, (0.0, -1.0), -WELL_EDGE);
    for normal in [(1.0, 0.0), (-1.0, 0.0)] {
        let shoulder = super::halls::clip(&north, normal, -WELL_HALF);
        if shoulder.len() >= 3 {
            brushes.push_str(&sloped_prism(&shoulder, 0.0, plane, None));
        }
    }
    brushes.push_str("// Parapet along the open edge, climbing with the walk\n");
    let rail = [
        (-WELL_HALF, WELL_EDGE - 6.0),
        (WELL_HALF, WELL_EDGE - 6.0),
        (WELL_HALF, WELL_EDGE),
        (-WELL_HALF, WELL_EDGE),
    ];
    brushes.push_str(&sloped_prism(
        &rail,
        0.0,
        [
            (plane[0].0, plane[0].1, plane[0].2 + PARAPET),
            (plane[1].0, plane[1].1, plane[1].2 + PARAPET),
            (plane[2].0, plane[2].1, plane[2].2 + PARAPET),
        ],
        None,
    ));
    brushes.push_str(&hex_slab(top - FLOOR_TOP, top, 0.0, 3.0));

    for face in 0..6 {
        if face == 3 {
            brushes.push_str(&door_wall_default(face, 0.0, top));
        } else if face == 0 {
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
    // One practical over the walk and one down in the well, so the drop reads
    // as a place rather than as a dark hole beside the route.
    for (face, along, z) in [(2, 0.72, 88.0), (1, 0.5, 40.0), (5, 0.28, 184.0)] {
        let (fixture, source) = wall_fixture(face, along, z, 18.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = String::from(
        "// Two-level ramp variant: the same climb with an open gallery well beside it.\n",
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/hall_ramp_gallery", "hall_ramp", 1, 2, 7)
            .with_register_scope("all")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 2, "ramp"));
    out.push_str(&lateral_port(3, "door", "west_entry", 0, 0, 0));
    out.push_str(&vertical_port("up", "ramp_open", "upper_ramp", 0));
    out.push_str(&ramp_spine());
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
        ("hall_ramp_gallery", hall_ramp_gallery),
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
