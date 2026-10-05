//! Static inspection poses and an actual-controller tour of the open Sky bridges.
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
            Vec3::new(-3.2, 0.5, -5.4),
            Vec3::new(10.0, 3.0, 3.0),
            "promenade-arrival",
        ),
        1 => (
            0,
            Vec3::new(4.0, 2.5, 0.0),
            Vec3::new(8.0, -2.0, 6.0),
            "promenade-open-well",
        ),
        2 => (
            0,
            Vec3::new(-1.0, 2.5, 0.5),
            Vec3::new(10.0, 2.7, 5.5),
            "promenade-gathering-landing",
        ),
        _ => (
            0,
            Vec3::new(0.0, 2.5, 0.0),
            Vec3::new(12.0, 4.6, 0.0),
            "promenade-unpowered",
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
    let centers = cells.map(|c| Vec3::from_array(hex_origin(c)) + Vec3::Y * 2.5);
    let entry = Vec3::from_array(hex_origin(cells[0])) + turn(Vec3::new(-3.2, 0.5, -5.4), rotation);
    vec![
        entry, centers[0], centers[1], centers[2], centers[0], centers[2], centers[1], centers[0],
        entry,
    ]
}
