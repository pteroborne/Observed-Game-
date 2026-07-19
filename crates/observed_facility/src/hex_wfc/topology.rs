//! Connectivity and routing over collapsed placements.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};

use observed_hex::{HexCoord, HexFace, PortClass};

use super::blueprint::StampedBlueprint;
use super::{
    HexArchetype, HexCorridorInstance, HexObservationFrame, HexPlacement, HexSpace,
    HexThresholdAttachment, HexWfcConfig, HexWfcWorld,
};

/// Per-port-class travel costs (Phase 94). Lateral `Door` steps are the
/// hall/room tier; the vertical channels mirror the square lattice's
/// `Climb` and `Shaft` archetype tiers so bot pacing carries over.
const COST_DOOR_LATERAL: u32 = 1_000;
const COST_RAMP_LEVEL: u32 = 2_000;
const COST_SHAFT_LEVEL: u32 = 2_500;

/// A weighted route over typed ports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexRoute {
    pub cells: Vec<HexCoord>,
    /// Weighted traversal time in deterministic milli-cost units.
    pub cost_millis: u32,
}

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
        Some(COST_DOOR_LATERAL)
    } else {
        let a_class = if face == HexFace::Up { a.up } else { a.down };
        match a_class {
            PortClass::RampOpen => Some(COST_RAMP_LEVEL),
            PortClass::ShaftOpen => Some(COST_SHAFT_LEVEL),
            _ => None,
        }
    }
}

/// Admissible hex+level heuristic: lateral hex distance at the door tier plus
/// the level difference at the cheapest vertical (ramp) tier. This equals
/// `travel_distance * COST_DOOR_LATERAL` because `travel_distance` already
/// weights levels double.
fn heuristic(a: HexCoord, b: HexCoord) -> u32 {
    observed_hex::travel_distance(a, b) * COST_DOOR_LATERAL
}

/// A* shortest route over open doors and vertical channels, weighted by the
/// per-port-class cost tiers. Ramp pairs thread naturally: the only vertical
/// `RampOpen` bond is low cell -> head cell, and the head's lateral door is
/// the exit.
pub(super) fn costed_route_between(
    config: HexWfcConfig,
    placements: &BTreeMap<HexCoord, HexPlacement>,
    from: HexCoord,
    to: HexCoord,
) -> Option<HexRoute> {
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
        f_score: heuristic(from, to),
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
            return Some(HexRoute {
                cells: path,
                cost_millis: g_score[&to],
            });
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
                let f = tentative_g + heuristic(next, to);
                open_set.push(AStarNode {
                    coord: next,
                    f_score: f,
                });
            }
        }
    }

    None
}

/// Cell-list projection of [`costed_route_between`] for callers that only
/// need the path.
pub(super) fn route_between(
    config: HexWfcConfig,
    placements: &BTreeMap<HexCoord, HexPlacement>,
    from: HexCoord,
    to: HexCoord,
) -> Option<Vec<HexCoord>> {
    costed_route_between(config, placements, from, to).map(|route| route.cells)
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

/// Cells whose geometry must survive a relayout. Blueprint cells are expanded
/// to their complete footprint and their immediately attached threshold cells;
/// ordinary hall pins remain local. A ramp half always expands to its mate,
/// while shaft cells deliberately do not expand into their column.
pub(super) fn pinned_cells(
    config: HexWfcConfig,
    placements: &BTreeMap<HexCoord, HexPlacement>,
    blueprints: &[StampedBlueprint],
    observation: &HexObservationFrame,
) -> BTreeSet<HexCoord> {
    let mut pins = BTreeSet::from([config.spawn(), config.exit()]);
    pins.extend(
        observation
            .visible_cells
            .iter()
            .copied()
            .filter(|coord| placements.contains_key(coord)),
    );
    pins.extend(
        observation
            .occupied_cells
            .values()
            .copied()
            .filter(|coord| placements.contains_key(coord)),
    );
    pins.extend(
        observation
            .landmark_cells
            .iter()
            .copied()
            .filter(|coord| placements.contains_key(coord)),
    );

    for threshold in &observation.visible_thresholds {
        if let Some(blueprint) = blueprints
            .iter()
            .find(|blueprint| blueprint.generation_key() == threshold.room_generation_key)
        {
            let definition = super::blueprint_for_role(blueprint.role);
            if let Some(&(_, offset, face)) = definition
                .named_ports
                .iter()
                .find(|&&(name, _, _)| name == threshold.port)
            {
                pin_blueprint_and_thresholds(config, placements, blueprint, &mut pins);
                if let Some(index) = definition.cells.iter().position(|&cell| cell == offset)
                    && let Some(next) = config.grid().neighbor(blueprint.cells[index], face)
                {
                    pins.insert(next);
                }
            }
        }
    }

    let blueprint_seeds = pins.clone();
    for blueprint in blueprints {
        if blueprint
            .cells
            .iter()
            .any(|cell| blueprint_seeds.contains(cell))
        {
            pin_blueprint_and_thresholds(config, placements, blueprint, &mut pins);
        }
    }

    // RampOpen is a two-cell traversal unit. Expand until stable so a cell
    // reached as an attached threshold cannot leave the other half mutable.
    loop {
        let mut mates = Vec::new();
        for &coord in &pins {
            let placement = &placements[&coord];
            let face = match placement.archetype {
                HexArchetype::RampUp => Some(HexFace::Up),
                HexArchetype::RampHead => Some(HexFace::Down),
                _ => None,
            };
            if let Some(mate) = face.and_then(|face| config.grid().neighbor(coord, face))
                && !pins.contains(&mate)
            {
                mates.push(mate);
            }
        }
        if mates.is_empty() {
            break;
        }
        pins.extend(mates);
    }
    pins
}

fn pin_blueprint_and_thresholds(
    config: HexWfcConfig,
    placements: &BTreeMap<HexCoord, HexPlacement>,
    blueprint: &StampedBlueprint,
    pins: &mut BTreeSet<HexCoord>,
) {
    let footprint: BTreeSet<_> = blueprint.cells.iter().copied().collect();
    pins.extend(footprint.iter().copied());
    for &cell in &blueprint.cells {
        let placement = &placements[&cell];
        for face in HexFace::ALL {
            if is_connection_open(placement, face, placements, config.grid())
                && let Some(neighbor) = config.grid().neighbor(cell, face)
                && !footprint.contains(&neighbor)
            {
                pins.insert(neighbor);
            }
        }
    }
}

pub(super) fn corridor_instances(
    config: HexWfcConfig,
    placements: &BTreeMap<HexCoord, HexPlacement>,
) -> Vec<HexCorridorInstance> {
    let mut unseen = placements
        .iter()
        .filter_map(|(&coord, placement)| (placement.space == HexSpace::Hall).then_some(coord))
        .collect::<BTreeSet<_>>();
    let mut corridors = Vec::new();
    while let Some(start) = unseen.pop_first() {
        let mut cells = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(coord) = queue.pop_front() {
            let placement = &placements[&coord];
            for face in HexFace::ALL {
                if !is_connection_open(placement, face, placements, config.grid()) {
                    continue;
                }
                let Some(next) = config.grid().neighbor(coord, face) else {
                    continue;
                };
                if placements[&next].space == HexSpace::Hall && unseen.remove(&next) {
                    cells.insert(next);
                    queue.push_back(next);
                }
            }
        }
        let generation_key = corridor_generation_key(&cells);
        corridors.push(HexCorridorInstance {
            id: observed_core::CorridorId(super::relayout::fold_generation_key(generation_key)),
            generation_key,
            cells,
        });
    }
    corridors
}

fn corridor_generation_key(cells: &BTreeSet<HexCoord>) -> u64 {
    cells.iter().fold(0xCBF2_9CE4_8422_2325u64, |hash, coord| {
        let key = u64::from(coord.q) | (u64::from(coord.r) << 16) | (u64::from(coord.level) << 32);
        (hash ^ key).wrapping_mul(0x0000_0100_0000_01B3)
    })
}

pub(super) fn threshold_attachments(world: &HexWfcWorld) -> Vec<HexThresholdAttachment> {
    let corridors = world.corridors();
    let mut corridor_at = BTreeMap::new();
    for corridor in corridors {
        for cell in corridor.cells {
            corridor_at.insert(cell, corridor.id);
        }
    }
    let mut attachments = Vec::new();
    for stamped in &world.blueprints {
        let blueprint = super::blueprint_for_role(stamped.role);
        let room = observed_core::RoomId(super::relayout::fold_generation_key(
            stamped.generation_key(),
        ));
        for &(port_name, offset, face) in &blueprint.named_ports {
            let Some(index) = blueprint.cells.iter().position(|&cell| cell == offset) else {
                continue;
            };
            let room_cell = stamped.cells[index];
            let Some(neighbor) = world.config.grid().neighbor(room_cell, face) else {
                continue;
            };
            if world.placements[&room_cell].is_open(face)
                && let Some(&corridor) = corridor_at.get(&neighbor)
            {
                attachments.push(HexThresholdAttachment {
                    room,
                    port_name,
                    room_cell,
                    face,
                    corridor,
                });
            }
        }
    }
    attachments.sort_by_key(|attachment| {
        (
            attachment.room,
            attachment.port_name,
            attachment.room_cell,
            attachment.face,
        )
    });
    attachments
}
