//! Caller-stepped raw Rapier. All durable solver state is included in snapshots.
use observed_traversal::FIXED_DT;
use rapier3d::prelude::*;

#[derive(Clone)]
pub struct Physics {
    pub islands: IslandManager,
    pub broad: BroadPhaseBvh,
    pub narrow: NarrowPhase,
    pub bodies: RigidBodySet,
    pub colliders: ColliderSet,
    pub joints: ImpulseJointSet,
    pub multibody: MultibodyJointSet,
    pub ccd: CCDSolver,
}
impl Default for Physics {
    fn default() -> Self {
        Self {
            islands: IslandManager::new(),
            broad: BroadPhaseBvh::new(),
            narrow: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            joints: ImpulseJointSet::new(),
            multibody: MultibodyJointSet::new(),
            ccd: CCDSolver::new(),
        }
    }
}
impl Physics {
    pub fn step(&mut self) {
        // PhysicsPipeline holds disposable scratch buffers, not warm-start state.
        // Keeping it outside snapshots cannot alter continuation (tested mid-flight).
        PhysicsPipeline::new().step(
            Vector::new(0., -20., 0.),
            &IntegrationParameters {
                dt: FIXED_DT,
                ..Default::default()
            },
            &mut self.islands,
            &mut self.broad,
            &mut self.narrow,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.joints,
            &mut self.multibody,
            &mut self.ccd,
            &(),
            &(),
        );
    }
    pub fn walk_minor(&self, body: RigidBodyHandle, desired: glam::Vec3) -> glam::Vec3 {
        use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};
        let controller = KinematicCharacterController {
            autostep: Some(CharacterAutostep {
                max_height: CharacterLength::Absolute(0.4),
                min_width: CharacterLength::Absolute(0.15),
                include_dynamic_bodies: false,
            }),
            snap_to_ground: Some(CharacterLength::Absolute(0.12)),
            ..Default::default()
        };
        let pose = *self.bodies[body].position();
        gv(controller
            .move_shape(
                FIXED_DT,
                &self.query(body),
                &Cuboid::new(Vector::splat(0.55)),
                &pose,
                rv(desired * FIXED_DT),
                |_| {},
            )
            .translation)
            / FIXED_DT
    }
    pub fn query(&self, exclude: RigidBodyHandle) -> QueryPipeline<'_> {
        self.broad.as_query_pipeline(
            self.narrow.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            QueryFilter::default().exclude_rigid_body(exclude),
        )
    }
    /// Stable-ID ordered ray query also works immediately after spawn/reset,
    /// before the broad phase has been stepped. Equal distances prefer lower IDs.
    pub fn ray(
        &self,
        origin: glam::Vec3,
        direction: glam::Vec3,
        reach: f32,
        exclude: Option<ColliderHandle>,
    ) -> Option<(ColliderHandle, f32)> {
        let ray = Ray::new(rv(origin), rv(direction));
        self.colliders
            .iter()
            .filter(|(h, c)| Some(*h) != exclude && c.is_enabled())
            .filter_map(|(h, c)| {
                c.shape()
                    .cast_ray(c.position(), &ray, reach, true)
                    .map(|t| (h, t, c.user_data))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.2.cmp(&b.2)))
            .map(|(h, t, _)| (h, t))
    }
}

pub fn rv(v: glam::Vec3) -> Vector {
    Vector::new(v.x, v.y, v.z)
}
pub fn gv(v: Vector) -> glam::Vec3 {
    glam::Vec3::new(v.x, v.y, v.z)
}
