//! Whole-layout validation for a collapsed hex facility.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use observed_hex::{HexFace, PortClass, lateral_distance};

use super::blueprint::StampedBlueprint;
use super::constraints::all_coords;
use super::topology::route_between;
use super::{HexArchetype, HexCoord, HexPlacement, HexSpace, HexWfcConfig};

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
        if placement.space == HexSpace::Hall
            && !matches!(
                placement.archetype,
                HexArchetype::RampUp | HexArchetype::RampHead | HexArchetype::Shaft
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
                if open != other.is_open(face.opposite())
                    || (open && placement.space == HexSpace::Room && other.space == HexSpace::Room)
                {
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
