//! Deterministic objective bot that emits the same abstract command as a client.

use std::f32::consts::PI;

use glam::{Vec2, Vec3};
use observed_core::PlayerId;
use observed_facility::{full_wfc::CellCoord, map_spec::RoomRole};
use player_input::PlayerIntent;

use super::{ActionButtons, FullWfcMatch, PlayerCommand, cell_origin};

impl FullWfcMatch {
    pub fn bot_command(&self, id: PlayerId) -> PlayerCommand {
        let Some(player) = self.players.get(&id) else {
            return PlayerCommand::default();
        };
        let team = &self.teams[&player.team];
        if player.escaped || team.eliminated || team.escaped {
            return PlayerCommand::default();
        }
        let Some(target) = self.bot_objective_cell(id) else {
            return PlayerCommand::default();
        };
        if target == player.cell {
            let center = cell_origin(target) + Vec3::Y * player.position.y.fract();
            if Vec2::new(center.x - player.position.x, center.z - player.position.z)
                .length_squared()
                > 1.0
            {
                return steer_toward(player.yaw, player.position, center);
            }
            return PlayerCommand {
                actions: ActionButtons {
                    interact: true,
                    ..Default::default()
                },
                ..Default::default()
            };
        }
        let Some(route) = self.facility.route_between_cells(player.cell, target) else {
            return PlayerCommand::default();
        };
        let Some(&next) = route.cells.get(1) else {
            return PlayerCommand::default();
        };
        if next.level != player.cell.level {
            let center = cell_origin(player.cell);
            if Vec2::new(center.x - player.position.x, center.z - player.position.z)
                .length_squared()
                > 1.0
            {
                return steer_toward(player.yaw, player.position, center);
            }
            return PlayerCommand {
                intent: PlayerIntent {
                    jump_pressed: next.level > player.cell.level,
                    interact_held: next.level < player.cell.level,
                    ..Default::default()
                },
                ..Default::default()
            };
        }
        steer_toward(player.yaw, player.position, cell_origin(next))
    }

    fn bot_objective_cell(&self, id: PlayerId) -> Option<CellCoord> {
        let player = &self.players[&id];
        let team = &self.teams[&player.team];
        if team.keystones < 2 {
            return self
                .available_keystones
                .iter()
                .filter_map(|room| {
                    let cell = self.facility.rooms[room].coord;
                    self.facility
                        .route_between_cells(player.cell, cell)
                        .map(|route| (route.cost_millis, *room, cell))
                })
                .min_by_key(|(cost, room, _)| (*cost, *room))
                .map(|(_, _, cell)| cell);
        }
        if !team.dual_station_complete {
            return self
                .facility
                .rooms
                .values()
                .find(|room| room.role == RoomRole::DualStation)
                .map(|room| room.coord);
        }
        Some(self.facility.exit())
    }
}

fn steer_toward(yaw: f32, position: Vec3, target: Vec3) -> PlayerCommand {
    let direction = target - position;
    let desired_yaw = direction.x.atan2(-direction.z);
    let look = wrap_angle(desired_yaw - yaw).clamp(-0.09, 0.09);
    PlayerCommand {
        intent: PlayerIntent {
            movement: Vec2::Y,
            look: Vec2::new(look, 0.0),
            sprint_held: true,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(PI * 2.0) - PI
}
