//! Connectivity and routing over collapsed placements.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};

use observed_hex::{HexCoord, HexFace, PortClass};

use super::{HexPlacement, HexSpace, HexWfcConfig};

#[derive(Copy, Clone, Eq, PartialEq)]
struct AStarNode {
    coord: HexCoord,
    f_score: u32,
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap, reverse comparison to get min-heap
        other
            .f_score
            .cmp(&self.f_score)
            .then_with(|| self.coord.cmp(&other.coord))
    }
}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn is_connection_open(
    a: &HexPlacement,
    face: HexFace,
    placements: &BTreeMap<HexCoord, HexPlacement>,
    grid: observed_hex::HexGridSize,
) -> bool {
    let Some(neighbor_coord) = grid.neighbor(a.coord, face) else {
        return false;
    };
    let Some(b) = placements.get(&neighbor_coord) else {
        return false;
    };
    if face.is_lateral() {
        a.is_open(face) && b.is_open(face.opposite())
    } else {
        let a_class = if face == HexFace::Up { a.up } else { a.down };
        let b_class = if face == HexFace::Up { b.down } else { b.up };
        a_class != PortClass::Sealed && a_class == b_class
    }
}

fn connection_cost(
    a: &HexPlacement,
    face: HexFace,
    placements: &BTreeMap<HexCoord, HexPlacement>,
    grid: observed_hex::HexGridSize,
) -> Option<u32> {
    if !is_connection_open(a, face, placements, grid) {
        return None;
    }
    if face.is_lateral() {
        Some(10) // COST_NORMAL
    } else {
        let a_class = if face == HexFace::Up { a.up } else { a.down };
        match a_class {
            PortClass::RampOpen => Some(20),  // COST_RAMP (Climb tier)
            PortClass::ShaftOpen => Some(50), // COST_SHAFT (Shaft tier)
            _ => None,
        }
    }
}

/// A* shortest route over open doors and vertical channels, weighted by cost tiers.
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

    let mut g_score: BTreeMap<HexCoord, u32> = BTreeMap::new();
    let mut previous: BTreeMap<HexCoord, HexCoord> = BTreeMap::new();
    let mut open_set = BinaryHeap::new();

    g_score.insert(from, 0);
    open_set.push(AStarNode {
        coord: from,
        f_score: observed_hex::travel_distance(from, to) * 10,
    });

    while let Some(AStarNode { coord, f_score: _ }) = open_set.pop() {
        if coord == to {
            let mut path = vec![to];
            let mut walk = to;
            while walk != from {
                walk = previous[&walk];
                path.push(walk);
            }
            path.reverse();
            return Some(path);
        }

        let current_g = g_score[&coord];
        let Some(placement) = placements.get(&coord) else {
            continue;
        };

        for face in HexFace::ALL {
            let Some(cost) = connection_cost(placement, face, placements, grid) else {
                continue;
            };
            let Some(next) = grid.neighbor(coord, face) else {
                continue;
            };

            let tentative_g = current_g + cost;
            let current_next_g = g_score.get(&next).copied().unwrap_or(u32::MAX);

            if tentative_g < current_next_g {
                previous.insert(next, coord);
                g_score.insert(next, tentative_g);
                let f = tentative_g + observed_hex::travel_distance(next, to) * 10;
                open_set.push(AStarNode {
                    coord: next,
                    f_score: f,
                });
            }
        }
    }

    None
}

/// Flood fill over open doors and vertical ports from `start`.
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
        for face in HexFace::ALL {
            if !is_connection_open(placement, face, placements, grid) {
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
