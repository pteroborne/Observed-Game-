//! `OBSERVED2_CAPTURE_HEX_WFC_SURFACES`: what each floor is made of.
//!
//! Evidence for the surface materials. One pose a floor, from the ground up: standing
//! in a hall or room of that floor with no open edge, facing a sealed wall from across
//! the cell, eyes level, so the wall, the floor and the ceiling are all in frame and
//! nothing outside is. Chosen deterministically, so a before and an after are the
//! same places.
use observed_facility::hex_wfc::{HexArchetype, HexCoord, HexFace, HexSpace, HexWfcWorld};
use observed_match::hex_wfc::open_edges;

use super::{VistaPose, pose};

pub(in crate::hex_wfc) fn poses(world: &HexWfcWorld) -> Vec<VistaPose> {
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
                            HexArchetype::Shaft
                                | HexArchetype::RampUp
                                | HexArchetype::RampHead
                                | HexArchetype::Climb { .. }
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
