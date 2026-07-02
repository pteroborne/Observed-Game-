//! Phase 27: the competitive hybrid match spatial and action model.

use crate::competition::RaceAction;
use crate::maze::Tile;
use glam::Vec3;
use observed_core::{RoomId, TeamId};
use observed_observation::DoorId;

pub mod match_state;
pub mod replay;
pub mod round_step;

#[cfg(test)]
pub mod test;

pub use match_state::HybridMatch;
pub use replay::HybridTape;

pub const LOCAL_TEAM: TeamId = TeamId(0);
pub const CONTROL_ROOM: RoomId = RoomId(3);
pub const MAX_ROUNDS: usize = 64;
pub const SPINE_RADIUS: usize = 1;
pub const SIDE_RADIUS: usize = 0;
pub const CONTROL_RADIUS: f32 = crate::maze::TILE_SIZE * 1.25;
pub const WALL_HEIGHT: f32 = 4.0;
pub const VIEW_RANGE_TILES: f32 = 16.0;
pub const VIEW_HALF_DEG: f32 = 48.0;
pub const TRAP_PERIOD_TICKS: u64 = 120;
pub const TRAP_ACTIVE_TICKS: u64 = 72;
pub const TRAP_SETBACK_TICKS: u16 = 30;
pub const REROUTE_FEEDBACK_TICKS: u16 = 45;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalAction {
    Advance,
    Seize,
    Wait,
}

impl LocalAction {
    pub fn race_action(self) -> RaceAction {
        match self {
            Self::Seize => RaceAction::Seize,
            Self::Advance | Self::Wait => RaceAction::Advance,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HybridFrame {
    pub local: LocalAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorridorRoute {
    pub key: (DoorId, DoorId),
    pub rooms: (RoomId, RoomId),
    pub path: Vec<(usize, usize)>,
    pub spine: bool,
    pub safe_path: Vec<(usize, usize)>,
    pub trap_tiles: Vec<(usize, usize)>,
}

/// Comparable fingerprint of the complete integration state.
#[derive(Clone, Debug, PartialEq)]
pub struct HybridSnapshot {
    pub round: u32,
    pub local_room: RoomId,
    pub player_room: Option<RoomId>,
    pub body_position: Vec3,
    pub body_yaw: f32,
    pub body_pitch: f32,
    pub team_rooms: Vec<RoomId>,
    pub roles: Vec<u8>,
    pub placements: Vec<Option<u8>>,
    pub graph_links: Vec<DoorId>,
    pub control_holder: Option<TeamId>,
    pub purge_line: f32,
    pub winner: Option<TeamId>,
    pub finished: bool,
    pub rendered_routes: Vec<(DoorId, DoorId)>,
    pub target_routes: Vec<(DoorId, DoorId)>,
    pub maze_tiles: Vec<Tile>,
    pub elevation_steps: Vec<u8>,
    pub safe_tiles: Vec<(usize, usize)>,
    pub trap_tiles: Vec<(usize, usize)>,
    pub reroute_commits: u32,
}
