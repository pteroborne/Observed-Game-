//! Observation-chain pinning, relayout safety, connectivity, and weighted routing.

use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};

use super::{
    CellCoord, FullWfcConfig, ModuleFace, ModulePlacement, ModuleSpace, ObservationFrame, Route,
    ThresholdKey,
};

pub(super) fn pinned_cells(
    config: FullWfcConfig,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
    observation: &ObservationFrame,
) -> BTreeSet<CellCoord> {
    let mut pins = BTreeSet::from([config.spawn(), config.exit()]);
    pins.extend(observation.visible_cells.iter().copied());
    pins.extend(observation.occupied_cells.values().copied());
    for &threshold in &observation.visible_thresholds {
        pins.insert(threshold.room);
        pins.extend(threshold_destination_chain(config, placements, threshold));
    }
    pins
}

fn threshold_destination_chain(
    config: FullWfcConfig,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
    threshold: ThresholdKey,
) -> BTreeSet<CellCoord> {
    threshold_destination_path(config, placements, threshold)
        .into_iter()
        .collect()
}

pub(super) fn threshold_destination_path(
    config: FullWfcConfig,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
    threshold: ThresholdKey,
) -> Vec<CellCoord> {
    let mut out = BTreeSet::new();
    let Some(room) = placements.get(&threshold.room) else {
        return Vec::new();
    };
    if room.space != ModuleSpace::Room || !room.is_open(threshold.face) {
        return Vec::new();
    }
    let Some(mut current) = config.neighbor(threshold.room, threshold.face) else {
        return Vec::new();
    };
    let mut previous = threshold.room;
    while out.insert(current) {
        let placement = &placements[&current];
        if placement.space == ModuleSpace::Room {
            break;
        }
        if placement.space != ModuleSpace::Hall {
            break;
        }
        let Some(next) = ModuleFace::ALL.iter().find_map(|&face| {
            if !placement.is_open(face) {
                return None;
            }
            config
                .neighbor(current, face)
                .filter(|&candidate| candidate != previous)
        }) else {
            break;
        };
        previous = current;
        current = next;
    }
    let mut ordered = Vec::with_capacity(out.len());
    let Some(mut current) = config.neighbor(threshold.room, threshold.face) else {
        return ordered;
    };
    let mut previous = threshold.room;
    while out.contains(&current) {
        ordered.push(current);
        let placement = &placements[&current];
        if placement.space == ModuleSpace::Room {
            break;
        }
        let Some(next) = ModuleFace::ALL.iter().find_map(|&face| {
            placement
                .is_open(face)
                .then(|| config.neighbor(current, face))
                .flatten()
                .filter(|&candidate| candidate != previous)
        }) else {
            break;
        };
        previous = current;
        current = next;
    }
    ordered
}

pub(super) fn change_is_safe(
    config: FullWfcConfig,
    before: &ModulePlacement,
    after: &ModulePlacement,
    latest: &ObservationFrame,
) -> bool {
    let occupied = latest
        .occupied_cells
        .values()
        .any(|&coord| coord == before.coord);
    let visible = latest.visible_cells.contains(&before.coord);
    if before.geometry_identity() != after.geometry_identity() {
        return !occupied && !visible;
    }
    if before.openings == after.openings {
        return true;
    }
    let changed = before.openings ^ after.openings;
    ModuleFace::ALL.iter().all(|&face| {
        changed & face.bit() == 0
            || !latest.visible_thresholds.contains(&ThresholdKey {
                room: before.coord,
                face,
            })
            || config.neighbor(before.coord, face).is_none()
    })
}

pub(super) fn active_component(
    config: FullWfcConfig,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
    start: CellCoord,
) -> BTreeSet<CellCoord> {
    if placements
        .get(&start)
        .is_none_or(|placement| placement.space == ModuleSpace::Void)
    {
        return BTreeSet::new();
    }
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(coord) = queue.pop_front() {
        for (next, _) in live_neighbors(config, placements, coord, &BTreeSet::new(), None) {
            if seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    seen
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpenNode {
    estimated: u32,
    cost: u32,
    coord: CellCoord,
}

impl Ord for OpenNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.estimated
            .cmp(&other.estimated)
            .then_with(|| self.cost.cmp(&other.cost))
            .then_with(|| self.coord.cmp(&other.coord))
    }
}

impl PartialOrd for OpenNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(super) fn route_between(
    config: FullWfcConfig,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
    start: CellCoord,
    goal: CellCoord,
    reserved_exit_faces: &BTreeSet<ModuleFace>,
    allowed: Option<&BTreeSet<CellCoord>>,
) -> Option<Route> {
    if placements.get(&start)?.space == ModuleSpace::Void
        || placements.get(&goal)?.space == ModuleSpace::Void
        || allowed.is_some_and(|allowed| !allowed.contains(&start) || !allowed.contains(&goal))
    {
        return None;
    }
    if start == goal {
        return Some(Route {
            cells: vec![start],
            cost_millis: 0,
        });
    }

    let mut open = BinaryHeap::new();
    let mut cost = BTreeMap::from([(start, 0u32)]);
    let mut previous = BTreeMap::new();
    open.push(Reverse(OpenNode {
        estimated: heuristic(start, goal),
        cost: 0,
        coord: start,
    }));

    while let Some(Reverse(node)) = open.pop() {
        if node.coord == goal {
            let mut cells = vec![goal];
            let mut current = goal;
            while current != start {
                current = previous[&current];
                cells.push(current);
            }
            cells.reverse();
            return Some(Route {
                cells,
                cost_millis: node.cost,
            });
        }
        if cost
            .get(&node.coord)
            .is_some_and(|&known| node.cost > known)
        {
            continue;
        }
        for (next, step) in
            live_neighbors(config, placements, node.coord, reserved_exit_faces, allowed)
        {
            let next_cost = node.cost.saturating_add(step);
            if cost.get(&next).is_none_or(|&known| next_cost < known) {
                cost.insert(next, next_cost);
                previous.insert(next, node.coord);
                open.push(Reverse(OpenNode {
                    estimated: next_cost.saturating_add(heuristic(next, goal)),
                    cost: next_cost,
                    coord: next,
                }));
            }
        }
    }
    None
}

pub(super) fn exit_reservation_preserves_routes(
    config: FullWfcConfig,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
    origins: &BTreeSet<CellCoord>,
    claimed_face: ModuleFace,
) -> bool {
    let exit = config.exit();
    let reserved = ModuleFace::ALL
        .into_iter()
        .filter(|&face| placements[&exit].is_open(face) && face != claimed_face)
        .collect();
    origins
        .iter()
        .all(|&origin| route_between(config, placements, origin, exit, &reserved, None).is_some())
}

fn live_neighbors(
    config: FullWfcConfig,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
    coord: CellCoord,
    reserved_exit_faces: &BTreeSet<ModuleFace>,
    allowed: Option<&BTreeSet<CellCoord>>,
) -> Vec<(CellCoord, u32)> {
    let placement = &placements[&coord];
    ModuleFace::ALL
        .iter()
        .filter_map(|&face| {
            if !placement.is_open(face) {
                return None;
            }
            let next = config.neighbor(coord, face)?;
            if allowed.is_some_and(|allowed| !allowed.contains(&next)) {
                return None;
            }
            let other = &placements[&next];
            if !other.is_open(face.opposite()) {
                return None;
            }
            if (coord == config.exit() && reserved_exit_faces.contains(&face))
                || (next == config.exit() && reserved_exit_faces.contains(&face.opposite()))
            {
                return None;
            }
            Some((next, other.archetype.travel_cost()))
        })
        .collect()
}

fn heuristic(a: CellCoord, b: CellCoord) -> u32 {
    let horizontal = u32::from(a.x.abs_diff(b.x)) + u32::from(a.z.abs_diff(b.z));
    let vertical = u32::from(a.level.abs_diff(b.level));
    horizontal * 1_000 + vertical * 2_000
}

pub(super) fn face_between(
    config: FullWfcConfig,
    a: CellCoord,
    b: CellCoord,
) -> Option<ModuleFace> {
    ModuleFace::ALL
        .into_iter()
        .find(|&face| config.neighbor(a, face) == Some(b))
}
