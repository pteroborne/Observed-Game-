//! Deterministic raw-Rapier kinematic capsule controller.
//!
//! Rapier owns collision queries, grounding, slope response, depenetration, and
//! autostepping. Locomotion continues to consume the shared [`PlayerIntent`]
//! boundary at the production fixed timestep. No Bevy or `bevy_rapier` types enter
//! this module.

use std::f32::consts::TAU;

use glam::Vec3;
use player_input::PlayerIntent;
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};
use rapier3d::prelude::*;

use super::{Aabb3, FpsArena, FpsBody, FpsConfig, FpsStep, approach, clamp_len};

/// A semantic, engine-independent structural collider. Rendering, navigation, and
/// Rapier scene construction should project from the same list of these primitives;
/// `yaw` keeps rotated stair treads and polygon walls exact instead of inflating them
/// into conservative world-axis boxes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StructuralCollider {
    pub center: Vec3,
    pub half: Vec3,
    pub yaw: f32,
}

impl StructuralCollider {
    pub const fn axis_aligned(center: Vec3, half: Vec3) -> Self {
        Self {
            center,
            half,
            yaw: 0.0,
        }
    }
}

/// A collision scene for one discrete room or corridor.
pub struct RapierTraversalScene {
    bodies: RigidBodySet,
    colliders: ColliderSet,
    broad_phase: BroadPhaseBvh,
    narrow_phase: NarrowPhase,
    floor_y: f32,
    floor_half: f32,
}

impl RapierTraversalScene {
    /// Compatibility projection for the existing structural arena. This is the
    /// cutover bridge; richer oriented primitives can be inserted with
    /// [`Self::from_cuboids`] without changing the controller.
    pub fn from_arena(arena: &FpsArena) -> Self {
        let solids = arena
            .solids
            .iter()
            .map(|solid| {
                StructuralCollider::axis_aligned(
                    (solid.min + solid.max) * 0.5,
                    (solid.max - solid.min) * 0.5,
                )
            })
            .collect::<Vec<_>>();
        Self::from_primitives(arena.floor_y, arena.floor_half, &solids)
    }

    pub fn from_cuboids(floor_y: f32, floor_half: f32, solids: &[Aabb3]) -> Self {
        let primitives = solids
            .iter()
            .map(|solid| {
                StructuralCollider::axis_aligned(
                    (solid.min + solid.max) * 0.5,
                    (solid.max - solid.min) * 0.5,
                )
            })
            .collect::<Vec<_>>();
        Self::from_primitives(floor_y, floor_half, &primitives)
    }

    pub fn from_primitives(
        floor_y: f32,
        floor_half: f32,
        primitives: &[StructuralCollider],
    ) -> Self {
        let bodies = RigidBodySet::new();
        let mut colliders = ColliderSet::new();
        let mut handles = Vec::with_capacity(primitives.len() + 1);

        // A finite floor keeps the discrete-place respawn contract meaningful.
        handles.push(
            colliders.insert(
                ColliderBuilder::cuboid(floor_half, 0.1, floor_half)
                    .translation(Vector::new(0.0, floor_y - 0.1, 0.0))
                    .friction(0.9)
                    .build(),
            ),
        );
        for primitive in primitives {
            handles.push(
                colliders.insert(
                    ColliderBuilder::cuboid(primitive.half.x, primitive.half.y, primitive.half.z)
                        .translation(Vector::new(
                            primitive.center.x,
                            primitive.center.y,
                            primitive.center.z,
                        ))
                        .rotation(Vector::Y * primitive.yaw)
                        .friction(0.8)
                        .build(),
                ),
            );
        }

        let mut broad_phase = BroadPhaseBvh::new();
        let narrow_phase = NarrowPhase::new();
        let mut events = Vec::new();
        broad_phase.update(
            &IntegrationParameters::default(),
            &colliders,
            &bodies,
            &handles,
            &[],
            &mut events,
        );
        Self {
            bodies,
            colliders,
            broad_phase,
            narrow_phase,
            floor_y,
            floor_half,
        }
    }

    pub fn collider_count(&self) -> usize {
        self.colliders.len()
    }

    pub fn floor_y(&self) -> f32 {
        self.floor_y
    }

    pub fn floor_half(&self) -> f32 {
        self.floor_half
    }
}

/// Advance a character with Rapier owning all physical resolution.
pub fn step_character(
    scene: &RapierTraversalScene,
    body: &mut FpsBody,
    intent: PlayerIntent,
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
        if wish.length_squared() > 1.0e-4 {
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

    let was_grounded = body.grounded;
    let radius = config.radius;
    let segment_half = (config.half_height - radius).max(0.01);
    let capsule = Capsule::new_y(segment_half, radius);
    let controller = KinematicCharacterController {
        offset: CharacterLength::Absolute(0.01),
        autostep: Some(CharacterAutostep {
            max_height: CharacterLength::Absolute(config.step_height),
            min_width: CharacterLength::Absolute(config.radius * 1.25),
            include_dynamic_bodies: true,
        }),
        snap_to_ground: (!report.jumped).then_some(CharacterLength::Absolute(0.08)),
        max_slope_climb_angle: 50.0_f32.to_radians(),
        min_slope_slide_angle: 52.0_f32.to_radians(),
        ..Default::default()
    };
    let query = scene.broad_phase.as_query_pipeline(
        scene.narrow_phase.query_dispatcher(),
        &scene.bodies,
        &scene.colliders,
        QueryFilter::default(),
    );
    let pose = Pose::translation(body.position.x, body.position.y, body.position.z);
    let desired = body.velocity * dt;
    let movement = controller.move_shape(
        dt,
        &query,
        &capsule,
        &pose,
        Vector::new(desired.x, desired.y, desired.z),
        |_| {},
    );
    body.position += Vec3::new(
        movement.translation.x,
        movement.translation.y,
        movement.translation.z,
    );
    body.grounded = movement.grounded;
    if body.grounded && body.velocity.y <= 0.0 {
        if !was_grounded {
            report.landed = true;
        }
        body.velocity.y = 0.0;
    }

    if body.position.y < scene.floor_y - 10.0
        || body.position.x.abs() > scene.floor_half + 5.0
        || body.position.z.abs() > scene.floor_half + 5.0
    {
        let (spawn, yaw) = (body.spawn, body.spawn_yaw);
        *body = FpsBody::spawned(spawn, yaw);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn forward() -> PlayerIntent {
        PlayerIntent {
            movement: Vec2::new(0.0, 1.0),
            ..Default::default()
        }
    }

    #[test]
    fn identical_rapier_scenes_and_intents_are_bit_identical() {
        let arena = FpsArena::authored();
        let a_scene = RapierTraversalScene::from_arena(&arena);
        let b_scene = RapierTraversalScene::from_arena(&arena);
        let config = FpsConfig::default();
        let spawn = Vec3::new(0.0, config.half_height, 15.0);
        let mut a = FpsBody::spawned(spawn, 0.0);
        let mut b = a;
        for tick in 0..360 {
            let mut intent = forward();
            intent.sprint_held = tick > 90;
            intent.jump_pressed = tick == 120;
            step_character(&a_scene, &mut a, intent, &config, super::super::FIXED_DT);
            step_character(&b_scene, &mut b, intent, &config, super::super::FIXED_DT);
            assert_eq!(a, b, "Rapier controller diverged at tick {tick}");
        }
    }

    #[test]
    fn capsule_is_blocked_by_the_authored_wall() {
        let arena = FpsArena::authored();
        let scene = RapierTraversalScene::from_arena(&arena);
        let config = FpsConfig::default();
        let mut body = FpsBody::spawned(Vec3::new(0.0, config.half_height, 15.0), 0.0);
        for _ in 0..600 {
            step_character(
                &scene,
                &mut body,
                forward(),
                &config,
                super::super::FIXED_DT,
            );
        }
        assert!(body.position.z > -arena.floor_half);
        assert!(body.grounded);
    }

    #[test]
    fn collider_scene_contains_floor_and_every_structural_solid() {
        let arena = FpsArena::authored();
        let scene = RapierTraversalScene::from_arena(&arena);
        assert_eq!(scene.collider_count(), arena.solids.len() + 1);
    }
}
