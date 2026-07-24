//! Collision-resolved per-player movement and world-position to lattice sync.
//! Stairs, ramps, halls, and rooms all use the same physical controller; tile
//! traversal never takes ownership of a body or rewrites its pose.

use glam::{Vec2, Vec3};
use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexCoord, HexFace, HexSpace, HexWfcConfig};
use observed_hex::{TILE_LEVEL_HEIGHT, hex_origin};
use observed_traversal::rapier_controller::step_character;
use player_input::PlayerIntent;

use super::{FIXED_DT, FLOOR_SLAB_TOP, HexMatchEvent, HexMatchEventKind, HexWfcMatch};

/// Net displacement required before the body establishes a new progress
/// anchor. Small collision jitter does not count as useful movement.
const STUCK_PROGRESS_EPS: f32 = 0.4;

/// Plan-view deadband a body must gain toward a new same-level cell's centre
/// before its logical cell switches, preventing boundary jitter.
const CELL_SWITCH_HYSTERESIS: f32 = 1.5;

impl HexWfcMatch {
    /// Advance one player by one fixed physical step.
    pub(super) fn move_player(&mut self, id: PlayerId, intent: PlayerIntent) {
        if self.players[&id].escaped {
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

    /// Project the Rapier body back onto the lattice. Logical level follows
    /// actual height so a body partway up a ramp or stair resolves naturally.
    pub(super) fn sync_player_from_body(&mut self, id: PlayerId) {
        let body = self.bodies[&id];
        let top_level = f32::from(self.facility.config.levels.saturating_sub(1));
        let level = (body.position.y / TILE_LEVEL_HEIGHT)
            .round()
            .clamp(0.0, top_level) as u8;
        let candidate =
            horizontal_cell(self.facility.config, body.position, level).filter(|cell| {
                self.facility
                    .placements
                    .get(cell)
                    .is_some_and(|placement| placement.space != HexSpace::Void)
            });
        let current = self.players[&id].cell;
        let current_valid = self
            .facility
            .placements
            .get(&current)
            .is_some_and(|placement| placement.space != HexSpace::Void);
        let player = self.players.get_mut(&id).expect("player");
        if let Some(cell) = candidate {
            let switch = !current_valid
                || cell.level != current.level
                || plan_distance_xz(body.position, hex_origin(cell)) + CELL_SWITCH_HYSTERESIS
                    < plan_distance_xz(body.position, hex_origin(current));
            if switch {
                player.cell = cell;
            }
        }
        player.position = body.position;
        player.yaw = body.yaw;
        player.pitch = body.pitch;
    }

    /// Recover only bodies that genuinely leave the arena. Ordinary physical
    /// falls between levels remain gameplay and are never converted to motion.
    pub(super) fn recover_fallen_bodies(&mut self) {
        let floor_y = self.geometry.arena.floor_y;
        let half_height = self.traversal_config.half_height;
        let tick = self.tick;
        let mut recovered = Vec::new();
        for player in self.players.values() {
            if player.escaped {
                continue;
            }
            let body = self.bodies[&player.id];
            let out_of_world = !body.position.is_finite() || body.position.y < floor_y - 4.0;
            if out_of_world {
                recovered.push((player.id, player.cell));
            }
        }
        for (id, cell) in recovered {
            let anchor =
                Vec3::from_array(hex_origin(cell)) + Vec3::Y * (FLOOR_SLAB_TOP + half_height);
            *self.bodies.get_mut(&id).expect("body") =
                observed_traversal::FpsBody::spawned(anchor, self.players[&id].yaw);
            self.players.get_mut(&id).expect("player").position = anchor;
            self.recent_events.push(HexMatchEvent {
                tick,
                kind: HexMatchEventKind::PlayerRecovered,
                player: Some(id),
                cell: Some(cell),
            });
        }
    }

    /// Update each player's no-progress counter for the objective bot's
    /// collision recovery steering.
    pub(super) fn update_stuck_ticks(&mut self) {
        for id in self.players.keys().copied().collect::<Vec<_>>() {
            let player = &self.players[&id];
            let position = player.position;
            if player.escaped {
                self.progress_anchor.insert(id, position);
                self.stuck_ticks.insert(id, 0);
                continue;
            }
            let anchor = *self.progress_anchor.entry(id).or_insert(position);
            if position.distance(anchor) > STUCK_PROGRESS_EPS {
                self.progress_anchor.insert(id, position);
                self.stuck_ticks.insert(id, 0);
            } else {
                let counter = self.stuck_ticks.entry(id).or_insert(0);
                *counter = counter.saturating_add(1);
            }
        }
    }

    /// Reconcile simulation-authorized teleports (spawn, setbacks, escape) to
    /// fresh physical bodies. Tile traversal never calls this itself.
    pub(super) fn sync_teleports_to_bodies(&mut self) {
        for player in self.players.values() {
            let body = self.bodies.get_mut(&player.id).expect("body");
            if body.position.distance_squared(player.position) > 0.000_001 {
                *body = observed_traversal::FpsBody::spawned(player.position, player.yaw);
                body.pitch = player.pitch;
            }
        }
    }
}

fn plan_distance_xz(position: Vec3, center: [f32; 3]) -> f32 {
    Vec2::new(position.x - center[0], position.z - center[2]).length()
}

/// Recover `(q, r)` from a plan-view world position at a known level.
pub(super) fn horizontal_cell(config: HexWfcConfig, position: Vec3, level: u8) -> Option<HexCoord> {
    let r = (position.z / 12.0).round();
    let q = ((position.x - r * 7.0) / 14.0).round();
    let (qi, ri) = (q as i32, r as i32);
    (qi >= 0 && ri >= 0 && qi < i32::from(config.cols) && ri < i32::from(config.rows)).then_some(
        HexCoord {
            q: qi as u16,
            r: ri as u16,
            level,
        },
    )
}

/// Plan-view unit direction of a lateral face.
pub(super) fn face_plan_dir(face: HexFace) -> Vec2 {
    let (dq, dr, _) = face.delta();
    let x = dq * 14 + dr * 7;
    let z = dr * 12;
    Vec2::new(x as f32, z as f32).normalize_or_zero()
}

/// Coarse looked-at face for threshold observation.
pub(super) fn look_face(yaw: f32, pitch: f32) -> HexFace {
    if pitch > 0.72 {
        return HexFace::Up;
    }
    if pitch < -0.72 {
        return HexFace::Down;
    }
    let heading = Vec2::new(yaw.sin(), -yaw.cos());
    HexFace::LATERAL
        .into_iter()
        .max_by(|&a, &b| {
            heading
                .dot(face_plan_dir(a))
                .total_cmp(&heading.dot(face_plan_dir(b)))
        })
        .expect("lateral faces are non-empty")
}
