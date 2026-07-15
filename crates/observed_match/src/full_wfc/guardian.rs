use std::collections::{BTreeMap, BTreeSet};

use glam::Vec3;
use observed_core::{PlayerId, RoomId, TeamId};
use observed_facility::full_wfc::{CellCoord, FullWfcWorld};

use super::cell_origin;
use super::model::{GameplayEvent, GameplayEventKind, PlayerState, TeamState};

const MOVE_PERIOD_TICKS: u64 = 120;
const GUARDIAN_SPEED: f32 = 2.5;
const CATCH_DISTANCE: f32 = 1.1;
const FIXED_DT: f32 = 1.0 / 60.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuardianStatus {
    Active,
    FrozenByPlayer,
    FrozenByAnchor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GuardianState {
    pub room: RoomId,
    pub cell: CellCoord,
    pub position: Vec3,
    pub status: GuardianStatus,
    pub target_team: Option<TeamId>,
    pub anchor_ticks: u16,
}

impl GuardianState {
    pub fn new(world: &FullWfcWorld) -> Self {
        let room = world
            .rooms
            .values()
            .find(|room| room.role == observed_facility::map_spec::RoomRole::GuardianControl)
            .or_else(|| world.rooms.values().next())
            .expect("full-WFC world contains rooms");
        Self {
            room: room.id,
            cell: room.coord,
            position: cell_origin(room.coord) + Vec3::Y * 0.9,
            status: GuardianStatus::Active,
            target_team: None,
            anchor_ticks: 0,
        }
    }

    pub fn pressure_for(&self, world: &FullWfcWorld, player: &PlayerState) -> f32 {
        if player.cell == self.cell {
            return (1.0 - player.position.distance(self.position) / 12.0).clamp(0.55, 1.0);
        }
        world
            .route_between_cells(player.cell, self.cell)
            .map_or(0.0, |route| {
                (1.0 - route.cost_millis as f32 / 9_000.0).clamp(0.0, 0.8)
            })
    }

    pub(super) fn step(
        &mut self,
        tick: u64,
        world: &FullWfcWorld,
        anchor_cells: &BTreeSet<CellCoord>,
        players: &mut BTreeMap<PlayerId, PlayerState>,
        teams: &BTreeMap<TeamId, TeamState>,
        events: &mut Vec<GameplayEvent>,
    ) {
        let observed = players
            .values()
            .any(|player| player_sees_guardian(player, self));
        let anchored = anchor_cells.contains(&self.cell);
        self.status = if observed {
            GuardianStatus::FrozenByPlayer
        } else if anchored {
            GuardianStatus::FrozenByAnchor
        } else {
            GuardianStatus::Active
        };
        if self.status != GuardianStatus::Active {
            return;
        }

        let target_team = self.target_team.unwrap_or_else(|| leading_team(teams));
        let Some(target_id) = players
            .values()
            .filter(|player| player.team == target_team && !player.escaped)
            .min_by_key(|player| {
                world
                    .route_between_cells(self.cell, player.cell)
                    .map_or(u32::MAX, |route| route.cost_millis)
            })
            .map(|player| player.id)
        else {
            return;
        };
        let target_cell = players[&target_id].cell;
        if target_cell == self.cell {
            let target_position = players[&target_id].position;
            self.position +=
                (target_position - self.position).normalize_or_zero() * GUARDIAN_SPEED * FIXED_DT;
            if self.position.distance(target_position) <= CATCH_DISTANCE {
                let excluded = BTreeSet::from([self.room]);
                if let Some(destination) = world.farthest_room_from_exit(&excluded) {
                    let room = &world.rooms[&destination];
                    let player = players.get_mut(&target_id).expect("target exists");
                    let from = player.cell;
                    player.cell = room.coord;
                    player.position = cell_origin(room.coord) + Vec3::Y * 0.9;
                    events.push(GameplayEvent::new(
                        tick,
                        GameplayEventKind::GuardianCatch,
                        Some(target_id),
                        Some(player.team),
                        Some(from),
                        Some(room.coord),
                    ));
                }
            }
        } else if tick.is_multiple_of(MOVE_PERIOD_TICKS)
            && let Some(route) = world.route_between_cells(self.cell, target_cell)
            && let Some(&next) = route.cells.get(1)
        {
            self.cell = next;
            self.position = cell_origin(next) + Vec3::Y * 0.9;
            if let Some(room) = world.room_at(next) {
                self.room = room;
            }
        }
    }
}

fn player_sees_guardian(player: &PlayerState, guardian: &GuardianState) -> bool {
    if player.escaped || player.cell != guardian.cell {
        return false;
    }
    let offset = guardian.position - player.position;
    let distance = offset.length();
    if !(0.15..=14.0).contains(&distance) {
        return false;
    }
    let forward = Vec3::new(player.yaw.sin(), 0.0, -player.yaw.cos());
    forward.dot(offset.normalize_or_zero()) > 0.42
}

fn leading_team(teams: &BTreeMap<TeamId, TeamState>) -> TeamId {
    teams
        .values()
        .filter(|team| !team.eliminated && !team.escaped)
        .max_by(|a, b| {
            a.keystones
                .cmp(&b.keystones)
                .then_with(|| a.dual_station_complete.cmp(&b.dual_station_complete))
                .then_with(|| b.id.cmp(&a.id))
        })
        .map_or(TeamId(0), |team| team.id)
}
