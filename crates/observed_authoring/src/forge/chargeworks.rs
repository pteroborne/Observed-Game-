//! Original industrial wonder: press, turning conveyor and receiving vault.
//! Belts and caged samples are inert architecture. Cargo simulation is deferred.

use super::entities::{
    Meta, ceiling_fixture, lateral_port, tile_cell_default, tile_light, worldspawn,
};
use super::geometry::{
    FLOOR_TOP, LEVEL, P2, band, boxed, door_wall_default, hex_slab, prism, sloped_prism, wall,
};
use super::grid_turn::{turn_brushes, turn_plan};
use super::{Builder, GENERATED_NOTE};

const STEMS: [[&str; 6]; 3] = [
    [
        "chargeworks_fabricator",
        "chargeworks_fabricator_se",
        "chargeworks_fabricator_sw",
        "chargeworks_fabricator_west",
        "chargeworks_fabricator_nw",
        "chargeworks_fabricator_ne",
    ],
    [
        "chargeworks_transfer",
        "chargeworks_transfer_se",
        "chargeworks_transfer_sw",
        "chargeworks_transfer_west",
        "chargeworks_transfer_nw",
        "chargeworks_transfer_ne",
    ],
    [
        "chargeworks_receiver",
        "chargeworks_receiver_se",
        "chargeworks_receiver_sw",
        "chargeworks_receiver_west",
        "chargeworks_receiver_nw",
        "chargeworks_receiver_ne",
    ],
];
const ROLES: [&str; 3] = [
    "chargeworks_fabricator",
    "chargeworks_transfer",
    "chargeworks_receiver",
];

#[must_use]
pub fn builders() -> Vec<Builder> {
    vec![
        (STEMS[0][0], || sector(0, 0)),
        (STEMS[0][1], || sector(0, 1)),
        (STEMS[0][2], || sector(0, 2)),
        (STEMS[0][3], || sector(0, 3)),
        (STEMS[0][4], || sector(0, 4)),
        (STEMS[0][5], || sector(0, 5)),
        (STEMS[1][0], || sector(1, 0)),
        (STEMS[1][1], || sector(1, 1)),
        (STEMS[1][2], || sector(1, 2)),
        (STEMS[1][3], || sector(1, 3)),
        (STEMS[1][4], || sector(1, 4)),
        (STEMS[1][5], || sector(1, 5)),
        (STEMS[2][0], || sector(2, 0)),
        (STEMS[2][1], || sector(2, 1)),
        (STEMS[2][2], || sector(2, 2)),
        (STEMS[2][3], || sector(2, 3)),
        (STEMS[2][4], || sector(2, 4)),
        (STEMS[2][5], || sector(2, 5)),
    ]
}

fn belt(from: P2, to: P2) -> String {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let len = dx.hypot(dy);
    let (nx, ny) = (-dy / len * 14.0, dx / len * 14.0);
    prism(
        &[
            (from.0 + nx, from.1 + ny),
            (to.0 + nx, to.1 + ny),
            (to.0 - nx, to.1 - ny),
            (from.0 - nx, from.1 - ny),
        ],
        FLOOR_TOP,
        12.0,
        None,
        0.0,
        0.0,
    )
}

/// Collision cages leave an inset for presentation's luminous charge cores.
fn canister(x: f64, y: f64) -> String {
    let mut out = boxed((x - 11.0, y - 11.0, 8.0), (x + 11.0, y + 11.0, 13.0));
    out.push_str(&boxed(
        (x - 11.0, y - 11.0, 41.0),
        (x + 11.0, y + 11.0, 46.0),
    ));
    for dx in [-9.0, 7.0] {
        out.push_str(&boxed(
            (x + dx, y - 9.0, 13.0),
            (x + dx + 2.0, y + 9.0, 41.0),
        ));
    }
    out
}

fn machinery(part: usize) -> String {
    let mut b = String::new();
    match part {
        0 => {
            // A six-metre press: a low throat frames the conveyor's source.
            b.push_str(&boxed((-100.0, -30.0, 8.0), (-70.0, 30.0, 100.0)));
            for (a, z) in [(-46.0, 100.0), (30.0, 100.0)] {
                b.push_str(&boxed((-70.0, a, 8.0), (-40.0, a + 16.0, z)));
            }
            b.push_str(&boxed((-70.0, -46.0, 100.0), (-32.0, 46.0, 114.0)));
            b.push_str(&boxed((-68.0, -28.0, 70.0), (-36.0, 28.0, 82.0)));
            b.push_str(&belt((-70.0, 0.0), (112.0, 0.0)));
            for y in [-46.0, 38.0] {
                b.push_str(&prism(
                    &[(-104.0, y), (-80.0, y), (-42.0, y + 8.0), (-104.0, y + 8.0)],
                    8.0,
                    100.0,
                    None,
                    7.0,
                    0.0,
                ));
            }
            for (x, y) in [(-24.0, -64.0), (40.0, -64.0)] {
                b.push_str(&canister(x, y));
            }
        }
        1 => {
            // Belt entering the slanted span and turning toward the receiving cell.
            b.push_str(&prism(
                &[(-14.0, 0.0), (42.0, -104.0), (70.0, -88.0), (14.0, 0.0)],
                8.0,
                12.0,
                None,
                0.0,
                0.0,
            ));
            b.push_str(&belt((0.0, 0.0), (112.0, 0.0)));
            // A climbable service gantry over the belt, with a long shallow ramp.
            b.push_str(&boxed((-64.0, -52.0, 44.0), (88.0, -28.0, 48.0)));
            b.push_str(&sloped_prism(
                &[(-96.0, 62.0), (-64.0, 62.0), (-64.0, -52.0), (-96.0, -52.0)],
                0.0,
                [(-96.0, 62.0, 8.0), (-64.0, 62.0, 8.0), (-96.0, -52.0, 48.0)],
                None,
            ));
            b.push_str(&boxed((-64.0, -54.0, 48.0), (88.0, -50.0, 64.0)));
            for x in [-54.0, 76.0] {
                b.push_str(&boxed((x, -48.0, 8.0), (x + 8.0, -32.0, 44.0)));
            }
            // An overhead track spine gives the transfer hall its own silhouette.
            b.push_str(&boxed((-16.0, -16.0, 105.0), (112.0, 16.0, 114.0)));
            for (x, y) in [(44.0, 48.0), (-32.0, 0.0)] {
                b.push_str(&canister(x, y));
            }
        }
        _ => {
            b.push_str(&prism(
                &[(-14.0, 0.0), (42.0, -104.0), (70.0, -88.0), (14.0, 0.0)],
                8.0,
                12.0,
                None,
                0.0,
                0.0,
            ));
            b.push_str(&belt((-70.0, 0.0), (0.0, 0.0)));
            // A tall recessed intake, framed by splayed armour and a crown.
            b.push_str(&boxed((-98.0, -34.0, 8.0), (-84.0, 34.0, 112.0)));
            for y in [-58.0, 34.0] {
                b.push_str(&prism(
                    &[(-98.0, y), (-52.0, y), (-48.0, y + 24.0), (-98.0, y + 24.0)],
                    8.0,
                    112.0,
                    None,
                    6.0,
                    0.0,
                ));
            }
            b.push_str(&boxed((-98.0, -58.0, 112.0), (-40.0, 58.0, 120.0)));
            b.push_str(&boxed((-84.0, -34.0, 8.0), (-52.0, 34.0, 16.0)));
            for (x, y) in [(32.0, 48.0), (72.0, 32.0)] {
                b.push_str(&canister(x, y));
            }
        }
    }
    b
}

#[must_use]
pub fn sector(part: usize, turn: u8) -> String {
    let turn = turn % 6;
    let mut brushes = hex_slab(0.0, FLOOR_TOP, 0.0, 0.0);
    brushes.push_str(&hex_slab(124.0, LEVEL, 0.0, 0.0));
    for face in [2, 3, 5] {
        brushes.push_str(&wall(face, 0.0, LEVEL));
    }
    brushes.push_str(&door_wall_default(4, 0.0, LEVEL));
    // Heavy base/plinths articulate the exterior, never the open internal seams.
    for face in [2, 3, 5] {
        brushes.push_str(&band(face, 8.0, 16.0, 8.0, 20.0));
    }
    brushes.push_str(&machinery(part));
    let mut lights = String::new();
    for (x, y) in [(-36.0, 48.0), (66.0, 24.0), (0.0, -64.0)] {
        let (fixture, _) = ceiling_fixture(x, y, LEVEL, 16.0, 4.0);
        brushes.push_str(&fixture);
        let (lx, ly) = turn_plan((x, y), turn);
        lights.push_str(&tile_light(lx, ly, 116.0));
    }
    let mut out =
        String::from("// The Chargeworks: fabrication, transfer and intake; static machinery.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&turn_brushes(&brushes, turn)));
    out.push_str(&Meta::cell(&format!("authored/{}",STEMS[part][usize::from(turn)]),ROLES[part],i32::from(turn),1,1)
        .with_register_scope("shadow_screen,monolith,overlit_grid,institutional,facet_monument,megastructure,wellshaft,infinite_gallery,thinning,liminal_grid")
        .with_rotation_policy("none").emit());
    out.push_str(&tile_cell_default());
    let face = |offset| (usize::from(turn) + offset) % 6;
    out.push_str(&lateral_port(face(0), "span", "works_east", 0, 0, 0));
    out.push_str(&lateral_port(face(1), "span", "works_south_east", 0, 0, 0));
    out.push_str(&lateral_port(face(4), "door", "works_threshold", 0, 0, 0));
    out.push_str(&lights);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chargeworks_sources_reproduce_and_validate() {
        super::super::assert_reproduces(&builders());
        for (_, build) in builders() {
            crate::parse_authored_module(&build()).expect("valid chargeworks sector");
        }
    }
}
