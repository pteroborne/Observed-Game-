//! Deterministic objective bot: emits the same `PlayerIntent` a client would,
//! routing through the facility A* and physically steering through hex centres,
//! walking ramps and climbing shafts to reach the exit.

use std::f32::consts::PI;

use glam::{Vec2, Vec3};
use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexArchetype, HexCoord, HexFace, HexPlacement};
use observed_hex::hex_origin;
use player_input::PlayerIntent;

use super::HexWfcMatch;
use super::movement::{face_plan_dir, shaft_target};

/// How close (metres, plan view) the bot steers toward a shaft column's centre
/// before it commits the climb press. Kept just inside the movement layer's
/// `SHAFT_CLIMB_RADIUS` so a committed press always takes.
const SHAFT_ALIGN_RADIUS: f32 = 4.5;

/// Consecutive no-progress ticks (reported by the step loop as `stuck_ticks`)
/// before the bot starts a sideways unstick sweep. Set clear of the ticks a
/// body legitimately spends accelerating off a hand-off so normal motion never
/// trips it, while a true jam (which never progresses) always does.
const STUCK_ENTER_TICKS: u16 = 12;
/// How many ticks each unstick sweep holds one side before reversing, so a bot
/// that guessed the blocked side reliably tries the other.
const STUCK_SWEEP_TICKS: u16 = 24;
/// Sideways and forward components of the unstick sweep intent.
const UNSTICK_STRAFE: f32 = 0.9;
const UNSTICK_FORWARD: f32 = 0.45;

impl HexWfcMatch {
    /// The abstract command the objective bot issues for `id` this tick.
    #[must_use]
    pub fn bot_command(&self, id: PlayerId) -> PlayerIntent {
        let Some(player) = self.players.get(&id) else {
            return PlayerIntent::default();
        };
        if player.escaped {
            return PlayerIntent::default();
        }
        // A scripted shaft transit — vertical climb or the lateral step off a
        // landing — owns the body; keep hands off until it completes.
        if player.climb_target.is_some() || player.transit_target.is_some() {
            return PlayerIntent::default();
        }
        let Some(target) = self.bot_objective_cell(id) else {
            return PlayerIntent::default();
        };
        if target == player.cell {
            return PlayerIntent::default();
        }
        let Some(route) = self.facility.route_between_cells(player.cell, target) else {
            return PlayerIntent::default();
        };
        let Some(&next) = route.cells.get(1) else {
            return PlayerIntent::default();
        };
        let base = if next.level == player.cell.level {
            steer_toward(
                player.yaw,
                player.position,
                Vec3::from_array(hex_origin(next)),
            )
        } else {
            self.vertical_command(player.cell, player.yaw, player.position, next)
        };
        self.apply_unstick(id, base)
    }

    /// Reactive wall-slide. Junction pylons, wall corners, and off-centre door
    /// approaches can leave a bot pushing into geometry with no forward progress
    /// — most often after a deterministic scripted glide deposits it dead on an
    /// axis. When the step loop reports a body wedged for several ticks
    /// (`stuck_ticks`), override its heading with a sideways sweep — alternating
    /// sides so it always finds the open way around — while keeping the target
    /// look direction, so it slides off the obstacle and resumes routing.
    fn apply_unstick(&self, id: PlayerId, mut intent: PlayerIntent) -> PlayerIntent {
        let stuck = self.stuck_ticks.get(&id).copied().unwrap_or(0);
        if stuck >= STUCK_ENTER_TICKS {
            let side = if (stuck / STUCK_SWEEP_TICKS).is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            intent.movement = Vec2::new(side * UNSTICK_STRAFE, UNSTICK_FORWARD);
        }
        intent
    }

    /// The bot's current destination cell. The hex match is a pure spawn→exit
    /// race, so the objective is always the exit rhombus corner.
    #[must_use]
    pub(crate) fn bot_objective_cell(&self, id: PlayerId) -> Option<HexCoord> {
        let player = self.players.get(&id)?;
        (!player.escaped).then(|| self.facility.config.exit())
    }

    fn vertical_command(
        &self,
        cell: HexCoord,
        yaw: f32,
        position: Vec3,
        next: HexCoord,
    ) -> PlayerIntent {
        let up = next.level > cell.level;
        let face = if up { HexFace::Up } else { HexFace::Down };
        // A shaft is climbed; a ramp is walked. `shaft_target` only fires on a
        // matched `ShaftOpen` pair, so its absence here means a ramp bond.
        if shaft_target(self.facility.config, &self.facility.placements, cell, face).is_some() {
            let center = Vec3::from_array(hex_origin(cell));
            if plan_distance(center, position) > SHAFT_ALIGN_RADIUS {
                return steer_toward(yaw, position, center);
            }
            return PlayerIntent {
                jump_pressed: up,
                interact_held: !up,
                ..PlayerIntent::default()
            };
        }
        let placement = &self.facility.placements[&cell];
        let dir = ramp_walk_dir(placement, up);
        if dir == Vec2::ZERO {
            // Degenerate ramp cell (should not occur on a solved route); fall
            // back to holding centre rather than emitting a null heading.
            return steer_toward(yaw, position, Vec3::from_array(hex_origin(cell)));
        }
        let aim = position + Vec3::new(dir.x, 0.0, dir.y) * 12.0;
        steer_toward(yaw, position, aim)
    }
}

/// The plan-view heading that walks a ramp cell toward `next`. `RampUp` rises
/// toward `d = opposite(entrance)`; `RampHead` tops out toward its exit face
/// `d`. Ascending walks toward `d`, descending back toward `opposite(d)`.
fn ramp_walk_dir(placement: &HexPlacement, up: bool) -> Vec2 {
    let Some(open) = HexFace::LATERAL.into_iter().find(|&f| placement.is_open(f)) else {
        return Vec2::ZERO;
    };
    let rise = match placement.archetype {
        HexArchetype::RampUp => open.opposite(),
        HexArchetype::RampHead => open,
        _ => open,
    };
    let walk = if up { rise } else { rise.opposite() };
    face_plan_dir(walk)
}

fn plan_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

fn steer_toward(yaw: f32, position: Vec3, target: Vec3) -> PlayerIntent {
    let direction = target - position;
    let desired_yaw = direction.x.atan2(-direction.z);
    let look = wrap_angle(desired_yaw - yaw).clamp(-0.25, 0.25);
    PlayerIntent {
        movement: Vec2::Y,
        look: Vec2::new(look, 0.0),
        sprint_held: true,
        ..PlayerIntent::default()
    }
}

fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(PI * 2.0) - PI
}
