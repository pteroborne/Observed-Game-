//! Canonical collision input and the temporary dual-backend traversal seam.
//!
//! `ArenaSpec` is pure, stable data. The game, headless match, replay, and network
//! layers build it without depending on Rapier handles. `TraversalWorld` consumes that
//! data and owns all backend runtime state. The legacy arm exists only for migration;
//! new authored convex geometry is accepted by the Rapier arm only.

use glam::Vec3;
use player_input::PlayerIntent;
use rapier3d::{
    control::{CharacterAutostep, CharacterLength, KinematicCharacterController},
    prelude::*,
};

use crate::{Aabb3, FpsArena, FpsBody, FpsConfig, FpsStep, approach, clamp_len, step_body};

/// Stable identity of one collider inside a baked/current-place arena.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StableColliderId(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub enum ColliderShape {
    Cuboid { half: Vec3 },
    ConvexHull { points: Vec<Vec3> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColliderSpec {
    pub id: StableColliderId,
    pub center: Vec3,
    /// Rotation stored as a normalized `(x, y, z, w)` quaternion.
    pub rotation: [f32; 4],
    pub shape: ColliderShape,
    pub friction: f32,
}

impl ColliderSpec {
    pub fn cuboid(id: StableColliderId, center: Vec3, half: Vec3) -> Self {
        Self {
            id,
            center,
            rotation: [0.0, 0.0, 0.0, 1.0],
            shape: ColliderShape::Cuboid { half },
            friction: 0.8,
        }
    }
}

/// Pure collision description for one isolated room/hallway Place.
#[derive(Clone, Debug, PartialEq)]
pub struct ArenaSpec {
    pub colliders: Vec<ColliderSpec>,
    pub floor_y: f32,
    pub floor_half: f32,
}

impl ArenaSpec {
    pub fn from_legacy(arena: &FpsArena) -> Self {
        let mut colliders = Vec::with_capacity(arena.solids.len() + 1);
        // The legacy floor is implicit. Rapier needs a real support surface with its
        // top face exactly at `floor_y`.
        colliders.push(ColliderSpec::cuboid(
            StableColliderId(0),
            Vec3::new(0.0, arena.floor_y - 0.5, 0.0),
            Vec3::new(arena.floor_half, 0.5, arena.floor_half),
        ));
        colliders.extend(arena.solids.iter().enumerate().map(|(index, solid)| {
            ColliderSpec::cuboid(
                StableColliderId(index as u32 + 1),
                (solid.min + solid.max) * 0.5,
                (solid.max - solid.min) * 0.5,
            )
        }));
        Self {
            colliders,
            floor_y: arena.floor_y,
            floor_half: arena.floor_half,
        }
    }

    pub fn validate(&self) -> Result<(), ArenaSpecError> {
        let mut ids = std::collections::BTreeSet::new();
        for collider in &self.colliders {
            if !ids.insert(collider.id) {
                return Err(ArenaSpecError::DuplicateId(collider.id));
            }
            if !collider.center.is_finite()
                || !collider.friction.is_finite()
                || collider.friction < 0.0
                || !collider.rotation.iter().all(|value| value.is_finite())
            {
                return Err(ArenaSpecError::InvalidCollider(collider.id));
            }
            match &collider.shape {
                ColliderShape::Cuboid { half } => {
                    if !half.is_finite() || half.min_element() <= 0.0 {
                        return Err(ArenaSpecError::InvalidCollider(collider.id));
                    }
                }
                ColliderShape::ConvexHull { points } => {
                    if points.len() < 4 || points.iter().any(|point| !point.is_finite()) {
                        return Err(ArenaSpecError::InvalidCollider(collider.id));
                    }
                    let rapier_points: Vec<Vector> =
                        points.iter().copied().map(into_rapier).collect();
                    if ColliderBuilder::convex_hull(&rapier_points).is_none() {
                        return Err(ArenaSpecError::DegenerateHull(collider.id));
                    }
                }
            }
        }
        if !self.floor_y.is_finite() || !self.floor_half.is_finite() || self.floor_half <= 0.0 {
            return Err(ArenaSpecError::InvalidBounds);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArenaSpecError {
    DuplicateId(StableColliderId),
    InvalidCollider(StableColliderId),
    DegenerateHull(StableColliderId),
    InvalidBounds,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicsBackend {
    LegacyAabb,
    Rapier,
}

/// Runtime owner for the selected traversal backend. Cloning rebuilds backend handles
/// from the pure `ArenaSpec`; persistent state never treats those handles as identity.
pub enum TraversalWorld {
    Legacy { arena: FpsArena },
    Rapier(Box<RapierTraversalWorld>),
}

impl Clone for TraversalWorld {
    fn clone(&self) -> Self {
        match self {
            Self::Legacy { arena } => Self::Legacy {
                arena: arena.clone(),
            },
            Self::Rapier(world) => Self::Rapier(Box::new(RapierTraversalWorld::new(
                world.spec.clone(),
                &world.config,
            ))),
        }
    }
}

impl std::fmt::Debug for TraversalWorld {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TraversalWorld")
            .field("backend", &self.backend())
            .field("arena", &self.arena_spec())
            .finish()
    }
}

impl TraversalWorld {
    pub fn from_legacy(arena: FpsArena) -> Self {
        Self::Legacy { arena }
    }

    pub fn from_spec(spec: ArenaSpec, backend: PhysicsBackend, config: &FpsConfig) -> Self {
        spec.validate().expect("validated traversal arena");
        match backend {
            PhysicsBackend::LegacyAabb => {
                let solids = spec
                    .colliders
                    .iter()
                    .filter_map(|collider| match collider.shape {
                        ColliderShape::Cuboid { half } if collider.id != StableColliderId(0) => {
                            Some(Aabb3::from_center_half(collider.center, half))
                        }
                        _ => None,
                    })
                    .collect();
                Self::Legacy {
                    arena: FpsArena {
                        solids,
                        floor_y: spec.floor_y,
                        floor_half: spec.floor_half,
                    },
                }
            }
            PhysicsBackend::Rapier => {
                Self::Rapier(Box::new(RapierTraversalWorld::new(spec, config)))
            }
        }
    }

    pub fn backend(&self) -> PhysicsBackend {
        match self {
            Self::Legacy { .. } => PhysicsBackend::LegacyAabb,
            Self::Rapier(_) => PhysicsBackend::Rapier,
        }
    }

    pub fn step_body(
        &mut self,
        body: &mut FpsBody,
        intent: PlayerIntent,
        config: &FpsConfig,
        dt: f32,
    ) -> FpsStep {
        match self {
            Self::Legacy { arena } => step_body(body, intent, arena, config, dt),
            Self::Rapier(world) => world.step_body(body, intent, config, dt),
        }
    }

    pub fn arena_spec(&self) -> ArenaSpec {
        match self {
            Self::Legacy { arena } => ArenaSpec::from_legacy(arena),
            Self::Rapier(world) => world.spec.clone(),
        }
    }
}

pub struct RapierTraversalWorld {
    spec: ArenaSpec,
    config: FpsConfig,
    pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad_phase: BroadPhaseBvh,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd: CCDSolver,
    integration: IntegrationParameters,
    controller: KinematicCharacterController,
    capsule: Capsule,
}

impl RapierTraversalWorld {
    fn new(spec: ArenaSpec, config: &FpsConfig) -> Self {
        let mut world = Self {
            spec,
            config: *config,
            pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad_phase: BroadPhaseBvh::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd: CCDSolver::new(),
            integration: IntegrationParameters {
                dt: crate::FIXED_DT,
                ..IntegrationParameters::default()
            },
            controller: KinematicCharacterController {
                offset: CharacterLength::Absolute(config.controller_offset),
                autostep: Some(CharacterAutostep {
                    max_height: CharacterLength::Absolute(config.step_height),
                    min_width: CharacterLength::Absolute(config.minimum_step_width),
                    include_dynamic_bodies: false,
                }),
                max_slope_climb_angle: config.maximum_slope_degrees.to_radians(),
                snap_to_ground: Some(CharacterLength::Absolute(config.ground_snap)),
                ..Default::default()
            },
            capsule: Capsule::new_y(
                (config.half_height - config.radius).max(0.01),
                config.radius,
            ),
        };
        for collider in &world.spec.colliders {
            let [x, y, z, w] = collider.rotation;
            let rotation = Rotation::from_xyzw(x, y, z, w).normalize();
            let builder = match &collider.shape {
                ColliderShape::Cuboid { half } => ColliderBuilder::cuboid(half.x, half.y, half.z),
                ColliderShape::ConvexHull { points } => {
                    let points: Vec<Vector> = points.iter().copied().map(into_rapier).collect();
                    ColliderBuilder::convex_hull(&points).expect("ArenaSpec validated convex hull")
                }
            }
            .position(Pose::from_parts(into_rapier(collider.center), rotation))
            .friction(collider.friction)
            .user_data(u128::from(collider.id.0));
            world.colliders.insert(builder);
        }
        world.pipeline.step(
            Vector::ZERO,
            &world.integration,
            &mut world.islands,
            &mut world.broad_phase,
            &mut world.narrow_phase,
            &mut world.bodies,
            &mut world.colliders,
            &mut world.impulse_joints,
            &mut world.multibody_joints,
            &mut world.ccd,
            &(),
            &(),
        );
        world
    }

    fn step_body(
        &mut self,
        body: &mut FpsBody,
        mut intent: PlayerIntent,
        config: &FpsConfig,
        dt: f32,
    ) -> FpsStep {
        if intent.movement.length_squared() > 1.0 {
            intent.movement = intent.movement.normalize_or_zero();
        }
        let mut report = FpsStep::default();
        body.yaw = (body.yaw + intent.look.x * config.look_step).rem_euclid(std::f32::consts::TAU);
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

        self.integration.dt = dt;
        let desired = into_rapier(body.velocity * dt);
        let pose = Pose::from_translation(into_rapier(body.position));
        let queries = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            QueryFilter::default(),
        );
        let movement =
            self.controller
                .move_shape(dt, &queries, &self.capsule, &pose, desired, |_| {});
        let was_grounded = body.grounded;
        body.position += from_rapier(movement.translation);
        body.grounded = movement.grounded;
        if movement.grounded && body.velocity.y < 0.0 {
            body.velocity.y = 0.0;
        }
        report.landed = !was_grounded && movement.grounded && !report.jumped;

        if body.position.y < self.spec.floor_y - 10.0
            || body.position.x.abs() > self.spec.floor_half + 5.0
            || body.position.z.abs() > self.spec.floor_half + 5.0
        {
            let (spawn, yaw) = (body.spawn, body.spawn_yaw);
            *body = FpsBody::spawned(spawn, yaw);
        }
        report
    }
}

fn into_rapier(value: Vec3) -> Vector {
    Vector::new(value.x, value.y, value.z)
}

fn from_rapier(value: Vector) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn tape() -> Vec<PlayerIntent> {
        (0..360)
            .map(|tick| PlayerIntent {
                movement: if tick < 240 {
                    Vec2::Y
                } else {
                    Vec2::new(0.4, 0.8)
                },
                look: if (120..150).contains(&tick) {
                    Vec2::new(0.25, 0.0)
                } else {
                    Vec2::ZERO
                },
                jump_pressed: tick == 90,
                sprint_held: tick > 30,
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn legacy_conversion_is_canonical_and_valid() {
        let arena = FpsArena::authored();
        let a = ArenaSpec::from_legacy(&arena);
        let b = ArenaSpec::from_legacy(&arena);
        assert_eq!(a, b);
        assert_eq!(a.colliders.len(), arena.solids.len() + 1);
        a.validate().unwrap();
    }

    #[test]
    fn authored_convex_hull_is_accepted() {
        let spec = ArenaSpec {
            colliders: vec![ColliderSpec {
                id: StableColliderId(1),
                center: Vec3::ZERO,
                rotation: [0.0, 0.0, 0.0, 1.0],
                shape: ColliderShape::ConvexHull {
                    points: vec![
                        Vec3::new(-1.0, 0.0, -1.0),
                        Vec3::new(1.0, 0.0, -1.0),
                        Vec3::new(0.0, 0.0, 1.0),
                        Vec3::new(0.0, 1.0, 0.0),
                    ],
                },
                friction: 0.8,
            }],
            floor_y: 0.0,
            floor_half: 10.0,
        };
        spec.validate().unwrap();
        let _world = TraversalWorld::from_spec(
            spec,
            PhysicsBackend::Rapier,
            &FpsConfig::deliberate_rapier(),
        );
    }

    #[test]
    fn two_rapier_worlds_replay_bit_identically() {
        let config = FpsConfig::deliberate_rapier();
        let spec = ArenaSpec::from_legacy(&FpsArena::authored());
        let mut a = TraversalWorld::from_spec(spec.clone(), PhysicsBackend::Rapier, &config);
        let mut b = TraversalWorld::from_spec(spec, PhysicsBackend::Rapier, &config);
        let mut body_a = FpsBody::spawned(Vec3::new(0.0, config.half_height, 15.0), 0.0);
        let mut body_b = body_a;
        for (tick, intent) in tape().into_iter().enumerate() {
            assert_eq!(
                a.step_body(&mut body_a, intent, &config, crate::FIXED_DT),
                b.step_body(&mut body_b, intent, &config, crate::FIXED_DT),
                "events diverged at tick {tick}"
            );
            assert_eq!(body_a, body_b, "body diverged at tick {tick}");
        }
    }

    #[test]
    fn rapier_capsule_walks_jumps_and_stays_inside_walls() {
        let config = FpsConfig::deliberate_rapier();
        let arena = FpsArena::authored();
        let spec = ArenaSpec::from_legacy(&arena);
        let mut world = TraversalWorld::from_spec(spec, PhysicsBackend::Rapier, &config);
        let mut body = FpsBody::spawned(Vec3::new(0.0, config.half_height, 15.0), 0.0);
        let mut jumped = false;
        let mut landed = false;
        for intent in tape() {
            let report = world.step_body(&mut body, intent, &config, crate::FIXED_DT);
            jumped |= report.jumped;
            landed |= report.landed;
        }
        assert!(jumped && landed);
        assert!(body.position.z > -arena.floor_half);
        assert!(body.position.is_finite());
    }
}
