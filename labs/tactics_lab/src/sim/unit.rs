//! A unit: where it stands, what it has left to spend, and what it carries.

use observed_core::{PlayerId, TeamId};
use observed_hex::HexCoord;

/// The player's squad.
pub const PLAYER_TEAM: TeamId = TeamId(0);
/// The rival squad, when the match has one.
pub const RIVAL_TEAM: TeamId = TeamId(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Unit {
    pub id: PlayerId,
    pub team: TeamId,
    pub cell: HexCoord,
    /// Action points remaining this turn.
    pub action_points: u8,
    pub escaped: bool,
}

impl Unit {
    #[must_use]
    pub const fn new(id: PlayerId, team: TeamId, cell: HexCoord, action_points: u8) -> Self {
        Self {
            id,
            team,
            cell,
            action_points,
            escaped: false,
        }
    }

    /// Whether this unit can still act: it has the points and has not left.
    #[must_use]
    pub const fn can_afford(&self, cost: u8) -> bool {
        !self.escaped && self.action_points >= cost
    }

    pub(super) fn spend(&mut self, cost: u8) {
        self.action_points = self.action_points.saturating_sub(cost);
    }
}
