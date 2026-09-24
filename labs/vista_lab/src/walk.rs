//! Walking the vista with the production controller.
//!
//! The walkways are only walkways if a body can cross them, so the lab proves it the
//! way the game would: the shared Rapier character controller, stepping the same
//! `PlayerIntent`, colliding with exactly the pieces the renderer draws. The capture
//! walk and the headless tests drive the same [`Walker`], so a recording is never of
//! a route the tests did not also cross.
use bevy::math::{Vec2, Vec3};
use observed_traversal::rapier_controller::{RapierTraversalScene, step_character};
use observed_traversal::{ArenaSpec, FpsBody, FpsConfig};
use player_input::PlayerIntent;

use crate::composition::{CONFIG, Vista};
use crate::geometry::{Build, floor_point, origin, yaw_toward};

/// One simulation step, seconds.
pub const STEP: f32 = 1.0 / 60.0;
/// How close, in plan, counts as having reached a waypoint.
const ARRIVED: f32 = 1.2;

/// The collision world for a built vista.
///
/// The safety volume is the lattice plus forty metres of drop below it: a body that
/// falls further than that has fallen into true void, and the controller's recovery
/// puts it back at its spawn.
#[must_use]
pub fn scene(build: &Build) -> RapierTraversalScene {
    let far = origin(observed_hex::HexCoord {
        q: CONFIG.cols - 1,
        r: CONFIG.rows - 1,
        level: CONFIG.levels - 1,
    });
    let center = Vec3::new(far.x * 0.5, far.y * 0.5, far.z * 0.5);
    let half = Vec3::new(far.x * 0.5 + 60.0, far.y * 0.5 + 40.0, far.z * 0.5 + 60.0);
    RapierTraversalScene::from_arena_spec(&ArenaSpec {
        colliders: build.collider_specs(),
        floor_y: -1_000.0,
        safety_center: center,
        safety_half: half,
    })
}

/// A body steering itself through waypoints.
pub struct Walker {
    pub scene: RapierTraversalScene,
    pub body: FpsBody,
    pub config: FpsConfig,
    pub waypoints: Vec<Vec3>,
    pub next: usize,
    pub elapsed: f32,
    /// The lowest the body's feet have been.
    pub lowest_feet: f32,
    /// The controller had to rescue the body from below the world.
    pub recovered: bool,
}

impl Walker {
    /// A body standing at `feet`, facing along `facing`.
    #[must_use]
    pub fn standing(build: &Build, feet: Vec3, facing: Vec2) -> Self {
        let config = FpsConfig::default();
        let body = FpsBody::spawned(feet + Vec3::Y * config.half_height, yaw_toward(facing));
        Self {
            scene: scene(build),
            lowest_feet: feet.y,
            body,
            config,
            waypoints: Vec::new(),
            next: 0,
            elapsed: 0.0,
            recovered: false,
        }
    }

    /// A body at the start of the vista's tour, with the tour as its waypoints.
    #[must_use]
    pub fn touring(vista: &Vista, build: &Build) -> Self {
        let waypoints: Vec<Vec3> = vista.tour.iter().map(|&at| floor_point(at)).collect();
        let facing = (waypoints[1] - waypoints[0]).normalize_or_zero();
        let mut walker = Self::standing(build, waypoints[0], Vec2::new(facing.x, facing.z));
        walker.waypoints = waypoints;
        walker.next = 1;
        walker
    }

    #[must_use]
    pub fn feet(&self) -> Vec3 {
        self.body.position - Vec3::Y * self.config.half_height
    }

    #[must_use]
    pub fn finished(&self) -> bool {
        self.next >= self.waypoints.len()
    }

    /// Step the controller once with an explicit intent.
    pub fn step_with(&mut self, intent: PlayerIntent) {
        let outcome = step_character(&self.scene, &mut self.body, intent, &self.config, STEP);
        self.recovered |= outcome.recovered;
        self.elapsed += STEP;
        self.lowest_feet = self.lowest_feet.min(self.feet().y);
    }

    /// One step toward the next waypoint: turn to face it, walk forward.
    ///
    /// The turn is capped per step, so the body walks a curve through a corner as a
    /// person would rather than pivoting on the spot; that is also what makes a
    /// recorded walk watchable.
    pub fn step_tour(&mut self) {
        while let Some(&target) = self.waypoints.get(self.next) {
            let to = Vec2::new(
                target.x - self.body.position.x,
                target.z - self.body.position.z,
            );
            if to.length() < ARRIVED {
                self.next += 1;
                continue;
            }
            let error = (yaw_toward(to) - self.body.yaw + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            self.body.yaw += error.clamp(-0.06, 0.06);
            let movement = if error.abs() > 0.9 {
                Vec2::ZERO
            } else {
                Vec2::new(0.0, 1.0)
            };
            self.step_with(PlayerIntent {
                movement,
                ..PlayerIntent::default()
            });
            return;
        }
        self.step_with(PlayerIntent::default());
    }
}
