//! The **deterministic first-person controller** as a production model: a pure
//! `step_body` advanced at a fixed timestep, with substep integration, axis-by-axis
//! AABB collision, and authored step-up (stairs / raised walkways climb the same
//! kernel, tall obstacles block). Built on the shared [`PlayerIntent`] boundary.
//!
//! Because the step depends only on its inputs (body + intent + arena + dt) and never
//! on wall-clock time or iteration order, the same sequence of intents always produces
//! the same path — the property that powers the replay tape (`match_replay`) and
//! lockstep networking (`network_lab`). This is the single production controller path
//! that first-person movement, elevation, replay, and lockstep all call.
//!
//! Promoted out of `fps_controller_lab` in refactor R6; the lab re-exports this crate
//! as its `controller` and is the debug projection. It depends only on `glam` for
//! vector math and the pure `player_input` data. The optional, default-on `bevy`
//! feature is the adapter: it derives `Resource` on [`FpsArena`] / [`FpsConfig`] so a
//! consumer can insert them directly; the controller itself never renders.

use std::f32::consts::TAU;

use glam::Vec3;
use player_input::PlayerIntent;

pub mod gantry;
pub mod rapier_controller;

/// Fixed simulation timestep. The controller is only ever stepped at this dt.
pub const FIXED_DT: f32 = 1.0 / 60.0;

/// An axis-aligned box, used for the floor-relative solids (walls and pillars).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb3 {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb3 {
    pub fn from_center_half(center: Vec3, half: Vec3) -> Self {
        Self {
            min: center - half,
            max: center + half,
        }
    }

    fn overlaps(&self, center: Vec3, half: Vec3) -> bool {
        let min = center - half;
        let max = center + half;
        min.x < self.max.x
            && max.x > self.min.x
            && min.y < self.max.y
            && max.y > self.min.y
            && min.z < self.max.z
            && max.z > self.min.z
    }
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct FpsArena {
    pub solids: Vec<Aabb3>,
    pub floor_y: f32,
    /// Half-extent of the square floor (for the debug grid + respawn bounds).
    pub floor_half: f32,
}

impl FpsArena {
    /// A square room: a floor, four perimeter walls, and two interior pillars.
    pub fn authored() -> Self {
        let half = 18.0;
        let wall_h = 6.0;
        let t = 0.5; // wall half-thickness
        let wall = |center: Vec3, hx: f32, hz: f32| {
            Aabb3::from_center_half(center, Vec3::new(hx, wall_h, hz))
        };
        let solids = vec![
            wall(Vec3::new(0.0, wall_h, -half), half, t), // north
            wall(Vec3::new(0.0, wall_h, half), half, t),  // south
            wall(Vec3::new(half, wall_h, 0.0), t, half),  // east
            wall(Vec3::new(-half, wall_h, 0.0), t, half), // west
            // Two interior pillars to collide with.
            Aabb3::from_center_half(Vec3::new(-6.0, 1.5, -2.0), Vec3::new(1.2, 1.5, 1.2)),
            Aabb3::from_center_half(Vec3::new(6.5, 1.5, 4.0), Vec3::new(1.2, 1.5, 1.2)),
        ];
        Self {
            solids,
            floor_y: 0.0,
            floor_half: half,
        }
    }
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Copy, Debug)]
pub struct FpsConfig {
    pub walk_speed: f32,
    pub run_speed: f32,
    pub ground_accel: f32,
    pub ground_decel: f32,
    pub air_accel: f32,
    pub gravity: f32,
    pub max_fall: f32,
    pub jump_speed: f32,
    pub jump_cooldown: f32,
    pub radius: f32,
    pub half_height: f32,
    pub eye_height: f32,
    pub look_step: f32,
    pub pitch_limit: f32,
    pub substep: f32,
    /// Maximum vertical rise that horizontal movement can step onto.
    pub step_height: f32,
}

impl Default for FpsConfig {
    fn default() -> Self {
        Self {
            walk_speed: 7.0,
            run_speed: 11.0,
            ground_accel: 60.0,
            ground_decel: 90.0,
            air_accel: 18.0,
            gravity: 24.0,
            max_fall: 45.0,
            jump_speed: 8.0,
            jump_cooldown: 0.25,
            radius: 0.4,
            half_height: 0.9,
            eye_height: 1.6,
            look_step: 0.035,
            pitch_limit: 1.4,
            substep: 0.25,
            step_height: 0.45,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FpsBody {
    /// Centre of the body box. Feet are at `position.y - half_height`.
    pub position: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub grounded: bool,
    pub jump_cd: f32,
    pub spawn: Vec3,
    pub spawn_yaw: f32,
}

impl FpsBody {
    pub fn spawned(position: Vec3, yaw: f32) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            yaw,
            pitch: 0.0,
            grounded: false,
            jump_cd: 0.0,
            spawn: position,
            spawn_yaw: yaw,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::spawned(self.spawn, self.spawn_yaw);
    }

    /// Horizontal forward direction (yaw 0 looks toward `-Z`).
    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos())
    }

    /// Strafe-right direction.
    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y)
    }

    /// World-space eye position the camera renders from.
    pub fn eye(&self, config: &FpsConfig) -> Vec3 {
        Vec3::new(
            self.position.x,
            self.position.y - config.half_height + config.eye_height,
            self.position.z,
        )
    }

    /// The look direction (forward tilted by pitch).
    pub fn look_dir(&self) -> Vec3 {
        let f = self.forward();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(f.x * cp, sp, f.z * cp)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FpsStep {
    pub jumped: bool,
    pub landed: bool,
}

/// Advance the body one fixed step from an intent. Pure and deterministic.
pub fn step_body(
    body: &mut FpsBody,
    intent: PlayerIntent,
    arena: &FpsArena,
    config: &FpsConfig,
    dt: f32,
) -> FpsStep {
    // Clamp movement to unit length, but leave `look` free so mouse-look deltas
    // (which can far exceed 1 on a fast flick) are not capped to a sluggish turn.
    let mut intent = intent;
    if intent.movement.length_squared() > 1.0 {
        intent.movement = intent.movement.normalize_or_zero();
    }
    let mut report = FpsStep::default();

    // --- look ---
    body.yaw = (body.yaw + intent.look.x * config.look_step).rem_euclid(TAU);
    body.pitch = (body.pitch - intent.look.y * config.look_step)
        .clamp(-config.pitch_limit, config.pitch_limit);

    // --- horizontal acceleration toward the wish direction ---
    let wish = clamp_len(body.right() * intent.movement.x + body.forward() * intent.movement.y);
    let speed = if intent.sprint_held {
        config.run_speed
    } else {
        config.walk_speed
    };
    let target = wish * speed;
    let accel = if body.grounded {
        if wish.length_squared() > 1e-4 {
            config.ground_accel
        } else {
            config.ground_decel
        }
    } else {
        config.air_accel
    };
    body.velocity.x = approach(body.velocity.x, target.x, accel * dt);
    body.velocity.z = approach(body.velocity.z, target.z, accel * dt);

    // --- jump ---
    body.jump_cd = (body.jump_cd - dt).max(0.0);
    if body.grounded && intent.jump_pressed && body.jump_cd <= 0.0 {
        body.velocity.y = config.jump_speed;
        body.grounded = false;
        body.jump_cd = config.jump_cooldown;
        report.jumped = true;
    }

    // --- gravity ---
    if !body.grounded {
        body.velocity.y = (body.velocity.y - config.gravity * dt).max(-config.max_fall);
    }

    // --- integrate with substeps + collide ---
    let displacement = body.velocity * dt;
    let substeps = (displacement.length() / config.substep).ceil().max(1.0) as usize;
    let sub = displacement / substeps as f32;
    for _ in 0..substeps {
        collide(body, arena, config, sub, &mut report);
    }

    // --- respawn if it somehow leaves the world ---
    if body.position.y < arena.floor_y - 10.0
        || body.position.x.abs() > arena.floor_half + 5.0
        || body.position.z.abs() > arena.floor_half + 5.0
    {
        let (spawn, yaw) = (body.spawn, body.spawn_yaw);
        *body = FpsBody::spawned(spawn, yaw);
    }

    report
}

fn collide(
    body: &mut FpsBody,
    arena: &FpsArena,
    config: &FpsConfig,
    sub: Vec3,
    report: &mut FpsStep,
) {
    let half = Vec3::new(config.radius, config.half_height, config.radius);

    // X axis.
    body.position.x += sub.x;
    for solid in &arena.solids {
        if !solid.overlaps(body.position, half) {
            continue;
        }
        match classify_horizontal_contact(body, solid, arena, config) {
            HorizontalContact::Underfoot => {}
            HorizontalContact::StepUp => step_onto(body, solid, config),
            HorizontalContact::Wall => {
                body.position.x = if sub.x > 0.0 {
                    solid.min.x - config.radius
                } else {
                    solid.max.x + config.radius
                };
                body.velocity.x = 0.0;
            }
        }
    }

    // Z axis.
    body.position.z += sub.z;
    for solid in &arena.solids {
        if !solid.overlaps(body.position, half) {
            continue;
        }
        match classify_horizontal_contact(body, solid, arena, config) {
            HorizontalContact::Underfoot => {}
            HorizontalContact::StepUp => step_onto(body, solid, config),
            HorizontalContact::Wall => {
                body.position.z = if sub.z > 0.0 {
                    solid.min.z - config.radius
                } else {
                    solid.max.z + config.radius
                };
                body.velocity.z = 0.0;
            }
        }
    }

    // Y axis.
    let was_grounded = body.grounded;
    let was_falling = !was_grounded && body.velocity.y < -0.001;
    body.position.y += sub.y;
    body.grounded = false;
    for solid in &arena.solids {
        if !solid.overlaps(body.position, half) {
            continue;
        }
        if sub.y > 0.0 {
            // Rising into a solid's underside: stop at its bottom (bonk head).
            body.position.y = solid.min.y - config.half_height;
            body.velocity.y = 0.0;
        } else {
            // Descending into a solid. Land on its top ONLY if the feet were already at
            // or above that top before this step — i.e. the body is settling onto the
            // surface. A body wedged into the *side* of a tall wall (its feet far below
            // the top, overlapping because the X/Z passes left a diagonal penetration)
            // must NOT be lifted onto the wall's roof; horizontal penetration is the
            // X/Z passes' job to resolve, and lifting it produces the "stuck on the
            // roof" bug. Leaving Y untouched lets it stay at floor level and slide free.
            let feet_before = body.position.y - sub.y - config.half_height;
            if feet_before >= solid.max.y - UNDERFOOT_EPS {
                body.position.y = solid.max.y + config.half_height;
                body.grounded = true;
                if was_falling {
                    report.landed = true;
                }
                body.velocity.y = 0.0;
            }
        }
    }

    // Floor and exact resting support. Without this tolerance a body standing exactly
    // on a floor/deck can end this substep ungrounded (strict AABB overlap does not
    // count touching faces), then "land" again on the next fixed step.
    let feet = body.position.y - config.half_height;
    if body.velocity.y <= 0.0 {
        let mut support_y = if feet <= arena.floor_y + UNDERFOOT_EPS {
            Some(arena.floor_y)
        } else {
            None
        };
        let body_min_x = body.position.x - config.radius;
        let body_max_x = body.position.x + config.radius;
        let body_min_z = body.position.z - config.radius;
        let body_max_z = body.position.z + config.radius;
        for solid in &arena.solids {
            let xz_overlap = body_min_x < solid.max.x
                && body_max_x > solid.min.x
                && body_min_z < solid.max.z
                && body_max_z > solid.min.z;
            let top = solid.max.y;
            if xz_overlap && feet >= top - UNDERFOOT_EPS && feet <= top + UNDERFOOT_EPS {
                support_y = Some(support_y.map_or(top, |current: f32| current.max(top)));
            }
        }
        if let Some(support_y) = support_y {
            if was_falling {
                report.landed = true;
            }
            body.position.y = support_y + config.half_height;
            body.velocity.y = 0.0;
            body.grounded = true;
        }
    }
}

/// Horizontal contact distinguishes the surface supporting the body, a climbable
/// step, and a true wall. This keeps flat-floor behavior unchanged while allowing
/// authored stairs and raised walkways to use the same deterministic controller.
enum HorizontalContact {
    Underfoot,
    StepUp,
    Wall,
}

/// Absorbs the small float error produced while a body rests flush on a solid.
const UNDERFOOT_EPS: f32 = 0.06;

fn classify_horizontal_contact(
    body: &FpsBody,
    solid: &Aabb3,
    arena: &FpsArena,
    config: &FpsConfig,
) -> HorizontalContact {
    let feet = body.position.y - config.half_height;
    let rise = solid.max.y - feet;
    if rise <= UNDERFOOT_EPS {
        return HorizontalContact::Underfoot;
    }
    if rise > config.step_height || body.velocity.y > 1.0 {
        return HorizontalContact::Wall;
    }

    let half = Vec3::new(config.radius, config.half_height, config.radius);
    let stepped = Vec3::new(
        body.position.x,
        solid.max.y + config.half_height,
        body.position.z,
    );
    let blocked = arena
        .solids
        .iter()
        .any(|other| !std::ptr::eq(other, solid) && other.overlaps(stepped, half));
    if blocked {
        HorizontalContact::Wall
    } else {
        HorizontalContact::StepUp
    }
}

fn step_onto(body: &mut FpsBody, solid: &Aabb3, config: &FpsConfig) {
    body.position.y = solid.max.y + config.half_height;
    body.velocity.y = 0.0;
    body.grounded = true;
}

/// Run a whole intent sequence from a fresh body, returning the position after
/// each tick. The deterministic core both tests and the lab's replay use.
pub fn run_path(
    spawn: FpsBody,
    intents: &[PlayerIntent],
    arena: &FpsArena,
    config: &FpsConfig,
    dt: f32,
) -> Vec<Vec3> {
    let mut body = spawn;
    let mut out = Vec::with_capacity(intents.len());
    for intent in intents {
        step_body(&mut body, *intent, arena, config, dt);
        out.push(body.position);
    }
    out
}

fn clamp_len(v: Vec3) -> Vec3 {
    if v.length_squared() > 1.0 {
        v.normalize()
    } else {
        v
    }
}

fn approach(current: f32, target: f32, max_delta: f32) -> f32 {
    if current < target {
        (current + max_delta).min(target)
    } else {
        (current - max_delta).max(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn spawn() -> FpsBody {
        // Stand on the floor near the south wall, facing north (-Z) into the room.
        FpsBody::spawned(Vec3::new(0.0, 0.9, 15.0), 0.0)
    }

    fn forward_tape(ticks: usize) -> Vec<PlayerIntent> {
        vec![
            PlayerIntent {
                movement: Vec2::new(0.0, 1.0),
                ..Default::default()
            };
            ticks
        ]
    }

    #[test]
    fn the_same_intent_sequence_replays_to_an_identical_path() {
        // The exit criterion: determinism. A mixed tape — walk, turn, strafe, jump —
        // produces the same path bit-for-bit on every run.
        let arena = FpsArena::authored();
        let config = FpsConfig::default();
        let mut tape = forward_tape(60);
        for _ in 0..40 {
            tape.push(PlayerIntent {
                movement: Vec2::new(0.0, 1.0),
                look: Vec2::new(1.0, 0.0),
                ..Default::default()
            });
        }
        for _ in 0..30 {
            tape.push(PlayerIntent {
                movement: Vec2::new(1.0, 0.6),
                jump_pressed: true,
                sprint_held: true,
                ..Default::default()
            });
        }

        let a = run_path(spawn(), &tape, &arena, &config, FIXED_DT);
        let b = run_path(spawn(), &tape, &arena, &config, FIXED_DT);
        assert_eq!(a, b, "identical inputs must yield an identical path");
        assert_eq!(a.len(), tape.len());
    }

    #[test]
    fn walking_forward_moves_along_the_facing_direction() {
        let arena = FpsArena::authored();
        let config = FpsConfig::default();
        let path = run_path(spawn(), &forward_tape(30), &arena, &config, FIXED_DT);
        let end = *path.last().unwrap();
        assert!(end.z < 15.0 - 1.0, "moved north (toward -Z)");
        assert!(end.x.abs() < 0.05, "no sideways drift");
    }

    #[test]
    fn turning_then_walking_changes_the_direction_of_travel() {
        let arena = FpsArena::authored();
        let config = FpsConfig::default();
        // Turn right ~90° (look.x for enough ticks), then walk forward.
        let mut tape = Vec::new();
        let quarter = (std::f32::consts::FRAC_PI_2 / config.look_step).ceil() as usize;
        for _ in 0..quarter {
            tape.push(PlayerIntent {
                look: Vec2::new(1.0, 0.0),
                ..Default::default()
            });
        }
        tape.extend(forward_tape(40));
        let path = run_path(spawn(), &tape, &arena, &config, FIXED_DT);
        let end = *path.last().unwrap();
        // Facing was rotated by +90° (yaw 0 = -Z, +yaw turns toward +X), so forward
        // travel now heads east (+X).
        assert!(end.x > 1.0, "now travelling along +X, got {end:?}");
    }

    #[test]
    fn a_wall_blocks_movement() {
        let arena = FpsArena::authored();
        let config = FpsConfig::default();
        // Walk north into the far wall for a long time; never pass it.
        let path = run_path(spawn(), &forward_tape(600), &arena, &config, FIXED_DT);
        let end = *path.last().unwrap();
        assert!(
            end.z > -arena.floor_half,
            "stopped before the north wall, got z={}",
            end.z
        );
    }

    #[test]
    fn jumping_leaves_the_ground_then_gravity_returns_it() {
        let arena = FpsArena::authored();
        let config = FpsConfig::default();
        let mut body = spawn();
        // Settle on the floor.
        step_body(
            &mut body,
            PlayerIntent::default(),
            &arena,
            &config,
            FIXED_DT,
        );
        assert!(body.grounded);
        // Jump.
        let report = step_body(
            &mut body,
            PlayerIntent {
                jump_pressed: true,
                ..Default::default()
            },
            &arena,
            &config,
            FIXED_DT,
        );
        assert!(report.jumped && !body.grounded);
        let peak = body.position.y;
        assert!(peak > 0.9, "rose off the floor");
        // Fall back; eventually grounded again.
        let mut landed = false;
        for _ in 0..300 {
            let r = step_body(
                &mut body,
                PlayerIntent::default(),
                &arena,
                &config,
                FIXED_DT,
            );
            if r.landed || body.grounded {
                landed = true;
                break;
            }
        }
        assert!(landed && body.grounded, "gravity returned it to the floor");
    }

    #[test]
    fn resting_on_floor_stays_grounded_without_repeated_landings() {
        let arena = FpsArena::authored();
        let config = FpsConfig::default();
        let mut body = FpsBody::spawned(Vec3::new(0.0, config.half_height, 0.0), 0.0);
        let mut landed_count = 0;

        for _ in 0..120 {
            let report = step_body(
                &mut body,
                PlayerIntent::default(),
                &arena,
                &config,
                FIXED_DT,
            );
            landed_count += usize::from(report.landed);
            assert!(
                body.grounded,
                "exact floor support should remain grounded after each fixed step"
            );
        }

        assert_eq!(
            landed_count, 1,
            "settling onto the floor may land once, but must not produce a stream"
        );
    }

    #[test]
    fn resting_on_deck_stays_grounded_without_repeated_landings() {
        let config = FpsConfig::default();
        let deck_top = 0.4;
        let arena = FpsArena {
            solids: vec![Aabb3 {
                min: Vec3::new(-2.0, 0.0, -2.0),
                max: Vec3::new(2.0, deck_top, 2.0),
            }],
            floor_y: 0.0,
            floor_half: 10.0,
        };
        let mut body = FpsBody::spawned(Vec3::new(0.0, deck_top + config.half_height, 0.0), 0.0);
        let mut landed_count = 0;

        for _ in 0..120 {
            let report = step_body(
                &mut body,
                PlayerIntent::default(),
                &arena,
                &config,
                FIXED_DT,
            );
            landed_count += usize::from(report.landed);
            assert!(
                body.grounded,
                "exact deck support should remain grounded after each fixed step"
            );
            assert_eq!(
                body.position.y,
                deck_top + config.half_height,
                "resting support should keep the body flush with the deck"
            );
        }

        assert_eq!(
            landed_count, 1,
            "settling onto the deck may land once, but must not produce a stream"
        );
    }

    #[test]
    fn reset_restores_the_spawn() {
        let arena = FpsArena::authored();
        let config = FpsConfig::default();
        let mut body = spawn();
        for intent in forward_tape(50) {
            step_body(&mut body, intent, &arena, &config, FIXED_DT);
        }
        body.reset();
        assert_eq!(body.position, Vec3::new(0.0, 0.9, 15.0));
        assert_eq!(body.velocity, Vec3::ZERO);
        assert_eq!(body.yaw, 0.0);
    }

    #[test]
    fn wedging_into_a_tall_wall_corner_does_not_climb_onto_the_roof() {
        // Two perpendicular full-height walls meeting at a corner. Driving diagonally
        // into the corner can leave the body penetrating a solid after the X/Z passes;
        // the Y pass must keep it on the floor, not snap it to the wall tops (the roof).
        let config = FpsConfig::default();
        let wall_h = 3.4;
        let h = wall_h * 0.5;
        let arena = FpsArena {
            solids: vec![
                // Wall along X at z = -2 (north), and along Z at x = -2 (west).
                Aabb3::from_center_half(Vec3::new(0.0, h, -2.0), Vec3::new(6.0, h, 0.4)),
                Aabb3::from_center_half(Vec3::new(-2.0, h, 0.0), Vec3::new(0.4, h, 6.0)),
            ],
            floor_y: 0.0,
            floor_half: 10.0,
        };
        // Start in the open quadrant, facing the corner (−X/−Z), and shove into it.
        let mut body = FpsBody::spawned(Vec3::new(1.5, config.half_height, 1.5), 0.0);
        body.grounded = true;
        for _ in 0..240 {
            step_body(
                &mut body,
                PlayerIntent {
                    movement: Vec2::new(-1.0, 1.0),
                    sprint_held: true,
                    ..Default::default()
                },
                &arena,
                &config,
                FIXED_DT,
            );
            let feet = body.position.y - config.half_height;
            assert!(
                feet < 0.5,
                "the body climbed the wall corner onto the roof: feet y={feet}"
            );
        }
    }

    #[test]
    fn a_small_solid_is_climbed_but_a_tall_one_blocks() {
        let config = FpsConfig::default();
        let arena = FpsArena {
            solids: vec![
                Aabb3 {
                    min: Vec3::new(1.0, 0.0, -1.0),
                    max: Vec3::new(2.0, config.step_height - 0.05, 1.0),
                },
                Aabb3 {
                    min: Vec3::new(3.0, 0.0, -1.0),
                    max: Vec3::new(4.0, config.step_height + 0.4, 1.0),
                },
            ],
            floor_y: 0.0,
            floor_half: 10.0,
        };
        let mut body = FpsBody::spawned(Vec3::new(0.0, config.half_height, 0.0), 1.5707964);
        body.grounded = true;
        let mut max_feet = 0.0f32;
        for intent in forward_tape(180) {
            step_body(&mut body, intent, &arena, &config, FIXED_DT);
            max_feet = max_feet.max(body.position.y - config.half_height);
        }
        assert!(
            max_feet >= config.step_height - 0.1,
            "the low step was climbed"
        );
        assert!(body.position.x < 3.0, "the taller obstacle still blocks");
    }
}
