//! Session module types and configurations.

use observed_core::{PlayerId, TeamId};
use std::collections::BTreeSet;

pub const ROSTER_SIZE: usize = 4;
pub const TEAM_COUNT: usize = 2;
pub const TEAM_SIZE: usize = ROSTER_SIZE / TEAM_COUNT;
pub const COUNTDOWN_TICKS: u8 = 3;
pub const RECONNECT_GRACE_TICKS: u8 = 4;
pub const POST_MATCH_TICKS: u8 = 3;
pub const SKILL_WINDOW: u16 = 400;
pub const PROTOCOL_VERSION: u16 = 1;
pub const CURRENT_BUILD: u32 = 0x2026_0619;

pub mod connection;
pub mod lobby;
pub mod matchmaking;

#[cfg(test)]
pub mod test;

pub use lobby::{Session, SessionLabWorld};
pub use matchmaking::{Matchmaker, QueueError, QueueTicket};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AccountId(pub u16);

impl AccountId {
    pub fn label(self) -> String {
        format!("U{}", self.0 + 1)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(pub u32);

impl SessionId {
    pub fn label(self) -> String {
        format!("S{:04}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Region {
    East,
    West,
}

impl Region {
    pub fn label(self) -> &'static str {
        match self {
            Self::East => "EAST",
            Self::West => "WEST",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Participant {
    pub account: AccountId,
    pub player: PlayerId,
    pub team: TeamId,
    pub rating: u16,
    pub connection: ConnectionState,
    pub ready: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionPhase {
    Lobby,
    Countdown {
        remaining: u8,
    },
    InMatch {
        frame: u32,
    },
    ReconnectGrace {
        missing: Vec<AccountId>,
        remaining: u8,
        resume_frame: u32,
    },
    PostMatch {
        remaining: u8,
    },
    Closed {
        reason: &'static str,
    },
}

impl SessionPhase {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Lobby => "LOBBY",
            Self::Countdown { .. } => "COUNTDOWN",
            Self::InMatch { .. } => "IN MATCH",
            Self::ReconnectGrace { .. } => "RECONNECT",
            Self::PostMatch { .. } => "POST MATCH",
            Self::Closed { .. } => "CLOSED",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchSeat {
    pub account: AccountId,
    pub player: PlayerId,
    pub team: TeamId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchManifest {
    pub session: SessionId,
    pub match_number: u32,
    pub lockstep_session: u32,
    pub seed: u64,
    pub protocol_version: u16,
    pub build: u32,
    pub host: AccountId,
    pub roster: Vec<LaunchSeat>,
}

impl LaunchManifest {
    pub fn valid(&self) -> bool {
        if self.roster.len() != ROSTER_SIZE
            || self.protocol_version != PROTOCOL_VERSION
            || self.build != CURRENT_BUILD
        {
            return false;
        }
        let accounts: BTreeSet<AccountId> = self.roster.iter().map(|seat| seat.account).collect();
        let players: BTreeSet<PlayerId> = self.roster.iter().map(|seat| seat.player).collect();
        if accounts.len() != ROSTER_SIZE || players.len() != ROSTER_SIZE {
            return false;
        }
        if !accounts.contains(&self.host) || self.lockstep_session == 0 {
            return false;
        }
        (0..TEAM_COUNT).all(|team| {
            self.roster
                .iter()
                .filter(|seat| seat.team == TeamId(team as u8))
                .count()
                == TEAM_SIZE
        })
    }
}
