//! Rapier collision consumer for whichever isolated Place is currently occupied.

use player_input::PlayerIntent;
use rapier3d::{
    control::{CharacterAutostep, CharacterLength, KinematicCharacterController},
    prelude::*,
};

use crate::{
    authoring::{AuthoredModule, ConvexHullData},
    model::{Place, PortalModel, SOURCE_THRESHOLD_Z},
};

pub const DT: f32 = 1.0 / 60.0;
pub const CAPSULE_RADIUS: f32 = 0.38;
pub const CAPSULE_HALF_HEIGHT: f32 = 0.52;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerState {
    pub position: Vector,
    pub vertical_velocity: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub grounded: bool,
}

struct CollisionWorld {
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
}

impl CollisionWorld {
    fn empty() -> Self {
        Self {
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
                dt: DT,
                ..IntegrationParameters::default()
            },
        }
    }

    fn add_box(&mut self, center: Vector, half: Vector) {
        self.colliders.insert(
            ColliderBuilder::cuboid(half.x, half.y, half.z)
                .translation(center)
                .friction(0.8),
        );
    }

    fn add_hull(&mut self, hull: &ConvexHullData, z_offset: f32) {
        let points: Vec<Vector> = hull
            .points
            .iter()
            .map(|point| Vector::new(point[0], point[1], point[2] + z_offset))
            .collect();
        self.colliders.insert(
            ColliderBuilder::convex_hull(&points)
                .expect("authored importer rejects degenerate brushes")
                .friction(0.8),
        );
    }

    fn finish(&mut self) {
        self.pipeline.step(
            Vector::ZERO,
            &self.integration,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd,
            &(),
            &(),
        );
    }

    fn room(threshold_at_front: bool) -> Self {
        let mut world = Self::empty();
        world.add_box(Vector::new(0.0, -0.2, 0.0), Vector::new(8.0, 0.2, 8.0));
        world.add_box(Vector::new(-8.2, 2.5, 0.0), Vector::new(0.2, 2.5, 8.2));
        world.add_box(Vector::new(8.2, 2.5, 0.0), Vector::new(0.2, 2.5, 8.2));
        let solid_z = if threshold_at_front { 8.2 } else { -8.2 };
        world.add_box(Vector::new(0.0, 2.5, solid_z), Vector::new(8.2, 2.5, 0.2));
        let gap_z = -solid_z;
        for x in [-5.2, 5.2] {
            world.add_box(Vector::new(x, 2.5, gap_z), Vector::new(2.8, 2.5, 0.2));
        }
        world.finish();
        world
    }

    fn hallway(model: &PortalModel, modules: &[AuthoredModule]) -> Self {
        let mut world = Self::empty();
        for placement in &model.composition.placements {
            let module = modules
                .iter()
                .find(|module| module.kind == placement.kind)
                .expect("WFC placement references an authored module");
            for hull in &module.hulls {
                world.add_hull(hull, placement.z_offset);
            }
        }
        world.finish();
        world
    }

    fn count(&self) -> usize {
        self.colliders.len()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimulationSnapshot {
    pub model: PortalModel,
    pub player: PlayerState,
    pub collider_count: usize,
}

pub struct PortalSimulation {
    modules: Vec<AuthoredModule>,
    pub model: PortalModel,
    collision: CollisionWorld,
    controller: KinematicCharacterController,
    capsule: Capsule,
    player: PlayerState,
}

impl PortalSimulation {
    pub fn new(seed: u64, modules: Vec<AuthoredModule>) -> Self {
        let model = PortalModel::new(seed, &modules);
        let collision = CollisionWorld::room(true);
        Self {
            modules,
            model,
            collision,
            controller: KinematicCharacterController {
                offset: CharacterLength::Absolute(0.025),
                autostep: Some(CharacterAutostep {
                    max_height: CharacterLength::Absolute(0.42),
                    min_width: CharacterLength::Absolute(0.25),
                    include_dynamic_bodies: false,
                }),
                max_slope_climb_angle: 36.0_f32.to_radians(),
                snap_to_ground: Some(CharacterLength::Absolute(0.24)),
                ..Default::default()
            },
            capsule: Capsule::new_y(CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS),
            player: PlayerState {
                position: Vector::new(0.0, 0.93, 2.5),
                vertical_velocity: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                grounded: false,
            },
        }
    }

    fn rebuild_collision(&mut self) {
        self.collision = match self.model.place {
            Place::SourceRoom => CollisionWorld::room(true),
            Place::Hallway => CollisionWorld::hallway(&self.model, &self.modules),
            Place::DestinationRoom => CollisionWorld::room(false),
        };
    }

    pub fn step(&mut self, intent: PlayerIntent) {
        let intent = intent.sanitized();
        self.player.yaw -= intent.look.x * 0.035;
        self.player.pitch = (self.player.pitch - intent.look.y * 0.025).clamp(-1.35, 1.35);
        if intent.jump_pressed && self.player.grounded {
            self.player.vertical_velocity = 6.2;
            self.player.grounded = false;
        }
        self.player.vertical_velocity -= 19.0 * DT;
        let (sin, cos) = self.player.yaw.sin_cos();
        let forward = Vector::new(-sin, 0.0, -cos);
        let right = Vector::new(cos, 0.0, -sin);
        let speed = if intent.sprint_held { 7.0 } else { 4.6 };
        let horizontal = (right * intent.movement.x + forward * intent.movement.y) * speed * DT;
        let desired = horizontal + Vector::Y * self.player.vertical_velocity * DT;
        let pose = Pose::from_translation(self.player.position);
        let queries = self.collision.broad_phase.as_query_pipeline(
            self.collision.narrow_phase.query_dispatcher(),
            &self.collision.bodies,
            &self.collision.colliders,
            QueryFilter::default(),
        );
        let movement =
            self.controller
                .move_shape(DT, &queries, &self.capsule, &pose, desired, |_| {});
        self.player.position += movement.translation;
        self.player.grounded = movement.grounded;
        if movement.grounded && self.player.vertical_velocity < 0.0 {
            self.player.vertical_velocity = 0.0;
        }
        self.handle_crossing();
        self.model.update_player_observation(
            bevy::math::Vec3::new(
                self.player.position.x,
                self.player.position.y,
                self.player.position.z,
            ),
            self.player.yaw,
        );
    }

    fn handle_crossing(&mut self) {
        match self.model.place {
            Place::SourceRoom
                if self.player.position.z < SOURCE_THRESHOLD_Z
                    && self.player.position.x.abs() < 2.3 =>
            {
                self.model.place = Place::Hallway;
                self.player.position.z = -0.7;
                self.player.position.y = 0.93;
                self.player.vertical_velocity = 0.0;
                self.rebuild_collision();
            }
            Place::Hallway if self.player.position.z > 0.15 => {
                self.model.place = Place::SourceRoom;
                self.player.position.z = SOURCE_THRESHOLD_Z + 0.7;
                self.player.position.y = 0.93;
                self.player.vertical_velocity = 0.0;
                self.rebuild_collision();
            }
            Place::Hallway if self.player.position.z < -self.model.composition.total_length() => {
                self.model.place = Place::DestinationRoom;
                self.player.position.z = 7.2;
                self.player.position.y = 0.93;
                self.player.vertical_velocity = 0.0;
                self.rebuild_collision();
            }
            Place::DestinationRoom if self.player.position.z > 8.0 => {
                self.model.place = Place::Hallway;
                self.player.position.z = -self.model.composition.total_length() + 0.7;
                self.player.position.y = 0.93;
                self.player.vertical_velocity = 0.0;
                self.rebuild_collision();
            }
            _ => {}
        }
    }

    pub fn request_blame(&mut self) -> bool {
        let changed = self.model.request_blame(&self.modules);
        if changed && self.model.place == Place::Hallway {
            self.rebuild_collision();
        }
        changed
    }

    pub fn toggle_anchor(&mut self) {
        self.model.toggle_anchor();
    }

    pub fn player(&self) -> PlayerState {
        self.player
    }

    pub fn set_player_pose_for_test(&mut self, position: Vector, yaw: f32) {
        self.player.position = position;
        self.player.yaw = yaw;
        self.model.update_player_observation(
            bevy::math::Vec3::new(position.x, position.y, position.z),
            yaw,
        );
    }

    pub fn modules(&self) -> &[AuthoredModule] {
        &self.modules
    }

    pub fn collider_count(&self) -> usize {
        self.collision.count()
    }

    pub fn snapshot(&self) -> SimulationSnapshot {
        SimulationSnapshot {
            model: self.model.clone(),
            player: self.player,
            collider_count: self.collider_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec2;

    use super::*;
    use crate::authoring::authored_modules;

    fn forward() -> PlayerIntent {
        PlayerIntent {
            movement: Vec2::Y,
            sprint_held: true,
            ..Default::default()
        }
    }

    #[test]
    fn crossing_swaps_isolated_collision_worlds_and_occupancy_keeps_lock() {
        let mut sim = PortalSimulation::new(12, authored_modules());
        let room_colliders = sim.collider_count();
        for _ in 0..150 {
            sim.step(forward());
        }
        assert_eq!(sim.model.place, Place::Hallway);
        assert!(sim.model.player_observed);
        assert!(sim.collider_count() > room_colliders);
        let before = sim.model.composition.clone();
        assert!(!sim.request_blame());
        assert_eq!(sim.model.composition, before);
    }

    #[test]
    fn previewed_composition_is_the_same_snapshot_crossed_into() {
        let mut sim = PortalSimulation::new(18, authored_modules());
        sim.set_player_pose_for_test(Vector::new(0.0, 0.93, 2.5), std::f32::consts::PI);
        assert!(sim.request_blame(), "unobserved threshold may refactor");
        sim.set_player_pose_for_test(Vector::new(0.0, 0.93, 2.5), 0.0);
        assert!(sim.model.player_observed);
        let preview_snapshot = sim.model.composition.clone();
        for _ in 0..150 {
            sim.step(forward());
        }
        assert_eq!(sim.model.place, Place::Hallway);
        assert_eq!(sim.model.composition, preview_snapshot);
    }

    #[test]
    fn scripted_crossing_reaches_destination_room() {
        let mut sim = PortalSimulation::new(12, authored_modules());
        for _ in 0..600 {
            sim.step(forward());
        }
        assert_eq!(sim.model.place, Place::DestinationRoom);
        assert!(sim.player().position.z < 8.0);
    }

    #[test]
    fn identical_intents_locks_and_blame_requests_replay_exactly() {
        let modules = authored_modules();
        let mut a = PortalSimulation::new(91, modules.clone());
        let mut b = PortalSimulation::new(91, modules);
        a.set_player_pose_for_test(Vector::new(0.0, 0.93, 2.5), std::f32::consts::PI);
        b.set_player_pose_for_test(Vector::new(0.0, 0.93, 2.5), std::f32::consts::PI);
        assert!(a.request_blame());
        assert!(b.request_blame());
        a.toggle_anchor();
        b.toggle_anchor();
        for tick in 0..480 {
            let mut intent = forward();
            intent.jump_pressed = tick == 240;
            a.step(intent);
            b.step(intent);
            assert_eq!(a.snapshot(), b.snapshot(), "diverged at tick {tick}");
        }
    }
}
