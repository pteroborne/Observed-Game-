//! Elevation feasibility: a **deterministic first-person controller that traverses
//! vertical geometry** — stairs, raised platforms, and ledges.
//!
//! The proven Phase 20 controller ([`fps_controller_lab`]) already stands on top of
//! AABB solids and lands when it falls, but it has **no step-up**: a stair riser just
//! blocks it. This module adds a deterministic step offset — when a horizontal move is
//! blocked by a solid whose top is within `STEP_HEIGHT` of the feet and the space above
//! is clear, the body climbs onto it instead of stopping — and an authored multi-level
//! course to prove it. Anything taller than `STEP_HEIGHT` still blocks (you must jump).
//!
//! It began as an **isolated variant** of the controller (reusing `FpsBody` /
//! `FpsConfig` / `Aabb3`) so the behavior could be proven before promotion. The same
//! contact rules now live in shared `step_body` for the generated maze; this lab
//! remains the focused authored course. The step depends only on
//! (body, intent, arena, dt), so the same inputs yield the same path.

use std::f32::consts::TAU;

use bevy::math::Vec3;
use bevy::prelude::Resource;
use fps_controller_lab::controller::{Aabb3, FpsBody, FpsConfig, FpsStep};
use player_input::PlayerIntent;

/// Maximum height the controller auto-climbs without jumping (one stair step).
pub const STEP_HEIGHT: f32 = 0.45;

#[derive(Resource, Clone, Debug)]
pub struct ElevArena {
    pub solids: Vec<Aabb3>,
    pub floor_y: f32,
    pub floor_half: f32,
}

impl ElevArena {
    /// A multi-level test course: a flight of stairs climbing onto a raised platform,
    /// a too-tall block that must be jumped (never stepped), and perimeter walls.
    pub fn authored() -> Self {
        let floor_half = 20.0;
        let wall_h = 6.0;
        let t = 0.5;
        let wall = |center: Vec3, hx: f32, hz: f32| {
            Aabb3::from_center_half(center, Vec3::new(hx, wall_h, hz))
        };
        let mut solids = vec![
            wall(Vec3::new(0.0, wall_h, -floor_half), floor_half, t),
            wall(Vec3::new(0.0, wall_h, floor_half), floor_half, t),
            wall(Vec3::new(floor_half, wall_h, 0.0), t, floor_half),
            wall(Vec3::new(-floor_half, wall_h, 0.0), t, floor_half),
        ];

        // Stairs ascending toward +X: four steps, each a separate one-tread block at
        // an increasing height. Each rise is below STEP_HEIGHT so it is auto-climbed.
        let rise = STEP_RISE;
        let tread = 1.6;
        let stair_x0 = 3.0;
        for i in 0..STEP_COUNT {
            let top = (i + 1) as f32 * rise;
            let x0 = stair_x0 + i as f32 * tread;
            solids.push(Aabb3 {
                min: Vec3::new(x0, 0.0, -3.0),
                max: Vec3::new(x0 + tread, top, 3.0),
            });
        }
        // The platform you arrive on (its top matches the highest step).
        let platform_top = STEP_COUNT as f32 * rise;
        let platform_x0 = stair_x0 + STEP_COUNT as f32 * tread;
        solids.push(Aabb3 {
            min: Vec3::new(platform_x0, 0.0, -6.0),
            max: Vec3::new(platform_x0 + 8.0, platform_top, 6.0),
        });

        // A block too tall to step (must be jumped or walked around), toward -X.
        solids.push(Aabb3 {
            min: Vec3::new(-9.0, 0.0, -2.0),
            max: Vec3::new(-7.0, 1.3, 2.0),
        });

        Self {
            solids,
            floor_y: 0.0,
            floor_half,
        }
    }

    /// The height the stairs/platform reach (top of the highest step).
    pub fn platform_top(&self) -> f32 {
        STEP_COUNT as f32 * STEP_RISE
    }
}

pub const STEP_COUNT: usize = 4;
pub const STEP_RISE: f32 = 0.34;

fn overlaps(solid: &Aabb3, center: Vec3, half: Vec3) -> bool {
    let min = center - half;
    let max = center + half;
    min.x < solid.max.x
        && max.x > solid.min.x
        && min.y < solid.max.y
        && max.y > solid.min.y
        && min.z < solid.max.z
        && max.z > solid.min.z
}

/// Advance the elevation body one fixed step. Pure and deterministic; identical to the
/// shared controller except the horizontal collision attempts a step-up first.
pub fn step_body_elev(
    body: &mut FpsBody,
    intent: PlayerIntent,
    arena: &ElevArena,
    config: &FpsConfig,
    dt: f32,
) -> FpsStep {
    let mut intent = intent;
    if intent.movement.length_squared() > 1.0 {
        intent.movement = intent.movement.normalize_or_zero();
    }
    let mut report = FpsStep::default();

    body.yaw = (body.yaw + intent.look.x * config.look_step).rem_euclid(TAU);
    body.pitch = (body.pitch - intent.look.y * config.look_step)
        .clamp(-config.pitch_limit, config.pitch_limit);

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

    body.jump_cd = (body.jump_cd - dt).max(0.0);
    if body.grounded && intent.jump_pressed && body.jump_cd <= 0.0 {
        body.velocity.y = config.jump_speed;
        body.grounded = false;
        body.jump_cd = config.jump_cooldown;
        report.jumped = true;
    }

    if !body.grounded {
        body.velocity.y = (body.velocity.y - config.gravity * dt).max(-config.max_fall);
    }

    let displacement = body.velocity * dt;
    let substeps = (displacement.length() / config.substep).ceil().max(1.0) as usize;
    let sub = displacement / substeps as f32;
    for _ in 0..substeps {
        collide(body, arena, config, sub, &mut report);
    }

    if body.position.y < arena.floor_y - 10.0
        || body.position.x.abs() > arena.floor_half + 5.0
        || body.position.z.abs() > arena.floor_half + 5.0
    {
        let (spawn, yaw) = (body.spawn, body.spawn_yaw);
        *body = FpsBody::spawned(spawn, yaw);
    }

    report
}

/// How a solid the body is touching relates to the feet, deciding the horizontal
/// response: the floor it stands on (ignore), a small step (climb), or a wall (block).
enum Contact {
    /// Top is at or below the feet — the surface being stood on. Not a side obstacle.
    Underfoot,
    /// Top is a small rise above the feet, with clear headroom — step up onto it.
    StepUp,
    /// Too tall (or no headroom) — a wall.
    Wall,
}

/// The tolerance below which a solid's top counts as "underfoot" (absorbs the float
/// error of resting flush on a surface).
const UNDERFOOT_EPS: f32 = 0.06;

fn classify(body: &FpsBody, solid: &Aabb3, arena: &ElevArena, config: &FpsConfig) -> Contact {
    let feet = body.position.y - config.half_height;
    let rise = solid.max.y - feet;
    if rise <= UNDERFOOT_EPS {
        return Contact::Underfoot;
    }
    if rise > STEP_HEIGHT || body.velocity.y > 1.0 {
        return Contact::Wall;
    }
    // Headroom: would the body fit standing on this solid? Exclude the solid itself.
    let half = Vec3::new(config.radius, config.half_height, config.radius);
    let stepped = Vec3::new(
        body.position.x,
        solid.max.y + config.half_height,
        body.position.z,
    );
    let blocked = arena
        .solids
        .iter()
        .any(|other| !std::ptr::eq(other, solid) && overlaps(other, stepped, half));
    if blocked {
        Contact::Wall
    } else {
        Contact::StepUp
    }
}

fn step_onto(body: &mut FpsBody, solid: &Aabb3, config: &FpsConfig) {
    body.position.y = solid.max.y + config.half_height;
    body.velocity.y = 0.0;
    body.grounded = true;
}

fn collide(
    body: &mut FpsBody,
    arena: &ElevArena,
    config: &FpsConfig,
    sub: Vec3,
    report: &mut FpsStep,
) {
    let half = Vec3::new(config.radius, config.half_height, config.radius);

    // X axis (with step-up).
    body.position.x += sub.x;
    for solid in &arena.solids {
        if !overlaps(solid, body.position, half) {
            continue;
        }
        match classify(body, solid, arena, config) {
            Contact::Underfoot => {}
            Contact::StepUp => step_onto(body, solid, config),
            Contact::Wall => {
                body.position.x = if sub.x > 0.0 {
                    solid.min.x - config.radius
                } else {
                    solid.max.x + config.radius
                };
                body.velocity.x = 0.0;
            }
        }
    }

    // Z axis (with step-up).
    body.position.z += sub.z;
    for solid in &arena.solids {
        if !overlaps(solid, body.position, half) {
            continue;
        }
        match classify(body, solid, arena, config) {
            Contact::Underfoot => {}
            Contact::StepUp => step_onto(body, solid, config),
            Contact::Wall => {
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
    body.position.y += sub.y;
    body.grounded = false;
    for solid in &arena.solids {
        if overlaps(solid, body.position, half) {
            if sub.y <= 0.0 {
                body.position.y = solid.max.y + config.half_height;
                body.grounded = true;
                report.landed = true;
            } else {
                body.position.y = solid.min.y - config.half_height;
            }
            body.velocity.y = 0.0;
        }
    }

    // Floor.
    let feet = body.position.y - config.half_height;
    if feet < arena.floor_y {
        if !body.grounded {
            report.landed = true;
        }
        body.position.y = arena.floor_y + config.half_height;
        body.velocity.y = 0.0;
        body.grounded = true;
    }
}

pub fn run_path_elev(
    spawn: FpsBody,
    intents: &[PlayerIntent],
    arena: &ElevArena,
    config: &FpsConfig,
    dt: f32,
) -> Vec<Vec3> {
    let mut body = spawn;
    let mut out = Vec::with_capacity(intents.len());
    for intent in intents {
        step_body_elev(&mut body, *intent, arena, config, dt);
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
    use bevy::math::Vec2;
    use fps_controller_lab::controller::FIXED_DT;
    use std::f32::consts::FRAC_PI_2;

    fn config() -> FpsConfig {
        FpsConfig::default()
    }

    /// Stand on the floor at the origin facing +X (toward the stairs).
    fn spawn_facing_east() -> FpsBody {
        FpsBody::spawned(Vec3::new(0.0, 0.9, 0.0), FRAC_PI_2)
    }

    fn forward(ticks: usize, sprint: bool) -> Vec<PlayerIntent> {
        vec![
            PlayerIntent {
                movement: Vec2::new(0.0, 1.0),
                sprint_held: sprint,
                ..Default::default()
            };
            ticks
        ]
    }

    #[test]
    fn the_same_intents_climb_to_an_identical_path() {
        let arena = ElevArena::authored();
        let config = config();
        let tape = forward(240, true);
        let a = run_path_elev(spawn_facing_east(), &tape, &arena, &config, FIXED_DT);
        let b = run_path_elev(spawn_facing_east(), &tape, &arena, &config, FIXED_DT);
        assert_eq!(a, b, "elevation traversal must be deterministic");
    }

    #[test]
    fn walking_into_the_stairs_climbs_onto_the_platform() {
        let arena = ElevArena::authored();
        let config = config();
        let mut body = spawn_facing_east();
        // Track the highest the feet reach: the body must ascend the stairs onto the
        // raised platform (it would otherwise stay on the floor at 0).
        let mut max_feet = 0.0f32;
        for intent in forward(170, false) {
            step_body_elev(&mut body, intent, &arena, &config, FIXED_DT);
            max_feet = max_feet.max(body.position.y - config.half_height);
        }
        assert!(
            max_feet >= arena.platform_top() - 0.05,
            "climbed onto the platform (reached {max_feet}, top is {}, x={})",
            arena.platform_top(),
            body.position.x
        );
        assert!(body.position.x > 9.0, "advanced through the facility");
    }

    #[test]
    fn a_block_taller_than_the_step_height_is_not_climbed() {
        let arena = ElevArena::authored();
        let config = config();
        // Face -X toward the 1.3-tall block and push into it.
        let mut body = FpsBody::spawned(Vec3::new(-4.0, 0.9, 0.0), FRAC_PI_2 * 3.0);
        for intent in forward(300, false) {
            step_body_elev(&mut body, intent, &arena, &config, FIXED_DT);
        }
        let feet = body.position.y - config.half_height;
        assert!(
            feet < STEP_HEIGHT,
            "never climbed the tall block, feet={feet}"
        );
        assert!(
            body.position.x > -7.0,
            "stopped at the block face, x={}",
            body.position.x
        );
    }

    #[test]
    fn walking_off_the_platform_falls_back_to_the_floor() {
        let arena = ElevArena::authored();
        let config = config();
        // Start standing on the platform, walk off the open +Z edge (the +X edge is
        // against the perimeter wall). yaw = PI faces +Z.
        let mut body = FpsBody::spawned(
            Vec3::new(14.0, arena.platform_top() + 0.9, 0.0),
            std::f32::consts::PI,
        );
        body.grounded = true;
        for intent in forward(240, true) {
            step_body_elev(&mut body, intent, &arena, &config, FIXED_DT);
        }
        // Fell off the platform's open edge back down to floor level. (The `grounded`
        // flag flickers one tick on/off while at rest — inherited from the base
        // controller — so position is the robust check.)
        let feet = body.position.y - config.half_height;
        assert!(
            (feet - arena.floor_y).abs() < 0.05,
            "fell back to the floor, feet={feet}"
        );
    }
}
