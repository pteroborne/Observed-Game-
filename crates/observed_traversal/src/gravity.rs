//! Gravity-relative capsule locomotion. The ordinary upright path remains exact.
use crate::{FpsBody, FpsConfig, FpsStep, RapierKinematicSettings, approach, clamp_len};
use glam::{Quat, Vec3};
use player_input::PlayerIntent;
use rapier3d::{
    control::{CharacterAutostep, CharacterLength, KinematicCharacterController},
    prelude::*,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyFrame {
    pub rotation: Quat,
}
impl Default for BodyFrame {
    fn default() -> Self {
        Self {
            rotation: Quat::IDENTITY,
        }
    }
}
impl BodyFrame {
    pub fn is_upright(self) -> bool {
        self.rotation.angle_between(Quat::IDENTITY) <= 0.001 || self.up().distance(Vec3::Y) <= 0.001
    }
    pub fn up(self) -> Vec3 {
        self.rotation * Vec3::Y
    }
    pub fn look(self, body: &FpsBody) -> Vec3 {
        self.rotation * body.look_dir()
    }
    pub fn eye(self, body: &FpsBody, config: &FpsConfig) -> Vec3 {
        body.position + self.up() * (config.eye_height - config.half_height)
    }
    pub fn toward(self, up: Vec3) -> Self {
        Self {
            rotation: (Quat::from_rotation_arc(self.up(), up.normalize_or(Vec3::Y))
                * self.rotation)
                .normalize(),
        }
    }
    pub fn pose(self, position: Vec3) -> Pose {
        Pose::from_parts(
            rv(position),
            Rotation::from_xyzw(
                self.rotation.x,
                self.rotation.y,
                self.rotation.z,
                self.rotation.w,
            ),
        )
    }
}

/// Reorient only where the capsule fits. A short swept clearance move can make
/// room at a corner on expiry; it cannot cross geometry or teleport through a wall.
pub fn reorient(
    query: &QueryPipeline<'_>,
    body: &FpsBody,
    from: BodyFrame,
    to: BodyFrame,
    config: &FpsConfig,
) -> Option<Vec3> {
    let capsule = Capsule::new_y(config.half_height - config.radius, config.radius - 0.005);
    let controller = KinematicCharacterController {
        up: rv(from.up()),
        ..Default::default()
    };
    for distance in [0., 0.1, 0.25, 0.5, 0.75, 1.] {
        for direction in [
            from.up(),
            to.up(),
            (from.up() + to.up()).normalize_or(to.up()),
        ] {
            let movement = controller.move_shape(
                crate::FIXED_DT,
                query,
                &capsule,
                &from.pose(body.position),
                rv(direction * distance),
                |_| {},
            );
            let position = body.position + gv(movement.translation);
            if query
                .intersect_shape(to.pose(position), &capsule)
                .next()
                .is_none()
            {
                return Some(position);
            }
        }
    }
    None
}

/// Same acceleration, jump and grounding contract as the upright controller,
/// expressed in a body frame. Bounds are always world-space bounds.
pub fn step(
    query: &QueryPipeline<'_>,
    body: &mut FpsBody,
    frame: BodyFrame,
    intent: PlayerIntent,
    config: &FpsConfig,
    bounds: (Vec3, Vec3),
) -> FpsStep {
    let dt = crate::FIXED_DT;
    let settings = RapierKinematicSettings::shipped(config);
    if frame.is_upright() {
        return crate::rapier_controller::step_character_in_query(
            query, body, intent, config, settings, dt, bounds,
        );
    }
    let mut intent = intent;
    intent.movement = intent.movement.clamp_length_max(1.);
    let mut report = FpsStep::default();
    body.yaw = (body.yaw + intent.look.x * config.look_step).rem_euclid(std::f32::consts::TAU);
    body.pitch = (body.pitch - intent.look.y * config.look_step)
        .clamp(-config.pitch_limit, config.pitch_limit);
    let wish = clamp_len(body.right() * intent.movement.x + body.forward() * intent.movement.y);
    let target = wish
        * if intent.sprint_held {
            config.run_speed
        } else {
            config.walk_speed
        };
    let accel = if body.grounded {
        if wish.length_squared() > 1e-4 {
            config.ground_accel
        } else {
            config.ground_decel
        }
    } else {
        config.air_accel
    };
    let mut velocity = frame.rotation.inverse() * body.velocity;
    velocity.x = approach(velocity.x, target.x, accel * dt);
    velocity.z = approach(velocity.z, target.z, accel * dt);
    body.jump_cd = (body.jump_cd - dt).max(0.);
    if body.grounded && intent.jump_pressed && body.jump_cd <= 0. {
        velocity.y = config.jump_speed;
        body.grounded = false;
        body.jump_cd = config.jump_cooldown;
        report.jumped = true;
    }
    if !body.grounded {
        velocity.y = (velocity.y - config.gravity * dt).max(-config.max_fall);
    }
    body.velocity = frame.rotation * velocity;
    let was_grounded = body.grounded;
    let capsule = Capsule::new_y(config.half_height - config.radius, config.radius);
    let controller = KinematicCharacterController {
        up: rv(frame.up()),
        offset: CharacterLength::Absolute(settings.controller_offset),
        autostep: Some(CharacterAutostep {
            max_height: CharacterLength::Absolute(config.step_height),
            min_width: CharacterLength::Absolute(settings.minimum_step_width),
            include_dynamic_bodies: true,
        }),
        snap_to_ground: (!report.jumped).then_some(CharacterLength::Absolute(settings.ground_snap)),
        max_slope_climb_angle: settings.maximum_slope_degrees.to_radians(),
        min_slope_slide_angle: settings.minimum_slope_slide_degrees.to_radians(),
        ..Default::default()
    };
    let movement = controller.move_shape(
        dt,
        query,
        &capsule,
        &frame.pose(body.position),
        rv(body.velocity * dt),
        |_| {},
    );
    body.position += gv(movement.translation);
    body.grounded = movement.grounded;
    if body.grounded && velocity.y <= 0. {
        report.landed = !was_grounded;
        velocity.y = 0.;
        body.velocity = frame.rotation * velocity;
    }
    if (body.position - bounds.0).abs().cmpgt(bounds.1).any() {
        body.reset();
        report.recovered = true;
    }
    report
}

/// Fixed-tick gravity lifecycle shared by the proof room and the solved floor.
/// Camera easing reads this state; it never feeds back into locomotion.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ObserverGravity {
    pub frame: BodyFrame,
    pub previous: BodyFrame,
    pub remaining: u32,
    pub transition: u32,
    pub returning: bool,
}
impl ObserverGravity {
    pub const SETTLE_TICKS: u32 = 18;
    pub fn activate(
        &mut self,
        query: &QueryPipeline<'_>,
        body: &mut FpsBody,
        config: &FpsConfig,
        down: Vec3,
        ticks: u32,
    ) -> bool {
        let next = self.frame.toward(-down);
        let Some(position) = reorient(query, body, self.frame, next, config) else {
            return false;
        };
        self.previous = self.frame;
        self.frame = next;
        self.transition = Self::SETTLE_TICKS;
        self.remaining = ticks;
        self.returning = false;
        body.position = position;
        body.grounded = false;
        true
    }
    pub fn release(&mut self) {
        self.remaining = 0;
        self.returning = !self.frame.is_upright();
    }
    pub fn visual_frame(self) -> BodyFrame {
        let t = 1. - self.transition as f32 / Self::SETTLE_TICKS as f32;
        BodyFrame {
            rotation: self
                .previous
                .rotation
                .slerp(self.frame.rotation, t * t * (3. - 2. * t)),
        }
    }
    pub fn step(
        &mut self,
        query: &QueryPipeline<'_>,
        body: &mut FpsBody,
        intent: PlayerIntent,
        config: &FpsConfig,
        bounds: (Vec3, Vec3),
    ) -> FpsStep {
        self.transition = self.transition.saturating_sub(1);
        if self.remaining > 0 {
            self.remaining -= 1;
            if self.remaining == 0 {
                self.release();
            }
        }
        if self.returning {
            let next = BodyFrame::default();
            if let Some(position) = reorient(query, body, self.frame, next, config) {
                body.position = position;
                body.grounded = false;
                self.previous = self.frame;
                self.frame = next;
                self.transition = Self::SETTLE_TICKS;
                self.returning = false;
            }
        }
        let report = step(query, body, self.frame, intent, config, bounds);
        if report.recovered {
            *self = Self::default();
        }
        report
    }
}

fn rv(v: Vec3) -> Vector {
    Vector::new(v.x, v.y, v.z)
}
fn gv(v: Vector) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    struct TestPhysics {
        broad: BroadPhaseBvh,
        narrow: NarrowPhase,
        bodies: RigidBodySet,
        colliders: ColliderSet,
    }

    impl TestPhysics {
        fn new() -> Self {
            Self {
                broad: BroadPhaseBvh::new(),
                narrow: NarrowPhase::new(),
                bodies: RigidBodySet::new(),
                colliders: ColliderSet::new(),
            }
        }

        fn update(&mut self) {
            let mut islands = IslandManager::new();
            let mut joints = ImpulseJointSet::new();
            let mut multibody = MultibodyJointSet::new();
            let mut ccd = CCDSolver::new();
            PhysicsPipeline::new().step(
                Vector::ZERO,
                &IntegrationParameters::default(),
                &mut islands,
                &mut self.broad,
                &mut self.narrow,
                &mut self.bodies,
                &mut self.colliders,
                &mut joints,
                &mut multibody,
                &mut ccd,
                &(),
                &(),
            );
        }

        fn query(&self) -> QueryPipeline<'_> {
            self.broad.as_query_pipeline(
                self.narrow.query_dispatcher(),
                &self.bodies,
                &self.colliders,
                QueryFilter::default(),
            )
        }
    }

    #[test]
    fn body_frame_upright_tolerance_and_fast_path() {
        let frame = BodyFrame::default();
        assert!(frame.is_upright());

        // Slight rotation within tolerance
        let near = BodyFrame {
            rotation: Quat::from_rotation_y(0.0005),
        };
        assert!(near.is_upright());

        // Significant rotation (e.g. wall walk)
        let wall = BodyFrame::default().toward(Vec3::X);
        assert!(!wall.is_upright());

        // Rotating back towards Y
        let returned = wall.toward(Vec3::Y);
        // Defect (a) test: toward(Vec3::Y) with float normalization is upright within tolerance
        assert!(returned.is_upright());
    }

    #[test]
    fn observer_gravity_lifecycle_activate_release_return() {
        let mut physics = TestPhysics::new();
        // Flat floor at y = 0
        physics.colliders.insert(
            ColliderBuilder::cuboid(10.0, 0.1, 10.0).translation(rv(Vec3::new(0.0, -0.1, 0.0))),
        );
        physics.update();
        let query = physics.query();

        let config = FpsConfig::default();
        let mut body = FpsBody::spawned(Vec3::new(0.0, 1.0, 0.0), 0.0);
        let mut gravity = ObserverGravity::default();

        // Initially upright
        assert!(gravity.frame.is_upright());
        assert_eq!(gravity.remaining, 0);
        assert!(!gravity.returning);

        // Activate wall walk (down = -Vec3::X, so up = Vec3::X)
        let activated = gravity.activate(&query, &mut body, &config, -Vec3::X, 100);
        assert!(activated);
        assert!(!gravity.frame.is_upright());
        assert_eq!(gravity.remaining, 100);
        assert_eq!(gravity.transition, ObserverGravity::SETTLE_TICKS);

        // Step 40 ticks: remaining should decrease, transition should reach 0
        let intent = PlayerIntent::default();
        let bounds = (Vec3::ZERO, Vec3::splat(100.0));
        for _ in 0..40 {
            gravity.step(&query, &mut body, intent, &config, bounds);
        }
        assert_eq!(gravity.remaining, 60);
        assert_eq!(gravity.transition, 0);

        // Step remaining 60 ticks until expiration
        for _ in 0..60 {
            gravity.step(&query, &mut body, intent, &config, bounds);
        }
        // Step a few ticks to complete return reorient
        for _ in 0..5 {
            gravity.step(&query, &mut body, intent, &config, bounds);
        }
        assert!(gravity.frame.is_upright());
        assert_eq!(gravity.frame, BodyFrame::default());
        assert_eq!(gravity.remaining, 0);
        assert!(!gravity.returning);
    }

    #[test]
    fn reorient_refuses_when_no_clearance_and_never_crosses_geometry() {
        let mut physics = TestPhysics::new();
        // A tight enclosed box where the upright capsule fits, but horizontal capsule does not
        physics.colliders.insert(
            ColliderBuilder::cuboid(10.0, 0.1, 10.0).translation(rv(Vec3::new(0.0, -0.1, 0.0))),
        ); // floor
        physics.colliders.insert(
            ColliderBuilder::cuboid(10.0, 0.1, 10.0).translation(rv(Vec3::new(0.0, 2.0, 0.0))),
        ); // ceiling
        physics.colliders.insert(
            ColliderBuilder::cuboid(0.1, 10.0, 10.0).translation(rv(Vec3::new(0.6, 0.0, 0.0))),
        ); // wall +X
        physics.colliders.insert(
            ColliderBuilder::cuboid(0.1, 10.0, 10.0).translation(rv(Vec3::new(-0.6, 0.0, 0.0))),
        ); // wall -X
        physics.update();
        let query = physics.query();

        let config = FpsConfig::default();
        let body = FpsBody::spawned(Vec3::new(0.0, 0.9, 0.0), 0.0);
        let from = BodyFrame::default();
        let to = from.toward(Vec3::X);

        // Reorient should refuse because there is not enough clearance for horizontal capsule
        let result = reorient(&query, &body, from, to, &config);
        assert!(
            result.is_none(),
            "reorient must refuse when space is too tight for capsule"
        );

        // Test that reorient never crosses / tunnels through geometry
        let mut tunnel_physics = TestPhysics::new();
        tunnel_physics.colliders.insert(
            ColliderBuilder::cuboid(0.5, 5.0, 5.0).translation(rv(Vec3::new(1.5, 0.0, 0.0))),
        ); // wall from x=1.0 to x=2.0
        tunnel_physics.update();
        let tunnel_query = tunnel_physics.query();

        let body_near_wall = FpsBody::spawned(Vec3::new(0.7, 0.9, 0.0), 0.0);
        if let Some(pos) = reorient(&tunnel_query, &body_near_wall, from, to, &config) {
            assert!(
                pos.x < 1.0,
                "reorient must not cross geometry: landed at x={}",
                pos.x
            );
        }
    }
}
