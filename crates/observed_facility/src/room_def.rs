//! Authored room templates: typed ports, surfaces, the room registry, and the
//! transform / world-port / collision helpers. Pure geometry (`glam`); the `color()`
//! presentation helpers and the `Resource` derive are behind the `bevy` feature.

use std::collections::BTreeMap;

#[cfg(feature = "bevy")]
use bevy::color::Color;
use glam::Vec2;
pub use observed_core::{PortId, RoomId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RoomTemplate {
    StraightCorridor,
    Corner,
    Junction,
    ControlRoom,
    MachineChamber,
    FreightRoom,
    Shaft,
    PlatformRoom,
}

impl RoomTemplate {
    pub const ALL: [Self; 8] = [
        Self::StraightCorridor,
        Self::Corner,
        Self::Junction,
        Self::ControlRoom,
        Self::MachineChamber,
        Self::FreightRoom,
        Self::Shaft,
        Self::PlatformRoom,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::StraightCorridor => "Straight corridor",
            Self::Corner => "Corner",
            Self::Junction => "Junction",
            Self::ControlRoom => "Control room",
            Self::MachineChamber => "Machine chamber",
            Self::FreightRoom => "Freight room",
            Self::Shaft => "Shaft",
            Self::PlatformRoom => "Platform room",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|candidate| *candidate == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    #[cfg(feature = "bevy")]
    pub fn color(self) -> Color {
        match self {
            Self::StraightCorridor => Color::srgb(0.18, 0.43, 0.58),
            Self::Corner => Color::srgb(0.20, 0.52, 0.62),
            Self::Junction => Color::srgb(0.30, 0.60, 0.48),
            Self::ControlRoom => Color::srgb(0.30, 0.46, 0.75),
            Self::MachineChamber => Color::srgb(0.62, 0.38, 0.24),
            Self::FreightRoom => Color::srgb(0.55, 0.47, 0.26),
            Self::Shaft => Color::srgb(0.42, 0.34, 0.62),
            Self::PlatformRoom => Color::srgb(0.20, 0.62, 0.72),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortType {
    Passage,
    Door,
    Ladder,
    Machinery,
    Equipment,
    Grapple,
    Observation,
}

impl PortType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Passage => "PASSAGE",
            Self::Door => "DOOR",
            Self::Ladder => "LADDER",
            Self::Machinery => "MACHINERY",
            Self::Equipment => "EQUIPMENT",
            Self::Grapple => "GRAPPLE",
            Self::Observation => "OBSERVATION",
        }
    }

    #[cfg(feature = "bevy")]
    pub fn color(self) -> Color {
        match self {
            Self::Passage => Color::srgb(0.25, 0.85, 1.0),
            Self::Door => Color::srgb(0.35, 1.0, 0.48),
            Self::Ladder => Color::srgb(0.92, 0.72, 0.25),
            Self::Machinery => Color::srgb(1.0, 0.38, 0.28),
            Self::Equipment => Color::srgb(0.85, 0.42, 1.0),
            Self::Grapple => Color::srgb(1.0, 0.62, 0.20),
            Self::Observation => Color::srgb(0.62, 0.78, 1.0),
        }
    }
}

pub use observed_core::Direction as Cardinal;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum QuarterTurn {
    #[default]
    R0,
    R90,
    R180,
    R270,
}

impl QuarterTurn {
    pub const ALL: [Self; 4] = [Self::R0, Self::R90, Self::R180, Self::R270];

    pub fn next(self) -> Self {
        match self {
            Self::R0 => Self::R90,
            Self::R90 => Self::R180,
            Self::R180 => Self::R270,
            Self::R270 => Self::R0,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::R0 => Self::R270,
            Self::R90 => Self::R0,
            Self::R180 => Self::R90,
            Self::R270 => Self::R180,
        }
    }

    pub fn radians(self) -> f32 {
        match self {
            Self::R0 => 0.0,
            Self::R90 => std::f32::consts::FRAC_PI_2,
            Self::R180 => std::f32::consts::PI,
            Self::R270 => std::f32::consts::PI * 1.5,
        }
    }

    pub fn rotate_point(self, point: Vec2) -> Vec2 {
        match self {
            Self::R0 => point,
            Self::R90 => Vec2::new(-point.y, point.x),
            Self::R180 => -point,
            Self::R270 => Vec2::new(point.y, -point.x),
        }
    }

    pub fn rotate_cardinal(self, facing: Cardinal) -> Cardinal {
        let turns = match self {
            Self::R0 => 0,
            Self::R90 => 1,
            Self::R180 => 2,
            Self::R270 => 3,
        };
        let mut result = facing;
        for _ in 0..turns {
            result = match result {
                Cardinal::North => Cardinal::West,
                Cardinal::West => Cardinal::South,
                Cardinal::South => Cardinal::East,
                Cardinal::East => Cardinal::North,
            };
        }
        result
    }

    pub fn rotate_size(self, size: Vec2) -> Vec2 {
        match self {
            Self::R0 | Self::R180 => size,
            Self::R90 | Self::R270 => Vec2::new(size.y, size.x),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RoomBounds {
    pub size: Vec2,
}

#[derive(Clone, Copy, Debug)]
pub struct RoomPort {
    pub id: PortId,
    pub kind: PortType,
    pub local_position: Vec2,
    pub facing: Cardinal,
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceDefinition {
    pub local_center: Vec2,
    pub size: Vec2,
    pub collision: bool,
}

#[derive(Clone, Debug)]
pub struct RoomDefinition {
    pub id: RoomTemplate,
    pub bounds: RoomBounds,
    pub ports: Vec<RoomPort>,
    pub surfaces: Vec<SurfaceDefinition>,
}

impl RoomDefinition {
    pub fn port(&self, id: PortId) -> Option<&RoomPort> {
        self.ports.iter().find(|port| port.id == id)
    }
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct RoomRegistry {
    definitions: BTreeMap<RoomTemplate, RoomDefinition>,
}

impl Default for RoomRegistry {
    fn default() -> Self {
        let definitions = RoomTemplate::ALL
            .into_iter()
            .map(|template| (template, authored_definition(template)))
            .collect();
        Self { definitions }
    }
}

impl RoomRegistry {
    pub fn load(&self, template: RoomTemplate) -> Option<&RoomDefinition> {
        self.definitions.get(&template)
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RoomTransform {
    pub translation: Vec2,
    pub rotation: QuarterTurn,
}

#[derive(Clone, Copy, Debug)]
pub struct WorldPort {
    pub reference: PortRef,
    pub kind: PortType,
    pub position: Vec2,
    pub facing: Cardinal,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PortRef {
    pub room: RoomId,
    pub port: PortId,
}

#[derive(Clone, Copy, Debug)]
pub struct CollisionRect {
    pub room: RoomId,
    pub surface_index: usize,
    pub center: Vec2,
    pub size: Vec2,
}

pub fn world_port(
    room: RoomId,
    definition: &RoomDefinition,
    transform: RoomTransform,
    port: PortId,
) -> Option<WorldPort> {
    let local = definition.port(port)?;
    Some(WorldPort {
        reference: PortRef { room, port },
        kind: local.kind,
        position: transform.translation + transform.rotation.rotate_point(local.local_position),
        facing: transform.rotation.rotate_cardinal(local.facing),
    })
}

pub fn generate_collisions(
    room: RoomId,
    definition: &RoomDefinition,
    transform: RoomTransform,
) -> Vec<CollisionRect> {
    definition
        .surfaces
        .iter()
        .enumerate()
        .filter(|(_, surface)| surface.collision)
        .map(|(index, surface)| CollisionRect {
            room,
            surface_index: index,
            center: transform.translation + transform.rotation.rotate_point(surface.local_center),
            size: transform.rotation.rotate_size(surface.size),
        })
        .collect()
}

fn authored_definition(template: RoomTemplate) -> RoomDefinition {
    let bounds = RoomBounds {
        size: Vec2::new(200.0, 140.0),
    };
    let ports = match template {
        RoomTemplate::StraightCorridor => vec![
            port(0, PortType::Passage, -100.0, 0.0, Cardinal::West),
            port(1, PortType::Passage, 100.0, 0.0, Cardinal::East),
        ],
        RoomTemplate::Corner => vec![
            port(0, PortType::Passage, -100.0, 0.0, Cardinal::West),
            port(1, PortType::Passage, 0.0, 70.0, Cardinal::North),
        ],
        RoomTemplate::Junction => vec![
            port(0, PortType::Passage, -100.0, 0.0, Cardinal::West),
            port(1, PortType::Passage, 100.0, 0.0, Cardinal::East),
            port(2, PortType::Passage, 0.0, 70.0, Cardinal::North),
            port(3, PortType::Passage, 0.0, -70.0, Cardinal::South),
        ],
        RoomTemplate::ControlRoom => vec![
            port(0, PortType::Passage, -100.0, 0.0, Cardinal::West),
            port(1, PortType::Passage, 100.0, 0.0, Cardinal::East),
            port(2, PortType::Door, 0.0, -70.0, Cardinal::South),
            port(3, PortType::Observation, 0.0, 70.0, Cardinal::North),
        ],
        RoomTemplate::MachineChamber => vec![
            port(0, PortType::Passage, -100.0, 0.0, Cardinal::West),
            port(1, PortType::Passage, 100.0, 0.0, Cardinal::East),
            port(2, PortType::Machinery, 0.0, 70.0, Cardinal::North),
        ],
        RoomTemplate::FreightRoom => vec![
            port(0, PortType::Passage, -100.0, 0.0, Cardinal::West),
            port(1, PortType::Passage, 100.0, 0.0, Cardinal::East),
            port(2, PortType::Equipment, 0.0, 70.0, Cardinal::North),
            port(3, PortType::Door, 0.0, -70.0, Cardinal::South),
        ],
        RoomTemplate::Shaft => vec![
            port(0, PortType::Ladder, 0.0, 70.0, Cardinal::North),
            port(1, PortType::Ladder, 0.0, -70.0, Cardinal::South),
            port(2, PortType::Passage, -100.0, 0.0, Cardinal::West),
            port(3, PortType::Passage, 100.0, 0.0, Cardinal::East),
        ],
        RoomTemplate::PlatformRoom => vec![
            port(0, PortType::Passage, -100.0, 0.0, Cardinal::West),
            port(1, PortType::Passage, 100.0, 0.0, Cardinal::East),
            port(2, PortType::Grapple, 0.0, 70.0, Cardinal::North),
        ],
    };

    RoomDefinition {
        id: template,
        bounds,
        ports,
        surfaces: authored_surfaces(template),
    }
}

fn port(id: u32, kind: PortType, x: f32, y: f32, facing: Cardinal) -> RoomPort {
    RoomPort {
        id: PortId(id),
        kind,
        local_position: Vec2::new(x, y),
        facing,
    }
}

fn authored_surfaces(template: RoomTemplate) -> Vec<SurfaceDefinition> {
    let mut surfaces = vec![
        surface(0.0, -58.0, 150.0, 18.0),
        surface(0.0, 58.0, 150.0, 18.0),
    ];
    match template {
        RoomTemplate::StraightCorridor => {}
        RoomTemplate::Corner => {
            surfaces.push(surface(58.0, 20.0, 18.0, 76.0));
        }
        RoomTemplate::Junction => {
            surfaces.push(surface(-58.0, 38.0, 18.0, 34.0));
            surfaces.push(surface(58.0, 38.0, 18.0, 34.0));
        }
        RoomTemplate::ControlRoom => {
            surfaces.push(surface(0.0, 0.0, 62.0, 28.0));
        }
        RoomTemplate::MachineChamber => {
            surfaces.push(surface(-38.0, 0.0, 34.0, 52.0));
            surfaces.push(surface(38.0, 0.0, 34.0, 52.0));
        }
        RoomTemplate::FreightRoom => {
            surfaces.push(surface(-42.0, 10.0, 46.0, 38.0));
            surfaces.push(surface(42.0, -10.0, 46.0, 38.0));
        }
        RoomTemplate::Shaft => {
            surfaces.push(surface(-70.0, 0.0, 18.0, 90.0));
            surfaces.push(surface(70.0, 0.0, 18.0, 90.0));
        }
        RoomTemplate::PlatformRoom => {
            surfaces.push(surface(0.0, -8.0, 78.0, 24.0));
        }
    }
    surfaces
}

fn surface(x: f32, y: f32, width: f32, height: f32) -> SurfaceDefinition {
    SurfaceDefinition {
        local_center: Vec2::new(x, y),
        size: Vec2::new(width, height),
        collision: true,
    }
}
