use glam::{Vec2, Vec3};
use observed_core::PlayerId;
use observed_facility::full_wfc::{CellCoord, FullWfcConfig, ModuleFace, ModuleSpace};
use observed_traversal::{FpsBody, rapier_controller::step_character};
use player_input::PlayerIntent;

use super::{
    CLIMB_SPEED, FIXED_DT, FullWfcMatch, GameplayEventKind, INTERACTION_RADIUS, PlayerCommand,
};
use crate::full_wfc::{CELL_SIZE, DeployableKind, LEVEL_HEIGHT, cell_origin};

impl FullWfcMatch {
    pub(super) fn step_player(&mut self, id: PlayerId, command: PlayerCommand) {
        if self.players[&id].escaped {
            return;
        }
        self.move_player(id, command.intent);
        let (team, cell, position) = {
            let player = &self.players[&id];
            (player.team, player.cell, player.position)
        };
        self.equipment
            .clear_pad_latch_when_away(id, cell, position, INTERACTION_RADIUS);
        let actions = command.actions;
        if actions.deploy_anchor {
            if self
                .equipment
                .deploy(team, DeployableKind::Anchor, cell, position)
                .is_some()
            {
                self.push_player_event(GameplayEventKind::AnchorDeployed, id, cell, None);
            }
        } else if actions.deploy_pad {
            if self
                .equipment
                .deploy(team, DeployableKind::TeleportPad, cell, position)
                .is_some()
            {
                self.push_player_event(GameplayEventKind::PadDeployed, id, cell, None);
            }
        } else if actions.recover {
            if let Some((_item, deployed_kind)) =
                self.equipment
                    .recover_nearest(team, cell, position, INTERACTION_RADIUS)
            {
                let kind = if deployed_kind == DeployableKind::Anchor {
                    GameplayEventKind::AnchorRecovered
                } else {
                    GameplayEventKind::PadRecovered
                };
                self.push_player_event(kind, id, cell, None);
            }
        } else if actions.activate_pad
            && let Some((_, target_cell, target_position)) =
                self.equipment
                    .pad_target(id, team, cell, position, INTERACTION_RADIUS)
        {
            let player = self.players.get_mut(&id).expect("player");
            player.cell = target_cell;
            player.position = target_position;
            self.push_player_event(GameplayEventKind::PadUsed, id, cell, Some(target_cell));
        }
        if actions.interact {
            self.try_collect_keystone(id);
            self.try_survey_monitor(id);
            self.try_redirect_guardian(id);
        }
    }

    fn move_player(&mut self, id: PlayerId, intent: PlayerIntent) {
        let climb_target = self.players[&id].climb_target;
        if let Some(target) = climb_target {
            let target_y = cell_origin(target).y + self.traversal_config.half_height;
            let body = self.bodies.get_mut(&id).expect("body");
            let delta = target_y - body.position.y;
            let step = CLIMB_SPEED * FIXED_DT;
            if delta.abs() <= step {
                let mut offset = Vec3::ZERO;
                for face in [
                    ModuleFace::East,
                    ModuleFace::West,
                    ModuleFace::South,
                    ModuleFace::North,
                ] {
                    if self.facility.placements[&target].is_open(face) {
                        let (dx, dz, _) = face.delta();
                        offset = Vec3::new(dx as f32 * 2.2, 0.0, dz as f32 * 2.2);
                        break;
                    }
                }
                let angle = (id.0 as f32) * (std::f32::consts::TAU / 8.0);
                let perturb = Vec3::new(angle.cos() * 0.5, 0.0, angle.sin() * 0.5);
                body.position = cell_origin(target)
                    + Vec3::Y * self.traversal_config.half_height
                    + offset
                    + perturb;
                body.velocity = Vec3::ZERO;
                let player = self.players.get_mut(&id).expect("player");
                player.cell = target;
                player.climb_target = None;
            } else {
                body.position.y += delta.signum() * step;
                body.velocity = Vec3::ZERO;
            }
            self.sync_player_from_body(id);
            return;
        }
        let climb_face = if intent.jump_pressed {
            Some(ModuleFace::Up)
        } else if intent.interact_held {
            Some(ModuleFace::Down)
        } else {
            None
        };
        let center = cell_origin(self.players[&id].cell);
        let near_shaft = Vec2::new(
            self.players[&id].position.x - center.x,
            self.players[&id].position.z - center.z,
        )
        .length_squared()
            <= 1.65 * 1.65;
        if let Some(face) = climb_face
            && near_shaft
            && self.facility.placements[&self.players[&id].cell].is_open(face)
        {
            let player = self.players.get_mut(&id).expect("player");
            player.climb_target = self.facility.config.neighbor(player.cell, face);
            return;
        }
        step_character(
            &self.physics,
            self.bodies.get_mut(&id).expect("body"),
            intent,
            &self.traversal_config,
            FIXED_DT,
        );
        self.sync_player_from_body(id);
    }

    fn sync_player_from_body(&mut self, id: PlayerId) {
        let body = self.bodies[&id];
        let level = (body.position.y / LEVEL_HEIGHT).round().clamp(0.0, 2.0) as u8;
        let candidate = horizontal_cell(self.facility.config, body.position, level);
        let player = self.players.get_mut(&id).expect("player");
        if let Some(cell) = candidate
            && self
                .facility
                .placement(cell)
                .is_some_and(|placement| placement.space != ModuleSpace::Void)
        {
            player.cell = cell;
        }
        player.position = body.position;
        player.yaw = body.yaw;
        player.pitch = body.pitch;
    }

    pub(super) fn sync_teleports_to_bodies(&mut self) {
        for player in self.players.values() {
            let body = self.bodies.get_mut(&player.id).expect("body");
            if body.position.distance_squared(player.position) > 0.000_001 {
                *body = FpsBody::spawned(player.position, player.yaw);
                body.pitch = player.pitch;
            }
        }
    }
}

fn horizontal_cell(config: FullWfcConfig, position: Vec3, level: u8) -> Option<CellCoord> {
    let x = ((position.x + CELL_SIZE * 0.5) / CELL_SIZE).floor() as i32;
    let z = ((position.z + CELL_SIZE * 0.5) / CELL_SIZE).floor() as i32;
    (x >= 0 && z >= 0 && x < i32::from(config.cols) && z < i32::from(config.rows))
        .then(|| CellCoord::new(x as u16, z as u16, level))
}

pub(super) fn look_face(yaw: f32, pitch: f32) -> ModuleFace {
    if pitch > 0.72 {
        return ModuleFace::Up;
    }
    if pitch < -0.72 {
        return ModuleFace::Down;
    }
    let direction = Vec2::new(-yaw.sin(), -yaw.cos());
    if direction.x.abs() > direction.y.abs() {
        if direction.x > 0.0 {
            ModuleFace::East
        } else {
            ModuleFace::West
        }
    } else if direction.y > 0.0 {
        ModuleFace::South
    } else {
        ModuleFace::North
    }
}
