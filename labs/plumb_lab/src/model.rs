//! Fixed-tick simulation for one subject under a redirected gravity.
//!
//! The whole lab is this question: if you tell one body that "down" is a
//! different direction, does it fall to that surface, stand on it, and walk
//! along it? Everything here exists to answer that and report what happened.

use glam::{Quat, Vec3};
use kinetic_lab::physics::{Physics, gv, rv};
use observed_traversal::FIXED_DT;
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};
use rapier3d::prelude::*;

use crate::room::{self, Solid, Surface};

/// World gravity magnitude, matching the kinetic labs.
pub const GRAVITY: f32 = 20.0;
/// How fast the subject tries to walk along whatever it is standing on.
pub const WALK_SPEED: f32 = 3.0;
/// Half-extent of the subject's collision cuboid.
pub const SUBJECT_HALF: f32 = 0.55;

/// A redirected gravity applied to one body for a while.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plumb {
    /// Which way "down" is for the subject. Unit length.
    pub direction: Vec3,
    /// Acceleration along that direction, in m/s^2.
    pub strength: f32,
    /// Ticks remaining before it wears off.
    pub ticks: u32,
}

impl Plumb {
    #[must_use]
    pub fn new(direction: Vec3, strength: f32, ticks: u32) -> Self {
        Self {
            direction: direction.normalize_or(Vec3::NEG_Y),
            strength,
            ticks,
        }
    }

    /// Which way is up for a body under this plumb.
    #[must_use]
    pub fn up(&self) -> Vec3 {
        -self.direction
    }
}

/// What the subject is doing, as the report cares about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Footing {
    /// In the air, under whatever gravity currently applies.
    Falling,
    /// Resting on a surface whose normal agrees with the current up.
    Planted,
}

#[derive(Clone)]
pub struct PlumbWorld {
    pub tick: u64,
    pub physics: Physics,
    pub solids: Vec<Solid>,
    pub subject: RigidBodyHandle,
    pub subject_collider: ColliderHandle,
    /// The live plumb, or `None` for ordinary world gravity.
    pub plumb: Option<Plumb>,
    pub footing: Footing,
    /// The surface normal the subject is resting on, if it is.
    pub contact_normal: Option<Vec3>,
    /// Which room surface that normal belongs to.
    pub contact_surface: Option<Surface>,
    /// Tangential direction the subject is trying to walk, in world space.
    pub heading: Vec3,
    /// Distance travelled along the surface while planted, this phase.
    pub walked: f32,
    structural: Vec<(u32, ColliderHandle, Surface)>,
    previous: Vec3,
}

impl PlumbWorld {
    #[must_use]
    pub fn new() -> Self {
        let mut physics = Physics::default();
        let solids = room::solids();
        let mut structural = Vec::new();
        for solid in &solids {
            let handle = physics.colliders.insert(
                ColliderBuilder::cuboid(solid.half.x, solid.half.y, solid.half.z)
                    .translation(rv(solid.center))
                    .friction(0.7)
                    .user_data(u128::from(solid.id))
                    .build(),
            );
            structural.push((solid.id, handle, solid.surface));
        }
        // Rotations stay locked. A body that tumbles tells you nothing about
        // whether it is standing on a wall, and the rig's orientation is driven
        // from the plumb rather than from the physics.
        let subject = physics.bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(rv(room::spawn()))
                .ccd_enabled(true)
                .lock_rotations()
                .linear_damping(0.1)
                .build(),
        );
        let subject_collider = physics.colliders.insert_with_parent(
            ColliderBuilder::cuboid(SUBJECT_HALF, SUBJECT_HALF, SUBJECT_HALF)
                .density(1.)
                .friction(0.6)
                .restitution(0.0)
                .user_data(u128::from(u32::MAX))
                .build(),
            subject,
            &mut physics.bodies,
        );
        physics.bodies[subject].recompute_mass_properties_from_colliders(&physics.colliders);
        physics
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut physics.colliders);
        let previous = room::spawn();
        let mut world = Self {
            tick: 0,
            physics,
            solids,
            subject,
            subject_collider,
            plumb: None,
            footing: Footing::Falling,
            contact_normal: None,
            contact_surface: None,
            heading: Vec3::X,
            walked: 0.,
            structural,
            previous,
        };
        world.physics.step();
        world
    }

    /// Where "down" currently points for the subject.
    #[must_use]
    pub fn down(&self) -> Vec3 {
        self.plumb.map_or(Vec3::NEG_Y, |plumb| plumb.direction)
    }

    /// Where "up" currently points for the subject.
    #[must_use]
    pub fn up(&self) -> Vec3 {
        -self.down()
    }

    #[must_use]
    pub fn position(&self) -> Vec3 {
        gv(self.physics.bodies[self.subject].translation())
    }

    #[must_use]
    pub fn velocity(&self) -> Vec3 {
        gv(self.physics.bodies[self.subject].linvel())
    }

    /// Apply a plumb to the subject, replacing any live one.
    ///
    /// The body's own gravity scale goes to zero and the acceleration is
    /// applied as a force instead. Rapier keeps a user force until it is reset,
    /// so this is set once per plumb rather than re-applied every tick.
    pub fn apply(&mut self, plumb: Plumb) {
        self.plumb = Some(plumb);
        self.walked = 0.;
        let mass = self.physics.bodies[self.subject].mass();
        let body = &mut self.physics.bodies[self.subject];
        body.set_gravity_scale(0., true);
        body.reset_forces(true);
        body.add_force(rv(plumb.direction * plumb.strength * mass), true);
        self.heading = tangent(plumb.up(), self.heading);
    }

    /// Drop any plumb and return the subject to the world's own gravity.
    pub fn release(&mut self) {
        self.plumb = None;
        self.walked = 0.;
        let body = &mut self.physics.bodies[self.subject];
        body.reset_forces(true);
        body.set_gravity_scale(1., true);
        self.heading = tangent(Vec3::Y, self.heading);
    }

    /// Advance one fixed tick.
    pub fn step(&mut self, walking: bool) {
        self.tick += 1;
        if let Some(plumb) = &mut self.plumb {
            if plumb.ticks <= 1 {
                self.release();
            } else {
                plumb.ticks -= 1;
            }
        }
        self.previous = self.position();
        let up = self.up();
        self.resolve_footing(up);
        if walking && self.footing == Footing::Planted {
            self.walk(up);
        }
        self.physics.step();
        if self.footing == Footing::Planted {
            // Distance along the surface only: falling does not count as
            // walking, and neither does being dragged into it.
            let moved = self.position() - self.previous;
            self.walked += (moved - up * moved.dot(up)).length();
        }
    }

    /// Whether the subject is resting on something that counts as ground for
    /// the current up, and on which room surface.
    ///
    /// This is the Y-hardcoded grounded test from the kinetic labs, generalised:
    /// there the normal was compared against `Vec3::Y`, which is exactly the
    /// assumption a plumb breaks.
    fn resolve_footing(&mut self, up: Vec3) {
        let collider = self.subject_collider;
        let mut best: Option<(f32, Vec3, Surface)> = None;
        for pair in self.physics.narrow.contact_pairs_with(collider) {
            let sign = if pair.collider1 == collider { -1. } else { 1. };
            let other = if pair.collider1 == collider {
                pair.collider2
            } else {
                pair.collider1
            };
            let surface = self
                .structural
                .iter()
                .find(|(_, handle, _)| *handle == other)
                .map(|(_, _, surface)| *surface);
            for manifold in &pair.manifolds {
                if manifold.data.solver_contacts.is_empty() {
                    continue;
                }
                let normal = gv(manifold.data.normal) * sign;
                let agreement = normal.dot(up);
                if best.is_none_or(|(previous, _, _)| agreement > previous) {
                    best = Some((agreement, normal, surface.unwrap_or(Surface::Obstacle)));
                }
            }
        }
        match best {
            Some((agreement, normal, surface)) if agreement > 0.5 => {
                self.footing = Footing::Planted;
                self.contact_normal = Some(normal);
                self.contact_surface = Some(surface);
            }
            _ => {
                self.footing = Footing::Falling;
                self.contact_normal = None;
                self.contact_surface = None;
            }
        }
    }

    /// Walk along the current surface.
    ///
    /// Rapier's character controller takes `up` as a vector rather than
    /// assuming `Y`, which is the single fact this lab turns on: the same
    /// controller that walks a floor walks a wall if you tell it which way is
    /// up. Movement is resolved through it and applied as a tangential
    /// velocity, leaving the normal component to the plumb.
    fn walk(&mut self, up: Vec3) {
        let controller = KinematicCharacterController {
            up: rv(up),
            autostep: Some(CharacterAutostep {
                max_height: CharacterLength::Absolute(0.45),
                min_width: CharacterLength::Absolute(0.2),
                include_dynamic_bodies: false,
            }),
            snap_to_ground: Some(CharacterLength::Absolute(0.25)),
            ..Default::default()
        };
        let pose = *self.physics.bodies[self.subject].position();
        let desired = self.heading * WALK_SPEED;
        let movement = controller.move_shape(
            FIXED_DT,
            &self.physics.query(self.subject),
            &Cuboid::new(Vector::splat(SUBJECT_HALF)),
            &pose,
            rv(desired * FIXED_DT),
            |_| {},
        );
        let resolved = gv(movement.translation) / FIXED_DT;
        let body = &mut self.physics.bodies[self.subject];
        let current = gv(body.linvel());
        // Keep whatever the plumb is doing along the normal; replace the
        // tangential part with the walk the controller says is possible.
        let normal_part = up * current.dot(up);
        let tangential = resolved - up * resolved.dot(up);
        body.set_linvel(rv(normal_part + tangential), true);
    }

    /// Turn the subject's heading around, keeping it tangential.
    pub fn reverse(&mut self) {
        self.heading = tangent(self.up(), -self.heading);
    }

    /// The orientation a rig should take to stand on the current surface.
    #[must_use]
    pub fn attitude(&self) -> Quat {
        Quat::from_rotation_arc(Vec3::Y, self.up())
    }

    /// Replay fingerprint.
    #[must_use]
    pub fn digest(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut add = |value: u64| {
            hash ^= value;
            hash = hash.wrapping_mul(0x100_0000_01b3);
        };
        add(self.tick);
        add(self.footing as u64);
        add(self.plumb.map_or(0, |plumb| u64::from(plumb.ticks)));
        for value in [
            self.position().x,
            self.position().y,
            self.position().z,
            self.velocity().x,
            self.velocity().y,
            self.velocity().z,
            self.down().x,
            self.down().y,
            self.down().z,
            self.walked,
        ] {
            add(u64::from(value.to_bits()));
        }
        hash
    }
}

impl Default for PlumbWorld {
    fn default() -> Self {
        Self::new()
    }
}

/// A unit vector perpendicular to `up`, as close to `wanted` as possible.
///
/// A heading has to lie in the surface, and "the direction I was walking" stops
/// being tangential the moment the surface changes.
#[must_use]
pub fn tangent(up: Vec3, wanted: Vec3) -> Vec3 {
    let up = up.normalize_or(Vec3::Y);
    let projected = wanted - up * wanted.dot(up);
    if projected.length_squared() > 1.0e-4 {
        return projected.normalize();
    }
    // `wanted` was parallel to up; any tangent will do, chosen deterministically.
    let reference = if up.dot(Vec3::Y).abs() > 0.9 {
        Vec3::X
    } else {
        Vec3::Y
    };
    up.cross(reference).normalize()
}

#[cfg(test)]
mod tests;
