//! Gravity-relative capsule locomotion. The ordinary upright path remains exact.
use glam::{Quat, Vec3};
use player_input::PlayerIntent;
use rapier3d::{control::{CharacterAutostep, CharacterLength, KinematicCharacterController}, prelude::*};
use crate::{FpsBody, FpsConfig, FpsStep, RapierKinematicSettings, approach, clamp_len};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyFrame { pub rotation: Quat }
impl Default for BodyFrame { fn default() -> Self { Self { rotation: Quat::IDENTITY } } }
impl BodyFrame {
    pub fn up(self) -> Vec3 { self.rotation * Vec3::Y }
    pub fn look(self, body: &FpsBody) -> Vec3 { self.rotation * body.look_dir() }
    pub fn eye(self, body: &FpsBody, config: &FpsConfig) -> Vec3 {
        body.position + self.up() * (config.eye_height - config.half_height)
    }
    pub fn toward(self, up: Vec3) -> Self {
        Self { rotation: (Quat::from_rotation_arc(self.up(), up.normalize_or(Vec3::Y)) * self.rotation).normalize() }
    }
    pub fn pose(self, position: Vec3) -> Pose {
        Pose::from_parts(rv(position), Rotation::from_xyzw(self.rotation.x, self.rotation.y, self.rotation.z, self.rotation.w))
    }
}

/// Reorient only where the capsule fits. A short swept clearance move can make
/// room at a corner on expiry; it cannot cross geometry or teleport through a wall.
pub fn reorient(query: &QueryPipeline<'_>, body: &FpsBody, from: BodyFrame, to: BodyFrame, config: &FpsConfig) -> Option<Vec3> {
    let capsule = Capsule::new_y(config.half_height - config.radius, config.radius - 0.005);
    let controller = KinematicCharacterController { up: rv(from.up()), ..Default::default() };
    for distance in [0., 0.1, 0.25, 0.5, 0.75, 1.] {
        for direction in [from.up(), to.up(), (from.up() + to.up()).normalize_or(to.up())] {
            let movement = controller.move_shape(crate::FIXED_DT, query, &capsule, &from.pose(body.position), rv(direction * distance), |_| {});
            let position = body.position + gv(movement.translation);
            if query.intersect_shape(to.pose(position), &capsule).next().is_none() {
                return Some(position);
            }
        }
    }
    None
}

/// Same acceleration, jump and grounding contract as the upright controller,
/// expressed in a body frame. Bounds are always world-space bounds.
pub fn step(query: &QueryPipeline<'_>, body: &mut FpsBody, frame: BodyFrame, intent: PlayerIntent, config: &FpsConfig, bounds: (Vec3, Vec3)) -> FpsStep {
    let dt = crate::FIXED_DT;
    let settings = RapierKinematicSettings::shipped(config);
    if frame == BodyFrame::default() {
        return crate::rapier_controller::step_character_in_query(query, body, intent, config, settings, dt, bounds);
    }
    let mut intent = intent;
    intent.movement = intent.movement.clamp_length_max(1.);
    let mut report = FpsStep::default();
    body.yaw = (body.yaw + intent.look.x * config.look_step).rem_euclid(std::f32::consts::TAU);
    body.pitch = (body.pitch - intent.look.y * config.look_step).clamp(-config.pitch_limit, config.pitch_limit);
    let wish = clamp_len(body.right() * intent.movement.x + body.forward() * intent.movement.y);
    let target = wish * if intent.sprint_held { config.run_speed } else { config.walk_speed };
    let accel = if body.grounded { if wish.length_squared() > 1e-4 { config.ground_accel } else { config.ground_decel } } else { config.air_accel };
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
    if !body.grounded { velocity.y = (velocity.y - config.gravity * dt).max(-config.max_fall); }
    body.velocity = frame.rotation * velocity;
    let was_grounded = body.grounded;
    let capsule = Capsule::new_y(config.half_height - config.radius, config.radius);
    let controller = KinematicCharacterController {
        up: rv(frame.up()),
        offset: CharacterLength::Absolute(settings.controller_offset),
        autostep: Some(CharacterAutostep { max_height: CharacterLength::Absolute(config.step_height), min_width: CharacterLength::Absolute(settings.minimum_step_width), include_dynamic_bodies: true }),
        snap_to_ground: (!report.jumped).then_some(CharacterLength::Absolute(settings.ground_snap)),
        max_slope_climb_angle: settings.maximum_slope_degrees.to_radians(),
        min_slope_slide_angle: settings.minimum_slope_slide_degrees.to_radians(),
        ..Default::default()
    };
    let movement = controller.move_shape(dt, query, &capsule, &frame.pose(body.position), rv(body.velocity * dt), |_| {});
    body.position += gv(movement.translation);
    body.grounded = movement.grounded;
    if body.grounded && velocity.y <= 0. {
        report.landed = !was_grounded;
        velocity.y = 0.;
        body.velocity = frame.rotation * velocity;
    }
    if (body.position - bounds.0).abs().cmpgt(bounds.1).any() {
        body.reset(); report.recovered = true;
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
    pub fn activate(&mut self, query: &QueryPipeline<'_>, body: &mut FpsBody, config: &FpsConfig, down: Vec3, ticks: u32) -> bool {
        let next = self.frame.toward(-down);
        let Some(position) = reorient(query, body, self.frame, next, config) else { return false; };
        self.previous = self.frame;
        self.frame = next;
        self.transition = Self::SETTLE_TICKS;
        self.remaining = ticks;
        self.returning = false;
        body.position = position;
        body.grounded = false;
        true
    }
    pub fn release(&mut self) { self.remaining = 0; self.returning = self.frame.up().distance(Vec3::Y) > 0.001; }
    pub fn visual_frame(self) -> BodyFrame {
        let t = 1. - self.transition as f32 / Self::SETTLE_TICKS as f32;
        BodyFrame { rotation: self.previous.rotation.slerp(self.frame.rotation, t * t * (3. - 2. * t)) }
    }
    pub fn step(&mut self, query: &QueryPipeline<'_>, body: &mut FpsBody, intent: PlayerIntent, config: &FpsConfig, bounds: (Vec3, Vec3)) -> FpsStep {
        self.transition = self.transition.saturating_sub(1);
        if self.remaining > 0 {
            self.remaining -= 1;
            if self.remaining == 0 { self.release(); }
        }
        if self.returning {
            let next = self.frame.toward(Vec3::Y);
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
        if report.recovered { *self = Self::default(); }
        report
    }
}

fn rv(v: Vec3) -> Vector { Vector::new(v.x,v.y,v.z) }
fn gv(v: Vector) -> Vec3 { Vec3::new(v.x,v.y,v.z) }
