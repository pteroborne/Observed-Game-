//! Monumental opaque jade piers frame separated landings and elevated crossings.
use super::entities::{
    Meta, ceiling_fixture, lateral_port, tile_cell_default, tile_light, worldspawn,
};
use super::geometry::{
    FLOOR_TOP, LEVEL, boxed, door_wall_default, hex_slab, prism, pylon, sloped_prism, translate,
    wall,
};
use super::grid_turn::{turn_brushes, turn_plan};
use super::{Builder, GENERATED_NOTE};

const STEMS: [&str; 6] = [
    "jade_nave",
    "jade_nave_se",
    "jade_nave_sw",
    "jade_nave_west",
    "jade_nave_nw",
    "jade_nave_ne",
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
    }
    brushes.push_str(&door_wall_default(4, 0.0, LEVEL));
    // Two bridge arms meet exactly at both Span face midpoints. Three sectors
    // make one traversable upper circuit, with a generous lower nave floor.
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
    // Leave the flight open overhead; the side landing reconnects after its landing.
    brushes.push_str(&boxed((-64.0, -32.0, 44.0), (-48.0, 16.0, 48.0)));
    brushes.push_str(&sloped_prism(
        &[(-96.0, 62.0), (-64.0, 62.0), (-64.0, -32.0), (-96.0, -32.0)],
        0.0,
        [(-96.0, 62.0, 8.0), (-64.0, 62.0, 8.0), (-96.0, -32.0, 48.0)],
        None,
    ));
    // A solid balustrade protects the watching landing, leaving crossings exposed.
    brushes.push_str(&boxed((-96.0, -48.0, 48.0), (-48.0, -46.0, 62.0)));
    // Tall opaque faceted piers interrupt sightlines while both crossing arms
    // remain clear. Stepped crowns stay above the Observer's upper-route eye.
    for (x, y) in [(-24.0, -60.0), (56.0, 56.0)] {
        brushes.push_str(&translate(
            &pylon(18.0, 8.0, 116.0, 0.0, 3.0, 0.0),
            x,
            y,
            0.0,
        ));
        brushes.push_str(&translate(
            &pylon(24.0, 110.0, 120.0, 0.0, 2.0, 0.0),
            x,
            y,
            0.0,
        ));
    }
    // Cantilever arms carry the upper slabs from the tall piers. Their underside
    // retains two metres of clear height above the floor, including below joins.
    brushes.push_str(&boxed((-30.0, -60.0, 40.0), (-18.0, 16.0, 44.0)));
    brushes.push_str(&boxed((50.0, -16.0, 40.0), (62.0, 56.0, 44.0)));
    let mut lights = String::new();
    for (x, y) in [(-36.0, 48.0), (66.0, 24.0), (0.0, -64.0)] {
        let (fixture, _) = ceiling_fixture(x, y, LEVEL, 16.0, 4.0);
        brushes.push_str(&fixture);
        let (lx, ly) = turn_plan((x, y), turn);
        lights.push_str(&tile_light(lx, ly, 116.0));
    }
    let mut out =
        String::from("// The Jade Nave: faceted stone piers and exposed upper crossings.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&turn_brushes(&brushes, turn)));
    out.push_str(
        &Meta::cell(
            &format!("authored/{}", STEMS[usize::from(turn)]),
            "jade_nave",
            i32::from(turn),
            1,
            1,
        )
        .with_register_scope("facet_monument")
        .with_rotation_policy("none")
        .emit(),
    );
    out.push_str(&tile_cell_default());
    let face = |offset| (usize::from(turn) + offset) % 6;
    out.push_str(&lateral_port(face(0), "span", "jade_east", 0, 0, 0));
    out.push_str(&lateral_port(face(1), "span", "jade_south_east", 0, 0, 0));
    out.push_str(&lateral_port(face(4), "door", "jade_threshold", 0, 0, 0));
    out.push_str(&lights);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jade_sources_reproduce_and_validate() {
        super::super::assert_reproduces(&builders());
        for (_, build) in builders() {
            let module = crate::parse_authored_module(&build()).expect("valid jade sector");
            assert!(module.prototype.hulls.len() * 3 <= 128);
        }
    }
}
