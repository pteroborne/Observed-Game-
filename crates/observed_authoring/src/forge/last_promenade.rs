//! Sky loses its floor and roof: a thin bridge loop around a genuine open well.
use super::entities::{Meta, deck_node, lateral_port, tile_cell, tile_light, worldspawn};
use super::geometry::{FLOOR_TOP, boxed, door_wall_default, pylon, sloped_prism, wall};
use super::grid_turn::{turn_brushes, turn_plan};
use super::{Builder, GENERATED_NOTE};
const STEMS: [&str; 6] = [
    "last_promenade",
    "last_promenade_se",
    "last_promenade_sw",
    "last_promenade_west",
    "last_promenade_nw",
    "last_promenade_ne",
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
    // No hidden full slab: the floor contract explicitly declares open floor.
    let mut brushes = pylon(42.0, 32.0, 40.0, 0.0, 0.0, 0.0);
    for heading in [0, 1] {
        brushes.push_str(&turn_brushes(
            &boxed((0.0, -21.0, 36.0), (112.0, 21.0, 40.0)),
            heading,
        ));
    }
    // The outer approach is as wide as a normal door; bridges narrow thereafter.
    brushes.push_str(&turn_brushes(
        &sloped_prism(
            &[(44.0, -36.0), (100.0, -36.0), (100.0, 36.0), (44.0, 36.0)],
            0.0,
            [
                (44.0, -36.0, 40.0),
                (44.0, 36.0, 40.0),
                (100.0, -36.0, FLOOR_TOP),
            ],
            None,
        ),
        4,
    ));
    brushes.push_str(&turn_brushes(
        &boxed((100.0, -36.0, 0.0), (112.0, 36.0, FLOOR_TOP)),
        4,
    ));
    brushes.push_str(&turn_brushes(
        &boxed((0.0, -36.0, 32.0), (44.0, 36.0, 40.0)),
        4,
    ));
    // Sealed boundaries remain physical. Low mineral walls stop horizontal exits
    // while leaving a wide band of sky above them. No roof or vertical route.
    for face in [2, 3, 5] {
        brushes.push_str(&wall(face, 0.0, 44.0));
    }
    brushes.push_str(&door_wall_default(4, 0.0, 98.0));
    // One slender portal frames the next landing across each exposed bridge.
    for z in [-30.0, 26.0] {
        brushes.push_str(&boxed((84.0, z, 8.0), (88.0, z + 4.0, 88.0)));
    }
    brushes.push_str(&boxed((82.0, -30.0, 88.0), (90.0, 30.0, 92.0)));
    // A short canopy and an opaque side blade shelter the arrival/waiting bay.
    brushes.push_str(&turn_brushes(
        &boxed((44.0, -42.0, 92.0), (112.0, 42.0, 98.0)),
        4,
    ));
    brushes.push_str(&turn_brushes(
        &boxed((48.0, -38.0, 8.0), (80.0, -34.0, 92.0)),
        4,
    ));
    brushes.push_str(&turn_brushes(
        &boxed((48.0, 34.0, 8.0), (52.0, 38.0, 92.0)),
        4,
    ));
    // Seat and broad landing curb give waiting teammates a recognizable position.
    brushes.push_str(&turn_brushes(
        &boxed((102.0, -32.0, 8.0), (110.0, -24.0, 15.0)),
        4,
    ));
    // The well-facing lip is deliberately open. An actual fall reaches surviving
    // lower geometry, or true void, through the normal production controller.
    let mut lights = String::new();
    for (x, y, h) in [(86.0, 0.0, 86.0), (-38.0, 65.0, 90.0), (-26.0, 44.0, 90.0)] {
        let (lx, ly) = turn_plan((x, y), turn);
        lights.push_str(&tile_light(lx, ly, h));
    }
    let mut out =
        String::from("// Last Promenade: open sky, thin bridges, sheltered waiting landings.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&turn_brushes(&brushes, turn)));
    out.push_str(
        &Meta::cell(
            &format!("authored/{}", STEMS[usize::from(turn)]),
            "last_promenade",
            i32::from(turn),
            1,
            1,
        )
        .with_register_scope("thinning")
        .with_rotation_policy("none")
        .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 1, "open"));
    let face = |offset| (usize::from(turn) + offset) % 6;
    out.push_str(&lateral_port(face(0), "span", "promenade_east", 0, 0, 0));
    out.push_str(&lateral_port(
        face(1),
        "span",
        "promenade_south_east",
        0,
        0,
        0,
    ));
    out.push_str(&lateral_port(
        face(4),
        "door",
        "promenade_threshold",
        0,
        0,
        0,
    ));
    out.push_str(&lights);
    // Repeated hub/ramp positions weld into a T-shaped traversal graph. The
    // bridge terminals sit at their real height, while the Door binds at datum.
    let approach = |x| turn_plan((x, 0.0), 4);
    let deck = [
        ((112.0, 0.0), 40.0),
        ((0.0, 0.0), 40.0),
        (approach(44.0), 40.0),
        (approach(72.0), 24.0),
        (approach(112.0), FLOOR_TOP),
        (approach(72.0), 24.0),
        (approach(44.0), 40.0),
        ((0.0, 0.0), 40.0),
        (turn_plan((112.0, 0.0), 1), 40.0),
    ];
    for (index, (point, height)) in deck.into_iter().enumerate() {
        let (x, y) = turn_plan(point, turn);
        out.push_str(&deck_node(index as u16, x, y, height));
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn promenade_sources_reproduce_and_validate_open_floor_at_every_heading() {
        super::super::assert_reproduces(&builders());
        for (_, build) in builders() {
            let module = crate::parse_authored_module(&build()).expect("valid Sky sector");
            assert_eq!(module.footprint[0].floor, crate::source::FloorPolicy::Open);
            assert!(module.prototype.hulls.len() * 3 <= 128);
            assert_eq!(module.prototype.deck.nodes.len(), 9);
            let graph =
                observed_traversal::compile_compatibility_graph(Some(&module.prototype.deck), None)
                    .expect("raised bridge and ramp guide");
            assert_eq!(graph.guide.nodes().len(), 6);
            assert_eq!(graph.guide.edges().len(), 5);
        }
    }
}
