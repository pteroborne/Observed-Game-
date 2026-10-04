//! A continuous reading chamber with shelved outer walls and a raised bridge circuit.
use super::entities::{
    Meta, ceiling_fixture, lateral_port, tile_cell_default, tile_light, worldspawn,
};
use super::geometry::{
    FLOOR_TOP, LEVEL, band, boxed, door_wall_default, hex_slab, prism, sloped_prism, wall,
};
use super::grid_turn::{turn_brushes, turn_plan};
use super::{Builder, GENERATED_NOTE};

const STEMS: [&str; 6] = [
    "archive_well",
    "archive_well_se",
    "archive_well_sw",
    "archive_well_west",
    "archive_well_nw",
    "archive_well_ne",
];

#[must_use]
pub fn builders() -> Vec<Builder> {
    vec![
        (STEMS[0], || sector(0)),
        (STEMS[1], || sector(1)),
        (STEMS[2], || sector(2)),
        (STEMS[3], || sector(3)),
        (STEMS[4], || sector(4)),
        (STEMS[5], || sector(5)),
    ]
}

#[must_use]
pub fn sector(turn: u8) -> String {
    let turn = turn % 6;
    let mut brushes = hex_slab(0.0, FLOOR_TOP, 0.0, 0.0);
    brushes.push_str(&hex_slab(124.0, LEVEL, 0.0, 0.0));
    for face in [2, 3, 5] {
        brushes.push_str(&wall(face, 0.0, LEVEL));
        // Deep, continuous shelf courses leave the inner chamber open.
        for z in [16.0, 36.0, 64.0, 86.0, 108.0] {
            brushes.push_str(&band(face, 8.0, 24.0, z, z + 3.0));
        }
    }
    brushes.push_str(&door_wall_default(4, 0.0, LEVEL));
    // Two bridge arms meet exactly at both Span face midpoints. Three sectors
    // make one traversable upper circuit, with a generous lower reading floor.
    brushes.push_str(&boxed((-64.0, -16.0, 44.0), (112.0, 16.0, 48.0)));
    brushes.push_str(&prism(
        &[(-16.0, 0.0), (42.0, -104.0), (70.0, -88.0), (16.0, 0.0)],
        44.0,
        48.0,
        None,
        0.0,
        0.0,
    ));
    brushes.push_str(&boxed((-96.0, -48.0, 44.0), (-48.0, -32.0, 48.0)));
    // Leave the flight open overhead; the side gallery reconnects after its landing.
    brushes.push_str(&boxed((-64.0, -32.0, 44.0), (-48.0, 16.0, 48.0)));
    brushes.push_str(&sloped_prism(
        &[(-96.0, 62.0), (-64.0, 62.0), (-64.0, -32.0), (-96.0, -32.0)],
        0.0,
        [(-96.0, 62.0, 8.0), (-64.0, 62.0, 8.0), (-96.0, -32.0, 48.0)],
        None,
    ));
    // A solid waist-high rail anchors the reading perch. Exposed bridge arms
    // stay open, making crossing a commitment rather than a protected corridor.
    brushes.push_str(&boxed((-96.0, -48.0, 48.0), (-48.0, -46.0, 62.0)));
    for (x, y) in [(-60.0, -12.0), (40.0, -44.0)] {
        brushes.push_str(&boxed((x, y, 8.0), (x + 6.0, y + 6.0, 44.0)));
    }
    // A low, physical reading desk away from the ground crossing.
    brushes.push_str(&boxed((-42.0, 44.0, 8.0), (-14.0, 60.0, 21.0)));
    let mut lights = String::new();
    for (x, y) in [(-36.0, 48.0), (66.0, 24.0), (0.0, -64.0)] {
        let (fixture, _) = ceiling_fixture(x, y, LEVEL, 16.0, 4.0);
        brushes.push_str(&fixture);
        let (lx, ly) = turn_plan((x, y), turn);
        lights.push_str(&tile_light(lx, ly, 116.0));
    }
    let mut out =
        String::from("// The Archive Well: shelved reading bowl and connected upper galleries.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&turn_brushes(&brushes, turn)));
    out.push_str(
        &Meta::cell(
            &format!("authored/{}", STEMS[usize::from(turn)]),
            "archive_well",
            i32::from(turn),
            1,
            1,
        )
        .with_register_scope("infinite_gallery")
        .with_rotation_policy("none")
        .emit(),
    );
    out.push_str(&tile_cell_default());
    let face = |offset| (usize::from(turn) + offset) % 6;
    out.push_str(&lateral_port(face(0), "span", "archive_east", 0, 0, 0));
    out.push_str(&lateral_port(
        face(1),
        "span",
        "archive_south_east",
        0,
        0,
        0,
    ));
    out.push_str(&lateral_port(face(4), "door", "archive_threshold", 0, 0, 0));
    out.push_str(&lights);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_sources_reproduce_and_validate() {
        super::super::assert_reproduces(&builders());
        for (_, build) in builders() {
            let module = crate::parse_authored_module(&build()).expect("valid archive sector");
            assert!(module.prototype.hulls.len() * 3 <= 128);
        }
    }
}
