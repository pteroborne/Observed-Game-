//! Deterministic route constraints injected before each WFC collapse.

use std::collections::{BTreeMap, BTreeSet};

use observed_core::SplitMix;

use super::topology::{face_between, route_between};
use super::{CellCoord, FullWfcConfig, ModuleFace, ModulePlacement, ObservationFrame};

pub(super) fn all_coords(config: FullWfcConfig) -> impl Iterator<Item = CellCoord> {
    (0..config.cell_count()).map(move |index| config.coord(index))
}

pub(super) fn forced_route_edges(
    config: FullWfcConfig,
    observation: &ObservationFrame,
    previous: Option<&BTreeMap<CellCoord, ModulePlacement>>,
    previous_pins: Option<&BTreeSet<CellCoord>>,
    reserved_exit_faces: &BTreeSet<ModuleFace>,
    rng: &mut SplitMix,
) -> BTreeMap<CellCoord, u8> {
    let mut origins = BTreeSet::from([config.spawn()]);
    origins.extend(observation.occupied_cells.values().copied());
    if let Some(pins) = previous_pins {
        origins.extend(pins.iter().copied());
    }

    let mut required = BTreeMap::<CellCoord, u8>::new();
    for origin in origins {
        let path = previous
            .and_then(|placements| {
                route_between(
                    config,
                    placements,
                    origin,
                    config.exit(),
                    reserved_exit_faces,
                    None,
                )
                .map(|route| route.cells)
            })
            .unwrap_or_else(|| randomized_manhattan_path(origin, config.exit(), rng));
        for pair in path.windows(2) {
            let face = face_between(config, pair[0], pair[1]).expect("path cells are adjacent");
            *required.entry(pair[0]).or_default() |= face.bit();
            *required.entry(pair[1]).or_default() |= face.opposite().bit();
        }
    }
    required
}

fn randomized_manhattan_path(
    start: CellCoord,
    goal: CellCoord,
    rng: &mut SplitMix,
) -> Vec<CellCoord> {
    let mut current = start;
    let mut out = vec![current];
    while current != goal {
        let mut choices = Vec::new();
        if current.x < goal.x {
            choices.push(ModuleFace::East);
        } else if current.x > goal.x {
            choices.push(ModuleFace::West);
        }
        if current.z < goal.z {
            choices.push(ModuleFace::South);
        } else if current.z > goal.z {
            choices.push(ModuleFace::North);
        }
        if current.level < goal.level {
            choices.push(ModuleFace::Up);
        } else if current.level > goal.level {
            choices.push(ModuleFace::Down);
        }
        let face = choices[rng.below(choices.len())];
        let (dx, dz, dl) = face.delta();
        current = CellCoord::new(
            (i32::from(current.x) + i32::from(dx)) as u16,
            (i32::from(current.z) + i32::from(dz)) as u16,
            (i16::from(current.level) + i16::from(dl)) as u8,
        );
        out.push(current);
    }
    out
}
