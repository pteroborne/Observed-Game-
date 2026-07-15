use std::collections::{BTreeMap, BTreeSet, VecDeque};

use observed_content::ArchitectureRegister;
use observed_core::{CorridorId, PlaceId, RoomId, ThresholdId, ThresholdSlotId};

use crate::map_spec::{CorridorRole, RoomRole, TraversalArchetype, room_template_for_role};
use crate::room_def::RoomTemplate;

use super::{CellCoord, FullWfcConfig, ModuleFace, ModulePlacement, ModuleSpace};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoomInstance {
    pub id: RoomId,
    pub coord: CellCoord,
    pub role: RoomRole,
    pub template: RoomTemplate,
    pub register: ArchitectureRegister,
    pub generation_key: u64,
    pub landmark: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorridorInstance {
    pub id: CorridorId,
    pub cells: BTreeSet<CellCoord>,
    pub endpoints: Vec<ThresholdId>,
    pub attachments: Vec<(ThresholdId, ThresholdId)>,
    pub role: CorridorRole,
    pub traversal: TraversalArchetype,
    pub register: ArchitectureRegister,
    pub generation_key: u64,
}

pub(super) struct CatalogProjection {
    pub rooms: BTreeMap<RoomId, RoomInstance>,
    pub corridors: BTreeMap<CorridorId, CorridorInstance>,
    pub landmark_cells: BTreeSet<CellCoord>,
}

pub(super) fn apply_complete_fixture(
    rooms: &mut BTreeMap<RoomId, RoomInstance>,
    corridors: &mut BTreeMap<CorridorId, CorridorInstance>,
) {
    for (room, template) in rooms.values_mut().zip(RoomTemplate::ALL) {
        room.template = template;
    }
    for (corridor, traversal) in corridors.values_mut().zip(TraversalArchetype::ALL) {
        corridor.traversal = traversal;
        (corridor.role, corridor.register) = fixture_corridor_treatment(traversal);
    }
}

fn fixture_corridor_treatment(
    traversal: TraversalArchetype,
) -> (CorridorRole, ArchitectureRegister) {
    match traversal {
        TraversalArchetype::Long => (CorridorRole::LongRoute, ArchitectureRegister::Thinning),
        TraversalArchetype::Maze => (CorridorRole::Mystery, ArchitectureRegister::ShadowScreen),
        TraversalArchetype::GantryExpanse => {
            (CorridorRole::Gantry, ArchitectureRegister::Megastructure)
        }
        TraversalArchetype::Wellshaft => (CorridorRole::Vertical, ArchitectureRegister::Wellshaft),
        TraversalArchetype::Orthogonal => {
            (CorridorRole::Connector, ArchitectureRegister::Institutional)
        }
        TraversalArchetype::Chicane => {
            (CorridorRole::Connector, ArchitectureRegister::Megastructure)
        }
        _ => (CorridorRole::Connector, ArchitectureRegister::Monolith),
    }
}

pub(super) fn project_catalog(
    seed: u64,
    config: FullWfcConfig,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
    previous_rooms: Option<&BTreeMap<RoomId, RoomInstance>>,
    previous_corridors: Option<&BTreeMap<CorridorId, CorridorInstance>>,
    next_room_id: &mut u32,
    next_corridor_id: &mut u32,
) -> CatalogProjection {
    let previous_room_at = previous_rooms
        .into_iter()
        .flat_map(BTreeMap::values)
        .map(|room| (room.coord, room))
        .collect::<BTreeMap<_, _>>();
    let mut rooms = BTreeMap::new();
    for (&coord, placement) in placements {
        if placement.space != ModuleSpace::Room {
            continue;
        }
        let retained = previous_room_at.get(&coord).copied().filter(|room| {
            previous_rooms.is_some_and(|previous| {
                previous
                    .get(&room.id)
                    .is_some_and(|_| placement.instance.key == room.generation_key)
            })
        });
        let id = retained.map_or_else(
            || {
                let id = RoomId(*next_room_id);
                *next_room_id = (*next_room_id).wrapping_add(1);
                id
            },
            |room| room.id,
        );
        let role = retained.map_or(RoomRole::Recovery, |room| room.role);
        rooms.insert(
            id,
            RoomInstance {
                id,
                coord,
                role,
                template: retained.map_or(RoomTemplate::Junction, |room| room.template),
                register: placement.architecture,
                generation_key: placement.instance.key,
                landmark: retained.is_some_and(|room| room.landmark),
            },
        );
    }
    assign_room_catalog(
        seed,
        config,
        placements,
        &mut rooms,
        previous_rooms.is_none(),
    );

    let hall_components = hall_components(config, placements);
    let mut corridors = BTreeMap::new();
    for cells in hall_components {
        let generation_key = corridor_generation_key(placements, &cells);
        let retained = previous_corridors
            .into_iter()
            .flat_map(BTreeMap::values)
            .find(|corridor| corridor.cells == cells && corridor.generation_key == generation_key);
        let id = retained.map_or_else(
            || {
                let id = CorridorId(*next_corridor_id);
                *next_corridor_id = (*next_corridor_id).wrapping_add(1);
                id
            },
            |corridor| corridor.id,
        );
        let endpoints = corridor_room_endpoints(config, placements, &rooms, &cells);
        let design = retained
            .map(|corridor| (corridor.role, corridor.traversal, corridor.register))
            .unwrap_or_else(|| corridor_design(seed, id, placements, &cells));
        let corridor_thresholds = endpoints
            .iter()
            .enumerate()
            .map(|(index, _)| ThresholdId {
                place: PlaceId::Corridor(id),
                slot: ThresholdSlotId(index as u16),
            })
            .collect::<Vec<_>>();
        let attachments = endpoints
            .iter()
            .copied()
            .zip(corridor_thresholds.iter().copied())
            .collect();
        corridors.insert(
            id,
            CorridorInstance {
                id,
                cells: cells.clone(),
                endpoints: corridor_thresholds,
                attachments,
                role: design.0,
                traversal: design.1,
                register: design.2,
                generation_key,
            },
        );
    }

    let landmark_cells = rooms
        .values()
        .filter(|room| room.landmark)
        .map(|room| room.coord)
        .collect();
    CatalogProjection {
        rooms,
        corridors,
        landmark_cells,
    }
}

fn assign_room_catalog(
    seed: u64,
    config: FullWfcConfig,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
    rooms: &mut BTreeMap<RoomId, RoomInstance>,
    initial: bool,
) {
    let room_at = rooms
        .values()
        .map(|room| (room.coord, room.id))
        .collect::<BTreeMap<_, _>>();
    let spawn = room_at[&config.spawn()];
    let exit = room_at[&config.exit()];
    set_role(rooms, spawn, RoomRole::Start);
    set_role(rooms, exit, RoomRole::Exit);

    let monitor_count = rooms.len().div_ceil(9).max(1);
    let quotas = [
        (RoomRole::Keystone, 8usize),
        (RoomRole::Monitor, monitor_count),
        (RoomRole::TeleportRelay, 2),
        (RoomRole::AnchorCheckpoint, 1),
        (RoomRole::DualStation, 1),
        (RoomRole::GuardianControl, 1),
        (RoomRole::DecoherenceFork, 1),
        (RoomRole::Decision, 1),
        (RoomRole::Recovery, 1),
    ];
    let mut candidates = rooms
        .values()
        .filter(|room| room.id != spawn && room.id != exit && (initial || !room.landmark))
        .map(|room| room.id)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|room| catalog_key(seed, room.0));
    let mut available = candidates;
    for (role, quota) in quotas {
        let have = rooms.values().filter(|room| room.role == role).count();
        for _ in have..quota {
            let index = available
                .iter()
                .position(|id| role_supports_cell(role, &placements[&rooms[id].coord]))
                .or((!available.is_empty()).then_some(0));
            let Some(index) = index else {
                break;
            };
            let id = available.remove(index);
            set_role(rooms, id, role);
        }
    }
    for &id in &available {
        if initial || rooms[&id].role == RoomRole::Recovery {
            let role = if catalog_key(seed ^ 0xDEC0_F0A1, id.0).is_multiple_of(3) {
                RoomRole::DecoherenceFork
            } else if id.0.is_multiple_of(2) {
                RoomRole::Decision
            } else {
                RoomRole::Recovery
            };
            set_role(rooms, id, role);
        }
    }

    for room in rooms.values_mut() {
        let placement = &placements[&room.coord];
        room.landmark = landmark_role(room.role);
        room.template = template_for(room.role, placement, room.generation_key);
    }
}

fn role_supports_cell(role: RoomRole, placement: &ModulePlacement) -> bool {
    if matches!(
        role,
        RoomRole::Keystone | RoomRole::DualStation | RoomRole::GuardianControl | RoomRole::Monitor
    ) {
        !placement.is_open(ModuleFace::Down)
    } else {
        true
    }
}

fn set_role(rooms: &mut BTreeMap<RoomId, RoomInstance>, id: RoomId, role: RoomRole) {
    if let Some(room) = rooms.get_mut(&id) {
        room.role = role;
        room.landmark = landmark_role(role);
    }
}

fn landmark_role(role: RoomRole) -> bool {
    matches!(
        role,
        RoomRole::Start
            | RoomRole::Exit
            | RoomRole::AnchorCheckpoint
            | RoomRole::TeleportRelay
            | RoomRole::Keystone
            | RoomRole::DualStation
            | RoomRole::GuardianControl
            | RoomRole::Monitor
    )
}

fn template_for(role: RoomRole, placement: &ModulePlacement, key: u64) -> RoomTemplate {
    let vertical = placement.is_open(ModuleFace::Up) || placement.is_open(ModuleFace::Down);
    if vertical && key.is_multiple_of(2) {
        return RoomTemplate::Shaft;
    }
    if matches!(role, RoomRole::Decision | RoomRole::Recovery) && key.is_multiple_of(7) {
        return RoomTemplate::StraightCorridor;
    }
    if matches!(role, RoomRole::Decision | RoomRole::DecoherenceFork) && key.is_multiple_of(5) {
        return RoomTemplate::Corner;
    }
    if matches!(role, RoomRole::AnchorCheckpoint | RoomRole::TeleportRelay) && key.is_multiple_of(3)
    {
        return RoomTemplate::PlatformRoom;
    }
    room_template_for_role(role, key)
}

fn hall_components(
    config: FullWfcConfig,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
) -> Vec<BTreeSet<CellCoord>> {
    let mut unseen = placements
        .iter()
        .filter_map(|(&coord, placement)| (placement.space == ModuleSpace::Hall).then_some(coord))
        .collect::<BTreeSet<_>>();
    let mut components = Vec::new();
    while let Some(start) = unseen.pop_first() {
        let mut cells = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(coord) = queue.pop_front() {
            for face in ModuleFace::ALL {
                if !placements[&coord].is_open(face) {
                    continue;
                }
                if let Some(next) = config.neighbor(coord, face)
                    && placements[&next].space == ModuleSpace::Hall
                    && unseen.remove(&next)
                {
                    cells.insert(next);
                    queue.push_back(next);
                }
            }
        }
        components.push(cells);
    }
    components
}

fn corridor_room_endpoints(
    config: FullWfcConfig,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
    rooms: &BTreeMap<RoomId, RoomInstance>,
    cells: &BTreeSet<CellCoord>,
) -> Vec<ThresholdId> {
    let room_at = rooms
        .values()
        .map(|room| (room.coord, room.id))
        .collect::<BTreeMap<_, _>>();
    let mut endpoints = BTreeSet::new();
    for &coord in cells {
        for face in ModuleFace::ALL {
            if !placements[&coord].is_open(face) {
                continue;
            }
            let Some(next) = config.neighbor(coord, face) else {
                continue;
            };
            if let Some(&room) = room_at.get(&next) {
                endpoints.insert(ThresholdId {
                    place: PlaceId::Room(room),
                    slot: ThresholdSlotId(face.opposite() as u16),
                });
            }
        }
    }
    endpoints.into_iter().collect()
}

fn corridor_design(
    seed: u64,
    id: CorridorId,
    placements: &BTreeMap<CellCoord, ModulePlacement>,
    cells: &BTreeSet<CellCoord>,
) -> (CorridorRole, TraversalArchetype, ArchitectureRegister) {
    let key = catalog_key(seed ^ 0xC011_1D0A, id.0);
    let vertical = cells.iter().any(|coord| {
        placements[coord].is_open(ModuleFace::Up) || placements[coord].is_open(ModuleFace::Down)
    });
    let traversal = if vertical {
        if key.is_multiple_of(2) {
            TraversalArchetype::Wellshaft
        } else {
            TraversalArchetype::Climb
        }
    } else {
        let horizontal = [
            TraversalArchetype::Straight,
            TraversalArchetype::Long,
            TraversalArchetype::Pressure,
            TraversalArchetype::Maze,
            TraversalArchetype::Chicane,
            TraversalArchetype::GantryExpanse,
            TraversalArchetype::Colonnade,
            TraversalArchetype::Orthogonal,
        ];
        horizontal[key as usize % horizontal.len()]
    };
    let (role, register) = match traversal {
        TraversalArchetype::Long => (CorridorRole::LongRoute, ArchitectureRegister::Thinning),
        TraversalArchetype::Maze => (CorridorRole::Mystery, ArchitectureRegister::ShadowScreen),
        TraversalArchetype::GantryExpanse => {
            (CorridorRole::Gantry, ArchitectureRegister::Megastructure)
        }
        TraversalArchetype::Wellshaft => (CorridorRole::Vertical, ArchitectureRegister::Wellshaft),
        TraversalArchetype::Orthogonal => {
            (CorridorRole::Connector, ArchitectureRegister::Institutional)
        }
        _ => (
            CorridorRole::Connector,
            placements[cells.first().unwrap()].architecture,
        ),
    };
    (role, traversal, register)
}

fn corridor_generation_key(
    placements: &BTreeMap<CellCoord, ModulePlacement>,
    cells: &BTreeSet<CellCoord>,
) -> u64 {
    cells.iter().fold(0xCBF2_9CE4_8422_2325, |hash, coord| {
        hash.rotate_left(7) ^ placements[coord].instance.key
    })
}

fn catalog_key(seed: u64, value: u32) -> u64 {
    let mut key = seed ^ u64::from(value).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    key = (key ^ (key >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    key = (key ^ (key >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    key ^ (key >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::full_wfc::FullWfcWorld;

    #[test]
    fn default_world_has_multiplayer_gameplay_role_quotas() {
        let world = FullWfcWorld::new(7, FullWfcConfig::default()).expect("world");
        let count = |role| {
            world
                .rooms
                .values()
                .filter(|room| room.role == role)
                .count()
        };
        assert_eq!(count(RoomRole::Start), 1);
        assert_eq!(count(RoomRole::Exit), 1);
        assert_eq!(count(RoomRole::Keystone), 8);
        assert!(count(RoomRole::Monitor) >= world.rooms.len().div_ceil(9));
        assert!(count(RoomRole::TeleportRelay) >= 2);
        for role in [
            RoomRole::Decision,
            RoomRole::DecoherenceFork,
            RoomRole::AnchorCheckpoint,
            RoomRole::DualStation,
            RoomRole::GuardianControl,
            RoomRole::Recovery,
        ] {
            assert!(count(role) >= 1, "missing {role:?}");
        }
    }

    #[test]
    fn projected_threshold_attachments_use_stable_domain_ids() {
        let world = FullWfcWorld::new(11, FullWfcConfig::default()).expect("world");
        assert!(!world.corridors.is_empty());
        for corridor in world.corridors.values() {
            assert_eq!(corridor.endpoints.len(), corridor.attachments.len());
            assert!(corridor.attachments.iter().all(|(room, hall)| {
                matches!(room.place, PlaceId::Room(_))
                    && hall.place == PlaceId::Corridor(corridor.id)
            }));
        }
    }

    #[test]
    fn complete_fixture_corridors_are_two_way_and_have_two_to_four_endpoints() {
        let world = FullWfcWorld::catalog_fixture(0xF17E_7001).expect("fixture");
        for corridor in world.corridors.values() {
            assert!((2..=4).contains(&corridor.attachments.len()));
            let rooms = corridor
                .attachments
                .iter()
                .filter_map(|(threshold, _)| match threshold.place {
                    PlaceId::Room(room) => Some(world.rooms[&room].coord),
                    PlaceId::Corridor(_) => None,
                })
                .collect::<Vec<_>>();
            for pair in rooms.windows(2) {
                let forward = world
                    .route_between_cells(pair[0], pair[1])
                    .expect("forward corridor route");
                let reverse = world
                    .route_between_cells(pair[1], pair[0])
                    .expect("reverse corridor route");
                assert_eq!(forward.cost_millis, reverse.cost_millis);
            }
        }
    }
}
