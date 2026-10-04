//! Sealed indoor rain garden with a continuous dry veranda and opaque shoji screens.
use super::entities::{Meta, lateral_port, tile_cell_default, tile_light, worldspawn};
use super::geometry::{FLOOR_TOP, LEVEL, boxed, door_wall_default, hex_slab, prism, wall};
use super::grid_turn::{turn_brushes, turn_plan};
use super::{Builder, GENERATED_NOTE};

const STEMS: [&str; 6] = [
    "rain_court",
    "rain_court_se",
    "rain_court_sw",
    "rain_court_west",
    "rain_court_nw",
    "rain_court_ne",
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
    // World (x,z) -> TrenchBroom (16x,-16z). All traversable surfaces stay at 0.5m.
    let box_world = |a: (f64, f64, f64), b: (f64, f64, f64)| {
        boxed(
            (a.0 * 16.0, -b.2 * 16.0, a.1 * 16.0),
            (b.0 * 16.0, -a.2 * 16.0, b.1 * 16.0),
        )
    };
    // The screen hides one approach, with generous gaps on both sides. The other
    // sectors repeat at exact lattice headings, giving three distinct handoff bays.
    brushes.push_str(&box_world((-2.0, 0.5, 1.9), (1.0, 3.6, 2.1)));
    for x in [-2.18, 1.02] {
        brushes.push_str(&box_world((x, 0.5, 1.82), (x + 0.16, 4.7, 2.18)));
    }
    // Physical eaves supply shelter below the sealed storey roof. Their inner
    // edge frames the garden; rain is emitted only inside that aperture.
    brushes.push_str(&box_world((-5.8, 4.7, -3.8), (2.5, 4.9, 1.4)));
    brushes.push_str(&box_world((-5.8, 4.7, 1.4), (-0.2, 4.9, 4.3)));
    brushes.push_str(&box_world((2.5, 4.7, -3.8), (7.0, 4.9, -1.35)));
    brushes.push_str(&prism(
        &[
            (-2.5 * 16.0, -4.0 * 16.0),
            (0.0, -7.8 * 16.0),
            (3.0 * 16.0, -(44.0 / 7.0) * 16.0),
            (0.0, -4.0 * 16.0),
        ],
        75.2,
        78.4,
        None,
        0.0,
        0.0,
    ));
    for (x, z) in [(-4.8, -2.9), (-4.8, 3.4), (2.2, -3.65)] {
        brushes.push_str(&box_world((x, 0.5, z), (x + 0.18, 4.7, z + 0.18)));
    }
    // A low faceted stone island in each third. Moss follows these real hulls.
    for (x, z, r, h) in [
        (4.5, 1.8, 0.82, 0.85),
        (3.7, 1.4, 0.42, 0.55),
        (4.9, 2.5, 0.37, 0.40),
    ] {
        let outline: Vec<_> = [
            (1.0, 0.0),
            (0.45, 0.85),
            (-0.7, 0.7),
            (-1.0, -0.25),
            (-0.1, -0.95),
        ]
        .into_iter()
        .map(|(dx, dz)| ((x + dx * r) * 16.0, -(z + dz * r) * 16.0))
        .collect();
        brushes.push_str(&prism(
            &outline,
            FLOOR_TOP,
            (0.5 + h) * 16.0,
            None,
            h * 9.6,
            0.0,
        ));
    }
    // Bench lies against the outer shell, outside the covered circuit's clearance.
    brushes.push_str(&box_world((-6.2, 0.5, -1.3), (-5.65, 0.95, 1.3)));
    let mut lights = String::new();
    for (x, z, y) in [(-3.0, -0.8, 4.4), (0.0, -3.0, 4.4), (5.3, 3.5, 7.2)] {
        let (lx, ly) = turn_plan((x * 16.0, -z * 16.0), turn);
        lights.push_str(&tile_light(lx, ly, y * 16.0));
    }
    let mut out = String::from(
        "// Rain Court: sheltered engawa, opaque screens and a sealed indoor rain aperture.\n",
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&turn_brushes(&brushes, turn)));
    out.push_str(
        &Meta::cell(
            &format!("authored/{}", STEMS[usize::from(turn)]),
            "rain_court",
            i32::from(turn),
            1,
            1,
        )
        .with_register_scope("shadow_screen")
        .with_rotation_policy("none")
        .emit(),
    );
    out.push_str(&tile_cell_default());
    let face = |offset| (usize::from(turn) + offset) % 6;
    out.push_str(&lateral_port(face(0), "span", "rain_east", 0, 0, 0));
    out.push_str(&lateral_port(face(1), "span", "rain_south_east", 0, 0, 0));
    out.push_str(&lateral_port(face(4), "door", "rain_threshold", 0, 0, 0));
    out.push_str(&lights);
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rain_sources_reproduce_and_validate() {
        super::super::assert_reproduces(&builders());
        for (_, build) in builders() {
            let module = crate::parse_authored_module(&build()).expect("valid rain sector");
            assert!(module.prototype.hulls.len() * 3 <= 128);
        }
    }
}
