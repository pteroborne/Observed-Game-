//! Rain Court evidence poses and physical covered/exposed circuit.
use super::archive::turn;
use bevy::prelude::*;
use observed_hex::{HexCoord, hex_origin};
pub(super) fn portrait(
    slot: u16,
    cells: [HexCoord; 3],
    rotation: u8,
) -> (Vec3, Vec3, &'static str, HexCoord) {
    let (sector, feet, target, name) = match slot {
        0 => (
            0,
            Vec3::new(-3.0, 0.5, -4.4),
            Vec3::new(8.5, 2.5, 4.0),
            "rain-arrival",
        ),
        1 => (
            0,
            Vec3::new(0.0, 0.5, -0.8),
            Vec3::new(8.0, 1.1, 4.0),
            "rain-garden",
        ),
        2 => (
            1,
            Vec3::new(-3.3, 0.5, 0.0),
            Vec3::new(1.0, 2.3, 3.0),
            "rain-veranda",
        ),
        _ => (
            0,
            Vec3::new(-3.0, 0.5, -4.4),
            Vec3::new(8.5, 2.5, 4.0),
            "rain-unpowered",
        ),
    };
    let origin = Vec3::from_array(hex_origin(cells[sector]));
    let heading = rotation + sector as u8 * 2;
    (
        origin + turn(feet, heading),
        origin + turn(target, heading),
        name,
        cells[sector],
    )
}
pub(super) fn route(cells: [HexCoord; 3], rotation: u8) -> Vec<Vec3> {
    let mut path = Vec::new();
    let at = |sector: usize, p| {
        Vec3::from_array(hex_origin(cells[sector])) + turn(p, rotation + sector as u8 * 2)
    };
    path.push(at(0, Vec3::new(-3.0, 0.5, -4.4)));
    // Outer route goes through every screen gap, connecting the dry side of both spans.
    for sector in [0, 2, 1] {
        for (x, z) in [
            (0.0, -3.0),
            (-3.0, -1.0),
            (-3.0, 3.0),
            (0.0, 5.5),
            (1.0, 7.2),
        ] {
            path.push(at(sector, Vec3::new(x, 0.5, z)));
        }
    }
    path.push(at(0, Vec3::new(0.0, 0.5, -3.0)));
    path.push(at(0, Vec3::new(0.0, 0.5, 0.0)));
    // Walk each viewing bay to the central paved crossing through real screen gaps.
    for sector in [0, 2, 1, 0] {
        for (x, z) in [
            (7.0, 4.0),
            (3.0, 3.5),
            (3.0, 0.0),
            (0.0, 0.0),
            (3.0, 0.0),
            (3.0, 3.5),
            (7.0, 4.0),
        ] {
            path.push(at(sector, Vec3::new(x, 0.5, z)));
        }
    }
    for (x, z) in [(3.0, 3.5), (3.0, 0.0), (0.0, 0.0)] {
        path.push(at(0, Vec3::new(x, 0.5, z)));
    }
    path.push(at(0, Vec3::new(-3.0, 0.5, -4.4)));
    path
}
