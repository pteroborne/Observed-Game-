//! Deterministic objective bot. It emits the same `PlayerIntent` as a client
//! and physically walks halls, ramps, and grounded switchback stairs.

use std::f32::consts::PI;

use glam::{Vec2, Vec3};
use observed_core::PlayerId;
use observed_facility::hex_wfc::{
    HexArchetype, HexCoord, HexFace, HexPlacement, HexSpace, PortClass,
};
use observed_hex::{TILE_LEVEL_HEIGHT, hex_origin};
use player_input::PlayerIntent;

use super::movement::face_plan_dir;
use super::{FLOOR_SLAB_TOP, HexWfcMatch};

/// Most yaw a bot may turn in one 60 Hz tick, in radians.
///
/// `HexWfcMatch` overrides `FpsConfig::look_step` to `1.0`, so the look delta a
/// bot emits *is* the yaw delta the controller applies — this constant is
/// already in rad/tick, no scaling in between. It used to be 0.25, i.e. ~859
/// deg/s: a bot snapped to any new heading within a couple of ticks, which both
/// whipped the spectator camera and made reversing free, so an oscillating
/// waypoint cost the bot nothing. At ~275 deg/s a marginal flip is ridden out
/// instead of chased.
///
/// Bot-side on purpose: `step_character` is shared with human look input, so
/// clamping there would slow the player's mouse.
const MAX_TURN_PER_TICK: f32 = 0.08;

const STUCK_ENTER_TICKS: u16 = 45;
const STUCK_SWEEP_TICKS: u16 = 24;
const UNSTICK_STRAFE: f32 = 0.9;
const UNSTICK_FORWARD: f32 = 0.45;

/// What a bot is trying to do this tick.
///
/// Deliberately a plain enum chosen by one pure function, mirroring the
/// Guardian (`super::guardian::HexGuardianStatus`) rather than introducing a
/// behaviour-tree framework: there are four behaviours, none of them nest, and
/// `agents.md` asks for the smallest thing that works before any framework. If
/// behaviours ever need to compose or reorder, that is the moment to revisit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BotBehaviour {
    /// Escaped, absent, or already standing on the objective.
    Idle,
    /// A route to the objective exists: follow it.
    Seek,
    /// Following a route but making no headway; break contact.
    Recover,
    /// **No route to the objective exists.** Head for whichever open neighbour
    /// sits nearest the objective and try again next tick.
    Explore,
}

impl HexWfcMatch {
    /// Which behaviour `id` is in this tick. Pure: reads only existing state.
    #[must_use]
    pub fn bot_behaviour(&self, id: PlayerId) -> BotBehaviour {
        let Some(player) = self.players.get(&id) else {
            return BotBehaviour::Idle;
        };
        let Some(target) = self.bot_objective_cell(id) else {
            return BotBehaviour::Idle;
        };
        if target == player.cell {
            return BotBehaviour::Idle;
        }
        let routed = self
            .facility
            .route_between_cells(player.cell, target)
            .is_some_and(|route| route.cells.len() > 1);
        if !routed {
            return BotBehaviour::Explore;
        }
        if self.stuck_ticks.get(&id).copied().unwrap_or(0) >= STUCK_ENTER_TICKS {
            BotBehaviour::Recover
        } else {
            BotBehaviour::Seek
        }
    }

    /// The abstract command the objective bot issues for `id` this tick.
    #[must_use]
    pub fn bot_command(&self, id: PlayerId) -> PlayerIntent {
        let Some(player) = self.players.get(&id) else {
            return PlayerIntent::default();
        };
        match self.bot_behaviour(id) {
            BotBehaviour::Idle => return PlayerIntent::default(),
            BotBehaviour::Explore => return self.explore_command(player),
            BotBehaviour::Seek | BotBehaviour::Recover => {}
        }
        let target = self
            .bot_objective_cell(id)
            .expect("Seek/Recover imply an objective");
        let route = self
            .facility
            .route_between_cells(player.cell, target)
            .expect("Seek/Recover imply a route");
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
            let target = self
                .lateral_waypoint(player.cell, next, player.position)
                .unwrap_or_else(|| Vec3::from_array(hex_origin(next)));
            steer_toward(player.yaw, player.position, target)
        };
        self.apply_unstick(id, base)
    }

    /// Fallback when no route to the objective exists.
    ///
    /// Previously this case returned `PlayerIntent::default()`, so a bot that
    /// lost its route — stranded in a pocket, or on a cell orphaned by a
    /// relayout — stood still forever. It was never exercised: the stall soak
    /// skips any layout whose spawn cannot reach the exit at all.
    ///
    /// Instead, walk to whichever open lateral neighbour lies nearest the
    /// objective in plan. Greedy and deliberately memoryless: the real route is
    /// retried every tick, so this only has to break the deadlock, not solve
    /// the maze. Deterministic — ties fall to `HexFace::LATERAL` order.
    fn explore_command(&self, player: &super::HexPlayerState) -> PlayerIntent {
        let Some(placement) = self.facility.placements.get(&player.cell) else {
            return PlayerIntent::default();
        };
        let goal = Vec3::from_array(hex_origin(self.facility.config.exit()));
        let grid = self.facility.config.grid();
        let best = HexFace::LATERAL
            .into_iter()
            .filter(|&face| placement.is_open(face))
            .filter_map(|face| grid.neighbor(player.cell, face))
            .filter(|cell| {
                self.facility
                    .placements
                    .get(cell)
                    .is_some_and(|neighbour| neighbour.space != HexSpace::Void)
            })
            .min_by(|a, b| {
                let plan = |cell: &HexCoord| {
                    let origin = Vec3::from_array(hex_origin(*cell));
                    Vec2::new(origin.x - goal.x, origin.z - goal.z).length()
                };
                plan(a).total_cmp(&plan(b))
            });
        match best {
            Some(cell) => {
                let target = self
                    .lateral_waypoint(player.cell, cell, player.position)
                    .unwrap_or_else(|| Vec3::from_array(hex_origin(cell)));
                steer_toward(player.yaw, player.position, target)
            }
            None => PlayerIntent::default(),
        }
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

    /// Waypoint for an ordinary lateral hop: steer at the shared doorway until
    /// the body is through its plane, then at the neighbour's centre.
    ///
    /// Aiming straight at the neighbour's centre works only from near a cell's
    /// middle. From anywhere off-centre the heading runs wide of the 4.5 m
    /// aperture and into the corner where three cells meet — and there the
    /// logical cell can resolve to a third, *off-route* cell whose own route
    /// points back the way it came, so the bot shuttles between the two. That
    /// was measured as a 2.2 m limit cycle, identical positions each pass,
    /// repeated for ~2,300 ticks with the bot never registering as stuck.
    ///
    /// Steering at the door and only then at the centre keeps the path inside
    /// the two cells that actually share the aperture.
    fn lateral_waypoint(&self, cell: HexCoord, next: HexCoord, position: Vec3) -> Option<Vec3> {
        let face = HexFace::LATERAL
            .into_iter()
            .find(|&face| self.facility.config.grid().neighbor(cell, face) == Some(next))?;
        let origin = Vec3::from_array(hex_origin(cell));
        let [a, b] = observed_hex::face_edge(face);
        let door = Vec3::new(
            origin.x + (a.0 + b.0) as f32 * 0.5,
            position.y,
            origin.z + (a.1 + b.1) as f32 * 0.5,
        );
        let outward = face_plan_dir(face);
        let past_aperture = Vec2::new(position.x - door.x, position.z - door.z).dot(outward) > 0.0;
        if past_aperture {
            // Through the doorway; head for the neighbour's middle so the body
            // clears the threshold instead of loitering in it.
            let next_origin = Vec3::from_array(hex_origin(next));
            Some(Vec3::new(next_origin.x, position.y, next_origin.z))
        } else {
            Some(door)
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
///
/// Each stage hands over on a **monotonic** test — rise, or which side of the
/// turn the body is on — never on "am I within X of the waypoint". A proximity
/// test is not monotonic along the path: walking away from the landing onto the
/// upper flight grows that distance back past the threshold, so the target
/// flipped between the landing (east) and the flight's top (west) every few
/// ticks and the bot span on the spot just past the turn, in a ~30 cm band of
/// rise. It eventually escaped on jitter, having burnt ~31,000 ticks on one
/// storey. Both directions had it.
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
        if rise < 0.8 && local.x > -3.1 {
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
        } else if rise < 4.3 && local.y > -1.6 {
            // Crossing the turn landing toward the upper flight's band.
            point(4.25, -2.125)
        } else if rise < TILE_LEVEL_HEIGHT - 0.35 || local.x > -4.45 {
            point(-4.5, -2.125)
        } else {
            point(-4.5, -3.75)
        }
    } else if rise > TILE_LEVEL_HEIGHT - 0.35 && local.x < -4.0 {
        point(-4.1, -2.125)
    } else if rise > 4.3 {
        point(3.5, -2.125)
    } else if local.y < 1.6 {
        // Crossing the turn landing back toward the lower flight's band.
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
    let look = wrap_angle(desired_yaw - yaw).clamp(-MAX_TURN_PER_TICK, MAX_TURN_PER_TICK);
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
