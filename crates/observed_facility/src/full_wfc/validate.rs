//! Whole-layout validation for collapsed placements.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::constraints::all_coords;
use super::topology::{active_component, route_between};
use super::{CellCoord, FullWfcConfig, ModuleFace, ModulePlacement, ModuleSpace, ObservationFrame};

pub(super) fn layout_valid(
    config: FullWfcConfig,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
    observation: &ObservationFrame,
    reserved_exit_faces: &BTreeSet<ModuleFace>,
) -> bool {
    if placements[&config.spawn()].space != ModuleSpace::Room
        || placements[&config.exit()].space != ModuleSpace::Room
        || route_between(
            config,
            placements,
            config.spawn(),
            config.exit(),
            reserved_exit_faces,
            None,
        )
        .is_none()
    {
        return false;
    }
    if active_component(config, placements, config.spawn()).len()
        != placements
            .values()
            .filter(|placement| placement.space != ModuleSpace::Void)
            .count()
    {
        return false;
    }
    for (&player, &coord) in &observation.occupied_cells {
        if route_between(
            config,
            placements,
            coord,
            config.exit(),
            reserved_exit_faces,
            None,
        )
        .is_none()
        {
            let _ = player;
            return false;
        }
    }
    for &coord in &observation.visible_cells {
        if route_between(
            config,
            placements,
            coord,
            config.exit(),
            reserved_exit_faces,
            None,
        )
        .is_none()
        {
            return false;
        }
    }
    // Enforce minimum Manhattan distance between any two rooms
    if config.min_room_distance > 1 {
        let room_coords: Vec<CellCoord> = placements
            .values()
            .filter(|p| p.space == ModuleSpace::Room)
            .map(|p| p.coord)
            .collect();
        for i in 0..room_coords.len() {
            for j in (i + 1)..room_coords.len() {
                let a = room_coords[i];
                let b = room_coords[j];
                let dist = (a.x as i32 - b.x as i32).abs()
                    + (a.level as i32 - b.level as i32).abs()
                    + (a.z as i32 - b.z as i32).abs();
                if dist < config.min_room_distance as i32 {
                    return false;
                }
            }
        }
    }

    all_edges_match(config, placements) && hall_components_valid(config, placements)
}

fn all_edges_match(
    config: FullWfcConfig,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
) -> bool {
    for coord in all_coords(config) {
        let placement = &placements[&coord];
        if placement.space == ModuleSpace::Hall
            && !(2..=4).contains(&placement.openings.count_ones())
        {
            return false;
        }
        for face in ModuleFace::ALL {
            let open = placement.is_open(face);
            let Some(neighbor) = config.neighbor(coord, face) else {
                if open {
                    return false;
                }
                continue;
            };
            let other = &placements[&neighbor];
            if open != other.is_open(face.opposite())
                || (open
                    && placement.space == ModuleSpace::Room
                    && other.space == ModuleSpace::Room)
            {
                return false;
            }
        }
    }
    true
}

fn hall_components_valid(
    config: FullWfcConfig,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
) -> bool {
    let mut unseen = placements
        .iter()
        .filter_map(|(&coord, placement)| (placement.space == ModuleSpace::Hall).then_some(coord))
        .collect::<BTreeSet<_>>();
    while let Some(start) = unseen.pop_first() {
        let mut queue = VecDeque::from([start]);
        let mut endpoints = BTreeSet::new();
        while let Some(cell) = queue.pop_front() {
            for face in ModuleFace::ALL {
                if !placements[&cell].is_open(face) {
                    continue;
                }
                let Some(next) = config.neighbor(cell, face) else {
                    continue;
                };
                match placements[&next].space {
                    ModuleSpace::Hall if unseen.remove(&next) => queue.push_back(next),
                    ModuleSpace::Room => {
                        endpoints.insert(next);
                    }
                    ModuleSpace::Void | ModuleSpace::Hall => {}
                }
            }
        }
        if !(2..=4).contains(&endpoints.len()) {
            return false;
        }
    }
    true
}
