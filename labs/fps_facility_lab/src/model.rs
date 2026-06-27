//! Phase 23: project the proven room graph into a navigable 3D facility.
//!
//! The 2D `room_lab` vocabulary remains authoritative for template and port
//! semantics. This module promotes it into three dimensions:
//!
//! - every authored module has 3D bounds, collision solids, and typed `Port3d`s;
//! - every module exposes four `Passage` graph ports, one per cardinal wall;
//! - template-specific Door/Ladder/Machinery/Equipment/Grapple/Observation ports
//!   remain explicit typed fixtures;
//! - all 36 `observation_lab::DoorId`s map one-to-one onto those transformed graph
//!   ports;
//! - walking through a graph port follows `ObservationWorld::partner`, so the same
//!   mutable graph used by the 2D labs is the facility's actual navigation graph.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use bevy::prelude::*;
use fps_controller_lab::controller::{Aabb3, FIXED_DT, FpsArena, FpsBody, FpsConfig, step_body};
use observation_lab::model::{DOOR_COUNT, DoorId, ObservationWorld, ROOM_COUNT, Side};
use observed_core::{PortId, RoomId};
use player_input::PlayerIntent;
use room_lab::{PortType, QuarterTurn, RoomTemplate};

pub const MODULE_HALF: f32 = 6.0;
pub const MODULE_HEIGHT: f32 = 4.0;
pub const MODULE_SPACING: f32 = 18.0;
pub const PORT_HALF: f32 = 1.45;
const WALL_THICKNESS: f32 = 0.24;
const ARRIVAL_INSET: f32 = 1.0;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PortRef3d {
    pub room: RoomId,
    pub port: PortId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortRole3d {
    Graph(Side),
    Fixture,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Port3d {
    pub id: PortId,
    pub kind: PortType,
    pub local_position: Vec3,
    pub facing: Side,
    pub role: PortRole3d,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solid3d {
    pub local_center: Vec3,
    pub half: Vec3,
}

#[derive(Clone, Debug)]
pub struct ModuleDefinition3d {
    pub template: RoomTemplate,
    pub bounds: Vec3,
    pub ports: Vec<Port3d>,
    pub obstacles: Vec<Solid3d>,
}

impl ModuleDefinition3d {
    pub fn port(&self, id: PortId) -> Option<&Port3d> {
        self.ports.iter().find(|port| port.id == id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModulePose3d {
    pub translation: Vec3,
    pub rotation: QuarterTurn,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldPort3d {
    pub reference: PortRef3d,
    pub kind: PortType,
    pub position: Vec3,
    pub facing: Side,
    pub role: PortRole3d,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModuleInstance3d {
    pub room: RoomId,
    pub template: RoomTemplate,
    pub pose: ModulePose3d,
}

#[derive(Resource, Clone, Debug)]
pub struct ModuleRegistry3d {
    definitions: BTreeMap<RoomTemplate, ModuleDefinition3d>,
}

impl Default for ModuleRegistry3d {
    fn default() -> Self {
        let definitions = RoomTemplate::ALL
            .into_iter()
            .map(|template| (template, authored_definition(template)))
            .collect();
        Self { definitions }
    }
}

impl ModuleRegistry3d {
    pub fn load(&self, template: RoomTemplate) -> Option<&ModuleDefinition3d> {
        self.definitions.get(&template)
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub fn represented_port_types(&self) -> BTreeSet<&'static str> {
        self.definitions
            .values()
            .flat_map(|definition| definition.ports.iter())
            .map(|port| port.kind.label())
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionError {
    MissingModule,
    MissingDefinition,
    MissingGraphPort,
    TypeMismatch,
    RoleMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectedConnection {
    pub a: PortRef3d,
    pub b: PortRef3d,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphProjection3d {
    pub door_ports: Vec<PortRef3d>,
    pub connections: Vec<ProjectedConnection>,
}

impl GraphProjection3d {
    pub fn build(
        graph: &ObservationWorld,
        registry: &ModuleRegistry3d,
        modules: &[ModuleInstance3d],
    ) -> Result<Self, ProjectionError> {
        let mut door_ports = Vec::with_capacity(DOOR_COUNT);
        for index in 0..DOOR_COUNT {
            let door = DoorId(index as u16);
            let logical = graph.door(door);
            let reference = graph_port_ref(registry, modules, logical.room, logical.side)?;
            door_ports.push(reference);
        }

        let mut connections = graph
            .connections()
            .into_iter()
            .map(|(a, b)| {
                let a_ref = door_ports[a.0 as usize];
                let b_ref = door_ports[b.0 as usize];
                validate_port_pair(registry, modules, a_ref, b_ref)?;
                Ok(ProjectedConnection { a: a_ref, b: b_ref })
            })
            .collect::<Result<Vec<_>, ProjectionError>>()?;
        connections.sort_by_key(|connection| {
            (
                connection.a.room.0,
                connection.a.port.0,
                connection.b.room.0,
                connection.b.port.0,
            )
        });
        Ok(Self {
            door_ports,
            connections,
        })
    }

    pub fn port_for_door(&self, door: DoorId) -> PortRef3d {
        self.door_ports[door.0 as usize]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraversalRecord {
    pub from_room: RoomId,
    pub from_side: Side,
    pub source_door: DoorId,
    pub destination_door: DoorId,
    pub to_room: RoomId,
    pub to_side: Side,
}

#[derive(Resource, Clone, Debug)]
pub struct FacilityStage {
    pub registry: ModuleRegistry3d,
    pub modules: Vec<ModuleInstance3d>,
    pub graph: ObservationWorld,
    pub projection: GraphProjection3d,
    pub player_room: RoomId,
    pub body: FpsBody,
    pub config: FpsConfig,
    pub arena: FpsArena,
    pub traversal_history: Vec<TraversalRecord>,
    pub decohere_count: u32,
    pub reset_count: u32,
    pub last_event: String,
}

impl Default for FacilityStage {
    fn default() -> Self {
        let registry = ModuleRegistry3d::default();
        let modules = authored_modules();
        let mut graph = ObservationWorld::authored();
        let player_room = RoomId(4);
        graph.players = vec![player_room];
        let projection =
            GraphProjection3d::build(&graph, &registry, &modules).expect("authored projection");
        let config = FpsConfig::default();
        let centre = module_center(player_room);
        let body = FpsBody::spawned(Vec3::new(centre.x, config.half_height, centre.z + 1.5), 0.0);
        let arena = build_arena(&registry, &modules, &graph);
        Self {
            registry,
            modules,
            graph,
            projection,
            player_room,
            body,
            config,
            arena,
            traversal_history: Vec::new(),
            decohere_count: 0,
            reset_count: 0,
            last_event: "Walk through a 3D Passage port; it follows the current graph link."
                .to_string(),
        }
    }
}

impl FacilityStage {
    pub fn module(&self, room: RoomId) -> &ModuleInstance3d {
        self.modules
            .iter()
            .find(|module| module.room == room)
            .expect("all graph rooms have modules")
    }

    pub fn current_module(&self) -> &ModuleInstance3d {
        self.module(self.player_room)
    }

    pub fn current_template(&self) -> RoomTemplate {
        self.current_module().template
    }

    pub fn step(&mut self, intent: PlayerIntent) -> Option<TraversalRecord> {
        let before = self.body.position;
        step_body(&mut self.body, intent, &self.arena, &self.config, FIXED_DT);
        self.resolve_crossing(before)
    }

    pub fn resolve_crossing(&mut self, before: Vec3) -> Option<TraversalRecord> {
        let side = crossed_side(self.player_room, before, self.body.position)?;
        self.traverse(side)
    }

    pub fn traverse(&mut self, side: Side) -> Option<TraversalRecord> {
        let source = self.graph.door_id(self.player_room, side);
        if self.graph.is_sealed(source) {
            self.last_event = format!(
                "Room {} {} is sealed; collision keeps the player inside.",
                self.player_room.0,
                side.label()
            );
            return None;
        }
        let destination = self.graph.partner(source);
        let destination_door = *self.graph.door(destination);
        let from_room = self.player_room;
        self.player_room = destination_door.room;
        self.graph.players = vec![self.player_room];
        self.place_inside(destination_door.room, destination_door.side);

        let record = TraversalRecord {
            from_room,
            from_side: side,
            source_door: source,
            destination_door: destination,
            to_room: destination_door.room,
            to_side: destination_door.side,
        };
        self.traversal_history.push(record);
        self.last_event = format!(
            "Traversed room {} {} -> room {} {} via graph doors {} <-> {}.",
            from_room.0,
            side.label(),
            record.to_room.0,
            record.to_side.label(),
            source.0,
            destination.0
        );
        Some(record)
    }

    fn place_inside(&mut self, room: RoomId, entry_side: Side) {
        let centre = module_center(room);
        let outward = side_direction(entry_side);
        let inward = -outward;
        self.body.position = centre - outward * (MODULE_HALF - ARRIVAL_INSET);
        self.body.position.y = self.config.half_height;
        self.body.velocity = Vec3::ZERO;
        self.body.grounded = true;
        self.body.yaw = yaw_from_direction(inward);
        self.body.pitch = 0.0;
        self.body.spawn = self.body.position;
        self.body.spawn_yaw = self.body.yaw;
    }

    pub fn relocate_player(&mut self, room: RoomId, entry_side: Side) {
        self.player_room = room;
        self.graph.players = vec![room];
        self.place_inside(room, entry_side);
    }

    pub fn sync_graph(&mut self, graph: ObservationWorld) {
        self.graph = graph;
        self.projection = GraphProjection3d::build(&self.graph, &self.registry, &self.modules)
            .expect("graph state must remain compatible with the authored 3D ports");
        self.arena = build_arena(&self.registry, &self.modules, &self.graph);
    }

    pub fn decohere(&mut self) {
        self.graph.players = vec![self.player_room];
        self.graph.decohere();
        self.decohere_count += 1;
        self.projection = GraphProjection3d::build(&self.graph, &self.registry, &self.modules)
            .expect("decoherence preserves graph-port compatibility");
        self.arena = build_arena(&self.registry, &self.modules, &self.graph);
        self.last_event = format!(
            "Decohered: {} graph doors rewired; room {} remained observed.",
            self.graph.rewires_last, self.player_room.0
        );
    }

    pub fn reset(&mut self) {
        let resets = self.reset_count + 1;
        *self = Self::default();
        self.reset_count = resets;
    }

    pub fn all_rooms_reachable(&self) -> bool {
        reachable_rooms(&self.graph, RoomId(0)).len() == ROOM_COUNT
    }

    pub fn projection_exact(&self) -> bool {
        if self.projection.door_ports.len() != DOOR_COUNT {
            return false;
        }
        (0..DOOR_COUNT).all(|index| {
            let door = DoorId(index as u16);
            let partner = self.graph.partner(door);
            let reference = self.projection.port_for_door(door);
            let partner_ref = self.projection.port_for_door(partner);
            self.projection.connections.iter().any(|connection| {
                (connection.a == reference && connection.b == partner_ref)
                    || (connection.a == partner_ref && connection.b == reference)
            }) || partner == door
        })
    }
}

pub fn authored_modules() -> Vec<ModuleInstance3d> {
    let templates = [
        RoomTemplate::StraightCorridor,
        RoomTemplate::Corner,
        RoomTemplate::Junction,
        RoomTemplate::ControlRoom,
        RoomTemplate::MachineChamber,
        RoomTemplate::FreightRoom,
        RoomTemplate::Shaft,
        RoomTemplate::PlatformRoom,
        RoomTemplate::Junction,
    ];
    let rotations = [
        QuarterTurn::R0,
        QuarterTurn::R90,
        QuarterTurn::R180,
        QuarterTurn::R270,
        QuarterTurn::R0,
        QuarterTurn::R90,
        QuarterTurn::R180,
        QuarterTurn::R270,
        QuarterTurn::R0,
    ];
    (0..ROOM_COUNT)
        .map(|index| {
            let room = RoomId(index as u32);
            ModuleInstance3d {
                room,
                template: templates[index],
                pose: ModulePose3d {
                    translation: module_center(room),
                    rotation: rotations[index],
                },
            }
        })
        .collect()
}

pub fn module_center(room: RoomId) -> Vec3 {
    let row = room.0 / 3;
    let column = room.0 % 3;
    Vec3::new(
        (column as f32 - 1.0) * MODULE_SPACING,
        0.0,
        (row as f32 - 1.0) * MODULE_SPACING,
    )
}

pub fn world_port(
    module: &ModuleInstance3d,
    definition: &ModuleDefinition3d,
    port: PortId,
) -> Option<WorldPort3d> {
    let local = definition.port(port)?;
    Some(WorldPort3d {
        reference: PortRef3d {
            room: module.room,
            port,
        },
        kind: local.kind,
        position: module.pose.translation + rotate_vec3(module.pose.rotation, local.local_position),
        facing: rotate_side(module.pose.rotation, local.facing),
        role: match local.role {
            PortRole3d::Graph(side) => PortRole3d::Graph(rotate_side(module.pose.rotation, side)),
            PortRole3d::Fixture => PortRole3d::Fixture,
        },
    })
}

pub fn graph_port_ref(
    registry: &ModuleRegistry3d,
    modules: &[ModuleInstance3d],
    room: RoomId,
    side: Side,
) -> Result<PortRef3d, ProjectionError> {
    let module = modules
        .iter()
        .find(|module| module.room == room)
        .ok_or(ProjectionError::MissingModule)?;
    let definition = registry
        .load(module.template)
        .ok_or(ProjectionError::MissingDefinition)?;
    definition
        .ports
        .iter()
        .filter_map(|port| world_port(module, definition, port.id))
        .find(|port| port.role == PortRole3d::Graph(side))
        .map(|port| port.reference)
        .ok_or(ProjectionError::MissingGraphPort)
}

pub fn validate_port_pair(
    registry: &ModuleRegistry3d,
    modules: &[ModuleInstance3d],
    a: PortRef3d,
    b: PortRef3d,
) -> Result<(), ProjectionError> {
    let a = resolve_port(registry, modules, a)?;
    let b = resolve_port(registry, modules, b)?;
    if a.kind != b.kind {
        return Err(ProjectionError::TypeMismatch);
    }
    if !matches!(a.role, PortRole3d::Graph(_)) || !matches!(b.role, PortRole3d::Graph(_)) {
        return Err(ProjectionError::RoleMismatch);
    }
    Ok(())
}

fn resolve_port(
    registry: &ModuleRegistry3d,
    modules: &[ModuleInstance3d],
    reference: PortRef3d,
) -> Result<WorldPort3d, ProjectionError> {
    let module = modules
        .iter()
        .find(|module| module.room == reference.room)
        .ok_or(ProjectionError::MissingModule)?;
    let definition = registry
        .load(module.template)
        .ok_or(ProjectionError::MissingDefinition)?;
    world_port(module, definition, reference.port).ok_or(ProjectionError::MissingGraphPort)
}

pub fn build_arena(
    registry: &ModuleRegistry3d,
    modules: &[ModuleInstance3d],
    graph: &ObservationWorld,
) -> FpsArena {
    let mut solids = Vec::new();
    for module in modules {
        let definition = registry
            .load(module.template)
            .expect("authored module definition");
        solids.extend(module_wall_solids(module, graph));
        solids.extend(definition.obstacles.iter().map(|solid| {
            let centre =
                module.pose.translation + rotate_vec3(module.pose.rotation, solid.local_center);
            let half = rotate_half(module.pose.rotation, solid.half);
            Aabb3::from_center_half(centre, half)
        }));
    }
    FpsArena {
        solids,
        floor_y: 0.0,
        floor_half: MODULE_SPACING * 2.0,
    }
}

fn module_wall_solids(module: &ModuleInstance3d, graph: &ObservationWorld) -> Vec<Aabb3> {
    let mut solids = Vec::new();
    let segment_half = (MODULE_HALF - PORT_HALF) * 0.5;
    let tangent_offset = (MODULE_HALF + PORT_HALF) * 0.5;
    let y = MODULE_HEIGHT * 0.5;
    for side in Side::ALL {
        let direction = side_direction(side);
        let tangent = Vec3::new(-direction.z, 0.0, direction.x);
        let wall_center = module.pose.translation + direction * MODULE_HALF + Vec3::Y * y;
        for sign in [-1.0, 1.0] {
            let center = wall_center + tangent * tangent_offset * sign;
            let half = if direction.x.abs() > 0.5 {
                Vec3::new(WALL_THICKNESS, y, segment_half)
            } else {
                Vec3::new(segment_half, y, WALL_THICKNESS)
            };
            solids.push(Aabb3::from_center_half(center, half));
        }
        let door = graph.door_id(module.room, side);
        if graph.is_sealed(door) {
            let half = if direction.x.abs() > 0.5 {
                Vec3::new(WALL_THICKNESS, y, PORT_HALF)
            } else {
                Vec3::new(PORT_HALF, y, WALL_THICKNESS)
            };
            solids.push(Aabb3::from_center_half(wall_center, half));
        }
    }
    solids
}

fn crossed_side(room: RoomId, before: Vec3, after: Vec3) -> Option<Side> {
    let centre = module_center(room);
    let local_before = before - centre;
    let local_after = after - centre;
    let within_x = local_after.x.abs() <= PORT_HALF - 0.05;
    let within_z = local_after.z.abs() <= PORT_HALF - 0.05;
    if local_before.z >= -MODULE_HALF && local_after.z < -MODULE_HALF && within_x {
        Some(Side::North)
    } else if local_before.x <= MODULE_HALF && local_after.x > MODULE_HALF && within_z {
        Some(Side::East)
    } else if local_before.z <= MODULE_HALF && local_after.z > MODULE_HALF && within_x {
        Some(Side::South)
    } else if local_before.x >= -MODULE_HALF && local_after.x < -MODULE_HALF && within_z {
        Some(Side::West)
    } else {
        None
    }
}

pub fn reachable_rooms(graph: &ObservationWorld, start: RoomId) -> BTreeSet<RoomId> {
    let mut reached = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(room) = queue.pop_front() {
        for side in Side::ALL {
            let door = graph.door_id(room, side);
            if graph.is_sealed(door) {
                continue;
            }
            let destination = graph.door(graph.partner(door)).room;
            if reached.insert(destination) {
                queue.push_back(destination);
            }
        }
    }
    reached
}

pub fn side_direction(side: Side) -> Vec3 {
    match side {
        Side::North => Vec3::NEG_Z,
        Side::East => Vec3::X,
        Side::South => Vec3::Z,
        Side::West => Vec3::NEG_X,
    }
}

pub fn yaw_from_direction(direction: Vec3) -> f32 {
    direction.x.atan2(-direction.z)
}

fn rotate_vec3(rotation: QuarterTurn, vector: Vec3) -> Vec3 {
    match rotation {
        QuarterTurn::R0 => vector,
        QuarterTurn::R90 => Vec3::new(vector.z, vector.y, -vector.x),
        QuarterTurn::R180 => Vec3::new(-vector.x, vector.y, -vector.z),
        QuarterTurn::R270 => Vec3::new(-vector.z, vector.y, vector.x),
    }
}

fn rotate_half(rotation: QuarterTurn, half: Vec3) -> Vec3 {
    match rotation {
        QuarterTurn::R0 | QuarterTurn::R180 => half,
        QuarterTurn::R90 | QuarterTurn::R270 => Vec3::new(half.z, half.y, half.x),
    }
}

fn rotate_side(rotation: QuarterTurn, side: Side) -> Side {
    let turns = match rotation {
        QuarterTurn::R0 => 0,
        QuarterTurn::R90 => 1,
        QuarterTurn::R180 => 2,
        QuarterTurn::R270 => 3,
    };
    let mut side = side;
    for _ in 0..turns {
        side = match side {
            Side::North => Side::West,
            Side::West => Side::South,
            Side::South => Side::East,
            Side::East => Side::North,
        };
    }
    side
}

fn authored_definition(template: RoomTemplate) -> ModuleDefinition3d {
    let mut ports = graph_ports();
    ports.extend(fixture_ports(template));
    ModuleDefinition3d {
        template,
        bounds: Vec3::new(MODULE_HALF * 2.0, MODULE_HEIGHT, MODULE_HALF * 2.0),
        ports,
        obstacles: authored_obstacles(template),
    }
}

fn graph_ports() -> Vec<Port3d> {
    Side::ALL
        .into_iter()
        .enumerate()
        .map(|(index, side)| Port3d {
            id: PortId(index as u32),
            kind: PortType::Passage,
            local_position: side_direction(side) * MODULE_HALF + Vec3::Y * 1.2,
            facing: side,
            role: PortRole3d::Graph(side),
        })
        .collect()
}

fn fixture_ports(template: RoomTemplate) -> Vec<Port3d> {
    let fixture = |id, kind, position, facing| Port3d {
        id: PortId(id),
        kind,
        local_position: position,
        facing,
        role: PortRole3d::Fixture,
    };
    match template {
        RoomTemplate::StraightCorridor => vec![],
        RoomTemplate::Corner => vec![fixture(
            10,
            PortType::Door,
            Vec3::new(2.6, 1.1, 2.6),
            Side::South,
        )],
        RoomTemplate::Junction => vec![fixture(
            10,
            PortType::Observation,
            Vec3::new(0.0, 2.4, 0.0),
            Side::North,
        )],
        RoomTemplate::ControlRoom => vec![
            fixture(10, PortType::Door, Vec3::new(-2.4, 1.1, 2.3), Side::South),
            fixture(
                11,
                PortType::Observation,
                Vec3::new(2.4, 2.0, -2.3),
                Side::North,
            ),
        ],
        RoomTemplate::MachineChamber => vec![fixture(
            10,
            PortType::Machinery,
            Vec3::new(0.0, 1.1, 0.0),
            Side::North,
        )],
        RoomTemplate::FreightRoom => vec![
            fixture(
                10,
                PortType::Equipment,
                Vec3::new(-2.8, 0.8, 0.0),
                Side::West,
            ),
            fixture(11, PortType::Door, Vec3::new(2.8, 1.1, 0.0), Side::East),
        ],
        RoomTemplate::Shaft => vec![fixture(
            10,
            PortType::Ladder,
            Vec3::new(0.0, 2.0, 2.8),
            Side::South,
        )],
        RoomTemplate::PlatformRoom => vec![fixture(
            10,
            PortType::Grapple,
            Vec3::new(0.0, 3.2, -2.4),
            Side::North,
        )],
    }
}

fn authored_obstacles(template: RoomTemplate) -> Vec<Solid3d> {
    let solid = |x, y, z, hx, hy, hz| Solid3d {
        local_center: Vec3::new(x, y, z),
        half: Vec3::new(hx, hy, hz),
    };
    match template {
        RoomTemplate::StraightCorridor => vec![
            solid(-3.8, 0.45, 0.0, 0.5, 0.45, 2.8),
            solid(3.8, 0.45, 0.0, 0.5, 0.45, 2.8),
        ],
        RoomTemplate::Corner => vec![solid(2.5, 1.0, -2.5, 1.0, 1.0, 1.0)],
        RoomTemplate::Junction => vec![solid(0.0, 0.7, 0.0, 1.0, 0.7, 1.0)],
        RoomTemplate::ControlRoom => vec![
            solid(0.0, 0.65, -2.2, 2.4, 0.65, 0.55),
            solid(-3.5, 0.45, 2.7, 0.65, 0.45, 1.4),
        ],
        RoomTemplate::MachineChamber => vec![
            solid(-2.5, 1.25, 0.0, 0.9, 1.25, 1.5),
            solid(2.5, 1.25, 0.0, 0.9, 1.25, 1.5),
        ],
        RoomTemplate::FreightRoom => vec![
            solid(-2.5, 0.75, -1.7, 1.1, 0.75, 1.0),
            solid(2.5, 1.1, 1.7, 1.2, 1.1, 1.0),
        ],
        RoomTemplate::Shaft => vec![
            solid(-3.7, 1.8, 0.0, 0.45, 1.8, 2.8),
            solid(3.7, 1.8, 0.0, 0.45, 1.8, 2.8),
        ],
        RoomTemplate::PlatformRoom => vec![
            solid(0.0, 0.35, 0.0, 2.4, 0.35, 1.4),
            solid(0.0, 0.7, -2.4, 1.6, 0.7, 0.6),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_promotes_all_room_templates_and_port_types_to_3d() {
        let registry = ModuleRegistry3d::default();
        assert_eq!(registry.len(), RoomTemplate::ALL.len());
        let expected = [
            "PASSAGE",
            "DOOR",
            "LADDER",
            "MACHINERY",
            "EQUIPMENT",
            "GRAPPLE",
            "OBSERVATION",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(registry.represented_port_types(), expected);
        for template in RoomTemplate::ALL {
            let definition = registry.load(template).unwrap();
            assert_eq!(
                definition
                    .ports
                    .iter()
                    .filter(|port| matches!(port.role, PortRole3d::Graph(_)))
                    .count(),
                4
            );
            assert!(definition.bounds.x > 0.0 && definition.bounds.y > 0.0);
        }
    }

    #[test]
    fn quarter_turn_transforms_3d_positions_facings_and_graph_roles() {
        let registry = ModuleRegistry3d::default();
        let module = ModuleInstance3d {
            room: RoomId(7),
            template: RoomTemplate::StraightCorridor,
            pose: ModulePose3d {
                translation: Vec3::new(10.0, 0.0, 20.0),
                rotation: QuarterTurn::R90,
            },
        };
        let definition = registry.load(module.template).unwrap();
        let north = world_port(&module, definition, PortId(0)).unwrap();
        assert_eq!(north.position, Vec3::new(4.0, 1.2, 20.0));
        assert_eq!(north.facing, Side::West);
        assert_eq!(north.role, PortRole3d::Graph(Side::West));
    }

    #[test]
    fn every_graph_door_maps_to_one_passage_port_and_every_link_projects() {
        let stage = FacilityStage::default();
        assert_eq!(stage.projection.door_ports.len(), DOOR_COUNT);
        assert_eq!(
            stage.projection.connections.len(),
            stage.graph.connections().len()
        );
        assert!(stage.projection_exact());
        let unique = stage
            .projection
            .door_ports
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), DOOR_COUNT);
        for reference in &stage.projection.door_ports {
            let port = resolve_port(&stage.registry, &stage.modules, *reference).unwrap();
            assert_eq!(port.kind, PortType::Passage);
            assert!(matches!(port.role, PortRole3d::Graph(_)));
        }
    }

    #[test]
    fn fixture_ports_cannot_be_substituted_for_graph_passages() {
        let stage = FacilityStage::default();
        let passage = stage.projection.port_for_door(DoorId(0));
        let fixture = PortRef3d {
            room: RoomId(3),
            port: PortId(10),
        };
        assert_eq!(
            validate_port_pair(&stage.registry, &stage.modules, passage, fixture),
            Err(ProjectionError::TypeMismatch)
        );
    }

    #[test]
    fn the_authored_2d_graph_is_connected_as_a_3d_navigation_graph() {
        let stage = FacilityStage::default();
        assert!(stage.all_rooms_reachable());
        assert_eq!(reachable_rooms(&stage.graph, RoomId(0)).len(), ROOM_COUNT);
    }

    #[test]
    fn traversal_follows_the_exact_current_graph_partner() {
        let mut stage = FacilityStage::default();
        stage.relocate_player(RoomId(0), Side::West);
        let source = stage.graph.door_id(RoomId(0), Side::East);
        let destination = stage.graph.partner(source);
        let destination_door = *stage.graph.door(destination);
        let record = stage.traverse(Side::East).unwrap();
        assert_eq!(record.source_door, source);
        assert_eq!(record.destination_door, destination);
        assert_eq!(stage.player_room, destination_door.room);
        let centre = module_center(destination_door.room);
        assert!(
            (stage.body.position - centre).length() < MODULE_HALF,
            "arrival is inside the rendered destination module"
        );
    }

    #[test]
    fn the_first_person_controller_walks_through_an_open_graph_port() {
        let mut stage = FacilityStage::default();
        stage.relocate_player(RoomId(4), Side::West);
        let centre = module_center(RoomId(4));
        stage.body.position = centre + Vec3::new(MODULE_HALF - 1.0, stage.config.half_height, 0.0);
        stage.body.yaw = std::f32::consts::FRAC_PI_2;
        stage.body.grounded = true;
        let expected = stage
            .graph
            .door(
                stage
                    .graph
                    .partner(stage.graph.door_id(RoomId(4), Side::East)),
            )
            .room;
        for _ in 0..90 {
            stage.step(PlayerIntent {
                movement: Vec2::Y,
                ..default()
            });
            if stage.player_room != RoomId(4) {
                break;
            }
        }
        assert_eq!(stage.player_room, expected);
        assert_eq!(stage.traversal_history.len(), 1);
    }

    #[test]
    fn a_graph_sealed_port_is_a_real_collision_wall() {
        let mut stage = FacilityStage::default();
        stage.relocate_player(RoomId(0), Side::South);
        let centre = module_center(RoomId(0));
        stage.body.position = centre + Vec3::new(0.0, stage.config.half_height, -MODULE_HALF + 1.0);
        stage.body.yaw = 0.0;
        stage.body.grounded = true;
        assert!(
            stage
                .graph
                .is_sealed(stage.graph.door_id(RoomId(0), Side::North))
        );
        for _ in 0..90 {
            stage.step(PlayerIntent {
                movement: Vec2::Y,
                ..default()
            });
        }
        assert_eq!(stage.player_room, RoomId(0));
        assert!(stage.traversal_history.is_empty());
        assert!(
            stage.body.position.z > centre.z - MODULE_HALF,
            "the generated panel stops the controller before the threshold"
        );
    }

    #[test]
    fn a_rewired_door_navigates_to_its_new_nonphysical_partner() {
        let mut stage = FacilityStage::default();
        // Make room 0 unobserved, then rewire until its east door stops leading to
        // its authored physical neighbour.
        let door = stage.graph.door_id(RoomId(0), Side::East);
        let authored = stage.graph.partner(door);
        stage.graph.players = vec![RoomId(4)];
        for _ in 0..8 {
            stage.decohere();
            if stage.graph.partner(door) != authored {
                break;
            }
        }
        let rewired = stage.graph.partner(door);
        assert_ne!(rewired, authored);
        stage.relocate_player(RoomId(0), Side::West);
        let record = stage.traverse(Side::East).unwrap();
        assert_eq!(record.destination_door, rewired);
        assert_eq!(stage.player_room, stage.graph.door(rewired).room);
    }

    #[test]
    fn sealed_graph_ports_generate_collision_panels() {
        let stage = FacilityStage::default();
        let north = stage.graph.door_id(RoomId(0), Side::North);
        assert!(stage.graph.is_sealed(north));
        let module = stage.module(RoomId(0));
        let open_count = module_wall_solids(module, &stage.graph).len();
        let mut graph = stage.graph.clone();
        let partner = graph.door_id(RoomId(8), Side::South);
        graph.links[north.0 as usize] = partner;
        graph.links[partner.0 as usize] = north;
        let opened_count = module_wall_solids(module, &graph).len();
        assert_eq!(open_count, opened_count + 1);
    }

    #[test]
    fn deterministic_inputs_and_graph_actions_reproduce_the_same_run() {
        let run = || {
            let mut stage = FacilityStage::default();
            for _ in 0..30 {
                stage.step(PlayerIntent {
                    look: Vec2::new(0.25, 0.0),
                    movement: Vec2::new(0.0, 0.5),
                    ..default()
                });
            }
            stage.decohere();
            (
                stage.body,
                stage.graph.links,
                stage.projection,
                stage.player_room,
            )
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn reset_restores_the_authored_facility_and_spawn() {
        let mut stage = FacilityStage::default();
        stage.decohere();
        stage.traverse(Side::East);
        stage.reset();
        assert_eq!(stage.modules.len(), ROOM_COUNT);
        assert_eq!(stage.player_room, RoomId(4));
        assert!(stage.traversal_history.is_empty());
        assert_eq!(stage.decohere_count, 0);
        assert_eq!(stage.reset_count, 1);
        assert!(stage.projection_exact());
    }

    #[test]
    fn syncing_an_external_graph_rebuilds_projection_and_collision() {
        let mut stage = FacilityStage::default();
        let mut graph = stage.graph.clone();
        graph.players = vec![RoomId(4)];
        graph.decohere();
        let expected_links = graph.links.clone();
        stage.sync_graph(graph);
        assert_eq!(stage.graph.links, expected_links);
        assert!(stage.projection_exact());
        assert!(!stage.arena.solids.is_empty());
    }
}
