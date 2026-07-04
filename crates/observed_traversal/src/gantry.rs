//! Pure gantry traversal model: a two-level hallway where the upper route is fast
//! if the runner commits to jumps, while a fall lands on a navigable lower floor
//! with different threshold exits. Rendering code can project these dimensions, but
//! the timings and fall-recovery invariant are tested here.

use std::collections::{HashSet, VecDeque};
use std::f32::consts::PI;

use glam::{Vec2, Vec3};
use player_input::PlayerIntent;

use crate::{Aabb3, FIXED_DT, FpsArena, FpsBody, FpsConfig, step_body};

pub const GANTRY_LENGTH: f32 = 34.0;
pub const GANTRY_WIDTH: f32 = 12.0;
pub const UPPER_DECK_Y: f32 = 2.1;
pub const PLATFORM_HALF_WIDTH: f32 = 1.35;
pub const PLATFORM_HALF_LENGTH: f32 = 1.7;
pub const PLATFORM_THICKNESS: f32 = 0.18;
pub const PLATFORM_SPACING: f32 = 5.45;
pub const PLATFORM_COUNT: usize = 6;
pub const SAFE_BYPASS_X: f32 = -4.6;
pub const RECOVERY_LANE_X: f32 = 4.65;
pub const UNDERSTORY_SIDE_EXIT_Z: f32 = 8.4;
pub const WALL_HEIGHT: f32 = 4.8;
pub const CLEAN_MAX_SECS: f32 = 5.2;
pub const FALL_RECOVER_MAX_SECS: f32 = 8.8;
pub const SAFE_BYPASS_MIN_SECS: f32 = 4.6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GantryRoute {
    CleanJump,
    FallRecover,
    SafeBypass,
}

impl GantryRoute {
    pub const ALL: [Self; 3] = [Self::CleanJump, Self::FallRecover, Self::SafeBypass];

    pub fn label(self) -> &'static str {
        match self {
            Self::CleanJump => "clean jump",
            Self::FallRecover => "fall recover",
            Self::SafeBypass => "safe bypass",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GantryExit {
    UpperExit,
    UnderstoryReturn,
    UnderstorySideExit,
    SafeBypassExit,
}

impl GantryExit {
    pub fn label(self) -> &'static str {
        match self {
            Self::UpperExit => "upper exit",
            Self::UnderstoryReturn => "understory return",
            Self::UnderstorySideExit => "understory side exit",
            Self::SafeBypassExit => "safe-bypass exit",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GantryThreshold {
    pub exit: GantryExit,
    /// X/Z position in the hallway local frame.
    pub center: Vec2,
    /// Outward X/Z normal.
    pub normal: Vec2,
    pub width: f32,
    /// Floor height at this threshold.
    pub floor_y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GantryPlatform {
    pub index: usize,
    /// X/Z position in the hallway local frame.
    pub center: Vec2,
    pub half: Vec2,
    pub bottom_y: f32,
    pub top_y: f32,
}

impl GantryPlatform {
    pub fn min_z(self) -> f32 {
        self.center.y - self.half.y
    }

    pub fn max_z(self) -> f32 {
        self.center.y + self.half.y
    }

    pub fn contains_xz(self, point: Vec2, margin: f32) -> bool {
        (point.x - self.center.x).abs() <= self.half.x + margin
            && (point.y - self.center.y).abs() <= self.half.y + margin
    }

    pub fn solid(self, floor_y: f32) -> Aabb3 {
        let bottom = floor_y + self.bottom_y;
        let top = floor_y + self.top_y;
        let height = top - bottom;
        Aabb3::from_center_half(
            Vec3::new(self.center.x, bottom + height * 0.5, self.center.y),
            Vec3::new(self.half.x, height * 0.5, self.half.y),
        )
    }
}

#[derive(Clone, Debug)]
pub struct GantryCourse {
    pub arena: FpsArena,
    pub platforms: Vec<GantryPlatform>,
    pub upper_landing: GantryPlatform,
    pub thresholds: Vec<GantryThreshold>,
}

impl Default for GantryCourse {
    fn default() -> Self {
        Self::authored()
    }
}

impl GantryCourse {
    pub fn authored() -> Self {
        let platforms = authored_platforms();
        let upper_landing = authored_upper_landing(&platforms);
        let mut solids = Vec::new();

        let side_wall = |x: f32| {
            Aabb3::from_center_half(
                Vec3::new(x, WALL_HEIGHT * 0.5, 0.0),
                Vec3::new(0.35, WALL_HEIGHT * 0.5, GANTRY_LENGTH * 0.5),
            )
        };
        solids.push(side_wall(-GANTRY_WIDTH * 0.5));
        solids.push(side_wall(GANTRY_WIDTH * 0.5));
        solids.extend(platforms.iter().map(|platform| platform.solid(0.0)));
        solids.push(upper_landing.solid(0.0));

        Self {
            arena: FpsArena {
                solids,
                floor_y: 0.0,
                floor_half: GANTRY_LENGTH * 0.5 + 4.0,
            },
            platforms,
            upper_landing,
            thresholds: vec![
                GantryThreshold {
                    exit: GantryExit::UnderstoryReturn,
                    center: Vec2::new(0.0, -GANTRY_LENGTH * 0.5),
                    normal: Vec2::new(0.0, -1.0),
                    width: 3.2,
                    floor_y: 0.0,
                },
                GantryThreshold {
                    exit: GantryExit::UpperExit,
                    center: Vec2::new(0.0, GANTRY_LENGTH * 0.5),
                    normal: Vec2::new(0.0, 1.0),
                    width: 3.0,
                    floor_y: UPPER_DECK_Y,
                },
                GantryThreshold {
                    exit: GantryExit::UnderstorySideExit,
                    center: Vec2::new(GANTRY_WIDTH * 0.5 - 0.65, UNDERSTORY_SIDE_EXIT_Z),
                    normal: Vec2::new(1.0, 0.0),
                    width: 3.0,
                    floor_y: 0.0,
                },
                GantryThreshold {
                    exit: GantryExit::SafeBypassExit,
                    center: Vec2::new(SAFE_BYPASS_X, GANTRY_LENGTH * 0.5),
                    normal: Vec2::new(0.0, 1.0),
                    width: 2.7,
                    floor_y: 0.0,
                },
            ],
        }
    }

    pub fn threshold(&self, exit: GantryExit) -> GantryThreshold {
        self.thresholds
            .iter()
            .copied()
            .find(|threshold| threshold.exit == exit)
            .expect("authored gantry threshold exists")
    }

    pub fn spawn_for(&self, route: GantryRoute, config: &FpsConfig) -> FpsBody {
        match route {
            GantryRoute::CleanJump | GantryRoute::FallRecover => {
                let start = self.platforms[0];
                FpsBody::spawned(
                    Vec3::new(
                        start.center.x,
                        start.top_y + config.half_height,
                        start.center.y - 0.45,
                    ),
                    PI,
                )
            }
            GantryRoute::SafeBypass => FpsBody::spawned(
                Vec3::new(
                    SAFE_BYPASS_X,
                    self.arena.floor_y + config.half_height,
                    -GANTRY_LENGTH * 0.5 + 1.6,
                ),
                PI,
            ),
        }
    }

    pub fn platform_under(&self, body: &FpsBody, config: &FpsConfig) -> Option<GantryPlatform> {
        let feet = body.position.y - config.half_height;
        let point = Vec2::new(body.position.x, body.position.z);
        self.platforms.iter().copied().find(|platform| {
            platform.contains_xz(point, config.radius) && (feet - platform.top_y).abs() <= 0.12
        })
    }

    pub fn first_fall_landing(&self) -> Vec2 {
        let first = self.platforms[0];
        let second = self.platforms[1];
        Vec2::new(0.0, (first.max_z() + second.min_z()) * 0.5)
    }

    pub fn understory_has_exit(&self, config: &FpsConfig) -> bool {
        const STEP: f32 = 0.55;
        let start = self.first_fall_landing();
        let goals = [
            self.threshold(GantryExit::UnderstoryReturn).center
                - self.threshold(GantryExit::UnderstoryReturn).normal * 0.8,
            self.threshold(GantryExit::UnderstorySideExit).center
                - self.threshold(GantryExit::UnderstorySideExit).normal * 0.8,
        ];
        let min = Vec2::new(
            -GANTRY_WIDTH * 0.5 + config.radius + 0.1,
            -GANTRY_LENGTH * 0.5 + config.radius + 0.1,
        );
        let max = Vec2::new(
            GANTRY_WIDTH * 0.5 - config.radius - 0.1,
            GANTRY_LENGTH * 0.5 - config.radius - 0.1,
        );
        let cols = (((max.x - min.x) / STEP).ceil() as i32 + 1).max(2);
        let rows = (((max.y - min.y) / STEP).ceil() as i32 + 1).max(2);
        let key = |p: Vec2| -> (i32, i32) {
            (
                ((p.x.clamp(min.x, max.x) - min.x) / STEP).round() as i32,
                ((p.y.clamp(min.y, max.y) - min.y) / STEP).round() as i32,
            )
        };
        let pos = |key: (i32, i32)| -> Vec2 {
            Vec2::new(
                (min.x + key.0 as f32 * STEP).min(max.x),
                (min.y + key.1 as f32 * STEP).min(max.y),
            )
        };
        let blocked = |p: Vec2| {
            self.arena.solids.iter().any(|solid| {
                p.x - config.radius < solid.max.x
                    && p.x + config.radius > solid.min.x
                    && self.arena.floor_y < solid.max.y
                    && self.arena.floor_y + config.half_height * 2.0 > solid.min.y
                    && p.y - config.radius < solid.max.z
                    && p.y + config.radius > solid.min.z
            })
        };

        let mut queue = VecDeque::new();
        let mut seen = HashSet::new();
        let start_key = key(start);
        if blocked(pos(start_key)) {
            return false;
        }
        queue.push_back(start_key);
        seen.insert(start_key);
        while let Some(current) = queue.pop_front() {
            let here = pos(current);
            if goals.iter().any(|goal| here.distance(*goal) <= STEP * 1.2) {
                return true;
            }
            for delta in [
                (-1, 0),
                (1, 0),
                (0, -1),
                (0, 1),
                (-1, -1),
                (-1, 1),
                (1, -1),
                (1, 1),
            ] {
                let next = (current.0 + delta.0, current.1 + delta.1);
                if next.0 < 0 || next.1 < 0 || next.0 >= cols || next.1 >= rows {
                    continue;
                }
                if seen.contains(&next) || blocked(pos(next)) {
                    continue;
                }
                seen.insert(next);
                queue.push_back(next);
            }
        }
        false
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GantryRunResult {
    pub route: GantryRoute,
    pub exit: GantryExit,
    pub ticks: u32,
    pub seconds: f32,
    pub fell_to_understory: bool,
    pub max_feet_y: f32,
}

#[derive(Clone, Debug)]
pub struct GantryRunState {
    pub route: GantryRoute,
    pub body: FpsBody,
    pub ticks: u32,
    pub fell_to_understory: bool,
    pub max_feet_y: f32,
    recovery_waypoint: usize,
}

impl GantryRunState {
    pub fn new(route: GantryRoute, course: &GantryCourse, config: &FpsConfig) -> Self {
        let body = course.spawn_for(route, config);
        Self {
            route,
            body,
            ticks: 0,
            fell_to_understory: false,
            max_feet_y: body.position.y - config.half_height,
            recovery_waypoint: 0,
        }
    }

    pub fn step(&mut self, course: &GantryCourse, config: &FpsConfig) -> Option<GantryRunResult> {
        let feet = self.body.position.y - config.half_height;
        self.max_feet_y = self.max_feet_y.max(feet);
        if self.route != GantryRoute::SafeBypass && feet < UPPER_DECK_Y - 0.45 {
            self.fell_to_understory = true;
        }

        let intent = self.intent(course, config);
        step_body(&mut self.body, intent, &course.arena, config, FIXED_DT);
        self.ticks += 1;
        self.completed(course, config)
    }

    fn intent(&mut self, course: &GantryCourse, config: &FpsConfig) -> PlayerIntent {
        let target = match self.route {
            GantryRoute::CleanJump => self.clean_jump_target(course),
            GantryRoute::FallRecover if !self.fell_to_understory => {
                let first = course.platforms[0];
                Vec2::new(RECOVERY_LANE_X, first.max_z() + 0.4)
            }
            GantryRoute::FallRecover => self.recovery_target(course),
            GantryRoute::SafeBypass => Vec2::new(SAFE_BYPASS_X, GANTRY_LENGTH * 0.5 + 1.2),
        };

        self.body.yaw = yaw_toward(
            Vec2::new(self.body.position.x, self.body.position.z),
            target,
        );
        let jump_pressed = match self.route {
            GantryRoute::CleanJump => self.should_jump(course, config),
            GantryRoute::FallRecover => false,
            GantryRoute::SafeBypass => false,
        };
        PlayerIntent {
            movement: Vec2::new(0.0, 1.0),
            jump_pressed,
            sprint_held: match self.route {
                GantryRoute::CleanJump => true,
                GantryRoute::FallRecover => !self.fell_to_understory,
                GantryRoute::SafeBypass => false,
            },
            ..Default::default()
        }
    }

    fn clean_jump_target(&self, course: &GantryCourse) -> Vec2 {
        let here_z = self.body.position.z;
        course
            .platforms
            .iter()
            .find(|platform| platform.center.y > here_z + 0.9)
            .map(|platform| platform.center)
            .unwrap_or_else(|| Vec2::new(0.0, GANTRY_LENGTH * 0.5 + 1.2))
    }

    fn recovery_target(&mut self, course: &GantryCourse) -> Vec2 {
        const WAYPOINTS: [Vec2; 3] = [
            Vec2::new(RECOVERY_LANE_X, -8.6),
            Vec2::new(RECOVERY_LANE_X, UNDERSTORY_SIDE_EXIT_Z),
            Vec2::new(GANTRY_WIDTH * 0.5 - 0.75, UNDERSTORY_SIDE_EXIT_Z),
        ];
        let current = WAYPOINTS[self.recovery_waypoint.min(WAYPOINTS.len() - 1)];
        let here = Vec2::new(self.body.position.x, self.body.position.z);
        if here.distance(current) < 0.65 && self.recovery_waypoint + 1 < WAYPOINTS.len() {
            self.recovery_waypoint += 1;
        }
        let _ = course;
        WAYPOINTS[self.recovery_waypoint.min(WAYPOINTS.len() - 1)]
    }

    fn should_jump(&self, course: &GantryCourse, config: &FpsConfig) -> bool {
        let Some(platform) = course.platform_under(&self.body, config) else {
            return false;
        };
        platform.index + 1 < course.platforms.len()
            && self.body.grounded
            && self.body.position.z >= platform.max_z() - 0.55
            && self.body.velocity.z > 1.0
    }

    fn completed(&self, _course: &GantryCourse, config: &FpsConfig) -> Option<GantryRunResult> {
        let feet = self.body.position.y - config.half_height;
        let xz = Vec2::new(self.body.position.x, self.body.position.z);
        let exit = match self.route {
            GantryRoute::CleanJump
                if feet >= UPPER_DECK_Y - 0.15 && xz.y >= GANTRY_LENGTH * 0.5 - 0.35 =>
            {
                Some(GantryExit::UpperExit)
            }
            GantryRoute::FallRecover
                if feet <= 0.18
                    && xz.x >= GANTRY_WIDTH * 0.5 - 1.05
                    && (xz.y - UNDERSTORY_SIDE_EXIT_Z).abs() <= 1.8 =>
            {
                Some(GantryExit::UnderstorySideExit)
            }
            GantryRoute::SafeBypass
                if feet <= 0.18
                    && xz.y >= GANTRY_LENGTH * 0.5 - 0.35
                    && (xz.x - SAFE_BYPASS_X).abs() <= 1.0 =>
            {
                Some(GantryExit::SafeBypassExit)
            }
            _ => None,
        }?;
        Some(GantryRunResult {
            route: self.route,
            exit,
            ticks: self.ticks,
            seconds: self.ticks as f32 * FIXED_DT,
            fell_to_understory: self.fell_to_understory,
            max_feet_y: self.max_feet_y,
        })
    }
}

pub fn simulate_route(
    route: GantryRoute,
    course: &GantryCourse,
    config: &FpsConfig,
    max_ticks: u32,
) -> Option<GantryRunResult> {
    let mut state = GantryRunState::new(route, course, config);
    for _ in 0..max_ticks {
        if let Some(result) = state.step(course, config) {
            return Some(result);
        }
    }
    None
}

fn authored_platforms() -> Vec<GantryPlatform> {
    let first_z = -13.65;
    (0..PLATFORM_COUNT)
        .map(|index| GantryPlatform {
            index,
            center: Vec2::new(0.0, first_z + index as f32 * PLATFORM_SPACING),
            half: Vec2::new(PLATFORM_HALF_WIDTH, PLATFORM_HALF_LENGTH),
            bottom_y: UPPER_DECK_Y - PLATFORM_THICKNESS,
            top_y: UPPER_DECK_Y,
        })
        .collect()
}

fn authored_upper_landing(platforms: &[GantryPlatform]) -> GantryPlatform {
    let last = platforms
        .last()
        .copied()
        .expect("authored gantry has at least one platform");
    let min_z = last.max_z();
    let max_z = GANTRY_LENGTH * 0.5;
    GantryPlatform {
        index: PLATFORM_COUNT,
        center: Vec2::new(0.0, (min_z + max_z) * 0.5),
        half: Vec2::new(PLATFORM_HALF_WIDTH, (max_z - min_z) * 0.5),
        bottom_y: UPPER_DECK_Y - PLATFORM_THICKNESS,
        top_y: UPPER_DECK_Y,
    }
}

fn yaw_toward(from: Vec2, to: Vec2) -> f32 {
    let dir = (to - from).normalize_or_zero();
    dir.x.atan2(-dir.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> FpsConfig {
        FpsConfig::default()
    }

    #[test]
    fn authored_understory_is_navigable_after_a_fall() {
        let course = GantryCourse::authored();
        assert!(
            course.understory_has_exit(&config()),
            "fall landing must have a lower-floor route to an exit"
        );
    }

    #[test]
    fn clean_jump_fall_recover_and_safe_bypass_hit_timing_targets() {
        let course = GantryCourse::authored();
        let config = config();
        let clean = simulate_route(GantryRoute::CleanJump, &course, &config, 900)
            .expect("clean route reaches upper exit");
        let fall = simulate_route(GantryRoute::FallRecover, &course, &config, 900)
            .expect("fall route recovers to side exit");
        let safe = simulate_route(GantryRoute::SafeBypass, &course, &config, 900)
            .expect("safe bypass reaches lower exit");

        assert_eq!(clean.exit, GantryExit::UpperExit);
        assert_eq!(fall.exit, GantryExit::UnderstorySideExit);
        assert_eq!(safe.exit, GantryExit::SafeBypassExit);
        assert!(!clean.fell_to_understory);
        assert!(fall.fell_to_understory);
        assert!(!safe.fell_to_understory);
        assert!(
            clean.seconds < CLEAN_MAX_SECS,
            "clean jump route is fast: {}s",
            clean.seconds
        );
        assert!(
            fall.seconds < FALL_RECOVER_MAX_SECS,
            "fall recovery is bounded: {}s",
            fall.seconds
        );
        assert!(
            safe.seconds > SAFE_BYPASS_MIN_SECS,
            "safe bypass should take time: {}s",
            safe.seconds
        );
        assert!(
            clean.seconds < fall.seconds && fall.seconds < safe.seconds,
            "timing spread must be fast/medium/slow: clean={} fall={} safe={}",
            clean.seconds,
            fall.seconds,
            safe.seconds
        );
    }

    #[test]
    fn route_runs_are_deterministic() {
        let course = GantryCourse::authored();
        let config = config();
        for route in GantryRoute::ALL {
            let a = simulate_route(route, &course, &config, 900);
            let b = simulate_route(route, &course, &config, 900);
            assert_eq!(a, b, "{} route must replay exactly", route.label());
        }
    }

    #[test]
    fn thresholds_document_distinct_upper_and_lower_exits() {
        let course = GantryCourse::authored();
        let upper = course.threshold(GantryExit::UpperExit);
        let side = course.threshold(GantryExit::UnderstorySideExit);
        let safe = course.threshold(GantryExit::SafeBypassExit);
        assert!(upper.floor_y > side.floor_y);
        assert_ne!(upper.center, side.center);
        assert_ne!(safe.center, side.center);
    }

    #[test]
    fn platform_slabs_leave_lower_floor_clearance() {
        let course = GantryCourse::authored();
        let config = config();
        let platform = course.platforms[2];
        let solid = platform.solid(0.0);
        let lower_body_center = Vec3::new(platform.center.x, config.half_height, platform.center.y);
        let body_half = Vec3::new(config.radius, config.half_height, config.radius);

        assert!(
            solid.min.y > config.half_height * 2.0,
            "the slab underside must clear a standing lower-floor body"
        );
        assert!(
            !solid.overlaps(lower_body_center, body_half),
            "a body walking under the upper route must not collide with the platform slab"
        );
    }

    #[test]
    fn upper_landing_extends_the_deck_to_the_upper_exit() {
        let course = GantryCourse::authored();
        let landing = course.upper_landing;
        let upper = course.threshold(GantryExit::UpperExit);

        assert_eq!(landing.top_y, UPPER_DECK_Y);
        assert_eq!(landing.bottom_y, UPPER_DECK_Y - PLATFORM_THICKNESS);
        assert!(
            landing.max_z() >= upper.center.y - 0.01,
            "upper landing must reach the raised exit threshold"
        );
        assert!(
            landing.min_z() <= course.platforms.last().unwrap().max_z() + 0.01,
            "upper landing must be contiguous with the final jump platform"
        );
    }
}
