//! **Equipment persistence** as a production model: equipment whose state (carried,
//! deployed, socketed, on the ground, powered) is independent of any render entity —
//! it survives a carrier leaving, a room being replaced, and its visual being
//! despawned. Promoted out of `equipment_lab` in refactor R8; the lab is the
//! projection. Pure logic (`glam`); the `Resource` derive is behind the `bevy` feature.

use glam::Vec2;
use observed_core::{EquipmentId, PlayerId, RoomId};

/// How close a player must be to pick up, deploy, socket, or recover equipment.
pub const REACH_RADIUS: f32 = 96.0;
/// How close two players must be to hand equipment over.
pub const HANDOFF_RADIUS: f32 = 140.0;
/// Charge drained per second while a battery actively powers a room. A fresh
/// authored battery starts at 0.9 charge, so a room stays lit for ~18s.
pub const DRAIN_RATE: f32 = 0.05;

observed_core::domain_id!(
    /// A lab-local connection point. Equipment sockets are distinct from the
    /// interaction sockets in `interaction_lab`; this identifier is promoted into
    /// `observed_core` only once a second system genuinely shares it.
    SocketId,
    u16
);

impl SocketId {
    pub const fn as_u16(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EquipmentKind {
    Battery,
    StructuralJack,
    CableSpool,
    DeployableLight,
    GrappleDevice,
}

impl EquipmentKind {
    pub fn label(self) -> &'static str {
        match self {
            EquipmentKind::Battery => "BATTERY",
            EquipmentKind::StructuralJack => "STRUCTURAL JACK",
            EquipmentKind::CableSpool => "CABLE SPOOL",
            EquipmentKind::DeployableLight => "DEPLOYABLE LIGHT",
            EquipmentKind::GrappleDevice => "GRAPPLE DEVICE",
        }
    }

    /// Equipment that only functions while it receives power.
    pub fn needs_power(self) -> bool {
        matches!(self, EquipmentKind::DeployableLight)
    }
}

/// Where a piece of equipment currently lives. State is independent of any
/// render entity: equipment keeps its location while its visual is despawned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EquipmentLocation {
    Ground { room: RoomId, position: Vec2 },
    Carried { player: PlayerId },
    Socketed { socket: SocketId },
    Deployed { room: RoomId, position: Vec2 },
}

impl EquipmentLocation {
    /// The room + position for equipment resting in the world (not carried or
    /// socketed).
    pub fn placed(self) -> Option<(RoomId, Vec2)> {
        match self {
            EquipmentLocation::Ground { room, position }
            | EquipmentLocation::Deployed { room, position } => Some((room, position)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Equipment {
    pub id: EquipmentId,
    pub kind: EquipmentKind,
    pub location: EquipmentLocation,
    pub charge: f32,
    pub powered: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Socket {
    pub id: SocketId,
    pub room: RoomId,
    pub position: Vec2,
    pub accepts: EquipmentKind,
    pub provides_power: bool,
    pub occupied: Option<EquipmentId>,
}

#[derive(Clone, Copy, Debug)]
pub struct EquipPlayer {
    pub id: PlayerId,
    pub position: Vec2,
    pub spawn_position: Vec2,
    pub present: bool,
}

impl EquipPlayer {
    fn new(id: PlayerId, position: Vec2) -> Self {
        Self {
            id,
            position,
            spawn_position: position,
            present: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Room {
    pub id: RoomId,
    pub center: Vec2,
    pub half_size: Vec2,
    /// Where equipment is relocated to when the room is replaced.
    pub fallback: Vec2,
}

impl Room {
    pub fn contains(self, point: Vec2) -> bool {
        (point.x - self.center.x).abs() <= self.half_size.x
            && (point.y - self.center.y).abs() <= self.half_size.y
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EquipmentEvent {
    PickedUp {
        player: PlayerId,
        equipment: EquipmentId,
    },
    Dropped {
        player: PlayerId,
        equipment: EquipmentId,
    },
    HandedOff {
        from: PlayerId,
        to: PlayerId,
        equipment: EquipmentId,
    },
    Deployed {
        player: PlayerId,
        equipment: EquipmentId,
    },
    Socketed {
        player: PlayerId,
        equipment: EquipmentId,
        socket: SocketId,
    },
    Recovered {
        player: PlayerId,
        equipment: EquipmentId,
    },
    LostPower {
        equipment: EquipmentId,
    },
    PlayerLeft {
        player: PlayerId,
        dropped: Option<EquipmentId>,
    },
    PlayerReturned {
        player: PlayerId,
    },
    RoomReplaced {
        room: RoomId,
        relocated: u32,
    },
    Denied {
        reason: &'static str,
    },
}

impl EquipmentEvent {
    pub fn label(self) -> String {
        match self {
            EquipmentEvent::PickedUp { player, equipment } => {
                format!("{} picked up equipment {}.", player.label(), equipment.0)
            }
            EquipmentEvent::Dropped { player, equipment } => {
                format!("{} dropped equipment {}.", player.label(), equipment.0)
            }
            EquipmentEvent::HandedOff {
                from,
                to,
                equipment,
            } => format!(
                "{} handed equipment {} to {}.",
                from.label(),
                equipment.0,
                to.label()
            ),
            EquipmentEvent::Deployed { player, equipment } => {
                format!("{} deployed equipment {}.", player.label(), equipment.0)
            }
            EquipmentEvent::Socketed {
                player,
                equipment,
                socket,
            } => format!(
                "{} socketed equipment {} into socket {}.",
                player.label(),
                equipment.0,
                socket.0
            ),
            EquipmentEvent::Recovered { player, equipment } => {
                format!("{} recovered equipment {}.", player.label(), equipment.0)
            }
            EquipmentEvent::LostPower { equipment } => {
                format!("Equipment {} lost power.", equipment.0)
            }
            EquipmentEvent::PlayerLeft { player, dropped } => match dropped {
                Some(equipment) => format!(
                    "{} left; equipment {} stayed on the ground.",
                    player.label(),
                    equipment.0
                ),
                None => format!("{} left the facility.", player.label()),
            },
            EquipmentEvent::PlayerReturned { player } => {
                format!("{} returned to the facility.", player.label())
            }
            EquipmentEvent::RoomReplaced { room, relocated } => format!(
                "Room {} replaced; {relocated} item(s) relocated safely.",
                room.0
            ),
            EquipmentEvent::Denied { reason } => format!("Denied: {reason}."),
        }
    }
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct EquipmentWorld {
    pub rooms: Vec<Room>,
    pub players: Vec<EquipPlayer>,
    pub sockets: Vec<Socket>,
    pub equipment: Vec<Equipment>,
    pub recent_events: Vec<EquipmentEvent>,
    pub total_events: u32,
    pub room_replacements: u32,
}

impl EquipmentWorld {
    pub fn authored_lab() -> Self {
        let room_a = RoomId(0);
        let room_b = RoomId(1);
        let rooms = vec![
            Room {
                id: room_a,
                center: Vec2::new(-300.0, 0.0),
                half_size: Vec2::new(270.0, 210.0),
                fallback: Vec2::new(-300.0, -150.0),
            },
            Room {
                id: room_b,
                center: Vec2::new(320.0, 0.0),
                half_size: Vec2::new(270.0, 210.0),
                fallback: Vec2::new(320.0, -150.0),
            },
        ];
        let players = vec![
            EquipPlayer::new(PlayerId(0), Vec2::new(-360.0, -120.0)),
            EquipPlayer::new(PlayerId(1), Vec2::new(250.0, -120.0)),
            EquipPlayer::new(PlayerId(2), Vec2::new(400.0, 120.0)),
            EquipPlayer::new(PlayerId(3), Vec2::new(-200.0, 130.0)),
        ];
        let sockets = vec![
            Socket {
                id: SocketId(0),
                room: room_a,
                position: Vec2::new(-380.0, 60.0),
                accepts: EquipmentKind::Battery,
                provides_power: true,
                occupied: Some(EquipmentId(0)),
            },
            Socket {
                id: SocketId(1),
                room: room_b,
                position: Vec2::new(300.0, 70.0),
                accepts: EquipmentKind::Battery,
                provides_power: true,
                occupied: None,
            },
            Socket {
                id: SocketId(2),
                room: room_a,
                position: Vec2::new(-180.0, 90.0),
                accepts: EquipmentKind::GrappleDevice,
                provides_power: false,
                occupied: None,
            },
        ];
        let equipment = vec![
            Equipment {
                id: EquipmentId(0),
                kind: EquipmentKind::Battery,
                location: EquipmentLocation::Socketed {
                    socket: SocketId(0),
                },
                charge: 0.9,
                powered: true,
            },
            Equipment {
                id: EquipmentId(1),
                kind: EquipmentKind::DeployableLight,
                location: EquipmentLocation::Deployed {
                    room: room_a,
                    position: Vec2::new(-300.0, 90.0),
                },
                charge: 0.0,
                powered: true,
            },
            Equipment {
                id: EquipmentId(2),
                kind: EquipmentKind::StructuralJack,
                location: EquipmentLocation::Deployed {
                    room: room_a,
                    position: Vec2::new(-150.0, -80.0),
                },
                charge: 0.0,
                powered: false,
            },
            Equipment {
                id: EquipmentId(3),
                kind: EquipmentKind::CableSpool,
                location: EquipmentLocation::Ground {
                    room: room_b,
                    position: Vec2::new(330.0, -60.0),
                },
                charge: 0.0,
                powered: false,
            },
            Equipment {
                id: EquipmentId(4),
                kind: EquipmentKind::GrappleDevice,
                location: EquipmentLocation::Carried {
                    player: PlayerId(0),
                },
                charge: 0.0,
                powered: false,
            },
        ];

        let mut world = Self {
            rooms,
            players,
            sockets,
            equipment,
            recent_events: Vec::new(),
            total_events: 0,
            room_replacements: 0,
        };
        world.recompute_power();
        world
    }

    pub fn reset(&mut self) {
        *self = Self::authored_lab();
    }

    // -- lookups ----------------------------------------------------------

    pub fn player(&self, id: PlayerId) -> Option<&EquipPlayer> {
        self.players.iter().find(|player| player.id == id)
    }

    pub fn equipment(&self, id: EquipmentId) -> Option<&Equipment> {
        self.equipment.iter().find(|item| item.id == id)
    }

    pub fn socket(&self, id: SocketId) -> Option<&Socket> {
        self.sockets.iter().find(|socket| socket.id == id)
    }

    pub fn room(&self, id: RoomId) -> Option<&Room> {
        self.rooms.iter().find(|room| room.id == id)
    }

    pub fn room_of(&self, point: Vec2) -> Option<RoomId> {
        self.rooms
            .iter()
            .find(|room| room.contains(point))
            .map(|room| room.id)
    }

    pub fn carried_by(&self, player: PlayerId) -> Option<EquipmentId> {
        self.equipment
            .iter()
            .find(|item| item.location == EquipmentLocation::Carried { player })
            .map(|item| item.id)
    }

    fn equipment_index(&self, id: EquipmentId) -> Option<usize> {
        self.equipment.iter().position(|item| item.id == id)
    }

    fn socket_index(&self, id: SocketId) -> Option<usize> {
        self.sockets.iter().position(|socket| socket.id == id)
    }

    fn player_index(&self, id: PlayerId) -> Option<usize> {
        self.players.iter().position(|player| player.id == id)
    }

    // -- operations -------------------------------------------------------

    /// Pick up the nearest loose item in the player's room.
    pub fn pick_up(&mut self, player: PlayerId) -> bool {
        let Some(actor) = self.player(player).copied().filter(|p| p.present) else {
            return self.deny("no such present player");
        };
        if self.carried_by(player).is_some() {
            return self.deny("already carrying");
        }
        let Some(room) = self.room_of(actor.position) else {
            return self.deny("outside any room");
        };
        let nearest = self
            .equipment
            .iter()
            .filter_map(|item| {
                let (item_room, position) = item.location.placed()?;
                (item_room == room && position.distance(actor.position) <= REACH_RADIUS)
                    .then_some((item.id, position.distance(actor.position)))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id);
        let Some(id) = nearest else {
            return self.deny("nothing in reach");
        };
        let index = self.equipment_index(id).expect("equipment exists");
        self.equipment[index].location = EquipmentLocation::Carried { player };
        self.equipment[index].powered = false;
        self.push_event(EquipmentEvent::PickedUp {
            player,
            equipment: id,
        });
        self.recompute_power();
        true
    }

    /// Drop the carried item onto the ground at the player's feet.
    pub fn drop_carried(&mut self, player: PlayerId) -> bool {
        let Some(actor) = self.player(player).copied() else {
            return self.deny("no such player");
        };
        let Some(id) = self.carried_by(player) else {
            return self.deny("not carrying");
        };
        let room = self
            .room_of(actor.position)
            .or_else(|| self.rooms.first().map(|room| room.id))
            .expect("at least one room");
        let index = self.equipment_index(id).expect("equipment exists");
        self.equipment[index].location = EquipmentLocation::Ground {
            room,
            position: actor.position,
        };
        self.push_event(EquipmentEvent::Dropped {
            player,
            equipment: id,
        });
        true
    }

    /// Hand the carried item to the nearest other present, empty-handed player.
    pub fn hand_off(&mut self, from: PlayerId) -> bool {
        let Some(actor) = self.player(from).copied().filter(|p| p.present) else {
            return self.deny("no such present player");
        };
        let Some(id) = self.carried_by(from) else {
            return self.deny("not carrying");
        };
        let target = self
            .players
            .iter()
            .filter(|other| {
                other.id != from
                    && other.present
                    && other.position.distance(actor.position) <= HANDOFF_RADIUS
                    && self.carried_by(other.id).is_none()
            })
            .min_by(|a, b| {
                a.position
                    .distance(actor.position)
                    .total_cmp(&b.position.distance(actor.position))
            })
            .map(|other| other.id);
        let Some(to) = target else {
            return self.deny("no empty-handed player in reach");
        };
        let index = self.equipment_index(id).expect("equipment exists");
        self.equipment[index].location = EquipmentLocation::Carried { player: to };
        self.push_event(EquipmentEvent::HandedOff {
            from,
            to,
            equipment: id,
        });
        true
    }

    /// Place the carried item as a deployed object at the player's feet.
    pub fn deploy(&mut self, player: PlayerId) -> bool {
        let Some(actor) = self.player(player).copied().filter(|p| p.present) else {
            return self.deny("no such present player");
        };
        let Some(id) = self.carried_by(player) else {
            return self.deny("not carrying");
        };
        let Some(room) = self.room_of(actor.position) else {
            return self.deny("outside any room");
        };
        let index = self.equipment_index(id).expect("equipment exists");
        self.equipment[index].location = EquipmentLocation::Deployed {
            room,
            position: actor.position,
        };
        self.push_event(EquipmentEvent::Deployed {
            player,
            equipment: id,
        });
        self.recompute_power();
        true
    }

    /// Insert the carried item into the nearest compatible free socket.
    pub fn socket_carried(&mut self, player: PlayerId) -> bool {
        let Some(actor) = self.player(player).copied().filter(|p| p.present) else {
            return self.deny("no such present player");
        };
        let Some(id) = self.carried_by(player) else {
            return self.deny("not carrying");
        };
        let Some(room) = self.room_of(actor.position) else {
            return self.deny("outside any room");
        };
        let kind = self.equipment[self.equipment_index(id).unwrap()].kind;
        let target = self
            .sockets
            .iter()
            .filter(|socket| {
                socket.room == room
                    && socket.occupied.is_none()
                    && socket.accepts == kind
                    && socket.position.distance(actor.position) <= REACH_RADIUS
            })
            .min_by(|a, b| {
                a.position
                    .distance(actor.position)
                    .total_cmp(&b.position.distance(actor.position))
            })
            .map(|socket| socket.id);
        let Some(socket_id) = target else {
            return self.deny("no compatible socket in reach");
        };
        let equipment_index = self.equipment_index(id).unwrap();
        let socket_index = self.socket_index(socket_id).unwrap();
        self.equipment[equipment_index].location =
            EquipmentLocation::Socketed { socket: socket_id };
        self.sockets[socket_index].occupied = Some(id);
        self.push_event(EquipmentEvent::Socketed {
            player,
            equipment: id,
            socket: socket_id,
        });
        self.recompute_power();
        true
    }

    /// Carry-or-socket: socket the carried item if a compatible socket is in
    /// reach, otherwise deploy it. This is what the lab's single "place" key uses.
    pub fn place_carried(&mut self, player: PlayerId) -> bool {
        if self.socket_carried(player) {
            return true;
        }
        self.deploy(player)
    }

    /// Pick the nearest socketed or deployed item back up.
    pub fn recover(&mut self, player: PlayerId) -> bool {
        let Some(actor) = self.player(player).copied().filter(|p| p.present) else {
            return self.deny("no such present player");
        };
        if self.carried_by(player).is_some() {
            return self.deny("already carrying");
        }
        let Some(room) = self.room_of(actor.position) else {
            return self.deny("outside any room");
        };
        let mut best: Option<(usize, f32)> = None;
        for (index, item) in self.equipment.iter().enumerate() {
            let position = match item.location {
                EquipmentLocation::Deployed {
                    room: item_room,
                    position,
                } if item_room == room => Some(position),
                EquipmentLocation::Socketed { socket } => self
                    .sockets
                    .iter()
                    .find(|candidate| candidate.id == socket && candidate.room == room)
                    .map(|candidate| candidate.position),
                _ => None,
            };
            if let Some(position) = position {
                let distance = position.distance(actor.position);
                if distance <= REACH_RADIUS && best.is_none_or(|(_, best)| distance < best) {
                    best = Some((index, distance));
                }
            }
        }
        let Some((index, _)) = best else {
            return self.deny("nothing to recover in reach");
        };
        if let EquipmentLocation::Socketed { socket } = self.equipment[index].location
            && let Some(socket_index) = self.socket_index(socket)
        {
            self.sockets[socket_index].occupied = None;
        }
        let id = self.equipment[index].id;
        self.equipment[index].location = EquipmentLocation::Carried { player };
        self.equipment[index].powered = false;
        self.push_event(EquipmentEvent::Recovered {
            player,
            equipment: id,
        });
        self.recompute_power();
        true
    }

    /// Mark a player as having left the facility (or returned). A leaving player
    /// drops whatever they carry to the ground — equipment persists.
    pub fn set_player_present(&mut self, player: PlayerId, present: bool) -> bool {
        let Some(index) = self.player_index(player) else {
            return self.deny("no such player");
        };
        if self.players[index].present == present {
            return false;
        }
        if present {
            self.players[index].present = true;
            self.push_event(EquipmentEvent::PlayerReturned { player });
            return true;
        }

        let dropped = self.carried_by(player);
        if let Some(id) = dropped {
            let position = self.players[index].position;
            let room = self
                .room_of(position)
                .or_else(|| self.rooms.first().map(|room| room.id))
                .expect("at least one room");
            let equipment_index = self.equipment_index(id).expect("equipment exists");
            self.equipment[equipment_index].location = EquipmentLocation::Ground { room, position };
        }
        self.players[index].present = false;
        self.push_event(EquipmentEvent::PlayerLeft { player, dropped });
        self.recompute_power();
        true
    }

    /// Replace a room's contents. Equipment located in the room is relocated to
    /// the room's fallback point rather than destroyed, and sockets are emptied —
    /// equipment count is invariant across replacement.
    pub fn replace_room(&mut self, room: RoomId) -> u32 {
        self.room_replacements += 1;
        let fallback = self
            .room(room)
            .map(|room| room.fallback)
            .unwrap_or(Vec2::ZERO);
        let sockets_in_room: Vec<SocketId> = self
            .sockets
            .iter()
            .filter(|socket| socket.room == room)
            .map(|socket| socket.id)
            .collect();
        for socket in &mut self.sockets {
            if socket.room == room {
                socket.occupied = None;
            }
        }
        let mut relocated = 0;
        for item in &mut self.equipment {
            let in_room = match item.location {
                EquipmentLocation::Ground { room: r, .. }
                | EquipmentLocation::Deployed { room: r, .. } => r == room,
                EquipmentLocation::Socketed { socket } => sockets_in_room.contains(&socket),
                EquipmentLocation::Carried { .. } => false,
            };
            if in_room {
                item.location = EquipmentLocation::Ground {
                    room,
                    position: fallback,
                };
                item.powered = false;
                relocated += 1;
            }
        }
        self.push_event(EquipmentEvent::RoomReplaced { room, relocated });
        self.recompute_power();
        relocated
    }

    /// Drain actively-sourcing batteries and recompute every `powered` flag.
    pub fn tick_power(&mut self, dt: f32) {
        let power_sockets: Vec<SocketId> = self
            .sockets
            .iter()
            .filter(|socket| socket.provides_power)
            .map(|socket| socket.id)
            .collect();
        let mut lost = Vec::new();
        for item in &mut self.equipment {
            if item.kind == EquipmentKind::Battery
                && let EquipmentLocation::Socketed { socket } = item.location
                && power_sockets.contains(&socket)
                && item.charge > 0.0
            {
                let before = item.charge;
                item.charge = (item.charge - DRAIN_RATE * dt).max(0.0);
                if before > 0.0 && item.charge == 0.0 {
                    lost.push(item.id);
                }
            }
        }
        for id in lost {
            self.push_event(EquipmentEvent::LostPower { equipment: id });
        }
        self.recompute_power();
    }

    pub fn room_powered(&self, room: RoomId) -> bool {
        self.sockets.iter().any(|socket| {
            socket.room == room
                && socket.provides_power
                && socket.occupied.is_some_and(|id| {
                    self.equipment(id).is_some_and(|item| {
                        item.kind == EquipmentKind::Battery && item.charge > 0.0
                    })
                })
        })
    }

    fn recompute_power(&mut self) {
        let flags: Vec<bool> = self
            .equipment
            .iter()
            .map(|item| match item.location {
                EquipmentLocation::Socketed { socket } => {
                    item.kind == EquipmentKind::Battery
                        && item.charge > 0.0
                        && self
                            .sockets
                            .iter()
                            .any(|candidate| candidate.id == socket && candidate.provides_power)
                }
                EquipmentLocation::Deployed { room, .. } => {
                    item.kind.needs_power() && self.room_powered(room)
                }
                _ => false,
            })
            .collect();
        for (item, powered) in self.equipment.iter_mut().zip(flags) {
            item.powered = powered;
        }
    }

    fn deny(&mut self, reason: &'static str) -> bool {
        self.push_event(EquipmentEvent::Denied { reason });
        false
    }

    fn push_event(&mut self, event: EquipmentEvent) {
        self.total_events += 1;
        self.recent_events.push(event);
        if self.recent_events.len() > 8 {
            self.recent_events.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn move_player(world: &mut EquipmentWorld, player: PlayerId, position: Vec2) {
        let index = world.player_index(player).unwrap();
        world.players[index].position = position;
    }

    #[test]
    fn authored_lab_has_the_full_equipment_set() {
        let world = EquipmentWorld::authored_lab();
        assert_eq!(world.equipment.len(), 5);
        assert_eq!(world.players.len(), 4);
        assert_eq!(world.sockets.len(), 3);
        // The socketed battery powers room A's deployed light.
        assert!(world.room_powered(RoomId(0)));
        assert!(!world.room_powered(RoomId(1)));
        let light = world.equipment(EquipmentId(1)).unwrap();
        assert!(light.powered);
    }

    #[test]
    fn pick_up_then_drop_keeps_the_item_in_the_world() {
        let mut world = EquipmentWorld::authored_lab();
        // Stand P2 on the cable spool in room B.
        move_player(&mut world, PlayerId(1), Vec2::new(330.0, -60.0));
        assert!(world.pick_up(PlayerId(1)));
        assert_eq!(world.carried_by(PlayerId(1)), Some(EquipmentId(3)));

        assert!(world.drop_carried(PlayerId(1)));
        assert_eq!(world.carried_by(PlayerId(1)), None);
        assert_eq!(world.equipment.len(), 5);
        assert!(matches!(
            world.equipment(EquipmentId(3)).unwrap().location,
            EquipmentLocation::Ground { .. }
        ));
    }

    #[test]
    fn equipment_persists_when_its_carrier_leaves() {
        let mut world = EquipmentWorld::authored_lab();
        // P1 carries the grapple device in the authored state.
        assert_eq!(world.carried_by(PlayerId(0)), Some(EquipmentId(4)));

        assert!(world.set_player_present(PlayerId(0), false));
        assert!(!world.player(PlayerId(0)).unwrap().present);
        // The item still exists and is now on the ground, not destroyed.
        assert_eq!(world.equipment.len(), 5);
        assert_eq!(world.carried_by(PlayerId(0)), None);
        assert!(matches!(
            world.equipment(EquipmentId(4)).unwrap().location,
            EquipmentLocation::Ground { .. }
        ));
    }

    #[test]
    fn hand_off_transfers_ownership_between_players() {
        let mut world = EquipmentWorld::authored_lab();
        // Bring P4 next to P1 (who carries the grapple device) in room A.
        move_player(&mut world, PlayerId(0), Vec2::new(-300.0, -100.0));
        move_player(&mut world, PlayerId(3), Vec2::new(-300.0, -60.0));

        assert!(world.hand_off(PlayerId(0)));
        assert_eq!(world.carried_by(PlayerId(0)), None);
        assert_eq!(world.carried_by(PlayerId(3)), Some(EquipmentId(4)));
    }

    #[test]
    fn deploy_then_recover_round_trips() {
        let mut world = EquipmentWorld::authored_lab();
        move_player(&mut world, PlayerId(0), Vec2::new(-300.0, -100.0));
        assert!(world.deploy(PlayerId(0)));
        assert!(matches!(
            world.equipment(EquipmentId(4)).unwrap().location,
            EquipmentLocation::Deployed { .. }
        ));
        assert_eq!(world.carried_by(PlayerId(0)), None);

        assert!(world.recover(PlayerId(0)));
        assert_eq!(world.carried_by(PlayerId(0)), Some(EquipmentId(4)));
    }

    #[test]
    fn socketing_a_battery_powers_the_room_and_recovering_frees_the_socket() {
        let mut world = EquipmentWorld::authored_lab();
        // Recover the room-A battery, carry it to room B's empty power socket.
        move_player(&mut world, PlayerId(1), Vec2::new(-380.0, 60.0));
        assert!(world.recover(PlayerId(1)));
        assert_eq!(world.carried_by(PlayerId(1)), Some(EquipmentId(0)));
        assert!(world.socket(SocketId(0)).unwrap().occupied.is_none());

        move_player(&mut world, PlayerId(1), Vec2::new(300.0, 70.0));
        assert!(world.socket_carried(PlayerId(1)));
        assert_eq!(
            world.socket(SocketId(1)).unwrap().occupied,
            Some(EquipmentId(0))
        );
        assert!(world.room_powered(RoomId(1)));
    }

    #[test]
    fn battery_drains_until_it_loses_power() {
        let mut world = EquipmentWorld::authored_lab();
        assert!(world.room_powered(RoomId(0)));

        // Drain for well beyond the charge lifetime (0.9 / 0.05 = 18s).
        for _ in 0..2000 {
            world.tick_power(1.0 / 60.0);
        }
        assert_eq!(world.equipment(EquipmentId(0)).unwrap().charge, 0.0);
        assert!(!world.room_powered(RoomId(0)));
        assert!(!world.equipment(EquipmentId(1)).unwrap().powered);
        assert!(
            world
                .recent_events
                .iter()
                .any(|event| matches!(event, EquipmentEvent::LostPower { .. }))
        );
    }

    #[test]
    fn room_replacement_relocates_equipment_without_losing_any() {
        let mut world = EquipmentWorld::authored_lab();
        let before = world.equipment.len();
        let relocated = world.replace_room(RoomId(0));

        assert!(relocated >= 3); // battery (socketed), light, jack
        assert_eq!(world.equipment.len(), before);
        // Nothing remains socketed in the replaced room.
        assert!(world.socket(SocketId(0)).unwrap().occupied.is_none());
        // Room A is no longer powered and every relocated item sits at the fallback.
        assert!(!world.room_powered(RoomId(0)));
        let fallback = world.room(RoomId(0)).unwrap().fallback;
        for id in [EquipmentId(0), EquipmentId(1), EquipmentId(2)] {
            assert_eq!(
                world.equipment(id).unwrap().location,
                EquipmentLocation::Ground {
                    room: RoomId(0),
                    position: fallback
                }
            );
        }
        // The carried grapple device is untouched by the replacement.
        assert_eq!(world.carried_by(PlayerId(0)), Some(EquipmentId(4)));
    }

    #[test]
    fn reset_restores_the_authored_baseline() {
        let mut world = EquipmentWorld::authored_lab();
        world.replace_room(RoomId(0));
        world.set_player_present(PlayerId(0), false);
        world.reset();

        assert_eq!(world.equipment.len(), 5);
        assert_eq!(world.room_replacements, 0);
        assert!(world.player(PlayerId(0)).unwrap().present);
        assert!(world.room_powered(RoomId(0)));
        assert_eq!(world.carried_by(PlayerId(0)), Some(EquipmentId(4)));
    }
}
