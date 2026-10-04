//! `OBSERVED2_CAPTURE_HEX_WFC_SURFACES`: what each floor is made of.
//!
//! Evidence for the surface materials. One pose a floor, from the ground up: standing
//! in a hall or room of that floor with no open edge, facing a sealed wall from across
//! the cell, eyes level, so the wall, the floor and the ceiling are all in frame and
//! nothing outside is. Chosen deterministically, so a before and an after are the
//! same places.
use observed_facility::hex_wfc::{HexArchetype, HexCoord, HexFace, HexSpace, HexWfcWorld};
use observed_match::hex_wfc::open_edges;

use super::{Stage, VistaPose, face_dir, pose};
use bevy::prelude::*;

pub(in crate::hex_wfc) fn poses(world: &HexWfcWorld) -> Vec<VistaPose> {
    if std::env::var_os("OBSERVED2_LIBRARY_PORTRAITS").is_some() {
        return library_poses(world);
    }
    if std::env::var_os("OBSERVED2_ZEN_PORTRAITS").is_some() {
        return zen_poses(world);
    }
    const NAMES: [&str; 8] = [
        "floor_1", "floor_2", "floor_3", "floor_4", "floor_5", "floor_6", "floor_7", "floor_8",
    ];
    let grid = world.config.grid();
    let built = |cell: Option<HexCoord>| {
        cell.and_then(|cell| world.placements.get(&cell))
            .is_some_and(|placement| placement.space.built())
    };
    (0..world.config.levels.min(8))
        .filter_map(|level| {
            let (at, face) = world
                .placements
                .values()
                .filter(|placement| {
                    placement.coord.level == level
                        && matches!(placement.space, HexSpace::Hall | HexSpace::Room)
                        // A hall or a room of the floor's own, not a climb through it.
                        && !matches!(
                            placement.archetype,
                            HexArchetype::Climb { .. }
                        )
                        && open_edges(world, placement.coord).is_none()
                        && built(grid.neighbor(placement.coord, HexFace::Up))
                })
                .find_map(|placement| {
                    let face = HexFace::LATERAL
                        .into_iter()
                        .find(|&face| !placement.is_open(face))?;
                    // Somewhere to stand: the face opposite the wall, open or not,
                    // is the far side of the cell from it.
                    Some((placement.coord, face))
                })?;
            Some(pose(NAMES[usize::from(level)], at, face, 6.2, 0.02))
        })
        .collect()
}

/// Ordinary production tiles, with no staged props or altered source geometry.
fn library_poses(world: &HexWfcWorld) -> Vec<VistaPose> {
    use observed_content::ArchitectureRegister;
    let mut out = Vec::new();
    for (name, archetype, back) in [
        ("babel_straight", HexArchetype::Straight, 6.2),
        ("babel_corner", HexArchetype::Corner, 5.4),
        ("babel_room", HexArchetype::Room, 6.2),
        ("babel_junction", HexArchetype::Junction, 6.2),
    ] {
        if let Some((at, face)) = world.placements.values().find_map(|p| {
            (world.architecture.get(&p.coord) == Some(&ArchitectureRegister::InfiniteGallery)
                && p.archetype == archetype
                && (archetype == HexArchetype::Room || open_edges(world, p.coord).is_none()))
            .then(|| HexFace::LATERAL.into_iter().find(|&face| !p.is_open(face)))
            .flatten()
            .map(|face| (p.coord, face))
        }) {
            out.push(pose(name, at, face, back, 0.10));
        }
    }
    out
}

/// Deterministic ordinary Zen halls and a climb, without staging a wonder.
fn zen_poses(world: &HexWfcWorld) -> Vec<VistaPose> {
    use observed_content::ArchitectureRegister;
    if std::env::var("OBSERVED2_ZEN_PORTRAITS").as_deref() == Ok("climb") {
        return super::verticals::poses(world)
            .into_iter()
            .filter(|p| p.name == "zen_up")
            .collect();
    }
    let mut out = Vec::new();
    for (name, kind) in [
        ("zen_straight", 0),
        ("zen_corner", 1),
        ("zen_room", 2),
        ("zen_junction", 3),
    ] {
        if let Some((at, face)) = world.placements.values().find_map(|p| {
            let suitable = match kind {
                0 => p.archetype == HexArchetype::Straight,
                1 => p.archetype == HexArchetype::Corner,
                2 => p.archetype == HexArchetype::Room,
                _ => p.archetype == HexArchetype::Junction,
            };
            (world.architecture.get(&p.coord) == Some(&ArchitectureRegister::ShadowScreen)
                && suitable
                && open_edges(world, p.coord).is_none())
            .then(|| HexFace::LATERAL.into_iter().find(|&face| p.is_open(face)))
            .flatten()
            .map(|face| (p.coord, face))
        }) {
            let outward = face_dir(face);
            let feet = Vec3::from_array(observed_hex::hex_origin(at))
                + Vec3::new(
                    outward.x * 5.4,
                    observed_hex::FLOOR_SLAB_TOP,
                    outward.y * 5.4,
                );
            let look = (-outward).rotate(Vec2::from_angle(0.20));
            out.push(VistaPose {
                name,
                cell: at,
                feet,
                yaw: look.x.atan2(-look.y),
                pitch: 0.10,
                stage: Stage::Nothing,
            });
        }
    }
    out
}
