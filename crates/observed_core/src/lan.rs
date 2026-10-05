//! Transport ownership, distinct from embodied simulation players.
use crate::{PlayerId, TeamId};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LanRole {
    #[default]
    Observer,
    Architect,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LanSeatId {
    Observer(PlayerId),
    Architect(TeamId),
}
impl LanSeatId {
    pub const fn observer(self) -> Option<PlayerId> {
        match self {
            Self::Observer(player) => Some(player),
            Self::Architect(_) => None,
        }
    }
    pub const fn architect(self) -> Option<TeamId> {
        match self {
            Self::Architect(team) => Some(team),
            Self::Observer(_) => None,
        }
    }
    pub const fn token_key(self) -> u64 {
        match self {
            Self::Observer(player) => player.0 as u64,
            Self::Architect(team) => 0x1_0000 + team.0 as u64,
        }
    }
}
