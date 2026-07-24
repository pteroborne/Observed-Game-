//! Deterministic objective bot. It emits the same `PlayerIntent` as a client
//! and physically walks halls, ramps, and grounded switchback stairs.

use std::f32::consts::PI;

use glam::{Vec2, Vec3};
use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexArchetype, HexCoord, HexFace, HexPlacement, PortClass};
use observed_hex::{TILE_LEVEL_HEIGHT, hex_origin};
use player_input::PlayerIntent;

use super::movement::face_plan_dir;
use super::{FLOOR_SLAB_TOP, HexWfcMatch};

const STUCK_ENTER_TICKS: u16 = 45;
const STUCK_SWEEP_TICKS: u16 = 24;
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
        let base = if next.level != player.cell.level {
            self.vertical_command(player.cell, player.yaw, player.position, next)
        } else if let Some(command) =
            self.finish_stair_command(player.cell, player.yaw, player.position)
        {
            command
        } else if let Some(command) =
            self.stair_lateral_command(player.cell, next, player.yaw, player.position)
        {
            command
        } else {
            steer_toward(
                player.yaw,
                player.position,
                Vec3::from_array(hex_origin(next)),
            )
        };
        self.apply_unstick(id, base)
    }

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
        let placement = &self.facility.placements[&cell];
        let class = if up { placement.up } else { placement.down };
        if class == PortClass::ShaftOpen {
            let feet = position.y - self.traversal_config.half_height;
            let floor = hex_origin(cell)[1] + FLOOR_SLAB_TOP;
            let base = if up && feet < floor - 0.15 && cell.level > 0 {
                HexCoord {
                    level: cell.level - 1,
                    ..cell
                }
            } else if up || feet > floor + 0.35 {
                cell
            } else {
                next
            };
            return stair_command(base, yaw, position, self.traversal_config.half_height, up);
        }

        let dir = ramp_walk_dir(placement, up);
        if dir == Vec2::ZERO {
            return steer_toward(yaw, position, Vec3::from_array(hex_origin(cell)));
        }
        let aim = position + Vec3::new(dir.x, 0.0, dir.y) * 12.0;
        steer_toward(yaw, position, aim)
    }

    /// Continue a stair flight after height rounding has changed the logical
    /// cell but before the capsule's feet reach the destination deck.
    fn finish_stair_command(
        &self,
        cell: HexCoord,
        yaw: f32,
        position: Vec3,
    ) -> Option<PlayerIntent> {
        let placement = self.facility.placements.get(&cell)?;
        if placement.archetype != HexArchetype::Shaft {
            return None;
        }
        let feet = position.y - self.traversal_config.half_height;
        let origin = Vec3::from_array(hex_origin(cell));
        let floor = origin.y + FLOOR_SLAB_TOP;
        let local = Vec2::new(position.x - origin.x, position.z - origin.z);
        let on_incoming_flight = local.x > -4.7
            && local.x < 4.2
            && local.y > -3.3
            && local.y < -0.6
            && feet < floor + 0.75;
        if (feet < floor - 0.15 || on_incoming_flight) && cell.level > 0 {
            let base = HexCoord {
                level: cell.level - 1,
                ..cell
            };
            Some(stair_command(
                base,
                yaw,
                position,
                self.traversal_config.half_height,
                true,
            ))
        } else if feet > floor + 0.35 {
            Some(stair_command(
                cell,
                yaw,
                position,
                self.traversal_config.half_height,
                false,
            ))
        } else {
            None
        }
    }

    /// Follow the solid perimeter deck between a stair landing and its lateral
    /// door. A centre-to-centre heading would cut directly across the stairwell.
    fn stair_lateral_command(
        &self,
        cell: HexCoord,
        next: HexCoord,
        yaw: f32,
        position: Vec3,
    ) -> Option<PlayerIntent> {
        if self.facility.placements.get(&cell)?.archetype != HexArchetype::Shaft {
            return None;
        }
        let face = HexFace::LATERAL
            .into_iter()
            .find(|&face| self.facility.config.grid().neighbor(cell, face) == Some(next))?;
        let target = match face {
            HexFace::East => Vec2::new(5.5, 0.0),
            HexFace::SouthEast => Vec2::new(3.0, 5.2),
            HexFace::SouthWest => Vec2::new(-3.0, 5.2),
            HexFace::West => Vec2::new(-5.5, 0.0),
            HexFace::NorthWest => Vec2::new(-3.0, -5.2),
            HexFace::NorthEast => Vec2::new(3.0, -5.2),
            HexFace::Up | HexFace::Down => unreachable!(),
        };
        let origin = Vec3::from_array(hex_origin(cell));
        let local = Vec2::new(position.x - origin.x, position.z - origin.z);
        let outward = face_plan_dir(face);
        let across = Vec2::new(-outward.y, outward.x);
        let crossed_aligned_threshold = local.dot(outward) >= target.dot(outward) - 0.1
            && (local.dot(across) - target.dot(across)).abs() < 1.0;
        if local.distance(target) < 0.9 || crossed_aligned_threshold {
            return Some(walk_toward(
                yaw,
                position,
                Vec3::from_array(hex_origin(next)),
            ));
        }
        // A bot arriving from below first clears the stair opening along the
        // north and east perimeter. This keeps later southbound motion from
        // being mistaken for another pass along the incoming flight.
        let waypoint = if local.y < -4.3 {
            Vec2::new(local.x.clamp(-3.0, 3.0), -3.8)
        } else if local.y < -3.25 && local.x < 5.4 && target.y > -0.75 {
            Vec2::new(5.7, -4.0)
        } else if face == HexFace::East && local.x < -4.0 && local.y < 3.8 {
            Vec2::new(-4.5, 4.1)
        } else if face == HexFace::East && local.x < 5.0 && local.y < 3.8 {
            Vec2::new(3.0, 4.1)
        } else if face == HexFace::East && local.x < 5.2 {
            Vec2::new(5.5, 4.1)
        } else if face == HexFace::SouthEast && local.x < -4.0 && local.y < 3.8 {
            Vec2::new(-4.5, 4.1)
        } else if face == HexFace::SouthWest && local.x > 4.8 && local.y < 3.8 {
            Vec2::new(5.3, 4.1)
        } else if local.x > 5.2 && local.y < -0.2 && target.y > -0.75 {
            Vec2::new(5.7, -0.3)
        // The guarded stair opening occupies this rectangle. When a direct
        // chord would cross it, choose the shortest safe corner around it.
        } else if segment_crosses_rect(local, target, Vec2::new(-4.0, -3.5), Vec2::new(5.2, -0.75))
        {
            [
                Vec2::new(-4.5, -4.0),
                Vec2::new(5.7, -4.0),
                Vec2::new(-4.5, -0.3),
                Vec2::new(5.7, -0.3),
            ]
            .into_iter()
            .filter(|corner| {
                !segment_crosses_rect(local, *corner, Vec2::new(-4.0, -3.5), Vec2::new(5.2, -0.75))
                    && !segment_crosses_rect(
                        *corner,
                        target,
                        Vec2::new(-4.0, -3.5),
                        Vec2::new(5.2, -0.75),
                    )
            })
            .min_by(|a, b| {
                (local.distance(*a) + a.distance(target))
                    .total_cmp(&(local.distance(*b) + b.distance(target)))
            })
            .unwrap_or(target)
        } else {
            target
        };
        Some(walk_toward(
            yaw,
            position,
            origin + Vec3::new(waypoint.x, 0.0, waypoint.y),
        ))
    }
}

fn segment_crosses_rect(start: Vec2, end: Vec2, min: Vec2, max: Vec2) -> bool {
    let delta = end - start;
    let mut enter: f32 = 0.0;
    let mut exit: f32 = 1.0;
    for (origin, direction, low, high) in [
        (start.x, delta.x, min.x, max.x),
        (start.y, delta.y, min.y, max.y),
    ] {
        if direction.abs() < 1.0e-6 {
            if origin < low || origin > high {
                return false;
            }
            continue;
        }
        let a = (low - origin) / direction;
        let b = (high - origin) / direction;
        enter = enter.max(a.min(b));
        exit = exit.min(a.max(b));
        if enter > exit {
            return false;
        }
    }
    exit >= 0.0 && enter <= 1.0
}

/// Walk the generated switchback using waypoints on its real collision
/// surfaces. Height selects the current flight; no position is written here.
fn stair_command(
    base: HexCoord,
    yaw: f32,
    position: Vec3,
    half_height: f32,
    up: bool,
) -> PlayerIntent {
    let origin = Vec3::from_array(hex_origin(base));
    let rise = position.y - half_height - origin.y;
    let point = |x: f32, z: f32| origin + Vec3::new(x, 0.0, z);
    let local = Vec2::new(position.x - origin.x, position.z - origin.z);
    let low_start = point(-3.5, 2.125);

    let target = if up {
        if rise < 0.8 && plan_distance(position, low_start) > 0.75 {
            if local.y < 3.4 {
                if local.x >= 0.0 {
                    point(5.5, 3.75)
                } else {
                    point(-3.5, 3.75)
                }
            } else if (local.x + 3.5).abs() > 0.4 {
                point(-3.5, 3.75)
            } else {
                low_start
            }
        } else if rise < 3.7 {
            point(3.5, 2.125)
        } else if rise < 4.3 && plan_distance(position, point(4.25, -2.125)) > 0.75 {
            point(4.25, -2.125)
        } else if rise < TILE_LEVEL_HEIGHT - 0.35 || local.x > -4.45 {
            point(-4.5, -2.125)
        } else {
            point(-4.5, -3.75)
        }
    } else if rise > TILE_LEVEL_HEIGHT - 0.35 && plan_distance(position, point(-4.1, -2.125)) > 0.75
    {
        point(-4.1, -2.125)
    } else if rise > 4.3 {
        point(3.5, -2.125)
    } else if plan_distance(position, point(4.25, 2.125)) > 0.75 {
        point(4.25, 2.125)
    } else {
        point(-3.5, 2.125)
    };
    walk_toward(yaw, position, target)
}

/// Plan heading that walks a two-cell ramp in the requested direction.
fn ramp_walk_dir(placement: &HexPlacement, up: bool) -> Vec2 {
    let Some(open) = HexFace::LATERAL
        .into_iter()
        .find(|&face| placement.is_open(face))
    else {
        return Vec2::ZERO;
    };
    let rise = match placement.archetype {
        HexArchetype::RampUp => open.opposite(),
        HexArchetype::RampHead => open,
        _ => open,
    };
    face_plan_dir(if up { rise } else { rise.opposite() })
}

fn plan_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

fn steer_toward(yaw: f32, position: Vec3, target: Vec3) -> PlayerIntent {
    steer_toward_with_speed(yaw, position, target, true, 1.0)
}

fn walk_toward(yaw: f32, position: Vec3, target: Vec3) -> PlayerIntent {
    steer_toward_with_speed(yaw, position, target, false, 0.35)
}

fn steer_toward_with_speed(
    yaw: f32,
    position: Vec3,
    target: Vec3,
    sprint: bool,
    movement_scale: f32,
) -> PlayerIntent {
    let direction = target - position;
    let desired_yaw = direction.x.atan2(-direction.z);
    let look = wrap_angle(desired_yaw - yaw).clamp(-0.25, 0.25);
    PlayerIntent {
        movement: Vec2::Y * movement_scale,
        look: Vec2::new(look, 0.0),
        sprint_held: sprint,
        ..PlayerIntent::default()
    }
}

fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(PI * 2.0) - PI
}
