//! Whole-layout validation for a collapsed hex facility.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use observed_hex::{HexFace, PortClass, lateral_distance};

use super::HexRoomQuotas;
use super::blueprint::StampedBlueprint;
use super::constraints::all_coords;
use super::topology::route_between;
use super::{HexArchetype, HexCoord, HexPlacement, HexSpace, HexWfcConfig};
use crate::map_spec::RoomRole;

const OPEN_VOLUME_MIN_CELLS: usize = 7;
const OPEN_VOLUME_MIN_EXITS: usize = 3;
const MAX_DECISION_BEAT_DISTANCE: u16 = 24;

pub(super) fn room_quota_failure(
    blueprints: &[StampedBlueprint],
    quotas: HexRoomQuotas,
) -> Option<&'static str> {
    let count = |role| {
        blueprints
            .iter()
            .filter(|blueprint| blueprint.role == role)
            .count()
    };
    let requirements = [
        (RoomRole::Start, 1),
        (RoomRole::Exit, 1),
        (RoomRole::Decision, quotas.decision),
        (RoomRole::DecoherenceFork, quotas.decoherence_fork),
        (RoomRole::DualStation, quotas.dual_station),
        (RoomRole::Monitor, quotas.monitor),
        (RoomRole::AnchorCheckpoint, quotas.anchor_checkpoint),
        (RoomRole::Recovery, quotas.recovery),
        (RoomRole::GuardianControl, quotas.guardian_control),
        (RoomRole::Keystone, quotas.keystone),
    ];
    requirements
        .into_iter()
        .any(|(role, required)| count(role) < required)
        .then_some("production room quota not met")
}

/// Validate that Expanse quantity composes into large, multi-exit volumes and
/// that no traversable region drifts too far from a deliberate decision beat.
pub(super) fn open_volume_failure(
    config: HexWfcConfig,
    placements: &BTreeMap<HexCoord, HexPlacement>,
    blueprints: &[StampedBlueprint],
) -> Option<&'static str> {
    let grid = config.grid();
    let mut visited = BTreeSet::new();
    let mut decision_cells = BTreeSet::new();
    let mut open_levels = BTreeSet::new();

    for (&coord, placement) in placements {
        if placement.archetype != HexArchetype::Expanse || !visited.insert(coord) {
            continue;
        }
        let mut component = BTreeSet::from([coord]);
        let mut queue = VecDeque::from([coord]);
        while let Some(cell) = queue.pop_front() {
            let here = placements[&cell];
            for face in HexFace::LATERAL {
                if !here.is_open(face) {
                    continue;
                }
                let Some(next) = grid.neighbor(cell, face) else {
                    continue;
                };
                if placements[&next].archetype == HexArchetype::Expanse && visited.insert(next) {
                    component.insert(next);
                    queue.push_back(next);
                }
            }
        }
        let exits = component
            .iter()
            .flat_map(|&cell| {
                HexFace::LATERAL.iter().filter_map(move |&face| {
                    grid.neighbor(cell, face).map(|next| (cell, face, next))
                })
            })
            .filter(|(cell, face, next)| {
                !component.contains(next) && placements[cell].is_open(*face)
            })
            .map(|(_, _, next)| next)
            .collect::<BTreeSet<_>>();
        if component.len() >= OPEN_VOLUME_MIN_CELLS && exits.len() >= OPEN_VOLUME_MIN_EXITS {
            open_levels.extend(component.iter().map(|cell| cell.level));
            decision_cells.extend(component);
        }
    }

    let active_levels = placements
        .values()
        .filter(|placement| placement.space != HexSpace::Void)
        .map(|placement| placement.coord.level)
        .collect::<BTreeSet<_>>();
    if active_levels
        .iter()
        .any(|level| !open_levels.contains(level))
    {
        return Some("active level lacks a multi-exit open volume");
    }

    for blueprint in blueprints {
        if matches!(
            blueprint.role,
            RoomRole::Decision
                | RoomRole::DecoherenceFork
                | RoomRole::DualStation
                | RoomRole::Monitor
                | RoomRole::AnchorCheckpoint
        ) {
            decision_cells.extend(blueprint.cells.iter().copied());
        }
    }
    let mut distance = decision_cells
        .iter()
        .copied()
        .map(|cell| (cell, 0u16))
        .collect::<BTreeMap<_, _>>();
    let mut queue = decision_cells.into_iter().collect::<VecDeque<_>>();
    while let Some(cell) = queue.pop_front() {
        let next_distance = distance[&cell].saturating_add(1);
        if next_distance > MAX_DECISION_BEAT_DISTANCE {
            continue;
        }
        for face in HexFace::ALL {
            if !super::topology::is_connection_open(&placements[&cell], face, placements, grid) {
                continue;
            }
            let next = grid
                .neighbor(cell, face)
                .expect("open connection has neighbor");
            if let std::collections::btree_map::Entry::Vacant(entry) = distance.entry(next) {
                entry.insert(next_distance);
                queue.push_back(next);
            }
        }
    }
    placements
        .values()
        .any(|placement| {
            placement.space != HexSpace::Void && !distance.contains_key(&placement.coord)
        })
        .then_some("walkable cell farther than 24 edges from a decision beat")
}

/// The first invariant a layout violates, or `None` when it is valid.
pub(super) fn layout_failure(
    config: HexWfcConfig,
    placements: &BTreeMap<HexCoord, HexPlacement>,
    blueprints: &[StampedBlueprint],
) -> Option<&'static str> {
    if placements[&config.spawn()].space != HexSpace::Room
        || placements[&config.exit()].space != HexSpace::Room
    {
        return Some("spawn or exit is not a room");
    }

    // Footprints never overlap
    let mut occupied = BTreeSet::new();
    for sb in blueprints {
        for &cell in &sb.cells {
            if !occupied.insert(cell) {
                return Some("blueprint footprints overlap");
            }
        }
    }

    // Min room distance holds between blueprint anchors
    if config.min_room_distance > 1 {
        for i in 0..blueprints.len() {
            for j in (i + 1)..blueprints.len() {
                if lateral_distance(blueprints[i].anchor, blueprints[j].anchor)
                    < config.min_room_distance
                {
                    return Some("blueprint anchors closer than min_room_distance");
                }
            }
        }
    }

    // Ramp pairing validation
    let grid = config.grid();
    for placement in placements.values() {
        if placement.archetype == HexArchetype::RampUp {
            let Some(up_coord) = grid.neighbor(placement.coord, HexFace::Up) else {
                return Some("RampUp has no neighbor above");
            };
            let up_neighbor = &placements[&up_coord];
            if up_neighbor.archetype != HexArchetype::RampHead {
                return Some("RampUp not matched by RampHead above");
            }
        }
        if placement.archetype == HexArchetype::RampHead {
            let Some(down_coord) = grid.neighbor(placement.coord, HexFace::Down) else {
                return Some("RampHead has no neighbor below");
            };
            let down_neighbor = &placements[&down_coord];
            if down_neighbor.archetype != HexArchetype::RampUp {
                return Some("RampHead not matched by RampUp below");
            }
        }
    }

    if route_between(config, placements, config.spawn(), config.exit()).is_none() {
        return Some("no spawn->exit route");
    }

    let keep = super::topology::active_component(config, placements, config.spawn());
    if keep.len()
        != placements
            .values()
            .filter(|placement| placement.space != HexSpace::Void)
            .count()
    {
        return Some("disconnected component survived");
    }

    if !all_edges_match(config, placements, blueprints) {
        return Some("edge mismatch or room-room opening outside one footprint");
    }

    if !hall_components_valid(config, placements) {
        return Some("hall component with <2 or >4 room endpoints");
    }

    None
}

fn all_edges_match(
    config: HexWfcConfig,
    placements: &BTreeMap<HexCoord, HexPlacement>,
    blueprints: &[StampedBlueprint],
) -> bool {
    let grid = config.grid();
    let internal_room_edges = blueprints
        .iter()
        .flat_map(|blueprint| {
            let footprint = blueprint.cells.iter().copied().collect::<BTreeSet<_>>();
            blueprint.cells.iter().copied().flat_map(move |cell| {
                let footprint = footprint.clone();
                HexFace::LATERAL.into_iter().filter_map(move |face| {
                    let neighbor = grid.neighbor(cell, face)?;
                    footprint.contains(&neighbor).then_some((cell, neighbor))
                })
            })
        })
        .collect::<BTreeSet<_>>();
    for coord in all_coords(config) {
        let placement = &placements[&coord];
        // A hall cell carries two to four doors: fewer is a dead end, more is
        // not a corridor. Vertical circulation is exempt because its doors are
        // vertical, and an `Expanse` is exempt because merging with everything
        // around it is the entire archetype — capped at four it would just be a
        // junction with a different name.
        if placement.space == HexSpace::Hall
            && !matches!(
                placement.archetype,
                HexArchetype::RampUp
                    | HexArchetype::RampHead
                    | HexArchetype::Shaft
                    | HexArchetype::Expanse
            )
            && !(2..=4).contains(&placement.doors.count_ones())
        {
            return false;
        }

        for face in HexFace::ALL {
            if face.is_lateral() {
                let open = placement.is_open(face);
                let Some(neighbor) = grid.neighbor(coord, face) else {
                    if open {
                        return false;
                    }
                    continue;
                };
                let other = &placements[&neighbor];
                let unexpected_room_seam = open
                    && placement.space == HexSpace::Room
                    && other.space == HexSpace::Room
                    && !internal_room_edges.contains(&(coord, neighbor));
                if open != other.is_open(face.opposite()) || unexpected_room_seam {
                    return false;
                }
            } else {
                let a_port = if face == HexFace::Up {
                    placement.up
                } else {
                    placement.down
                };
                let Some(neighbor) = grid.neighbor(coord, face) else {
                    if a_port != PortClass::Sealed {
                        return false;
                    }
                    continue;
                };
                let other = &placements[&neighbor];
                let b_port = if face == HexFace::Up {
                    other.down
                } else {
                    other.up
                };
                if a_port as u8 != b_port as u8 {
                    return false;
                }
            }
        }
    }
    true
}

/// Every connected hall component must attach to at least two rooms — no
/// orphan pockets that lead nowhere. (Phase 88 also capped this at four, but a
/// multi-level facility legitimately has one large hall network reaching every
/// room, so Phase 90 keeps only the lower bound; connectivity to spawn is
/// enforced separately by [`super::topology::active_component`].)
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
            let placement = &placements[&cell];
            for face in HexFace::ALL {
                let is_open = if face.is_lateral() {
                    placement.is_open(face)
                } else {
                    let a_class = if face == HexFace::Up {
                        placement.up
                    } else {
                        placement.down
                    };
                    a_class != PortClass::Sealed
                };
                if !is_open {
                    continue;
                }
                let Some(next) = grid.neighbor(cell, face) else {
                    continue;
                };
                let other = &placements[&next];

                if face.is_vertical() {
                    let b_class = if face == HexFace::Up {
                        other.down
                    } else {
                        other.up
                    };
                    let a_class = if face == HexFace::Up {
                        placement.up
                    } else {
                        placement.down
                    };
                    if a_class != b_class {
                        continue;
                    }
                }

                match other.space {
                    HexSpace::Hall if unseen.remove(&next) => queue.push_back(next),
                    HexSpace::Room => {
                        endpoints.insert(next);
                    }
                    HexSpace::Void | HexSpace::Hall => {}
                }
            }
        }
        if endpoints.len() < 2 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_volume_fixture(
        expanse_cells: usize,
    ) -> (HexWfcConfig, BTreeMap<HexCoord, HexPlacement>) {
        let config = HexWfcConfig {
            cols: 4,
            rows: 3,
            levels: 1,
            min_rooms: 0,
            max_rooms: 0,
            retry_budget: 1,
            min_room_distance: 0,
        };
        let preferred = [(1, 1), (1, 0), (2, 0), (2, 1), (1, 2), (0, 2), (2, 2)];
        let expanse = preferred
            .into_iter()
            .take(expanse_cells)
            .map(|(q, r)| HexCoord { q, r, level: 0 })
            .collect::<BTreeSet<_>>();
        let grid = config.grid();
        let placements = all_coords(config)
            .map(|coord| {
                let doors = HexFace::LATERAL
                    .into_iter()
                    .filter(|face| grid.neighbor(coord, *face).is_some())
                    .fold(0, |mask, face| mask | super::super::lateral_bit(face));
                (
                    coord,
                    HexPlacement {
                        coord,
                        space: HexSpace::Hall,
                        archetype: if expanse.contains(&coord) {
                            HexArchetype::Expanse
                        } else {
                            HexArchetype::Junction
                        },
                        doors,
                        up: PortClass::Sealed,
                        down: PortClass::Sealed,
                    },
                )
            })
            .collect();
        (config, placements)
    }

    #[test]
    fn open_volume_gate_requires_a_connected_seven_cell_multi_exit_space() {
        let (config, valid) = open_volume_fixture(7);
        assert_eq!(open_volume_failure(config, &valid, &[]), None);

        let (_, too_small) = open_volume_fixture(6);
        assert_eq!(
            open_volume_failure(config, &too_small, &[]),
            Some("active level lacks a multi-exit open volume")
        );
    }

    #[test]
    fn team_scaled_room_quota_preserves_contested_keystone_supply() {
        assert_eq!(HexRoomQuotas::for_team_count(1).keystone, 4);
        assert_eq!(HexRoomQuotas::for_team_count(2).keystone, 5);
        assert_eq!(HexRoomQuotas::for_team_count(4).keystone, 10);
        assert_eq!(HexRoomQuotas::for_team_count(16).keystone, 40);
    }
}
