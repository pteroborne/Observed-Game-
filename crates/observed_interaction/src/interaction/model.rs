//! The logical interaction state machine's **data model**: players, objects,
//! policies (instant / exclusive-hold / shared-hold), carryables, equipment sockets,
//! climb points, and the event log. Pure logic (`glam`); the `Resource` derive is
//! behind the `bevy` feature. The engine ([`super::engine`]) drives it.

use std::collections::BTreeSet;

use glam::Vec2;
pub use observed_core::EquipmentId;
use observed_core::PlayerId;

pub const PLAYER_COUNT: usize = 4;
pub const PLAYERS: [PlayerId; PLAYER_COUNT] = [PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InteractionId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SocketId(pub u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionPolicy {
    Instant,
    ExclusiveHold,
    SharedHold { minimum_users: usize },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemLocation {
    Ground(Vec2),
    Carried(PlayerId),
    Socketed(SocketId),
}

#[derive(Clone, Debug, PartialEq)]
pub enum InteractionKind {
    Lever {
        active: bool,
    },
    Door {
        open: bool,
        requires_lever: Option<InteractionId>,
    },
    TimedControl {
        duration: f32,
        progress: f32,
        completions: u32,
    },
    TwoPlayerControl {
        duration: f32,
        progress: f32,
        completions: u32,
    },
    Carryable {
        equipment: EquipmentId,
        location: ItemLocation,
    },
    EquipmentSocket {
        socket: SocketId,
        accepts: EquipmentId,
        inserted: Option<EquipmentId>,
        operations: u32,
    },
    ClimbPoint {
        uses: u32,
    },
}

impl InteractionKind {
    pub fn state_label(&self) -> String {
        match self {
            Self::Lever { active } => if *active { "ACTIVE" } else { "INACTIVE" }.to_string(),
            Self::Door { open, .. } => if *open { "OPEN" } else { "CLOSED" }.to_string(),
            Self::TimedControl {
                duration,
                progress,
                completions,
            } => format!("{progress:.1}/{duration:.1}s • complete {completions}"),
            Self::TwoPlayerControl {
                duration,
                progress,
                completions,
            } => format!("{progress:.1}/{duration:.1}s • complete {completions}"),
            Self::Carryable { location, .. } => match location {
                ItemLocation::Ground(_) => "ON GROUND".to_string(),
                ItemLocation::Carried(player) => format!("CARRIED BY {}", player.label()),
                ItemLocation::Socketed(socket) => format!("SOCKETED {}", socket.0),
            },
            Self::EquipmentSocket {
                inserted,
                operations,
                ..
            } => inserted.map_or_else(
                || format!("EMPTY • operated {operations}"),
                |equipment| format!("EQUIPMENT {} • operated {operations}", equipment.0),
            ),
            Self::ClimbPoint { uses } => format!("CLIMBED {uses}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct InteractionObject {
    pub id: InteractionId,
    pub name: &'static str,
    pub position: Vec2,
    pub radius: f32,
    pub policy: InteractionPolicy,
    pub kind: InteractionKind,
    pub active_users: BTreeSet<PlayerId>,
}

#[derive(Clone, Copy, Debug)]
pub struct InteractionPlayer {
    pub id: PlayerId,
    pub position: Vec2,
    pub spawn_position: Vec2,
    pub active_target: Option<InteractionId>,
    pub carrying: Option<EquipmentId>,
    pub climb_uses: u32,
}

impl InteractionPlayer {
    pub fn new(id: PlayerId, position: Vec2) -> Self {
        Self {
            id,
            position,
            spawn_position: position,
            active_target: None,
            carrying: None,
            climb_uses: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InteractionEvent {
    Activated {
        player: PlayerId,
        target: InteractionId,
    },
    Started {
        player: PlayerId,
        target: InteractionId,
    },
    Joined {
        player: PlayerId,
        target: InteractionId,
    },
    Completed {
        target: InteractionId,
    },
    Interrupted {
        player: PlayerId,
        target: InteractionId,
    },
    Denied {
        player: PlayerId,
        target: InteractionId,
        reason: &'static str,
    },
    PickedUp {
        player: PlayerId,
        equipment: EquipmentId,
    },
    Dropped {
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
        socket: SocketId,
    },
    Operated {
        player: PlayerId,
        target: InteractionId,
    },
    Climbed {
        player: PlayerId,
        target: InteractionId,
    },
}

impl InteractionEvent {
    pub fn label(&self) -> String {
        match self {
            Self::Activated { player, target } => {
                format!("{} activated target {}.", player.label(), target.0)
            }
            Self::Started { player, target } => {
                format!("{} started target {}.", player.label(), target.0)
            }
            Self::Joined { player, target } => {
                format!("{} joined shared target {}.", player.label(), target.0)
            }
            Self::Completed { target } => format!("Target {} completed.", target.0),
            Self::Interrupted { player, target } => {
                format!("{} interrupted target {}.", player.label(), target.0)
            }
            Self::Denied {
                player,
                target,
                reason,
            } => format!(
                "{} denied at target {}: {reason}.",
                player.label(),
                target.0
            ),
            Self::PickedUp { player, equipment } => {
                format!("{} picked up equipment {}.", player.label(), equipment.0)
            }
            Self::Dropped { player, equipment } => {
                format!("{} dropped equipment {}.", player.label(), equipment.0)
            }
            Self::Socketed {
                player,
                equipment,
                socket,
            } => format!(
                "{} socketed equipment {} into socket {}.",
                player.label(),
                equipment.0,
                socket.0
            ),
            Self::Recovered {
                player,
                equipment,
                socket,
            } => format!(
                "{} recovered equipment {} from socket {}.",
                player.label(),
                equipment.0,
                socket.0
            ),
            Self::Operated { player, target } => {
                format!("{} operated target {}.", player.label(), target.0)
            }
            Self::Climbed { player, target } => {
                format!("{} climbed target {}.", player.label(), target.0)
            }
        }
    }
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct InteractionWorld {
    pub players: Vec<InteractionPlayer>,
    pub objects: Vec<InteractionObject>,
    pub recent_events: Vec<InteractionEvent>,
    pub total_events: u32,
}

impl InteractionWorld {
    pub fn authored_lab() -> Self {
        let players = vec![
            InteractionPlayer::new(PlayerId(0), Vec2::new(-520.0, -250.0)),
            InteractionPlayer::new(PlayerId(1), Vec2::new(-450.0, -250.0)),
            InteractionPlayer::new(PlayerId(2), Vec2::new(300.0, 220.0)),
            InteractionPlayer::new(PlayerId(3), Vec2::new(370.0, 220.0)),
        ];
        let objects = vec![
            object(
                0,
                "POWER LEVER",
                Vec2::new(-500.0, -80.0),
                InteractionPolicy::Instant,
                InteractionKind::Lever { active: false },
            ),
            object(
                1,
                "POWERED DOOR",
                Vec2::new(-250.0, -80.0),
                InteractionPolicy::Instant,
                InteractionKind::Door {
                    open: false,
                    requires_lever: Some(InteractionId(0)),
                },
            ),
            object(
                2,
                "TIMED CONTROL",
                Vec2::new(20.0, -80.0),
                InteractionPolicy::ExclusiveHold,
                InteractionKind::TimedControl {
                    duration: 1.5,
                    progress: 0.0,
                    completions: 0,
                },
            ),
            object(
                3,
                "TWO-PLAYER CONTROL",
                Vec2::new(320.0, -80.0),
                InteractionPolicy::SharedHold { minimum_users: 2 },
                InteractionKind::TwoPlayerControl {
                    duration: 1.25,
                    progress: 0.0,
                    completions: 0,
                },
            ),
            object(
                4,
                "PORTABLE BATTERY",
                Vec2::new(-360.0, 190.0),
                InteractionPolicy::Instant,
                InteractionKind::Carryable {
                    equipment: EquipmentId(0),
                    location: ItemLocation::Ground(Vec2::new(-360.0, 190.0)),
                },
            ),
            object(
                5,
                "EQUIPMENT SOCKET",
                Vec2::new(-60.0, 190.0),
                InteractionPolicy::Instant,
                InteractionKind::EquipmentSocket {
                    socket: SocketId(0),
                    accepts: EquipmentId(0),
                    inserted: None,
                    operations: 0,
                },
            ),
            object(
                6,
                "CLIMB MARKER",
                Vec2::new(250.0, 190.0),
                InteractionPolicy::Instant,
                InteractionKind::ClimbPoint { uses: 0 },
            ),
        ];
        Self {
            players,
            objects,
            recent_events: Vec::new(),
            total_events: 0,
        }
    }

    pub fn player(&self, id: PlayerId) -> Option<&InteractionPlayer> {
        self.players.iter().find(|player| player.id == id)
    }

    pub fn player_mut(&mut self, id: PlayerId) -> Option<&mut InteractionPlayer> {
        self.players.iter_mut().find(|player| player.id == id)
    }

    pub fn object(&self, id: InteractionId) -> Option<&InteractionObject> {
        self.objects.iter().find(|object| object.id == id)
    }

    pub fn object_mut(&mut self, id: InteractionId) -> Option<&mut InteractionObject> {
        self.objects.iter_mut().find(|object| object.id == id)
    }

    pub fn reset(&mut self) {
        *self = Self::authored_lab();
    }

    pub fn push_event(&mut self, event: InteractionEvent) {
        self.total_events += 1;
        self.recent_events.push(event);
        if self.recent_events.len() > 8 {
            self.recent_events.remove(0);
        }
    }
}

fn object(
    id: u16,
    name: &'static str,
    position: Vec2,
    policy: InteractionPolicy,
    kind: InteractionKind,
) -> InteractionObject {
    InteractionObject {
        id: InteractionId(id),
        name,
        position,
        radius: 92.0,
        policy,
        kind,
        active_users: BTreeSet::new(),
    }
}
