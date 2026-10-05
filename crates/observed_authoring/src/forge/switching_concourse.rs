//! Sealed Lumen transit hall: continuous crossing, platform bays and a ribbed canopy.
use super::entities::{Meta, lateral_port, tile_cell_default, tile_light, worldspawn};
use super::geometry::{FLOOR_TOP, LEVEL, boxed, door_wall_default, hex_slab, plane3, prism, wall};
use super::grid_turn::{turn_brushes, turn_plan};
use super::{Builder, GENERATED_NOTE};
const STEMS: [&str; 6] = [
    "switching_concourse",
    "switching_concourse_se",
    "switching_concourse_sw",
    "switching_concourse_west",
    "switching_concourse_nw",
    "switching_concourse_ne",
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
    let box_world = |a: (f64, f64, f64), b: (f64, f64, f64)| {
        boxed(
            (a.0 * 16.0, -b.2 * 16.0, a.1 * 16.0),
            (b.0 * 16.0, -a.2 * 16.0, b.1 * 16.0),
        )
    };
    // Columns live at the perimeter of circulation. Their physical capitals support
    // the canopy; no pillar or platform blocks a full-height internal span.
    for (x, z) in [(-4.0, -3.9), (4.0, -3.9)] {
        let corners = [
            (x - 0.32, z - 0.24),
            (x + 0.32, z - 0.24),
            (x + 0.32, z + 0.24),
            (x - 0.32, z + 0.24),
        ]
        .map(|(px, pz)| (px * 16.0, -pz * 16.0));
        brushes.push_str(&prism(&corners, 8.0, 101.6, None, 0.0, 0.0));
        let cap = [
            (x - 0.68, z - 0.55),
            (x + 0.68, z - 0.55),
            (x + 0.68, z + 0.55),
            (x - 0.68, z + 0.55),
        ]
        .map(|(px, pz)| (px * 16.0, -pz * 16.0));
        brushes.push_str(&prism(&cap, 96.0, 108.0, None, 0.0, 4.0));
    }
    // Five faceted arches fan from a shared header supported by the two piers.
    // Three sloped, convex segments per rib keep the crown below the sealed roof.
    brushes.push_str(&box_world((-4.6, 6.35, -4.22), (4.6, 6.8, -2.48)));
    for x in [-4.0_f64, -2.0, 0.0, 2.0, 4.0] {
        let a = (x, -2.8);
        let b = (x * 0.75 + 1.5, 4.2 - x.abs() * 0.35);
        for (t0, t1, y0, y1) in [
            (0.0, 0.3, 6.65, 7.23),
            (0.3, 0.7, 7.23, 7.23),
            (0.7, 1.0, 7.23, 6.65),
        ] {
            let at = |t: f64, y: f64| (a.0 + (b.0 - a.0) * t, y, a.1 + (b.1 - a.1) * t);
            brushes.push_str(&beam(at(t0, y0), at(t1, y1), 0.20, 0.26));
        }
    }
    // The two arms of the outer header meet every rib endpoint, completing a
    // single supported canopy rather than leaving unrelated bars in mid-air.
    brushes.push_str(&beam((-1.5, 6.62, 2.8), (1.5, 6.62, 4.2), 0.28, 0.34));
    brushes.push_str(&beam((1.5, 6.62, 4.2), (4.5, 6.62, 2.8), 0.28, 0.34));
    // Platform seats and a slim opaque service fin provide a tucked-away route,
    // without treating decorative track strips as pits or a second physics model.
    brushes.push_str(&box_world((-6.25, 0.5, -1.0), (-5.65, 0.95, 1.5)));
    brushes.push_str(&box_world((-6.15, 0.95, 1.3), (-5.85, 1.8, 1.5)));
    brushes.push_str(&box_world((-2.0, 0.5, 2.8), (0.8, 0.95, 3.45)));
    brushes.push_str(&box_world((-2.0, 0.95, 3.25), (0.8, 1.8, 3.45)));
    brushes.push_str(&box_world((-5.4, 0.5, 2.5), (-5.1, 3.4, 4.55)));
    let mut out = String::from(
        "// Switching Concourse: three luminous transit bays around an open crossing.\n",
    );
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&turn_brushes(&brushes, turn)));
    out.push_str(
        &Meta::cell(
            &format!("authored/{}", STEMS[usize::from(turn)]),
            "switching_concourse",
            i32::from(turn),
            1,
            1,
        )
        .with_register_scope("overlit_grid")
        .with_rotation_policy("none")
        .emit(),
    );
    out.push_str(&tile_cell_default());
    let face = |offset| (usize::from(turn) + offset) % 6;
    out.push_str(&lateral_port(face(0), "span", "concourse_east", 0, 0, 0));
    out.push_str(&lateral_port(
        face(1),
        "span",
        "concourse_south_east",
        0,
        0,
        0,
    ));
    out.push_str(&lateral_port(
        face(4),
        "door",
        "concourse_threshold",
        0,
        0,
        0,
    ));
    for (x, z) in [(-2.8, -1.5), (2.8, -0.6), (1.0, 5.3)] {
        let (lx, ly) = turn_plan((x * 16.0, -z * 16.0), turn);
        out.push_str(&tile_light(lx, ly, 120.0));
    }
    out
}
// Emit a sloping rectangular beam from six outward planes. The importer owns
// convex intersection and quantization; presentation consumes those same hulls.
fn beam(a: (f64, f64, f64), b: (f64, f64, f64), width: f64, depth: f64) -> String {
    let dx = b.0 - a.0;
    let dz = b.2 - a.2;
    let len = dx.hypot(dz);
    let nx = -dz / len * width * 0.5;
    let nz = dx / len * width * 0.5;
    let world = [
        (a.0 + nx, a.1, a.2 + nz),
        (a.0 - nx, a.1, a.2 - nz),
        (b.0 - nx, b.1, b.2 - nz),
        (b.0 + nx, b.1, b.2 + nz),
        (a.0 + nx, a.1 + depth, a.2 + nz),
        (a.0 - nx, a.1 + depth, a.2 - nz),
        (b.0 - nx, b.1 + depth, b.2 - nz),
        (b.0 + nx, b.1 + depth, b.2 + nz),
    ];
    let points = world.map(|(x, y, z)| (x * 16.0, -z * 16.0, y * 16.0));
    let hint = (
        (a.0 + b.0) * 8.0,
        -(a.2 + b.2) * 8.0,
        (a.1 + b.1 + depth) * 8.0,
    );
    let mut out = String::from("{\n");
    for [i, j, k] in [
        [0, 1, 2],
        [4, 5, 6],
        [0, 4, 5],
        [1, 5, 6],
        [2, 6, 7],
        [3, 7, 4],
    ] {
        out.push_str(&plane3(points[i], points[j], points[k], hint));
    }
    out.push_str("}\n");
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concourse_sources_reproduce_and_validate_at_every_heading() {
        super::super::assert_reproduces(&builders());
        for (_, build) in builders() {
            let module = crate::parse_authored_module(&build()).expect("valid Lumen sector");
            assert!(module.prototype.hulls.len() * 3 <= 128);
        }
    }
}
