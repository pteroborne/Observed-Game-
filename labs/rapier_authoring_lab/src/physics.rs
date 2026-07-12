//! Fixed-step raw-Rapier world for imported static convex brushes and one capsule.

use player_input::PlayerIntent;
use rapier3d::{
    control::{CharacterAutostep, CharacterLength, KinematicCharacterController},
    prelude::*,
};

use crate::import::{ConvexBrush, DoorState, Facility};

pub const DT: f32 = 1.0 / 60.0;
pub const CAPSULE_RADIUS: f32 = 0.38;
pub const CAPSULE_HALF_HEIGHT: f32 = 0.52;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerState {
    /// Capsule centre, not feet position.
    pub position: Vector,
    pub vertical_velocity: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub grounded: bool,
    pub collisions_this_tick: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub player: PlayerState,
    pub doors: Vec<DoorState>,
    pub collider_count: usize,
}

pub struct RapierCourse {
    facility: Facility,
    gravity: Vector,
    integration: IntegrationParameters,
    pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad_phase: BroadPhaseBvh,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd: CCDSolver,
    door_handles: Vec<Option<ColliderHandle>>,
    door_states: Vec<DoorState>,
    controller: KinematicCharacterController,
    capsule: Capsule,
    player: PlayerState,
}

fn collider(hull: &ConvexBrush) -> Collider {
    let points: Vec<Vector> = hull
        .points
        .iter()
        .map(|point| Vector::from(*point))
        .collect();
    ColliderBuilder::convex_hull(&points)
        .expect("import projection rejects degenerate brushes")
        .friction(0.8)
        .build()
}

impl RapierCourse {
    pub fn new(facility: Facility) -> Self {
        let center_y = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS + 0.03;
        let spawn = facility.spawn_ground;
        let mut course = Self {
            gravity: Vector::new(0.0, -19.0, 0.0),
            integration: IntegrationParameters {
                dt: DT,
                ..IntegrationParameters::default()
            },
            pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad_phase: BroadPhaseBvh::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd: CCDSolver::new(),
            door_handles: vec![None; facility.doors.len()],
            door_states: facility.doors.iter().map(|door| door.state).collect(),
            controller: KinematicCharacterController {
                offset: CharacterLength::Absolute(0.025),
                autostep: Some(CharacterAutostep {
                    max_height: CharacterLength::Absolute(0.42),
                    min_width: CharacterLength::Absolute(0.25),
                    include_dynamic_bodies: false,
                }),
                max_slope_climb_angle: 34.0_f32.to_radians(),
                min_slope_slide_angle: 42.0_f32.to_radians(),
                snap_to_ground: Some(CharacterLength::Absolute(0.25)),
                ..Default::default()
            },
            capsule: Capsule::new_y(CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS),
            player: PlayerState {
                position: Vector::new(spawn.x, spawn.y + center_y, spawn.z),
                vertical_velocity: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                grounded: false,
                collisions_this_tick: 0,
            },
            facility,
        };

        for hull in &course.facility.structural {
            course.colliders.insert(collider(hull));
        }
        for index in 0..course.facility.doors.len() {
            if course.door_states[index] == DoorState::Closed {
                course.door_handles[index] = Some(
                    course
                        .colliders
                        .insert(collider(&course.facility.doors[index].hull)),
                );
            }
        }
        course.refresh_queries();
        course
    }

    fn refresh_queries(&mut self) {
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

    pub fn step(&mut self, intent: PlayerIntent) {
        let intent = intent.sanitized();
        self.player.yaw -= intent.look.x * 0.035;
        self.player.pitch = (self.player.pitch - intent.look.y * 0.025).clamp(-1.35, 1.35);

        if intent.jump_pressed && self.player.grounded {
            self.player.vertical_velocity = 6.2;
            self.player.grounded = false;
        }
        self.player.vertical_velocity += self.gravity.y * DT;

        let (sin, cos) = self.player.yaw.sin_cos();
        let forward = Vector::new(-sin, 0.0, -cos);
        let right = Vector::new(cos, 0.0, -sin);
        let speed = if intent.sprint_held { 7.0 } else { 4.6 };
        let horizontal = (right * intent.movement.x + forward * intent.movement.y) * speed * DT;
        let desired = horizontal + Vector::Y * self.player.vertical_velocity * DT;
        let pose = Pose::from_translation(self.player.position);
        let queries = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            QueryFilter::default(),
        );
        let mut collisions = 0;
        let movement =
            self.controller
                .move_shape(DT, &queries, &self.capsule, &pose, desired, |_| {
                    collisions += 1
                });
        self.player.position += movement.translation;
        self.player.grounded = movement.grounded;
        self.player.collisions_this_tick = collisions;
        if movement.grounded && self.player.vertical_velocity < 0.0 {
            self.player.vertical_velocity = 0.0;
        }

        if self.player.position.y < -8.0 {
            let spawn = self.facility.spawn_ground;
            self.player.position = Vector::new(
                spawn.x,
                spawn.y + CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS + 0.03,
                spawn.z,
            );
            self.player.vertical_velocity = 0.0;
        }
    }

    pub fn toggle_door(&mut self, index: usize) {
        let Some(state) = self.door_states.get_mut(index) else {
            return;
        };
        match *state {
            DoorState::Closed => {
                if let Some(handle) = self.door_handles[index].take() {
                    self.colliders
                        .remove(handle, &mut self.islands, &mut self.bodies, true);
                }
                *state = DoorState::Open;
            }
            DoorState::Open => {
                self.door_handles[index] = Some(
                    self.colliders
                        .insert(collider(&self.facility.doors[index].hull)),
                );
                *state = DoorState::Closed;
            }
        }
        self.refresh_queries();
    }

    pub fn player(&self) -> PlayerState {
        self.player
    }

    pub fn door_states(&self) -> &[DoorState] {
        &self.door_states
    }

    pub fn collider_count(&self) -> usize {
        self.colliders.len()
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            player: self.player,
            doors: self.door_states.clone(),
            collider_count: self.collider_count(),
        }
    }

    pub fn state_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut fold = |bits: u32| {
            hash ^= u64::from(bits);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        };
        for value in [
            self.player.position.x,
            self.player.position.y,
            self.player.position.z,
        ] {
            fold(value.to_bits());
        }
        fold(self.player.vertical_velocity.to_bits());
        fold(self.player.yaw.to_bits());
        fold(self.player.pitch.to_bits());
        fold(u32::from(self.player.grounded));
        for state in &self.door_states {
            fold(u32::from(*state == DoorState::Closed));
        }
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::parse_course;
    use bevy::math::Vec2;

    fn forward() -> PlayerIntent {
        PlayerIntent {
            movement: Vec2::Y,
            sprint_held: true,
            ..Default::default()
        }
    }

    #[test]
    fn closed_door_blocks_then_model_mutation_opens_the_route() {
        let facility = parse_course();
        let door_z = facility.doors[0].hull.max.z;
        let mut course = RapierCourse::new(facility);
        for _ in 0..360 {
            course.step(forward());
        }
        assert!(
            course.player().position.z > door_z - 0.7,
            "capsule crossed closed door: player z {}, door z {door_z}",
            course.player().position.z
        );

        course.toggle_door(0);
        for _ in 0..360 {
            course.step(forward());
        }
        assert_eq!(course.door_states()[0], DoorState::Open);
        assert!(course.player().position.z < door_z - 2.0);
    }

    #[test]
    fn capsule_climbs_the_imported_convex_ramp_to_the_platform() {
        let mut course = RapierCourse::new(parse_course());
        course.toggle_door(0);
        for _ in 0..600 {
            course.step(forward());
        }
        let feet_y = course.player().position.y - CAPSULE_HALF_HEIGHT - CAPSULE_RADIUS;
        assert!(feet_y > 2.0, "expected raised platform, feet y={feet_y}");
        assert!(course.player().grounded);
    }

    #[test]
    fn scripted_intents_and_door_change_replay_bit_identically() {
        let facility = parse_course();
        let mut a = RapierCourse::new(facility.clone());
        let mut b = RapierCourse::new(facility);
        for tick in 0..900 {
            if tick == 300 {
                a.toggle_door(0);
                b.toggle_door(0);
            }
            let mut intent = forward();
            intent.movement.x = if (500..650).contains(&tick) { 0.2 } else { 0.0 };
            intent.jump_pressed = tick == 520;
            a.step(intent);
            b.step(intent);
            assert_eq!(a.snapshot(), b.snapshot(), "diverged at tick {tick}");
            assert_eq!(
                a.state_hash(),
                b.state_hash(),
                "hash diverged at tick {tick}"
            );
        }
    }

    #[test]
    fn rebuilding_restores_authored_state_and_counts() {
        let facility = parse_course();
        let baseline = RapierCourse::new(facility.clone()).snapshot();
        let mut changed = RapierCourse::new(facility.clone());
        changed.toggle_door(0);
        for _ in 0..120 {
            changed.step(forward());
        }
        assert_ne!(changed.snapshot(), baseline);
        assert_eq!(RapierCourse::new(facility).snapshot(), baseline);
    }
}
