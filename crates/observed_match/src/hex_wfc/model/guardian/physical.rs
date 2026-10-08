//! Major motion owns one physical pose. Sight and presentation read that pose.
use glam::{Vec2, Vec3};
use observed_hex::{HexCoord, hex_origin};
use observed_traversal::rapier_controller::step_solid_character_with_settings;
use observed_traversal::{FollowerPose, FpsBody, GraphFollowState};
use player_input::PlayerIntent;

use super::{
    HexGuardianBounds, HexGuardianState, MAJOR_HALF_HEIGHT, MAJOR_RADIUS, POSITION_ABOVE_FEET,
};
use crate::hex_wfc::HexTraversalCursor;
use crate::hex_wfc::model::{FIXED_DT, HexWfcMatch, bot::leg};

// The old cross-cell cadence was one 14 m cell every 120 ticks (two seconds).
// Preserve that pressure continuously instead of slowing directives into expiry.
const SPEED: f32 = 7.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::hex_wfc::model) struct MajorMotion {
    body: Option<FpsBody>,
    goal: Option<HexCoord>,
    from: Option<HexCoord>,
    generation: u32,
    route: Vec<HexCoord>,
    leg: Option<HexTraversalCursor>,
}

impl MajorMotion {
    pub(super) fn placed(&self) -> bool {
        self.body.is_some()
    }

    pub(super) fn stop(&mut self) {
        if let Some(body) = &mut self.body {
            body.velocity = Vec3::ZERO;
        }
    }

    /// Exact movement state participates in deterministic equality, including
    /// progress through a leased graph that will influence the next input.
    pub(in crate::hex_wfc::model) fn words(&self) -> Vec<u64> {
        let mut words = Vec::new();
        if let Some(body) = self.body {
            words.push(1);
            for v in [body.position, body.velocity, body.spawn] {
                words.extend(v.to_array().map(|v| u64::from(v.to_bits())));
            }
            words.extend(
                [body.yaw, body.pitch, body.jump_cd, body.spawn_yaw]
                    .map(|v| u64::from(v.to_bits())),
            );
            words.push(u64::from(body.grounded));
        } else {
            words.push(0);
        }
        words.push(u64::from(self.generation));
        for cell in [self.goal, self.from] {
            words.push(cell.map_or(u64::MAX, pack));
        }
        words.push(self.route.len() as u64);
        words.extend(self.route.iter().copied().map(pack));
        if let Some(cursor) = &self.leg {
            words.extend([
                1,
                pack(cursor.lease.instance.source_cell),
                u64::from(cursor.lease.entry.0),
                u64::from(cursor.lease.exit.0),
                u64::from(cursor.lease.projected),
                u64::from(cursor.local.edge.0),
            ]);
            words.push(match cursor.local.direction {
                observed_traversal::TraversalDirection::Forward => 0,
                observed_traversal::TraversalDirection::Reverse => 1,
            });
            words.push(cursor.lease.revision.cells().len() as u64);
            for (cell, revision) in cursor.lease.revision.cells() {
                words.extend([pack(*cell), u64::from(*revision)]);
            }
        } else {
            words.push(0);
        }
        words
    }
}

fn pack(cell: HexCoord) -> u64 {
    u64::from(cell.q) | (u64::from(cell.r) << 16) | (u64::from(cell.level) << 32)
}

impl HexWfcMatch {
    pub(in crate::hex_wfc::model) fn step_major(&mut self, major: &mut HexGuardianState) {
        self.prepare_major(major);
        if major.motion.body.is_none() {
            return;
        }
        let grid = self.facility.config.grid();
        let goal = major.step(
            self.tick,
            &self.facility,
            &self.lanterns,
            &mut self.players,
            &mut self.recent_events,
            HexGuardianBounds {
                prison: self.prison.as_ref(),
                closed: &|a, b| super::super::doors::closed_between(&self.doors, grid, a, b),
                directive: self.guardian_directive,
                clear: &|a, b| {
                    super::super::sight::line_is_clear(&self.physics, &self.geometry, a, b)
                },
                eye_height: self.content.traversal_profile().controller().eye_height
                    - self.content.traversal_profile().controller().half_height,
            },
        );
        let Some((goal, target)) = goal else {
            if !major.motion.placed() {
                self.prepare_major(major);
            }
            return;
        };
        let mut motion = std::mem::take(&mut major.motion);
        let Some(mut body) = motion.body else {
            major.motion = motion;
            return;
        };
        if motion.goal != Some(goal) {
            motion.leg = None;
        }
        if motion.goal != Some(goal)
            || motion.from != Some(major.cell)
            || motion.generation != self.facility.generation
        {
            motion.route = self
                .facility
                .route_between_cells(major.cell, goal)
                .map_or_else(Vec::new, |r| r.cells);
            motion.goal = Some(goal);
            motion.from = Some(major.cell);
            motion.generation = self.facility.generation;
        }
        let mut intent = PlayerIntent::default();
        let next = motion.route.get(1).copied();
        let sanctuary = |cell: HexCoord| {
            self.prison
                .as_ref()
                .is_some_and(|p| p.lobby.contains(&cell))
        };
        if let Some(next) = next {
            if !sanctuary(next)
                && !super::super::doors::closed_between(&self.doors, grid, major.cell, next)
            {
                if motion.leg.is_none()
                    && let Some(transition) =
                        leg::ExternalTransition::between(self, major.cell, next)
                {
                    motion.leg = leg::acquire(self, major.feet(), transition);
                }
                if let Some(cursor) = &mut motion.leg {
                    let decision = leg::follow_with_clearance(
                        self,
                        cursor,
                        FollowerPose {
                            feet: self
                                .physics
                                .ray_distance(major.feet() + Vec3::Y * 0.1, Vec3::NEG_Y, 2.5)
                                .map_or(major.feet(), |drop| major.feet() + Vec3::Y * (0.1 - drop)),
                            yaw: body.yaw,
                        },
                        2.0 * MAJOR_RADIUS,
                    );
                    if decision.state == GraphFollowState::Following {
                        intent = decision.intent.unwrap_or_default();
                    } else {
                        motion.leg = None;
                    }
                }
                if motion.leg.is_none() && next.level == major.cell.level {
                    let aim = self
                        .lateral_waypoint(major.cell, next, body.position)
                        .unwrap_or_else(|| Vec3::from_array(hex_origin(next)));
                    intent = super::super::bot::steer_toward_with_speed(
                        body.yaw,
                        body.position,
                        aim,
                        false,
                        1.0,
                    );
                }
            } else {
                motion.leg = None;
            }
        } else if goal == major.cell && !sanctuary(goal) {
            intent = super::super::bot::steer_toward_with_speed(
                body.yaw,
                body.position,
                target,
                false,
                1.0,
            );
        }
        intent.sprint_held = false;
        intent.jump_pressed = false;
        if intent.movement != Vec2::ZERO {
            // Followers use the Observer's travel speed. The major has its own
            // speed, so retain direction but not the follower's throttle.
            intent.movement = intent.movement.normalize_or_zero();
        }
        let mut config = self.content.traversal_profile().controller();
        config.radius = MAJOR_RADIUS;
        config.half_height = MAJOR_HALF_HEIGHT;
        config.walk_speed = SPEED;
        config.run_speed = SPEED;
        let before = body;
        let step = step_solid_character_with_settings(
            &self.physics,
            &mut body,
            intent,
            &config,
            self.content.traversal_profile().rapier(),
            FIXED_DT,
        );
        if step.recovered {
            body = before;
            body.velocity = Vec3::ZERO;
        }
        let feet = body.position - Vec3::Y * MAJOR_HALF_HEIGHT;
        let reference = feet + Vec3::Y * self.content.traversal_profile().controller().half_height;
        let cell = self.body_cell(reference, major.cell);
        if sanctuary(cell) {
            body = before;
            body.velocity = Vec3::ZERO;
        } else {
            major.cell = cell;
            major.position = feet + Vec3::Y * POSITION_ABOVE_FEET;
        }
        motion.body = Some(body);
        major.motion = motion;
    }

    /// A full-sized major's supported placement near a requested point. Used by
    /// authored inspection fixtures; this query never moves the live Guardian.
    #[must_use]
    pub fn major_standing_point(&self, cell: HexCoord, preferred: Vec3) -> Option<Vec3> {
        let mut candidate = HexGuardianState::at(cell);
        candidate.position = preferred + Vec3::Y * POSITION_ABOVE_FEET;
        self.prepare_major(&mut candidate);
        candidate.physically_placed().then(|| candidate.feet())
    }

    pub(in crate::hex_wfc::model) fn prepare_major(&self, major: &mut HexGuardianState) {
        let expected = major.feet() + Vec3::Y * MAJOR_HALF_HEIGHT;
        if major
            .motion
            .body
            .is_some_and(|body| body.position.distance(expected) < 0.001)
        {
            return;
        }
        // New release, catch return, or explicit fixture relocation. Find a
        // support under the real enclosure and a clear full-size envelope.
        let origin = Vec3::from_array(hex_origin(major.cell));
        let offsets = [
            Vec3::ZERO,
            Vec3::X * 2.0,
            -Vec3::X * 2.0,
            Vec3::Z * 2.0,
            -Vec3::Z * 2.0,
            Vec3::X * 4.0,
            -Vec3::X * 4.0,
            Vec3::Z * 4.0,
            -Vec3::Z * 4.0,
        ];
        let feet = std::iter::once(major.feet())
            .chain(offsets.into_iter().map(|o| origin + o + Vec3::Y * 0.5))
            .find_map(|feet| {
                let from = feet + Vec3::Y * 0.7;
                let drop = self.physics.ray_distance(from, Vec3::NEG_Y, 1.5)?;
                let support = from - Vec3::Y * drop;
                [0.03, 0.15, 0.35, 0.65, 0.95, 1.35]
                    .into_iter()
                    .map(|lift| support + Vec3::Y * lift)
                    .find(|&feet| {
                        self.physics.solid_is_clear(
                            feet + Vec3::Y * MAJOR_HALF_HEIGHT,
                            MAJOR_RADIUS,
                            MAJOR_HALF_HEIGHT,
                        )
                    })
            });
        if let Some(feet) = feet {
            let mut body = FpsBody::spawned(feet + Vec3::Y * MAJOR_HALF_HEIGHT, 0.0);
            let mut config = self.content.traversal_profile().controller();
            config.radius = MAJOR_RADIUS;
            config.half_height = MAJOR_HALF_HEIGHT;
            for _ in 0..32 {
                let report = step_solid_character_with_settings(
                    &self.physics,
                    &mut body,
                    PlayerIntent::default(),
                    &config,
                    self.content.traversal_profile().rapier(),
                    FIXED_DT,
                );
                if report.recovered {
                    return;
                }
                if body.grounded {
                    break;
                }
            }
            if !body.grounded {
                return;
            }
            major.position =
                body.position - Vec3::Y * MAJOR_HALF_HEIGHT + Vec3::Y * POSITION_ABOVE_FEET;
            major.motion = MajorMotion {
                body: Some(body),
                ..Default::default()
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex_wfc::{HexGuardianStatus, HexMatchConfig};
    use glam::Quat;
    use observed_traversal::rapier_controller::RapierTraversalScene;
    use observed_traversal::{ArenaSpec, ColliderSpec, StableColliderId};

    fn fixture(degrees: f32) -> (HexWfcMatch, HexGuardianState) {
        let config = HexMatchConfig {
            wfc: observed_facility::hex_wfc::HexWfcConfig {
                cols: 8,
                rows: 6,
                levels: 1,
                ..Default::default()
            },
            guardian: false,
            ..Default::default()
        };
        let mut game = HexWfcMatch::new_with_content(
            1,
            config,
            crate::hex_wfc::compatibility_test_content().clone(),
        )
        .unwrap();
        let cell = game.facility.config.spawn();
        let origin = Vec3::from_array(hex_origin(cell));
        let mut ramp =
            ColliderSpec::cuboid(StableColliderId(1), origin, Vec3::new(30.0, 0.5, 40.0));
        ramp.rotation = Quat::from_rotation_x(degrees.to_radians()).to_array();
        game.physics = RapierTraversalScene::from_arena_spec(&ArenaSpec {
            colliders: vec![ramp],
            floor_y: origin.y - 30.0,
            safety_center: origin,
            safety_half: Vec3::splat(100.0),
        });
        for player in game.players.values_mut() {
            player.cell = cell;
            player.position = origin + Vec3::new(0.0, 5.0, -12.0);
            player.yaw = 0.0;
            player.pitch = 0.0;
        }
        let mut major = HexGuardianState::at(cell);
        major.position = origin + Vec3::Y * 1.5;
        (game, major)
    }

    #[test]
    fn a_major_has_physical_ramp_support_and_freezes_at_the_pose_that_is_seen() {
        let (mut game, mut major) = fixture(20.0);
        let mut replica = game.clone();
        let mut other = major.clone();
        // One second keeps this synthetic same-cell pursuit inside its cell.
        // Crossing a cell boundary would test the generated maze, not this ramp.
        for tick in 1..=60 {
            game.tick = tick;
            replica.tick = tick;
            game.step_major(&mut major);
            replica.step_major(&mut other);
            assert_eq!(major, other);
        }
        assert!(major.motion.body.is_some());
        let body = major.motion.body.unwrap();
        assert!(body.grounded);
        assert!(
            major.feet().y > 0.8,
            "solid foot must stay above ramp, got {:?}",
            major.feet()
        );
        assert!(
            body.position.z < Vec3::from_array(hex_origin(major.cell)).z - 2.0,
            "must actually walk"
        );
        let id = *game.players.keys().next().unwrap();
        let observer = game.players.get_mut(&id).unwrap();
        observer.position = major.position + Vec3::Z * 5.0;
        observer.yaw = 0.0;
        observer.pitch = 0.0;
        let before = major.position;
        for _ in 0..120 {
            game.tick += 1;
            game.step_major(&mut major);
        }
        assert_eq!(major.status, HexGuardianStatus::FrozenByPlayer);
        assert_eq!(major.position, before);
        assert_eq!(major.motion.body.unwrap().velocity, Vec3::ZERO);
    }
}
