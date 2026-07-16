//! Connectivity and routing over collapsed placements.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use observed_hex::{HexCoord, HexFace};

use super::{HexPlacement, HexSpace, HexWfcConfig};

/// Breadth-first shortest route over open doors, inclusive of both ends.
pub(super) fn route_between(
    config: HexWfcConfig,
    placements: &BTreeMap<HexCoord, HexPlacement>,
    from: HexCoord,
    to: HexCoord,
) -> Option<Vec<HexCoord>> {
    let grid = config.grid();
    if !grid.contains(from) || !grid.contains(to) {
        return None;
    }
    let mut previous: BTreeMap<HexCoord, HexCoord> = BTreeMap::new();
    let mut queue = VecDeque::from([from]);
    let mut seen = BTreeSet::from([from]);
    while let Some(cell) = queue.pop_front() {
        if cell == to {
            let mut path = vec![to];
            let mut walk = to;
            while walk != from {
                walk = previous[&walk];
                path.push(walk);
            }
            path.reverse();
            return Some(path);
        }
        let placement = placements.get(&cell)?;
        for face in HexFace::LATERAL {
            if !placement.is_open(face) {
                continue;
            }
            let Some(next) = grid.neighbor(cell, face) else {
                continue;
            };
            if seen.insert(next) {
                previous.insert(next, cell);
                queue.push_back(next);
            }
        }
    }
    None
}

/// Flood fill over open doors from `start`.
pub(super) fn active_component(
    config: HexWfcConfig,
    placements: &BTreeMap<HexCoord, HexPlacement>,
    start: HexCoord,
) -> BTreeSet<HexCoord> {
    let grid = config.grid();
    let mut component = BTreeSet::new();
    if placements
        .get(&start)
        .is_none_or(|placement| placement.space == HexSpace::Void)
    {
        return component;
    }
    component.insert(start);
    let mut queue = VecDeque::from([start]);
    while let Some(cell) = queue.pop_front() {
        let placement = &placements[&cell];
        for face in HexFace::LATERAL {
            if !placement.is_open(face) {
                continue;
            }
            let Some(next) = grid.neighbor(cell, face) else {
                continue;
            };
            if placements[&next].space != HexSpace::Void && component.insert(next) {
                queue.push_back(next);
            }
        }
    }
    component
}
