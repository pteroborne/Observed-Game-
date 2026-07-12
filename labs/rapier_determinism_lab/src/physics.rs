//! Pure rapier3d harness — no Bevy, no rendering. This is the actual subject of
//! the feasibility spike: *can rapier3d step convex and smooth (curved) colliders
//! in fixed-dt lockstep and produce reproducible, bit-identical state?*
//!
//! The scene deliberately mixes the shape families the question is about — a
//! **ball** (a perfectly smooth analytic sphere), a **capsule** (a smooth
//! swept-sphere that rolls / topples), and a **convex hull** (an arbitrary convex
//! polyhedron built from jittered cube corners) — all given nonzero spin so
//! quaternion integration (the fiddliest bit for determinism) is exercised, then
//! dropped onto a static ground box.
//!
//! Determinism is proven two ways. `state_hash` FNV-folds the raw IEEE-754 bits of
//! every body's translation and rotation, so any divergence — even one ULP —
//! changes the hash. `replay_is_bit_identical` runs two independent worlds from the
//! same start and asserts equal snapshots, i.e. the lockstep guarantee a networked
//! or replay system relies on.

use rapier3d::prelude::*;

/// Fixed simulation step, matching the game's 60 Hz lockstep cadence.
pub const DT: Real = 1.0 / 60.0;

/// Which primitive a body is, so the renderer can pick a mesh. The collider under
/// test is always the real rapier shape — for `Convex` the render mesh is only an
/// illustrative box; the *collider* is the genuine convex hull.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BodyKind {
    Ball { radius: f32 },
    Capsule { half_height: f32, radius: f32 },
    Convex,
}

/// A single dynamic body's pose, captured as raw floats for hashing/rendering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyState {
    pub pos: [f32; 3],
    pub quat: [f32; 4],
}

impl BodyState {
    fn is_finite(&self) -> bool {
        self.pos
            .iter()
            .chain(self.quat.iter())
            .all(|v| v.is_finite())
    }
}

/// A self-contained rapier world plus the handles of the bodies we track.
pub struct PhysicsWorld {
    gravity: Vector,
    integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad_phase: BroadPhaseBvh,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd_solver: CCDSolver,
    dynamic: Vec<RigidBodyHandle>,
    kinds: Vec<BodyKind>,
}

/// Deterministic (RNG-free) jittered cube corners, so the convex hull is a fixed
/// arbitrary convex polyhedron rather than a plain box.
fn convex_points() -> Vec<Vector> {
    let j = 0.14;
    vec![
        Vector::new(-0.5, -0.5, -0.5),
        Vector::new(0.5 + j, -0.5, -0.5),
        Vector::new(0.5, 0.5, -0.5 - j),
        Vector::new(-0.5, 0.5 + j, -0.5),
        Vector::new(-0.5 - j, -0.5, 0.5),
        Vector::new(0.5, -0.5 - j, 0.5),
        Vector::new(0.5 + j, 0.5, 0.5),
        Vector::new(-0.5, 0.5, 0.5 + j),
    ]
}

impl PhysicsWorld {
    /// Build the fixed test scene. Identical inputs every call — that fixedness is
    /// what makes the two-world lockstep comparison meaningful.
    pub fn new_scene() -> Self {
        let mut bodies = RigidBodySet::new();
        let mut colliders = ColliderSet::new();

        // Static ground: top face at y = 0.
        let ground = bodies.insert(
            RigidBodyBuilder::fixed()
                .translation(Vector::new(0.0, -1.0, 0.0))
                .build(),
        );
        colliders.insert_with_parent(
            ColliderBuilder::cuboid(20.0, 1.0, 20.0)
                .friction(0.9)
                .build(),
            ground,
            &mut bodies,
        );

        let mut dynamic = Vec::new();
        let mut kinds = Vec::new();

        // Smooth sphere, spun about z so it rolls on landing. rapier has no rolling
        // resistance, so without damping a spun ball rolls forever on the plane;
        // modest damping (deterministic) lets the scene reach a testable rest.
        let h = bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(Vector::new(-1.6, 6.0, 0.0))
                .angvel(Vector::new(0.0, 0.0, 4.0))
                .linear_damping(0.4)
                .angular_damping(0.7)
                .ccd_enabled(true)
                .build(),
        );
        colliders.insert_with_parent(
            ColliderBuilder::ball(0.5)
                .restitution(0.5)
                .friction(0.6)
                .density(1.0)
                .build(),
            h,
            &mut bodies,
        );
        dynamic.push(h);
        kinds.push(BodyKind::Ball { radius: 0.5 });

        // Smooth capsule, tumbling so it must settle onto its side.
        let h = bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(Vector::new(0.0, 7.2, 0.25))
                .angvel(Vector::new(2.5, 0.0, 1.0))
                .linear_damping(0.4)
                .angular_damping(0.7)
                .ccd_enabled(true)
                .build(),
        );
        colliders.insert_with_parent(
            ColliderBuilder::capsule_y(0.5, 0.4)
                .restitution(0.35)
                .friction(0.7)
                .density(1.0)
                .build(),
            h,
            &mut bodies,
        );
        dynamic.push(h);
        kinds.push(BodyKind::Capsule {
            half_height: 0.5,
            radius: 0.4,
        });

        // Arbitrary convex hull, spun on two axes.
        let pts = convex_points();
        let h = bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(Vector::new(1.7, 6.6, -0.15))
                .angvel(Vector::new(1.0, 2.0, 0.5))
                .linear_damping(0.4)
                .angular_damping(0.7)
                .ccd_enabled(true)
                .build(),
        );
        colliders.insert_with_parent(
            ColliderBuilder::convex_hull(&pts)
                .expect("8 non-degenerate points form a valid convex hull")
                .restitution(0.2)
                .friction(0.8)
                .density(1.0)
                .build(),
            h,
            &mut bodies,
        );
        dynamic.push(h);
        kinds.push(BodyKind::Convex);

        Self {
            gravity: Vector::new(0.0, -9.81, 0.0),
            integration_parameters: IntegrationParameters {
                dt: DT,
                ..IntegrationParameters::default()
            },
            physics_pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad_phase: BroadPhaseBvh::new(),
            narrow_phase: NarrowPhase::new(),
            bodies,
            colliders,
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            dynamic,
            kinds,
        }
    }

    /// Advance one fixed lockstep tick. `()` is rapier's no-op hooks/event handler.
    pub fn step(&mut self) {
        self.physics_pipeline.step(
            self.gravity,
            &self.integration_parameters,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd_solver,
            &(),
            &(),
        );
    }

    pub fn kinds(&self) -> &[BodyKind] {
        &self.kinds
    }

    pub fn body_count(&self) -> usize {
        self.dynamic.len()
    }

    /// Pose of every tracked dynamic body, in insertion order.
    pub fn snapshot(&self) -> Vec<BodyState> {
        self.dynamic
            .iter()
            .map(|h| {
                let rb = &self.bodies[*h];
                let t = rb.translation();
                let r = rb.rotation();
                BodyState {
                    pos: [t.x, t.y, t.z],
                    quat: [r.x, r.y, r.z, r.w],
                }
            })
            .collect()
    }

    /// FNV-1a over the raw bits of the whole snapshot. A one-ULP drift anywhere
    /// flips this, so equal hashes across two worlds means bit-identical state.
    pub fn state_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in self.snapshot() {
            for v in b.pos.iter().chain(b.quat.iter()) {
                h ^= v.to_bits() as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        h
    }

    /// Largest linear speed across bodies — used to tell when the scene has come
    /// to rest (a stable settle, not an explosion, is the "handles it" evidence).
    pub fn max_speed(&self) -> f32 {
        self.dynamic
            .iter()
            .map(|h| self.bodies[*h].linvel().length())
            .fold(0.0, f32::max)
    }

    pub fn settled(&self) -> bool {
        self.max_speed() < 0.05
    }

    pub fn all_finite(&self) -> bool {
        self.snapshot().iter().all(BodyState::is_finite)
    }

    /// Run two independent worlds `steps` ticks from the identical start and report
    /// whether every body's pose stayed bit-for-bit equal the whole way. This is
    /// the lockstep guarantee itself.
    pub fn replay_is_bit_identical(steps: u32) -> bool {
        let mut a = Self::new_scene();
        let mut b = Self::new_scene();
        for _ in 0..steps {
            a.step();
            b.step();
            if a.snapshot() != b.snapshot() {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETTLE_STEPS: u32 = 600; // 10 s @ 60 Hz — ample for the drop to rest.

    #[test]
    fn two_worlds_stay_bit_identical_in_lockstep() {
        assert!(
            PhysicsWorld::replay_is_bit_identical(SETTLE_STEPS),
            "rapier3d diverged between two identical worlds — not lockstep-safe"
        );
    }

    #[test]
    fn convex_and_smooth_bodies_settle_without_exploding() {
        let mut world = PhysicsWorld::new_scene();
        for _ in 0..SETTLE_STEPS {
            world.step();
        }
        assert!(
            world.all_finite(),
            "a body produced NaN/inf — solver blew up"
        );
        assert!(
            world.settled(),
            "bodies never came to rest (max speed {:.4})",
            world.max_speed()
        );
        // They must rest *on* the ground (top at y=0), not tunnel through it.
        for (state, kind) in world.snapshot().iter().zip(world.kinds()) {
            assert!(
                state.pos[1] > -0.5,
                "{kind:?} fell through the ground to y={:.3}",
                state.pos[1]
            );
        }
    }

    #[test]
    fn hash_changes_when_state_changes() {
        let mut world = PhysicsWorld::new_scene();
        let start = world.state_hash();
        for _ in 0..30 {
            world.step();
        }
        assert_ne!(start, world.state_hash(), "hash ignored the bodies moving");
    }

    #[test]
    fn scene_has_one_of_each_shape_family() {
        let world = PhysicsWorld::new_scene();
        assert_eq!(world.body_count(), 3);
        assert!(world.kinds().contains(&BodyKind::Ball { radius: 0.5 }));
        assert!(world.kinds().contains(&BodyKind::Convex));
        assert!(matches!(world.kinds()[1], BodyKind::Capsule { .. }));
    }
}
