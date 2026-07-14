//! Semantic racecourse map specifications.
//!
//! A [`MapSpec`] is the source of truth for designed Observed maps. It describes the
//! playable graph: stable room IDs, doorway slots, schematic positions, semantic
//! room/corridor roles, and enough connectivity facts to prove the map is fair without
//! relying on a protected spine.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use glam::Vec2;
use observed_core::{CorridorId, Direction, PlaceId, RoomId, ThresholdId, ThresholdSlotId};

use crate::junction::{CorridorSpec, JunctionTopology, ThresholdAttachment};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoomRole {
    /// Teams enter here, learn the route grammar, and immediately choose between exits.
    Start,
    /// The objective room.
    Exit,
    /// A comparatively safe hub for reading doors, previews, map information, and calls.
    Decision,
    /// An unstable junction whose outgoing routes are worth anchoring or re-reading.
    DecoherenceFork,
    /// A room placed where freezing current thresholds is strategically valuable.
    AnchorCheckpoint,
    /// A room intended to form a team-keyed teleport-pad relay with another relay room.
    TeleportRelay,
    /// A side objective room holding a keystone.
    Keystone,
    /// A co-op puzzle room requiring two operators.
    DualStation,
    /// A room where teams can redirect guardian pressure.
    GuardianControl,
    /// An information room: reveals map state, guardian pressure, anchors, or exits.
    Monitor,
    /// A recovery route after poor tool placement, decoherence, or a guardian setback.
    Recovery,
}

impl RoomRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Exit => "exit",
            Self::Decision => "decision",
            Self::DecoherenceFork => "decoherence fork",
            Self::AnchorCheckpoint => "anchor checkpoint",
            Self::TeleportRelay => "teleport relay",
            Self::Keystone => "keystone",
            Self::DualStation => "dual station",
            Self::GuardianControl => "guardian control",
            Self::Monitor => "monitor",
            Self::Recovery => "recovery",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorridorRole {
    /// Ordinary low-drama traversal between rooms.
    Connector,
    /// Reliable but inefficient fallback movement.
    LongRoute,
    /// A closed or unstable threshold where the tension is uncertainty.
    Mystery,
    /// Authored movement challenge: elevation, stairs, ledges, ladders, or sockets.
    Vertical,
    /// A non-obvious alternate route around a contested or unstable area.
    Bypass,
    /// Gantry elevated 3D pathway.
    Gantry,
}

impl CorridorRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Connector => "connector",
            Self::LongRoute => "long route",
            Self::Mystery => "mystery",
            Self::Vertical => "vertical",
            Self::Bypass => "bypass",
            Self::Gantry => "gantry",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MapEndpoint {
    pub room: RoomId,
    pub side: Direction,
}

impl MapEndpoint {
    pub const fn new(room: u32, side: Direction) -> Self {
        Self {
            room: RoomId(room),
            side,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapRoom {
    pub id: RoomId,
    pub role: RoomRole,
    /// Schematic coordinate for TAC-MAP/debug views. This is not world geometry.
    pub schematic: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapEdge {
    pub a: MapEndpoint,
    pub b: MapEndpoint,
    pub role: CorridorRole,
    /// Mutable edges may be rewired by decoherence. The first racecourse uses no
    /// protected route; validators prove the graph remains recoverable.
    pub mutable: bool,
}

/// An authored multi-exit corridor. Every endpoint owns one room-side socket;
/// the corresponding corridor sockets are assigned in declaration order. The
/// legacy [`MapEdge`] list remains supported while existing maps migrate.
#[derive(Clone, Debug, PartialEq)]
pub struct MapCorridor {
    pub id: CorridorId,
    pub endpoints: Vec<MapEndpoint>,
    pub role: CorridorRole,
    pub mutable: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapSpec {
    pub name: &'static str,
    pub rooms: Vec<MapRoom>,
    /// Legacy two-ended corridors. Empty for a fully junction-authored map.
    pub edges: Vec<MapEdge>,
    /// First-class multi-endpoint corridors. When empty, [`Self::corridors`] derives
    /// one stable two-socket corridor per legacy edge.
    pub corridors: Vec<MapCorridor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MapValidationError {
    Empty,
    DuplicateRoom(RoomId),
    DuplicateEndpoint(RoomId, Direction),
    MissingRole(RoomRole),
    MultipleRole(RoomRole),
    InvalidEndpoint(RoomId),
    Disconnected(RoomId),
    NoRedundantExitPath,
    SingleEdgeCutDisconnects { edge: usize, room: RoomId },
    ObjectiveSingleEdgeCut { edge: usize, objective: RoomId },
    KeystoneAtTerminal(RoomId),
    MissingUsefulAnchorCheckpoint,
    MissingUsefulTeleportRelayPair,
    DuplicateCorridor(CorridorId),
    CorridorHasTooFewEndpoints(CorridorId),
    DuplicateCorridorEndpoint(CorridorId, RoomId, Direction),
}

impl MapSpec {
    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    pub fn room(&self, id: RoomId) -> Option<&MapRoom> {
        self.rooms.iter().find(|room| room.id == id)
    }

    pub fn start_room(&self) -> Option<RoomId> {
        self.role_room(RoomRole::Start)
    }

    pub fn exit_room(&self) -> Option<RoomId> {
        self.role_room(RoomRole::Exit)
    }

    pub fn role_room(&self, role: RoomRole) -> Option<RoomId> {
        self.rooms
            .iter()
            .find(|room| room.role == role)
            .map(|room| room.id)
    }

    pub fn rooms_with_role(&self, role: RoomRole) -> Vec<RoomId> {
        self.rooms
            .iter()
            .filter(|room| room.role == role)
            .map(|room| room.id)
            .collect()
    }

    pub fn keystone_rooms(&self) -> Vec<RoomId> {
        self.rooms_with_role(RoomRole::Keystone)
    }

    pub fn neighbors(&self, room: RoomId) -> Vec<RoomId> {
        let mut out = Vec::new();
        for corridor in self.corridors() {
            if corridor
                .endpoints
                .iter()
                .any(|endpoint| endpoint.room == room)
            {
                out.extend(
                    corridor
                        .endpoints
                        .iter()
                        .filter_map(|endpoint| (endpoint.room != room).then_some(endpoint.room)),
                );
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    pub fn shortest_path(&self, start: RoomId, goal: RoomId) -> Option<Vec<RoomId>> {
        shortest_path(self.room_ids(), &self.edge_pairs(), start, goal)
    }

    /// The [`CorridorRole`] of the edge between `a` and `b`, if one exists (edge
    /// endpoints are unordered — `(a, b)` and `(b, a)` return the same role).
    pub fn corridor_role_between(&self, a: RoomId, b: RoomId) -> Option<CorridorRole> {
        self.corridors()
            .iter()
            .find(|corridor| {
                corridor.endpoints.iter().any(|endpoint| endpoint.room == a)
                    && corridor.endpoints.iter().any(|endpoint| endpoint.room == b)
            })
            .map(|corridor| corridor.role)
    }

    /// Corridors in the active topology. Existing two-ended maps are lifted into
    /// stable corridor IDs by their authored edge order, preserving current output
    /// while opening an incremental path to junction-authored maps.
    pub fn corridors(&self) -> Vec<MapCorridor> {
        if !self.corridors.is_empty() {
            return self.corridors.clone();
        }
        self.edges
            .iter()
            .enumerate()
            .map(|(index, edge)| MapCorridor {
                id: CorridorId(index as u32),
                endpoints: vec![edge.a, edge.b],
                role: edge.role,
                mutable: edge.mutable,
            })
            .collect()
    }

    /// The reciprocal room/corridor socket graph used by threshold, anchor, and
    /// future crossing code. This accepts data-driven corridor endpoint counts.
    pub fn junction_topology(&self) -> Result<JunctionTopology, Vec<MapValidationError>> {
        let corridors = self.corridors();
        let mut errors = Vec::new();
        let mut specs = Vec::new();
        let mut attachments = Vec::new();
        let mut ids = BTreeSet::new();
        for corridor in corridors {
            if !ids.insert(corridor.id) {
                errors.push(MapValidationError::DuplicateCorridor(corridor.id));
                continue;
            }
            if corridor.endpoints.len() < 2 {
                errors.push(MapValidationError::CorridorHasTooFewEndpoints(corridor.id));
                continue;
            }
            let mut seen = BTreeSet::new();
            for (slot, endpoint) in corridor.endpoints.iter().copied().enumerate() {
                if !seen.insert((endpoint.room, endpoint.side)) {
                    errors.push(MapValidationError::DuplicateCorridorEndpoint(
                        corridor.id,
                        endpoint.room,
                        endpoint.side,
                    ));
                }
                let room = ThresholdId {
                    place: PlaceId::Room(endpoint.room),
                    slot: ThresholdSlotId(endpoint.side.index() as u16),
                };
                let hall = ThresholdId {
                    place: PlaceId::Corridor(corridor.id),
                    slot: ThresholdSlotId(slot as u16),
                };
                if let Ok(attachment) = ThresholdAttachment::new(room, hall) {
                    attachments.push(attachment);
                }
            }
            specs.push(CorridorSpec::with_slot_count(
                corridor.id,
                corridor.endpoints.len(),
            ));
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        JunctionTopology::new(specs, attachments).map_err(|errors| {
            errors
                .into_iter()
                .map(|_| MapValidationError::Empty)
                .collect()
        })
    }

    pub fn next_step_toward(&self, start: RoomId, goal: RoomId) -> Option<RoomId> {
        let path = self.shortest_path(start, goal)?;
        path.get(1).copied()
    }

    pub fn validate(&self) -> Result<(), Vec<MapValidationError>> {
        let mut errors = Vec::new();
        if self.rooms.is_empty() {
            errors.push(MapValidationError::Empty);
            return Err(errors);
        }

        self.validate_unique_rooms(&mut errors);
        self.validate_unique_endpoints(&mut errors);
        self.validate_required_roles(&mut errors);
        self.validate_endpoints_exist(&mut errors);
        if let Err(junction_errors) = self.junction_topology() {
            errors.extend(junction_errors);
        }

        if let (Some(start), Some(exit)) = (self.start_room(), self.exit_room()) {
            self.validate_connectivity(start, &mut errors);
            self.validate_redundancy(start, exit, &mut errors);
            self.validate_objective_recovery(start, &mut errors);
            self.validate_tool_roles(&mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn validate_or_panic(&self) {
        if let Err(errors) = self.validate() {
            panic!("invalid map spec {}: {errors:?}", self.name);
        }
    }

    fn room_ids(&self) -> Vec<RoomId> {
        self.rooms.iter().map(|room| room.id).collect()
    }

    fn edge_pairs(&self) -> Vec<(RoomId, RoomId)> {
        self.corridors()
            .into_iter()
            .flat_map(|corridor| {
                let mut pairs = Vec::new();
                for (index, a) in corridor.endpoints.iter().enumerate() {
                    for b in corridor.endpoints.iter().skip(index + 1) {
                        pairs.push((a.room, b.room));
                    }
                }
                pairs
            })
            .collect()
    }

    fn validate_unique_rooms(&self, errors: &mut Vec<MapValidationError>) {
        let mut seen = BTreeSet::new();
        for room in &self.rooms {
            if !seen.insert(room.id) {
                errors.push(MapValidationError::DuplicateRoom(room.id));
            }
        }
    }

    fn validate_unique_endpoints(&self, errors: &mut Vec<MapValidationError>) {
        let mut seen = BTreeSet::new();
        for corridor in self.corridors() {
            for endpoint in corridor.endpoints {
                if !seen.insert((endpoint.room, endpoint.side)) {
                    errors.push(MapValidationError::DuplicateEndpoint(
                        endpoint.room,
                        endpoint.side,
                    ));
                }
            }
        }
    }

    fn validate_required_roles(&self, errors: &mut Vec<MapValidationError>) {
        for role in [RoomRole::Start, RoomRole::Exit] {
            let count = self.rooms.iter().filter(|room| room.role == role).count();
            match count {
                0 => errors.push(MapValidationError::MissingRole(role)),
                1 => {}
                _ => errors.push(MapValidationError::MultipleRole(role)),
            }
        }

        for role in [
            RoomRole::Decision,
            RoomRole::DecoherenceFork,
            RoomRole::AnchorCheckpoint,
            RoomRole::TeleportRelay,
            RoomRole::Keystone,
            RoomRole::DualStation,
            RoomRole::GuardianControl,
            RoomRole::Monitor,
            RoomRole::Recovery,
        ] {
            if !self.rooms.iter().any(|room| room.role == role) {
                errors.push(MapValidationError::MissingRole(role));
            }
        }
    }

    fn validate_endpoints_exist(&self, errors: &mut Vec<MapValidationError>) {
        let rooms: BTreeSet<_> = self.rooms.iter().map(|room| room.id).collect();
        for corridor in self.corridors() {
            for endpoint in corridor.endpoints {
                if !rooms.contains(&endpoint.room) {
                    errors.push(MapValidationError::InvalidEndpoint(endpoint.room));
                }
            }
        }
    }

    fn validate_connectivity(&self, start: RoomId, errors: &mut Vec<MapValidationError>) {
        let reachable = reachable_set(self.room_ids(), &self.edge_pairs(), start);
        for room in self.room_ids() {
            if !reachable.contains(&room) {
                errors.push(MapValidationError::Disconnected(room));
            }
        }
    }

    fn validate_redundancy(
        &self,
        start: RoomId,
        exit: RoomId,
        errors: &mut Vec<MapValidationError>,
    ) {
        let pairs = self.edge_pairs();
        if count_edge_disjoint_paths(self.room_ids(), &pairs, start, exit, 2) < 2 {
            errors.push(MapValidationError::NoRedundantExitPath);
        }

        for (skip, _) in pairs.iter().enumerate() {
            let without = pairs
                .iter()
                .enumerate()
                .filter_map(|(index, edge)| (index != skip).then_some(*edge))
                .collect::<Vec<_>>();
            let reachable = reachable_set(self.room_ids(), &without, start);
            for room in self.room_ids() {
                if !reachable.contains(&room) {
                    errors.push(MapValidationError::SingleEdgeCutDisconnects { edge: skip, room });
                    break;
                }
            }
        }
    }

    fn validate_objective_recovery(&self, start: RoomId, errors: &mut Vec<MapValidationError>) {
        let exit = self.exit_room().expect("validated exit");
        for room in self
            .rooms
            .iter()
            .filter(|room| room.role == RoomRole::Keystone)
        {
            if room.id == start || room.id == exit {
                errors.push(MapValidationError::KeystoneAtTerminal(room.id));
            }
        }

        let objective_rooms: Vec<_> = self
            .rooms
            .iter()
            .filter(|room| {
                matches!(
                    room.role,
                    RoomRole::Keystone
                        | RoomRole::TeleportRelay
                        | RoomRole::AnchorCheckpoint
                        | RoomRole::Recovery
                        | RoomRole::DualStation
                )
            })
            .map(|room| room.id)
            .collect();
        let pairs = self.edge_pairs();
        for (skip, _) in pairs.iter().enumerate() {
            let without = pairs
                .iter()
                .enumerate()
                .filter_map(|(index, edge)| (index != skip).then_some(*edge))
                .collect::<Vec<_>>();
            let reachable = reachable_set(self.room_ids(), &without, start);
            for objective in &objective_rooms {
                if !reachable.contains(objective) {
                    errors.push(MapValidationError::ObjectiveSingleEdgeCut {
                        edge: skip,
                        objective: *objective,
                    });
                    break;
                }
            }
        }
    }

    fn validate_tool_roles(&self, errors: &mut Vec<MapValidationError>) {
        let useful_anchor = self.rooms.iter().any(|room| {
            room.role == RoomRole::AnchorCheckpoint && self.neighbors(room.id).len() >= 3
        });
        if !useful_anchor {
            errors.push(MapValidationError::MissingUsefulAnchorCheckpoint);
        }

        let relay_rooms = self.rooms_with_role(RoomRole::TeleportRelay);
        let useful_pair = relay_rooms.len() >= 2
            && relay_rooms.iter().any(|&a| {
                relay_rooms.iter().any(|&b| {
                    a != b && self.shortest_path(a, b).is_some_and(|path| path.len() >= 4)
                })
            });
        if !useful_pair {
            errors.push(MapValidationError::MissingUsefulTeleportRelayPair);
        }
    }
}

fn shortest_path(
    rooms: Vec<RoomId>,
    edges: &[(RoomId, RoomId)],
    start: RoomId,
    goal: RoomId,
) -> Option<Vec<RoomId>> {
    if start == goal {
        return Some(vec![start]);
    }
    let room_set: BTreeSet<_> = rooms.into_iter().collect();
    if !room_set.contains(&start) || !room_set.contains(&goal) {
        return None;
    }
    let mut parent = BTreeMap::<RoomId, RoomId>::new();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::new();
    seen.insert(start);
    queue.push_back(start);
    while let Some(room) = queue.pop_front() {
        for next in edge_neighbors(edges, room) {
            if seen.insert(next) {
                parent.insert(next, room);
                if next == goal {
                    let mut path = vec![goal];
                    let mut current = goal;
                    while let Some(&prev) = parent.get(&current) {
                        path.push(prev);
                        current = prev;
                        if current == start {
                            break;
                        }
                    }
                    path.reverse();
                    return Some(path);
                }
                queue.push_back(next);
            }
        }
    }
    None
}

fn reachable_set(
    rooms: Vec<RoomId>,
    edges: &[(RoomId, RoomId)],
    start: RoomId,
) -> BTreeSet<RoomId> {
    let room_set: BTreeSet<_> = rooms.into_iter().collect();
    let mut seen = BTreeSet::new();
    if !room_set.contains(&start) {
        return seen;
    }
    let mut queue = VecDeque::new();
    seen.insert(start);
    queue.push_back(start);
    while let Some(room) = queue.pop_front() {
        for next in edge_neighbors(edges, room) {
            if seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    seen
}

fn edge_neighbors(edges: &[(RoomId, RoomId)], room: RoomId) -> Vec<RoomId> {
    let mut out = Vec::new();
    for &(a, b) in edges {
        if a == room {
            out.push(b);
        } else if b == room {
            out.push(a);
        }
    }
    out.sort_unstable();
    out
}

fn count_edge_disjoint_paths(
    rooms: Vec<RoomId>,
    edges: &[(RoomId, RoomId)],
    start: RoomId,
    goal: RoomId,
    wanted: usize,
) -> usize {
    let mut remaining = edges.to_vec();
    let mut found = 0;
    while found < wanted {
        let Some(path) = shortest_path(rooms.clone(), &remaining, start, goal) else {
            break;
        };
        for pair in path.windows(2) {
            let a = pair[0];
            let b = pair[1];
            remaining.retain(|&(x, y)| !((x == a && y == b) || (x == b && y == a)));
        }
        found += 1;
    }
    found
}

pub fn sector_relay_v1() -> MapSpec {
    use CorridorRole as C;
    use Direction::{East, North, South, West};
    use RoomRole as R;

    let room = |id, role, x, y| MapRoom {
        id: RoomId(id),
        role,
        schematic: Vec2::new(x, y),
    };
    let edge = |a, a_side, b, b_side, role| MapEdge {
        a: MapEndpoint::new(a, a_side),
        b: MapEndpoint::new(b, b_side),
        role,
        mutable: true,
    };

    MapSpec {
        name: "Sector Relay V1",
        rooms: vec![
            room(0, R::Start, 0.0, 1.5),
            room(1, R::Decision, 1.0, 1.5),
            room(2, R::Keystone, 2.0, 0.5),
            room(3, R::DualStation, 2.0, 2.5),
            room(4, R::AnchorCheckpoint, 3.0, 1.5),
            room(5, R::TeleportRelay, 4.0, 1.5),
            room(6, R::DecoherenceFork, 5.0, 1.5),
            room(7, R::GuardianControl, 6.0, 0.5),
            room(8, R::Monitor, 2.0, 3.5),
            room(9, R::TeleportRelay, 5.0, 2.5),
            room(10, R::Recovery, 4.0, 3.5),
            room(11, R::Exit, 7.0, 1.5),
            room(12, R::Keystone, 3.0, 0.5),
            room(13, R::Keystone, 6.0, 2.5),
        ],
        edges: vec![
            edge(0, East, 1, West, C::Connector),
            edge(0, South, 10, North, C::LongRoute),
            edge(1, East, 2, West, C::Mystery),
            edge(1, South, 3, North, C::Connector),
            edge(1, North, 8, South, C::Connector),
            edge(2, North, 9, North, C::LongRoute),
            edge(2, South, 4, North, C::Vertical),
            edge(2, East, 12, West, C::Mystery),
            edge(3, East, 4, West, C::Connector),
            edge(3, West, 10, East, C::Bypass),
            edge(4, East, 5, West, C::Mystery),
            edge(5, East, 6, West, C::Connector),
            edge(5, North, 8, East, C::Bypass),
            edge(6, East, 7, West, C::Mystery),
            edge(6, South, 10, West, C::Bypass),
            edge(6, North, 12, South, C::LongRoute),
            edge(7, South, 11, North, C::Vertical),
            edge(7, East, 13, North, C::Mystery),
            edge(8, West, 10, South, C::LongRoute),
            edge(9, East, 11, West, C::Connector),
            edge(9, South, 13, West, C::Bypass),
        ],
        corridors: Vec::new(),
    }
}

pub fn multi_exit_fixture() -> MapSpec {
    use CorridorRole as C;
    use Direction::{East, North, South, West};
    use RoomRole as R;

    let room = |id, role, x, y| MapRoom {
        id: RoomId(id),
        role,
        schematic: Vec2::new(x, y),
    };
    let mc = |id, a, a_side, b, b_side, role| MapCorridor {
        id: CorridorId(id),
        endpoints: vec![MapEndpoint::new(a, a_side), MapEndpoint::new(b, b_side)],
        role,
        mutable: true,
    };

    MapSpec {
        name: "Multi-Exit Fixture",
        rooms: vec![
            room(0, R::Start, 0.0, 1.5),
            room(1, R::Decision, 1.0, 1.5),
            room(2, R::Keystone, 2.0, 0.5),
            room(3, R::DualStation, 2.0, 2.5),
            room(4, R::AnchorCheckpoint, 3.0, 1.5),
            room(5, R::TeleportRelay, 4.0, 1.5),
            room(6, R::DecoherenceFork, 5.0, 1.5),
            room(7, R::GuardianControl, 6.0, 0.5),
            room(8, R::Monitor, 2.0, 3.5),
            room(9, R::TeleportRelay, 5.0, 2.5),
            room(10, R::Recovery, 4.0, 3.5),
            room(11, R::Exit, 7.0, 1.5),
            room(12, R::Keystone, 3.0, 0.5),
            room(13, R::Keystone, 6.0, 2.5),
        ],
        edges: Vec::new(),
        corridors: vec![
            mc(0, 0, East, 1, West, C::Connector),
            mc(1, 0, South, 10, North, C::LongRoute),
            mc(2, 1, South, 3, North, C::Connector),
            mc(3, 1, North, 8, South, C::Connector),
            mc(4, 2, North, 9, North, C::LongRoute),
            mc(5, 3, East, 4, West, C::Connector),
            mc(6, 3, West, 10, East, C::Bypass),
            mc(7, 4, East, 5, West, C::Mystery),
            mc(8, 5, East, 6, West, C::Connector),
            mc(9, 5, North, 8, East, C::Bypass),
            mc(10, 6, East, 7, West, C::Mystery),
            mc(11, 6, South, 10, West, C::Bypass),
            mc(12, 6, North, 12, South, C::LongRoute),
            mc(13, 7, South, 11, North, C::Vertical),
            mc(14, 7, East, 13, North, C::Mystery),
            mc(15, 8, West, 10, South, C::LongRoute),
            mc(16, 9, East, 11, West, C::Connector),
            mc(17, 9, South, 13, West, C::Bypass),
            // The two 3-endpoint corridors:
            MapCorridor {
                id: CorridorId(80),
                endpoints: vec![
                    MapEndpoint::new(1, East),
                    MapEndpoint::new(2, West),
                    MapEndpoint::new(3, South),
                ],
                role: CorridorRole::Gantry,
                mutable: true,
            },
            MapCorridor {
                id: CorridorId(81),
                endpoints: vec![
                    MapEndpoint::new(2, South),
                    MapEndpoint::new(4, North),
                    MapEndpoint::new(12, West),
                ],
                role: CorridorRole::Vertical,
                mutable: true,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sector_relay_v1_is_a_valid_tool_solved_graph() {
        let map = sector_relay_v1();
        map.validate().expect("sector relay is graph-valid");
        assert_eq!(map.room_count(), 14);
        assert_eq!(map.start_room(), Some(RoomId(0)));
        assert_eq!(map.exit_room(), Some(RoomId(11)));
        assert_eq!(map.rooms_with_role(RoomRole::TeleportRelay).len(), 2);
        assert_eq!(
            map.keystone_rooms(),
            vec![RoomId(2), RoomId(12), RoomId(13)]
        );
    }

    #[test]
    fn multi_exit_fixture_is_a_valid_tool_solved_graph() {
        let map = multi_exit_fixture();
        println!("EDGE PAIRS: {:?}", map.edge_pairs());
        map.validate().expect("multi-exit fixture is graph-valid");
        assert_eq!(map.room_count(), 14);
        assert_eq!(map.start_room(), Some(RoomId(0)));
        assert_eq!(map.exit_room(), Some(RoomId(11)));
    }

    #[test]
    fn corridor_role_between_is_edge_symmetric_and_none_off_graph() {
        let map = sector_relay_v1();
        let role = map
            .corridor_role_between(RoomId(1), RoomId(2))
            .expect("rooms 1-2 are connected");
        assert_eq!(
            map.corridor_role_between(RoomId(2), RoomId(1)),
            Some(role),
            "edge lookup is unordered"
        );
        assert_eq!(
            map.corridor_role_between(RoomId(0), RoomId(11)),
            None,
            "unconnected rooms have no corridor role"
        );
    }

    #[test]
    fn explicit_junction_corridor_has_reciprocal_data_driven_sockets() {
        let mut spec = sector_relay_v1();
        spec.edges.clear();
        spec.corridors = vec![MapCorridor {
            id: CorridorId(77),
            endpoints: vec![
                MapEndpoint::new(0, Direction::East),
                MapEndpoint::new(1, Direction::West),
                MapEndpoint::new(10, Direction::North),
            ],
            role: CorridorRole::Mystery,
            mutable: true,
        }];
        let topology = spec.junction_topology().expect("junction is well formed");
        assert_eq!(topology.corridor_rooms(CorridorId(77)).len(), 3);
        assert_eq!(
            topology.partner(ThresholdId {
                place: PlaceId::Room(RoomId(10)),
                slot: ThresholdSlotId(Direction::North.index() as u16),
            }),
            Some(ThresholdId {
                place: PlaceId::Corridor(CorridorId(77)),
                slot: ThresholdSlotId(2),
            })
        );
    }

    #[test]
    fn validator_rejects_single_edge_exit_routes() {
        let mut map = sector_relay_v1();
        map.edges.retain(|edge| {
            !matches!(
                (edge.a.room.0, edge.b.room.0),
                (0, 10) | (6, 10) | (8, 10) | (3, 10) | (5, 9) | (9, 11)
            )
        });
        let errors = map.validate().expect_err("single route is invalid");
        assert!(errors.contains(&MapValidationError::NoRedundantExitPath));
    }

    #[test]
    fn next_step_uses_the_authored_graph_not_a_protected_spine() {
        let map = sector_relay_v1();
        let step = map.next_step_toward(RoomId(0), RoomId(11));
        assert!(matches!(step, Some(RoomId(1)) | Some(RoomId(10))));
    }
}
