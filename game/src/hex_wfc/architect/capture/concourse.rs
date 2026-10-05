//! Eye-level inspection and real-controller routes through the transit hall.
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
            Vec3::new(8.0, 3.2, 4.0),
            "concourse-arrival",
        ),
        1 => (
            0,
            Vec3::new(4.8, 0.5, 2.5),
            Vec3::new(-4.5, 4.0, -1.0),
            "concourse-canopy",
        ),
        2 => (
            1,
            Vec3::new(2.0, 0.5, -1.0),
            Vec3::new(-4.5, 1.5, 3.2),
            "concourse-platform",
        ),
        _ => (
            0,
            Vec3::new(-3.0, 0.5, -4.4),
            Vec3::new(8.0, 3.2, 4.0),
            "concourse-unpowered",
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
    let at = |sector: usize, p| {
        Vec3::from_array(hex_origin(cells[sector])) + turn(p, rotation + sector as u8 * 2)
    };
    let mut path = vec![at(0, Vec3::new(-3.0, 0.5, -4.4))];
    for sector in [0, 2, 1] {
        for (x, z) in [(0.0, -2.8), (0.0, 0.0), (2.0, 1.8), (3.0, 4.7), (1.0, 7.0)] {
            path.push(at(sector, Vec3::new(x, 0.5, z)));
        }
    }
    path.push(at(0, Vec3::new(0.0, 0.5, -2.8)));
    for sector in [0, 1, 2, 0] {
        path.push(at(sector, Vec3::new(0.0, 0.5, 0.0)));
        path.push(at(sector, Vec3::new(7.0, 0.5, 4.0)));
    }
    path.push(at(0, Vec3::new(0.0, 0.5, 0.0)));
    path.push(at(0, Vec3::new(-3.0, 0.5, -4.4)));
    path
}
