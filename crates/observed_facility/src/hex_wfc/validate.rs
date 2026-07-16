//! Whole-layout validation for a collapsed hex facility.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use observed_hex::{HexFace, lateral_distance};

use super::constraints::all_coords;
use super::topology::{active_component, route_between};
use super::{HexCoord, HexPlacement, HexSpace, HexWfcConfig};

/// The first invariant a layout violates, or `None` when it is valid. The
/// reason string feeds retry diagnostics (`HexWfcError::RetryBudgetExhausted`).
pub(super) fn layout_failure(
    config: HexWfcConfig,
    placements: &BTreeMap<HexCoord, HexPlacement>,
) -> Option<&'static str> {
    if placements[&config.spawn()].space != HexSpace::Room
        || placements[&config.exit()].space != HexSpace::Room
    {
        return Some("spawn or exit is not a room");
    }
    if route_between(config, placements, config.spawn(), config.exit()).is_none() {
        return Some("no spawn->exit route");
    }
    if active_component(config, placements, config.spawn()).len()
        != placements
            .values()
            .filter(|placement| placement.space != HexSpace::Void)
            .count()
    {
        return Some("disconnected component survived");
    }
    if config.min_room_distance > 1 {
        let rooms: Vec<HexCoord> = placements
            .values()
            .filter(|p| p.space == HexSpace::Room)
            .map(|p| p.coord)
            .collect();
        for i in 0..rooms.len() {
            for j in (i + 1)..rooms.len() {
                if lateral_distance(rooms[i], rooms[j]) < config.min_room_distance {
                    return Some("rooms closer than min_room_distance");
                }
            }
        }
    }
    if !all_edges_match(config, placements) {
        return Some("edge mismatch or open room-room face");
    }
    if !hall_components_valid(config, placements) {
        return Some("hall component with <2 or >4 room endpoints");
    }
    None
}

fn all_edges_match(config: HexWfcConfig, placements: &BTreeMap<HexCoord, HexPlacement>) -> bool {
    let grid = config.grid();
    for coord in all_coords(config) {
        let placement = &placements[&coord];
        if placement.space == HexSpace::Hall && !(2..=4).contains(&placement.doors.count_ones()) {
            return false;
        }
        for face in HexFace::LATERAL {
            let open = placement.is_open(face);
            let Some(neighbor) = grid.neighbor(coord, face) else {
                if open {
                    return false;
                }
                continue;
            };
            let other = &placements[&neighbor];
            if open != other.is_open(face.opposite())
                || (open && placement.space == HexSpace::Room && other.space == HexSpace::Room)
            {
                return false;
            }
        }
    }
    true
}

/// Every connected hall component must attach to two to four rooms.
fn hall_components_valid(
    config: HexWfcConfig,
    placements: &BTreeMap<HexCoord, HexPlacement>,
) -> bool {
    let grid = config.grid();
    let mut unseen = placements
        .iter()
        .filter_map(|(&coord, placement)| (placement.space == HexSpace::Hall).then_some(coord))
        .collect::<BTreeSet<_>>();
    while let Some(start) = unseen.pop_first() {
        let mut queue = VecDeque::from([start]);
        let mut endpoints = BTreeSet::new();
        while let Some(cell) = queue.pop_front() {
            for face in HexFace::LATERAL {
                if !placements[&cell].is_open(face) {
                    continue;
                }
                let Some(next) = grid.neighbor(cell, face) else {
                    continue;
                };
                match placements[&next].space {
                    HexSpace::Hall if unseen.remove(&next) => queue.push_back(next),
                    HexSpace::Room => {
                        endpoints.insert(next);
                    }
                    HexSpace::Void | HexSpace::Hall => {}
                }
            }
        }
        if !(2..=4).contains(&endpoints.len()) {
            return false;
        }
    }
    true
}
