//! Evidence poses and the elevated circuit of the physically committed Archive Well.
use bevy::prelude::*;
use observed_hex::{HexCoord, hex_origin};

pub(super) fn turn(mut p: Vec3, rotation: u8) -> Vec3 {
    for _ in 0..rotation % 6 {
        p = Vec3::new(0.5 * p.x - 0.875 * p.z, p.y, 6.0 / 7.0 * p.x + 0.5 * p.z);
    }
    p
}
pub(super) fn portrait(
    slot: u16,
    cells: [HexCoord; 3],
    rotation: u8,
) -> (Vec3, Vec3, &'static str, HexCoord) {
    let (sector, feet, target, name) = match slot {
        0 => (
            0,
            Vec3::new(-3.0, 0.5, -4.4),
            Vec3::new(11.0, 3.5, 4.5),
            "archive-arrival",
        ),
        1 => (
            0,
            Vec3::new(0.0, 0.5, -2.1),
            Vec3::new(-4.0, 4.3, 3.8),
            "archive-shelves",
        ),
        2 => (
            1,
            Vec3::new(-3.5, 3.0, 0.0),
            Vec3::new(8.0, 2.0, 4.0),
            "archive-reading-perch",
        ),
        _ => (
            0,
            Vec3::new(1.0, 3.0, 0.0),
            Vec3::new(10.0, 2.8, 5.0),
            "archive-bridge",
        ),
    };
    let origin = Vec3::from_array(hex_origin(cells[sector]));
    let bearing = rotation + sector as u8 * 2;
    (
        origin + turn(feet, bearing),
        origin + turn(target, bearing),
        name,
        cells[sector],
    )
}
pub(super) fn route(cells: [HexCoord; 3], rotation: u8) -> Vec<Vec3> {
    let centers = cells.map(|c| Vec3::from_array(hex_origin(c)) + Vec3::Y * 0.5);
    let origin = Vec3::from_array(hex_origin(cells[0]));
    let at = |p| origin + turn(p, rotation);
    vec![
        at(Vec3::new(-3.0, 0.5, -4.4)),
        centers[0],
        centers[1],
        centers[2],
        centers[0],
        at(Vec3::new(-3.5, 0.5, -2.0)),
        at(Vec3::new(-4.9, 0.5, -3.7)),
        at(Vec3::new(-4.9, 3.0, 2.5)),
        at(Vec3::new(-3.5, 3.0, 2.5)),
        at(Vec3::new(-3.5, 3.0, 0.0)),
        centers[0] + Vec3::Y * 2.5,
        centers[1] + Vec3::Y * 2.5,
        centers[2] + Vec3::Y * 2.5,
        centers[0] + Vec3::Y * 2.5,
        at(Vec3::new(-3.5, 3.0, 0.0)),
        at(Vec3::new(-3.5, 3.0, 2.5)),
        at(Vec3::new(-4.9, 3.0, 2.5)),
        at(Vec3::new(-4.9, 0.5, -3.7)),
        at(Vec3::new(-3.5, 0.5, -2.0)),
        at(Vec3::new(-3.0, 0.5, -4.4)),
    ]
}
