//! Climb compositions: a storey climbed across several cells instead of inside one.
//!
//! # Why several
//!
//! Every single-cell climb rises a full 8 m storey in a hexagon 14 m across, and the
//! first-person captures showed what that costs: the switchback ramp and the spiral
//! stair were both too steep and too narrow to read as anything but an obstacle. Laid
//! across three cells instead, the same storey is a broad flight about 17 degrees at
//! its steepest - something a body walks up rather than negotiates
//! (`docs/climb_compositions_plan.md`).
//!
//! # The four cells
//!
//! Authored climbing east, and turned to every heading by the catalogue:
//!
//! | cell | storey | floor | openings |
//! | --- | --- | --- | --- |
//! | foot | L | an entry pad, then rising 2.1 m | door west, span east |
//! | mid | L | rising a further 2.4 m | spans west and east |
//! | high | L | rising the last 3.5 m to the storey line | span west, open above |
//! | landing | L+1 | the flight's last few centimetres, then a pad | door east |
//!
//! Every floor runs wall to wall, so a flight has no edge to fall from and needs no
//! rail. The rise is back-loaded: the foot and mid keep the ordinary ceiling, and a
//! body's head clears it by about 0.7 m only if the flight is still low there, so the
//! high cell - open to the landing above - takes the steepest stretch.
//!
//! One plain dressing for every register for now. Districts get their own once the
//! mechanism is proven, on the same skeleton.

use super::Builder;
use super::GENERATED_NOTE;
use super::entities::{
    Meta, lateral_port, stair_node, tile_cell, vertical_port, wall_fixture, worldspawn,
};
use super::geometry::{
    FLOOR_TOP, LEVEL, P2, P3, centroid, corners, custom_plane, door_wall_default, flat_plane,
    hex_slab, prism, side_plane, wall,
};

/// The cell's half-width across its east and west faces, in plan `x`.
const APOTHEM: f64 = 112.0;
/// Where the foot's entry pad ends and its flight begins.
const PAD_END: f64 = -80.0;
/// Where the landing's exit pad begins: the flight arrives at the next floor here.
const ARRIVAL: f64 = 80.0;
/// The storey above's floor top, in the high cell's frame.
const NEXT_FLOOR: f64 = LEVEL + FLOOR_TOP;
/// The ordinary ceiling slab's underside.
const CEILING: f64 = LEVEL - FLOOR_TOP;
/// The flight's height where it crosses from the foot into the mid, and from the mid
/// into the high cell.
const FOOT_TOP: f64 = FLOOR_TOP + LOW_RISE * (APOTHEM - PAD_END) / LOW_RUN;
const MID_TOP: f64 = FLOOR_TOP + LOW_RISE;
/// The low two cells climb `LOW_RISE` over `LOW_RUN`; the high cell climbs the rest.
const LOW_RISE: f64 = 72.0;
const LOW_RUN: f64 = (APOTHEM - PAD_END) + 2.0 * APOTHEM;
/// Where the high cell's flight reaches the storey line, in plan `x`.
const STOREY_LINE: f64 = -APOTHEM + (LEVEL - MID_TOP) * HIGH_RUN / HIGH_RISE;
const HIGH_RISE: f64 = NEXT_FLOOR - MID_TOP;
const HIGH_RUN: f64 = ARRIVAL + APOTHEM;
/// The landing's floor starts this far above its base, where it meets the flight: a
/// lip of six centimetres rather than a wedge thinning to nothing, which is a brush
/// no hull can be made of.
const LIP: f64 = 1.0;
/// Faces, as the forge numbers them: east first, counterclockwise.
const EAST: usize = 0;
const WEST: usize = 3;

/// The cell's hexagon clipped to the side of `x = at` that `keep_east` names.
fn clip(at: f64, keep_east: bool) -> Vec<P2> {
    let inside = |p: P2| if keep_east { p.0 >= at } else { p.0 <= at };
    let hex = corners();
    let mut out = Vec::new();
    for index in 0..6 {
        let (a, b) = (hex[index], hex[(index + 1) % 6]);
        if inside(a) {
            out.push(a);
        }
        if inside(a) != inside(b) {
            let t = (at - a.0) / (b.0 - a.0);
            out.push((at, a.1 + (b.1 - a.1) * t));
        }
    }
    out
}

/// A floor brush over `plan` from the cell's base up to the plane rising east through
/// `(x0, z0)` and `(x1, z1)`, capped flat at `cap`.
fn flight(plan: &[P2], (x0, z0): (f64, f64), (x1, z1): (f64, f64), cap: Option<f64>) -> String {
    let hint = centroid(plan);
    let mut out = String::from("{\n");
    for index in 0..plan.len() {
        out.push_str(&side_plane(
            plan[index],
            plan[(index + 1) % plan.len()],
            0.0,
            hint,
        ));
    }
    out.push_str(&flat_plane(0.0, false));
    let top: [P3; 3] = [(x0, -100.0, z0), (x0, 100.0, z0), (x1, 0.0, z1)];
    out.push_str(&custom_plane(top[0], top[1], top[2], true));
    if let Some(cap) = cap {
        out.push_str(&flat_plane(cap, true));
    }
    out.push_str("}\n");
    out
}

/// The walls round a cell: a door on `door`, nothing on the `open` faces, and a
/// sealed wall everywhere else.
fn walls(door: Option<usize>, open: &[usize]) -> String {
    let mut out = String::new();
    for face in 0..6 {
        if Some(face) == door {
            out.push_str(&door_wall_default(face, 0.0, LEVEL));
        } else if !open.contains(&face) {
            out.push_str(&wall(face, 0.0, LEVEL));
        }
    }
    out
}

/// One sconce, centred on a side wall at height `z`.
fn sconce(face: usize, z: f64) -> (String, String) {
    wall_fixture(face, 0.5, z, 20.0)
}

/// The climb line through this cell, in its own frame.
fn spine(nodes: &[(f64, f64)]) -> String {
    let mut out = String::new();
    for (index, &(x, z)) in nodes.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        out.push_str(&stair_node(index as u16, x, 0.0, z));
    }
    out
}

/// Every climb cell's file: geometry, then its tile entities.
fn tile(
    stem: &str,
    archetype: &str,
    title: &str,
    brushes: String,
    light: (String, String),
    entities: &str,
) -> String {
    let (fixture, source) = light;
    let mut out = format!("// Climb composition, {title}.\n");
    out.push_str(GENERATED_NOTE);
    out.push_str(&worldspawn(&(brushes + &fixture)));
    out.push_str(
        &Meta::cell(&format!("authored/{stem}"), archetype, 0, 1, 10)
            .with_register_scope(&crate::tile_source::REGISTERS.join(","))
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 1, "flight"));
    out.push_str(entities);
    out.push_str(&source);
    out
}

#[must_use]
pub fn climb_foot() -> String {
    let mut brushes = String::from("// The entry pad\n");
    brushes.push_str(&prism(
        &clip(PAD_END, false),
        0.0,
        FLOOR_TOP,
        None,
        2.0,
        0.0,
    ));
    brushes.push_str("// The flight, rising east\n");
    brushes.push_str(&flight(
        &clip(PAD_END, true),
        (PAD_END, FLOOR_TOP),
        (APOTHEM, FOOT_TOP),
        None,
    ));
    brushes.push_str(&hex_slab(CEILING, LEVEL, 0.0, 3.0));
    brushes.push_str(&walls(Some(WEST), &[EAST]));
    let mut entities = lateral_port(WEST, "door", "entry", 0, 0, 0);
    entities.push_str(&lateral_port(EAST, "span", "flight_on", 0, 0, 0));
    entities.push_str(&spine(&[
        (-100.0, FLOOR_TOP),
        (PAD_END, FLOOR_TOP),
        (APOTHEM, FOOT_TOP),
    ]));
    tile(
        "climb_foot",
        "climb_foot",
        "the foot: in from the west, and the flight begins",
        brushes,
        sconce(1, 88.0),
        &entities,
    )
}

#[must_use]
pub fn climb_mid() -> String {
    let mut brushes = String::from("// The flight, carried across\n");
    brushes.push_str(&flight(
        &corners(),
        (-APOTHEM, FOOT_TOP),
        (APOTHEM, MID_TOP),
        None,
    ));
    brushes.push_str(&hex_slab(CEILING, LEVEL, 0.0, 3.0));
    brushes.push_str(&walls(None, &[EAST, WEST]));
    let mut entities = lateral_port(WEST, "span", "flight_back", 0, 0, 0);
    entities.push_str(&lateral_port(EAST, "span", "flight_on", 0, 0, 0));
    entities.push_str(&spine(&[(-APOTHEM, FOOT_TOP), (APOTHEM, MID_TOP)]));
    tile(
        "climb_mid",
        "climb_mid",
        "the middle of the flight",
        brushes,
        sconce(4, 100.0),
        &entities,
    )
}

#[must_use]
pub fn climb_high() -> String {
    let mut brushes =
        String::from("// The flight's steepest stretch, to the storey line, open above\n");
    brushes.push_str(&flight(
        &corners(),
        (-APOTHEM, MID_TOP),
        (ARRIVAL, NEXT_FLOOR),
        Some(LEVEL),
    ));
    brushes.push_str(&walls(None, &[WEST]));
    let mut entities = lateral_port(WEST, "span", "flight_back", 0, 0, 0);
    entities.push_str(&vertical_port("up", "ramp_open", "landing", 0));
    entities.push_str(&spine(&[(-APOTHEM, MID_TOP), (STOREY_LINE, LEVEL)]));
    tile(
        "climb_high",
        "climb_high",
        "the flight's last stretch, open to the landing above",
        brushes,
        sconce(2, 112.0),
        &entities,
    )
}

#[must_use]
pub fn climb_landing() -> String {
    let mut brushes = String::from("// The flight's last few centimetres, and the exit pad\n");
    brushes.push_str(&flight(
        &clip(STOREY_LINE, true),
        (STOREY_LINE, LIP),
        (ARRIVAL, FLOOR_TOP),
        Some(FLOOR_TOP),
    ));
    brushes.push_str(&hex_slab(CEILING, LEVEL, 0.0, 3.0));
    brushes.push_str(&walls(Some(EAST), &[]));
    let mut entities = vertical_port("down", "ramp_open", "flight", 0);
    entities.push_str(&lateral_port(EAST, "door", "exit", 0, 0, 0));
    entities.push_str(&spine(&[
        (STOREY_LINE, LIP),
        (ARRIVAL, FLOOR_TOP),
        (100.0, FLOOR_TOP),
    ]));
    tile(
        "climb_landing",
        "climb_landing",
        "the landing: the flight arrives, and out to the east",
        brushes,
        sconce(2, 40.0),
        &entities,
    )
}

#[must_use]
pub fn builders() -> Vec<Builder> {
    vec![
        ("climb_foot", climb_foot as fn() -> String),
        ("climb_mid", climb_mid),
        ("climb_high", climb_high),
        ("climb_landing", climb_landing),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::FLIGHT_MAX_SLOPE;

    #[test]
    fn every_climb_cell_reproduces_its_committed_file() {
        super::super::assert_reproduces(&builders());
    }

    #[test]
    fn the_flight_rises_a_storey_across_the_composition() {
        assert!((MID_TOP + HIGH_RISE - NEXT_FLOOR).abs() < 1e-9);
        assert!((NEXT_FLOOR - FLOOR_TOP - LEVEL).abs() < 1e-9);
    }

    #[test]
    fn no_stretch_of_the_flight_is_too_steep() {
        let low = LOW_RISE / LOW_RUN;
        let high = HIGH_RISE / HIGH_RUN;
        let limit = f64::from(FLIGHT_MAX_SLOPE);
        assert!(
            low <= limit && high <= limit,
            "low {low:.3}, high {high:.3}"
        );
    }

    /// The low cells keep the ordinary ceiling, so the flight under it must leave a
    /// body's head clear: 1.8 m tall, with at least 0.5 m to spare.
    #[test]
    fn a_body_clears_the_ceiling_wherever_there_is_one() {
        let body = 1.8 * 16.0;
        let spare = CEILING - (MID_TOP + body);
        assert!(spare >= 0.5 * 16.0, "{:.2} m to spare", spare / 16.0);
    }

    /// Checked at compile time: the storey line falls inside the cell, before the pad.
    const _: () = assert!(STOREY_LINE > -APOTHEM && STOREY_LINE < ARRIVAL);
    /// Checked at compile time: the step from flight to landing is too small to notice.
    const _: () = assert!(LIP < 2.0, "a lip a body would notice");
}
