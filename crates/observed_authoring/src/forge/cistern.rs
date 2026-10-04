//! A reservoir sector, repeated three times around one shared pool.
//!
//! The interior faces have no jambs, lintels or trim: two full-height spans join
//! three cells into one chamber. Causeways meet at span midpoints; the shallow
//! bed remains walkable, so a missed causeway is a recoverable wade.

use super::entities::{
    Meta, ceiling_fixture, lateral_port, tile_cell_default, tile_light, worldspawn,
};
use super::geometry::{
    FLOOR_TOP, LEVEL, P2, band, boxed, custom_plane, door_wall_default, flat_plane, fmt, hex_slab,
    prism, pylon, side_plane, translate, wall,
};
use super::{Builder, GENERATED_NOTE};

const SECTORS: [&str; 6] = [
    "cistern_sector",
    "cistern_sector_se",
    "cistern_sector_sw",
    "cistern_sector_west",
    "cistern_sector_nw",
    "cistern_sector_ne",
];

/// Explicit orientations fit the quantized grid exactly. Quaternion rotation
/// leaves a visible gap between the 14×16 m hex's corners at adjoining walls.
#[must_use]
pub fn builders() -> Vec<Builder> {
    vec![
        (SECTORS[0], || sector(0)),
        (SECTORS[1], || sector(1)),
        (SECTORS[2], || sector(2)),
        (SECTORS[3], || sector(3)),
        (SECTORS[4], || sector(4)),
        (SECTORS[5], || sector(5)),
    ]
}

/// Clockwise lattice turn in the editor's x/y plane (y is negative world z).
/// This maps every integer hex corner to the next corner, with no scale drift.
fn turn_plan(mut point: P2, turn: u8) -> P2 {
    for _ in 0..turn % 6 {
        point = (
            point.0 * 0.5 + point.1 * 0.875,
            -point.0 * 6.0 / 7.0 + point.1 * 0.5,
        );
    }
    point
}

/// Transform the three coordinate triples on each forge-emitted brush plane.
/// Point entities are emitted separately in the same oriented frame.
fn turn_brushes(brushes: &str, turn: u8) -> String {
    if turn == 0 {
        return brushes.to_string();
    }
    let mut out = String::new();
    for line in brushes.lines() {
        if line.starts_with("( ") {
            let mut parts: Vec<_> = line.split_whitespace().map(str::to_string).collect();
            for index in [1, 6, 11] {
                let point = (
                    parts[index].parse().expect("forge x coordinate"),
                    parts[index + 1].parse().expect("forge y coordinate"),
                );
                let turned = turn_plan(point, turn);
                parts[index] = fmt(turned.0);
                parts[index + 1] = fmt(turned.1);
            }
            out.push_str(&parts.join(" "));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

fn causeway(from: P2, to: P2, half: f64) -> String {
    let delta = (to.0 - from.0, to.1 - from.1);
    let len = delta.0.hypot(delta.1);
    let n = (-delta.1 / len * half, delta.0 / len * half);
    prism(
        &[
            (from.0 + n.0, from.1 + n.1),
            (to.0 + n.0, to.1 + n.1),
            (to.0 - n.0, to.1 - n.1),
            (from.0 - n.0, from.1 - n.1),
        ],
        FLOOR_TOP,
        12.0,
        None,
        0.0,
        0.0,
    )
}

/// Six convex spandrels form one tiled arch without a concave collision hull.
/// Its piers stand inside the cell, never on the composition's open seams.
fn arcade() -> String {
    let mut brushes = String::new();
    let samples = [-52.0_f64, -45.0, -26.0, 0.0, 26.0, 45.0, 52.0];
    let soffit = |x: f64| 76.0 + (52.0_f64.powi(2) - x * x).max(0.0).sqrt() * 0.65;
    for pair in samples.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let corners = [(a, 28.0), (b, 28.0), (b, 44.0), (a, 44.0)];
        let hint = ((a + b) * 0.5, 36.0);
        let mut brush = String::from("{\n");
        for index in 0..4 {
            brush.push_str(&side_plane(
                corners[index],
                corners[(index + 1) % 4],
                76.0,
                hint,
            ));
        }
        brush.push_str(&flat_plane(124.0, true));
        brush.push_str(&custom_plane(
            (a, 28.0, soffit(a)),
            (b, 28.0, soffit(b)),
            (a, 44.0, soffit(a)),
            false,
        ));
        brush.push_str("}\n");
        brushes.push_str(&brush);
    }
    brushes
}

/// Tiled bath, perimeter gallery, forked dry deck, arcade and aqueduct.
#[must_use]
pub fn sector(turn: u8) -> String {
    let turn = turn % 6;
    let mut brushes = hex_slab(0.0, FLOOR_TOP, 0.0, 0.0);
    brushes.push_str(&hex_slab(LEVEL - 4.0, LEVEL, 0.0, 0.0));
    for face in [2, 3, 5] {
        brushes.push_str(&wall(face, 0.0, LEVEL));
    }
    brushes.push_str(&door_wall_default(4, 0.0, LEVEL));
    // A second dry route hugs the outer boundary of the complete composition.
    // It rises only 25 cm above the canonical threshold, a normal controller step.
    for face in [2, 3, 4, 5] {
        brushes.push_str(&band(face, 8.0, 32.0, FLOOR_TOP, 12.0));
    }
    // Canonical threshold floor at 0.5 m; one 25 cm step onto the deck.
    brushes.push_str(&causeway((-45.0, 78.0), (0.0, 0.0), 18.0));
    brushes.push_str(&causeway((0.0, 0.0), (112.0, 0.0), 18.0));
    // End exactly on the slanted seam, with its cross section tangent to it.
    brushes.push_str(&prism(
        &[(0.0, 18.0), (71.68, -87.04), (40.32, -104.96), (-18.0, 0.0)],
        FLOOR_TOP,
        12.0,
        None,
        0.0,
        0.0,
    ));
    // Broad treads disappear beneath the still water beside the entry deck.
    brushes.push_str(&boxed((-28.0, 18.0, FLOOR_TOP), (12.0, 28.0, 10.0)));
    brushes.push_str(&boxed((-28.0, 28.0, FLOOR_TOP), (12.0, 38.0, 9.0)));
    let mut lights = String::new();
    for (x, y) in [(-62.0, 36.0), (62.0, 36.0), (-62.0, -52.0)] {
        brushes.push_str(&translate(
            &pylon(10.0, FLOOR_TOP, 82.0, 0.0, 1.5, 0.0),
            x,
            y,
            0.0,
        ));
        brushes.push_str(&translate(
            &pylon(17.0, 76.0, 90.0, 0.0, 0.0, 5.0),
            x,
            y,
            0.0,
        ));
        // A recessed fixture per bay. All light energy/colour comes from style.
        let (fixture, _) = ceiling_fixture(x, y + 23.0, LEVEL, 19.0, 5.0);
        brushes.push_str(&fixture);
        let (lx, ly) = turn_plan((x, y + 23.0), turn);
        lights.push_str(&tile_light(lx, ly, LEVEL - 12.0));
    }
    brushes.push_str(&arcade());
    // A visibly open elevated channel enters through the service edge. This is
    // inert architecture, not a hidden hydraulic simulation or usable valve.
    brushes.push_str(&boxed((-104.0, -62.0, 80.0), (88.0, -42.0, 86.0)));
    brushes.push_str(&boxed((-104.0, -62.0, 86.0), (88.0, -58.0, 96.0)));
    brushes.push_str(&boxed((-104.0, -46.0, 86.0), (88.0, -42.0, 96.0)));
    // A low outlet mouth at the western gallery, with room for future controls.
    brushes.push_str(&boxed((-104.0, -18.0, FLOOR_TOP), (-97.0, -14.0, 29.0)));
    brushes.push_str(&boxed((-104.0, 14.0, FLOOR_TOP), (-97.0, 18.0, 29.0)));
    brushes.push_str(&boxed((-104.0, -18.0, 29.0), (-97.0, 18.0, 33.0)));
    let mut out =
        String::from("// The Cistern: continuous bath, tiled arcades and inert aqueducts.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&turn_brushes(&brushes, turn)));
    out.push_str(&Meta::cell(&format!("authored/{}", SECTORS[usize::from(turn)]), "cistern", i32::from(turn), 1, 1)
        .with_register_scope("shadow_screen,monolith,overlit_grid,institutional,facet_monument,megastructure,wellshaft,infinite_gallery,thinning,liminal_grid")
        .with_rotation_policy("none")
        .emit());
    out.push_str(&tile_cell_default());
    let face = |offset| (usize::from(turn) + offset) % 6;
    out.push_str(&lateral_port(face(0), "span", "pool_east", 0, 0, 0));
    out.push_str(&lateral_port(face(1), "span", "pool_south_east", 0, 0, 0));
    out.push_str(&lateral_port(
        face(4),
        "door",
        "reservoir_threshold",
        0,
        0,
        0,
    ));
    out.push_str(&lights);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reservoir_sector_reproduces_and_validates() {
        super::super::assert_reproduces(&builders());
        for turn in 0..6 {
            crate::parse_authored_module(&sector(turn)).expect("reservoir sector validates");
        }
    }
}
