//! The validated room world: spawn, attach (auto-aligned to a target port), rotate,
//! replace (preserving compatible connections), despawn, and collision generation.
//! Connections are explicit and validated (type, position, facing, occupancy) — never
//! inferred. Pure logic (`glam`); the `Resource` derive is behind the `bevy` feature.

use std::collections::BTreeMap;

use glam::Vec2;

use crate::room_def::{
    CollisionRect, PortId, PortRef, PortType, QuarterTurn, RoomDefinition, RoomId, RoomRegistry,
    RoomTemplate, RoomTransform, WorldPort, generate_collisions, world_port,
};

const ALIGNMENT_EPSILON: f32 = 0.01;

#[derive(Clone, Copy, Debug)]
pub struct RoomInstance {
    pub id: RoomId,
    pub template: RoomTemplate,
    pub transform: RoomTransform,
    pub revision: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoomConnection {
    pub a: PortRef,
    pub b: PortRef,
}

impl RoomConnection {
    pub fn contains(self, room: RoomId) -> bool {
        self.a.room == room || self.b.room == room
    }

    pub fn external_to(self, room: RoomId) -> Option<PortRef> {
        if self.a.room == room {
            Some(self.b)
        } else if self.b.room == room {
            Some(self.a)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionError {
    MissingRoom,
    MissingPort,
    SameRoom,
    PortOccupied,
    TypeMismatch,
    PositionMismatch,
    FacingMismatch,
    NoCompatibleRotation,
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct RoomWorld {
    pub rooms: BTreeMap<RoomId, RoomInstance>,
    pub connections: Vec<RoomConnection>,
    pub next_room_id: u32,
    pub spawn_count: u32,
    pub despawn_count: u32,
    pub replacement_count: u32,
}

impl RoomWorld {
    pub fn empty() -> Self {
        Self {
            rooms: BTreeMap::new(),
            connections: Vec::new(),
            next_room_id: 0,
            spawn_count: 0,
            despawn_count: 0,
            replacement_count: 0,
        }
    }

    pub fn authored_facility(registry: &RoomRegistry) -> Self {
        let mut world = Self::empty();
        let straight = world.spawn_room(
            RoomTemplate::StraightCorridor,
            RoomTransform {
                translation: Vec2::new(-700.0, -120.0),
                rotation: QuarterTurn::R0,
            },
        );
        let corner = world
            .attach_room(
                registry,
                PortRef {
                    room: straight,
                    port: PortId(1),
                },
                RoomTemplate::Corner,
                PortId(0),
            )
            .expect("authored corner connection");
        let junction = world
            .attach_room(
                registry,
                PortRef {
                    room: corner,
                    port: PortId(1),
                },
                RoomTemplate::Junction,
                PortId(3),
            )
            .expect("authored junction connection");
        let control = world
            .attach_room(
                registry,
                PortRef {
                    room: junction,
                    port: PortId(1),
                },
                RoomTemplate::ControlRoom,
                PortId(0),
            )
            .expect("authored control-room connection");
        let machine = world
            .attach_room(
                registry,
                PortRef {
                    room: control,
                    port: PortId(1),
                },
                RoomTemplate::MachineChamber,
                PortId(0),
            )
            .expect("authored machine connection");
        let freight = world
            .attach_room(
                registry,
                PortRef {
                    room: machine,
                    port: PortId(1),
                },
                RoomTemplate::FreightRoom,
                PortId(0),
            )
            .expect("authored freight connection");
        let shaft = world
            .attach_room(
                registry,
                PortRef {
                    room: freight,
                    port: PortId(1),
                },
                RoomTemplate::Shaft,
                PortId(2),
            )
            .expect("authored shaft connection");
        world
            .attach_room(
                registry,
                PortRef {
                    room: shaft,
                    port: PortId(3),
                },
                RoomTemplate::PlatformRoom,
                PortId(0),
            )
            .expect("authored platform connection");
        world
    }

    pub fn spawn_room(&mut self, template: RoomTemplate, transform: RoomTransform) -> RoomId {
        let id = RoomId(self.next_room_id);
        self.next_room_id += 1;
        self.spawn_count += 1;
        self.rooms.insert(
            id,
            RoomInstance {
                id,
                template,
                transform,
                revision: 0,
            },
        );
        id
    }

    pub fn despawn_room(&mut self, room: RoomId) -> bool {
        if self.rooms.remove(&room).is_none() {
            return false;
        }
        self.connections
            .retain(|connection| !connection.contains(room));
        self.despawn_count += 1;
        true
    }

    pub fn room(&self, room: RoomId) -> Option<&RoomInstance> {
        self.rooms.get(&room)
    }

    pub fn room_mut(&mut self, room: RoomId) -> Option<&mut RoomInstance> {
        self.rooms.get_mut(&room)
    }

    pub fn port(
        &self,
        registry: &RoomRegistry,
        reference: PortRef,
    ) -> Result<WorldPort, ConnectionError> {
        let room = self
            .room(reference.room)
            .ok_or(ConnectionError::MissingRoom)?;
        let definition = registry
            .load(room.template)
            .ok_or(ConnectionError::MissingRoom)?;
        world_port(room.id, definition, room.transform, reference.port)
            .ok_or(ConnectionError::MissingPort)
    }

    pub fn connect(
        &mut self,
        registry: &RoomRegistry,
        a: PortRef,
        b: PortRef,
    ) -> Result<(), ConnectionError> {
        if a.room == b.room {
            return Err(ConnectionError::SameRoom);
        }
        if self.is_port_connected(a) || self.is_port_connected(b) {
            return Err(ConnectionError::PortOccupied);
        }
        let a_world = self.port(registry, a)?;
        let b_world = self.port(registry, b)?;
        validate_port_pair(a_world, b_world)?;
        self.connections.push(RoomConnection { a, b });
        Ok(())
    }

    pub fn attach_room(
        &mut self,
        registry: &RoomRegistry,
        existing: PortRef,
        template: RoomTemplate,
        new_port: PortId,
    ) -> Result<RoomId, ConnectionError> {
        if self.is_port_connected(existing) {
            return Err(ConnectionError::PortOccupied);
        }
        let target = self.port(registry, existing)?;
        let definition = registry
            .load(template)
            .ok_or(ConnectionError::MissingRoom)?;
        let local = definition
            .port(new_port)
            .ok_or(ConnectionError::MissingPort)?;
        if target.kind != local.kind {
            return Err(ConnectionError::TypeMismatch);
        }

        let rotation = QuarterTurn::ALL
            .into_iter()
            .find(|rotation| rotation.rotate_cardinal(local.facing) == target.facing.opposite())
            .ok_or(ConnectionError::NoCompatibleRotation)?;
        let transform = RoomTransform {
            translation: target.position - rotation.rotate_point(local.local_position),
            rotation,
        };
        let room = self.spawn_room(template, transform);
        let attached = PortRef {
            room,
            port: new_port,
        };
        if let Err(error) = self.connect(registry, existing, attached) {
            self.despawn_room(room);
            return Err(error);
        }
        Ok(room)
    }

    pub fn rotate_room(
        &mut self,
        room: RoomId,
        rotation: QuarterTurn,
    ) -> Result<usize, ConnectionError> {
        let instance = self.room_mut(room).ok_or(ConnectionError::MissingRoom)?;
        instance.transform.rotation = rotation;
        instance.revision += 1;
        let before = self.connections.len();
        self.connections
            .retain(|connection| !connection.contains(room));
        Ok(before - self.connections.len())
    }

    pub fn replace_room(
        &mut self,
        registry: &RoomRegistry,
        room: RoomId,
        template: RoomTemplate,
    ) -> Result<usize, ConnectionError> {
        let old = *self.room(room).ok_or(ConnectionError::MissingRoom)?;
        let external_ports = self
            .connections
            .iter()
            .filter_map(|connection| connection.external_to(room))
            .collect::<Vec<_>>();
        self.connections
            .retain(|connection| !connection.contains(room));

        let definition = registry
            .load(template)
            .ok_or(ConnectionError::MissingRoom)?;
        let mut transform = old.transform;
        if let Some(external) = external_ports.first().copied() {
            let target = self.port(registry, external)?;
            if let Some((port, rotation)) = compatible_alignment(definition, target) {
                transform = RoomTransform {
                    translation: target.position - rotation.rotate_point(port.local_position),
                    rotation,
                };
            }
        }

        let instance = self.room_mut(room).ok_or(ConnectionError::MissingRoom)?;
        instance.template = template;
        instance.transform = transform;
        instance.revision += 1;
        self.replacement_count += 1;

        let mut preserved = 0;
        for external in external_ports {
            let Ok(target) = self.port(registry, external) else {
                continue;
            };
            let ports = definition
                .ports
                .iter()
                .filter(|port| port.kind == target.kind)
                .map(|port| port.id)
                .collect::<Vec<_>>();
            for port in ports {
                let replacement = PortRef { room, port };
                if self.is_port_connected(replacement) {
                    continue;
                }
                if self.connect(registry, external, replacement).is_ok() {
                    preserved += 1;
                    break;
                }
            }
        }
        Ok(preserved)
    }

    pub fn is_port_connected(&self, reference: PortRef) -> bool {
        self.connections
            .iter()
            .any(|connection| connection.a == reference || connection.b == reference)
    }

    pub fn free_ports(
        &self,
        registry: &RoomRegistry,
        room: RoomId,
    ) -> Result<Vec<WorldPort>, ConnectionError> {
        let instance = self.room(room).ok_or(ConnectionError::MissingRoom)?;
        let definition = registry
            .load(instance.template)
            .ok_or(ConnectionError::MissingRoom)?;
        Ok(definition
            .ports
            .iter()
            .filter_map(|port| {
                let reference = PortRef {
                    room,
                    port: port.id,
                };
                (!self.is_port_connected(reference))
                    .then(|| self.port(registry, reference).ok())
                    .flatten()
            })
            .collect())
    }

    pub fn collisions(&self, registry: &RoomRegistry) -> Vec<CollisionRect> {
        self.rooms
            .values()
            .flat_map(|room| {
                registry
                    .load(room.template)
                    .map(|definition| generate_collisions(room.id, definition, room.transform))
                    .unwrap_or_default()
            })
            .collect()
    }
}

fn validate_port_pair(a: WorldPort, b: WorldPort) -> Result<(), ConnectionError> {
    if a.kind != b.kind {
        return Err(ConnectionError::TypeMismatch);
    }
    if a.position.distance(b.position) > ALIGNMENT_EPSILON {
        return Err(ConnectionError::PositionMismatch);
    }
    if a.facing.opposite() != b.facing {
        return Err(ConnectionError::FacingMismatch);
    }
    Ok(())
}

fn compatible_alignment(
    definition: &RoomDefinition,
    target: WorldPort,
) -> Option<(&crate::room_def::RoomPort, QuarterTurn)> {
    definition
        .ports
        .iter()
        .filter(|port| port.kind == target.kind)
        .find_map(|port| {
            QuarterTurn::ALL
                .into_iter()
                .find(|rotation| rotation.rotate_cardinal(port.facing) == target.facing.opposite())
                .map(|rotation| (port, rotation))
        })
}

pub fn port_types_match(left: PortType, right: PortType) -> bool {
    left == right
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room_def::Cardinal;

    #[test]
    fn registry_loads_the_complete_authored_vocabulary() {
        let registry = RoomRegistry::default();
        assert_eq!(registry.len(), 8);
        for template in RoomTemplate::ALL {
            let definition = registry.load(template).expect("definition must load");
            assert_eq!(definition.id, template);
            assert!(!definition.ports.is_empty());
            assert!(!definition.surfaces.is_empty());
            assert!(definition.surfaces.iter().any(|surface| surface.collision));
        }
    }

    #[test]
    fn quarter_turn_rotates_ports_bounds_and_collision_geometry() {
        let registry = RoomRegistry::default();
        let definition = registry.load(RoomTemplate::StraightCorridor).unwrap();
        let transform = RoomTransform {
            translation: Vec2::new(100.0, 50.0),
            rotation: QuarterTurn::R90,
        };
        let port = world_port(RoomId(7), definition, transform, PortId(1)).unwrap();
        assert_eq!(port.position, Vec2::new(100.0, 150.0));
        assert_eq!(port.facing, Cardinal::North);

        let collisions = generate_collisions(RoomId(7), definition, transform);
        assert_eq!(collisions.len(), definition.surfaces.len());
        assert_eq!(
            collisions[0].size,
            Vec2::new(definition.surfaces[0].size.y, definition.surfaces[0].size.x)
        );
    }

    #[test]
    fn attached_room_is_exactly_aligned_and_connected() {
        let registry = RoomRegistry::default();
        let mut world = RoomWorld::empty();
        let first = world.spawn_room(RoomTemplate::StraightCorridor, RoomTransform::default());
        let second = world
            .attach_room(
                &registry,
                PortRef {
                    room: first,
                    port: PortId(1),
                },
                RoomTemplate::Corner,
                PortId(0),
            )
            .unwrap();
        let a = world
            .port(
                &registry,
                PortRef {
                    room: first,
                    port: PortId(1),
                },
            )
            .unwrap();
        let b = world
            .port(
                &registry,
                PortRef {
                    room: second,
                    port: PortId(0),
                },
            )
            .unwrap();

        assert_eq!(a.position, b.position);
        assert_eq!(a.facing.opposite(), b.facing);
        assert_eq!(a.kind, b.kind);
        assert_eq!(world.connections.len(), 1);
    }

    #[test]
    fn connection_validation_rejects_invalid_pairs() {
        let registry = RoomRegistry::default();
        let mut world = RoomWorld::empty();
        let a = world.spawn_room(RoomTemplate::StraightCorridor, RoomTransform::default());
        let b = world.spawn_room(
            RoomTemplate::ControlRoom,
            RoomTransform {
                translation: Vec2::new(500.0, 0.0),
                rotation: QuarterTurn::R0,
            },
        );
        assert_eq!(
            world.connect(
                &registry,
                PortRef {
                    room: a,
                    port: PortId(0)
                },
                PortRef {
                    room: b,
                    port: PortId(2)
                }
            ),
            Err(ConnectionError::TypeMismatch)
        );
        assert_eq!(
            world.connect(
                &registry,
                PortRef {
                    room: a,
                    port: PortId(1)
                },
                PortRef {
                    room: b,
                    port: PortId(0)
                }
            ),
            Err(ConnectionError::PositionMismatch)
        );

        let c = world.spawn_room(RoomTemplate::StraightCorridor, RoomTransform::default());
        assert_eq!(
            world.connect(
                &registry,
                PortRef {
                    room: a,
                    port: PortId(1)
                },
                PortRef {
                    room: c,
                    port: PortId(1)
                }
            ),
            Err(ConnectionError::FacingMismatch)
        );

        let attached = world
            .attach_room(
                &registry,
                PortRef {
                    room: a,
                    port: PortId(1),
                },
                RoomTemplate::StraightCorridor,
                PortId(0),
            )
            .unwrap();
        assert_eq!(
            world.connect(
                &registry,
                PortRef {
                    room: a,
                    port: PortId(1)
                },
                PortRef {
                    room: attached,
                    port: PortId(1)
                }
            ),
            Err(ConnectionError::PortOccupied)
        );
    }

    #[test]
    fn authored_facility_contains_all_templates_and_valid_connections() {
        let registry = RoomRegistry::default();
        let world = RoomWorld::authored_facility(&registry);
        assert_eq!(world.rooms.len(), 8);
        assert_eq!(world.connections.len(), 7);
        let templates = world
            .rooms
            .values()
            .map(|room| room.template)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(templates.len(), 8);
        for connection in &world.connections {
            let a = world.port(&registry, connection.a).unwrap();
            let b = world.port(&registry, connection.b).unwrap();
            assert_eq!(a.position, b.position);
            assert_eq!(a.facing.opposite(), b.facing);
            assert_eq!(a.kind, b.kind);
        }
    }

    #[test]
    fn collision_generation_tracks_room_ownership_and_rotation() {
        let registry = RoomRegistry::default();
        let mut world = RoomWorld::empty();
        let room = world.spawn_room(
            RoomTemplate::MachineChamber,
            RoomTransform {
                translation: Vec2::new(40.0, -20.0),
                rotation: QuarterTurn::R270,
            },
        );
        let collisions = world.collisions(&registry);
        assert!(!collisions.is_empty());
        assert!(collisions.iter().all(|collision| collision.room == room));
        assert!(
            collisions
                .iter()
                .enumerate()
                .all(|(index, collision)| collision.surface_index == index)
        );
    }

    #[test]
    fn replacement_preserves_compatible_connections_and_invalidates_others() {
        let registry = RoomRegistry::default();
        let mut world = RoomWorld::authored_facility(&registry);
        let machine = RoomId(4);
        let preserved = world
            .replace_room(&registry, machine, RoomTemplate::StraightCorridor)
            .unwrap();
        assert_eq!(preserved, 2);
        assert_eq!(
            world.room(machine).unwrap().template,
            RoomTemplate::StraightCorridor
        );
        assert_eq!(
            world
                .connections
                .iter()
                .filter(|connection| connection.contains(machine))
                .count(),
            2
        );

        let corner = RoomId(1);
        let preserved = world
            .replace_room(&registry, corner, RoomTemplate::PlatformRoom)
            .unwrap();
        assert!(preserved <= 1);
        assert_eq!(world.room(corner).unwrap().revision, 1);
    }

    #[test]
    fn despawning_room_removes_every_owned_connection() {
        let registry = RoomRegistry::default();
        let mut world = RoomWorld::authored_facility(&registry);
        let target = RoomId(3);
        assert!(
            world
                .connections
                .iter()
                .any(|connection| connection.contains(target))
        );
        assert!(world.despawn_room(target));
        assert!(!world.rooms.contains_key(&target));
        assert!(
            !world
                .connections
                .iter()
                .any(|connection| connection.contains(target))
        );
    }
}
