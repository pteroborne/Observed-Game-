//! Pure gantry traversal model: a two-level hallway where the upper route is fast
//! if the runner commits to jumps, while a fall lands on a navigable lower floor
//! with different threshold exits. Rendering code can project these dimensions, but
//! the timings and fall-recovery invariant are tested here.

use std::collections::{HashSet, VecDeque};
use std::f32::consts::PI;

use glam::{Vec2, Vec3};
use player_input::PlayerIntent;

use crate::{Aabb3, FIXED_DT, FpsArena, FpsBody, FpsConfig, step_body};

pub const GANTRY_LENGTH: f32 = 102.0;
pub const GANTRY_WIDTH: f32 = 12.0;
pub const UPPER_DECK_Y: f32 = 2.1;
pub const PLATFORM_HALF_WIDTH: f32 = 1.35;
pub const PLATFORM_HALF_LENGTH: f32 = 1.7;
pub const PLATFORM_THICKNESS: f32 = 0.18;
pub const PLATFORM_SPACING: f32 = 5.45;
pub const PLATFORM_COUNT: usize = 18;
pub const SAFE_BYPASS_X: f32 = -4.6;
pub const RECOVERY_LANE_X: f32 = 4.65;
pub const UNDERSTORY_SIDE_EXIT_Z: f32 = 25.2;
pub const WALL_HEIGHT: f32 = 4.8;
pub const CLEAN_MAX_SECS: f32 = 15.0;
pub const FALL_RECOVER_MAX_SECS: f32 = 26.0;
pub const SAFE_BYPASS_MIN_SECS: f32 = 14.0;

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
    pub entry_landing: GantryPlatform,
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
        let entry_landing = authored_entry_landing(&platforms);
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
        solids.push(entry_landing.solid(0.0));

        Self {
            arena: FpsArena {
                solids,
                floor_y: 0.0,
                floor_half: GANTRY_LENGTH * 0.5 + 4.0,
            },
            platforms,
            upper_landing,
            entry_landing,
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
                    // Threshold centers are architectural boundary coordinates. Keeping
                    // this inset used to make crossing, frame, visible opening, and
                    // collision each project the doorway differently.
                    center: Vec2::new(GANTRY_WIDTH * 0.5, UNDERSTORY_SIDE_EXIT_Z),
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
    let platform_span = (PLATFORM_COUNT - 1) as f32 * PLATFORM_SPACING;
    let first_z =
        -GANTRY_LENGTH * 0.5 + (GANTRY_LENGTH - platform_span) * 0.5 + PLATFORM_HALF_LENGTH;
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

/// The entry landing: a deck-height slab spanning the −Z entry threshold, contiguous
/// with the first jump platform's near edge, so a threshold crossing (which teleports
/// the body, per the user's no-stairs ruling) delivers it directly onto the deck instead
/// of a ground-level mount stair. Mirrors [`authored_upper_landing`] on the opposite end.
fn authored_entry_landing(platforms: &[GantryPlatform]) -> GantryPlatform {
    let first = platforms
        .first()
        .copied()
        .expect("authored gantry has at least one platform");
    let max_z = first.min_z();
    let min_z = -GANTRY_LENGTH * 0.5;
    GantryPlatform {
        index: PLATFORM_COUNT + 1,
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

// -----------------------------------------------------------------------------
// Expanded Gantry proof
// -----------------------------------------------------------------------------

/// Width of the v2 Gantry expanse in its local X axis.
pub const GANTRY_EXPANSE_WIDTH: f32 = 128.0;
/// Length of the v2 Gantry expanse in its local Z axis.
pub const GANTRY_EXPANSE_LENGTH: f32 = 96.0;
/// The shared high-route floor. At this height the lab's gameplay fog fully hides
/// the understory when viewed horizontally from an entry or route platform.
pub const GANTRY_EXPANSE_DECK_Y: f32 = 36.0;
pub const GANTRY_EXPANSE_PLATFORM_THICKNESS: f32 = 0.32;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GantryExpanseRoute {
    JumpLine,
    HighBridge,
    UnderstoryRecovery,
}

impl GantryExpanseRoute {
    pub const ALL: [Self; 3] = [Self::JumpLine, Self::HighBridge, Self::UnderstoryRecovery];

    pub fn label(self) -> &'static str {
        match self {
            Self::JumpLine => "jump line",
            Self::HighBridge => "high bridge",
            Self::UnderstoryRecovery => "understory recovery",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GantryExpanseExit {
    Entry,
    UpperExit,
    LowerExit,
}

impl GantryExpanseExit {
    pub fn label(self) -> &'static str {
        match self {
            Self::Entry => "entry platform",
            Self::UpperExit => "upper exit",
            Self::LowerExit => "lower exit",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GantryRouteNodeKind {
    EntryPlatform,
    JumpPlatform,
    Bridge,
    Understory,
    ExitPlatform,
    LowerExit,
}

/// Stable, ordered navigation data. Consumers must follow `nodes` in order rather
/// than sorting them on an axis: every generated route deliberately changes heading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GantryRouteNode {
    pub id: u16,
    pub position: Vec3,
    pub kind: GantryRouteNodeKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GantryRoutePath {
    pub route: GantryExpanseRoute,
    pub exit: GantryExpanseExit,
    pub nodes: Vec<GantryRouteNode>,
}

impl GantryRoutePath {
    pub fn horizontal_length(&self) -> f32 {
        self.nodes
            .windows(2)
            .map(|pair| {
                Vec2::new(pair[0].position.x, pair[0].position.z)
                    .distance(Vec2::new(pair[1].position.x, pair[1].position.z))
            })
            .sum()
    }

    pub fn heading_changes(&self) -> usize {
        self.nodes
            .windows(3)
            .filter(|triple| {
                let a = Vec2::new(
                    triple[1].position.x - triple[0].position.x,
                    triple[1].position.z - triple[0].position.z,
                )
                .normalize_or_zero();
                let b = Vec2::new(
                    triple[2].position.x - triple[1].position.x,
                    triple[2].position.z - triple[1].position.z,
                )
                .normalize_or_zero();
                a.dot(b) < 0.94
            })
            .count()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GantryDeckKind {
    Entry,
    Jump,
    UpperExit,
}

/// An oriented high deck. `half.x` is its half-width and `half.y` its half-length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GantryDeck {
    pub id: u16,
    pub kind: GantryDeckKind,
    pub center: Vec2,
    pub half: Vec2,
    pub yaw: f32,
    pub bottom_y: f32,
    pub top_y: f32,
}

impl GantryDeck {
    pub fn contains_xz(self, point: Vec2, margin: f32) -> bool {
        let delta = point - self.center;
        let (sin, cos) = self.yaw.sin_cos();
        let local = Vec2::new(delta.x * cos - delta.y * sin, delta.x * sin + delta.y * cos);
        local.x.abs() <= self.half.x + margin && local.y.abs() <= self.half.y + margin
    }

    pub fn solid(self) -> Aabb3 {
        let (sin, cos) = self.yaw.sin_cos();
        let half_x = cos.abs() * self.half.x + sin.abs() * self.half.y;
        let half_z = sin.abs() * self.half.x + cos.abs() * self.half.y;
        Aabb3::from_center_half(
            Vec3::new(
                self.center.x,
                (self.bottom_y + self.top_y) * 0.5,
                self.center.y,
            ),
            Vec3::new(half_x, (self.top_y - self.bottom_y) * 0.5, half_z),
        )
    }
}

/// A connected high-route bridge span. The lab renders this as an oriented box;
/// the pure controller arena subdivides it into short conservative AABBs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GantryBridgeSpan {
    pub id: u16,
    pub start: Vec2,
    pub end: Vec2,
    pub width: f32,
    pub bottom_y: f32,
    pub top_y: f32,
}

impl GantryBridgeSpan {
    pub fn center(self) -> Vec2 {
        (self.start + self.end) * 0.5
    }

    pub fn length(self) -> f32 {
        self.start.distance(self.end)
    }

    pub fn yaw(self) -> f32 {
        yaw_toward(self.start, self.end)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GantryHexColumn {
    pub id: u16,
    pub center: Vec2,
    pub radius: f32,
    pub bottom_y: f32,
    pub top_y: f32,
}

impl GantryHexColumn {
    pub fn solid(self) -> Aabb3 {
        Aabb3::from_center_half(
            Vec3::new(
                self.center.x,
                (self.bottom_y + self.top_y) * 0.5,
                self.center.y,
            ),
            Vec3::new(self.radius, (self.top_y - self.bottom_y) * 0.5, self.radius),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GantryExpanseThreshold {
    pub exit: GantryExpanseExit,
    pub center: Vec2,
    pub normal: Vec2,
    pub width: f32,
    pub floor_y: f32,
}

/// Deterministic, engine-independent Gantry v2 course. Presentation, collision,
/// bots, previews, and evidence tooling can all project this same data.
#[derive(Clone, Debug)]
pub struct GantryExpanseCourse {
    pub seed: u64,
    pub entry_deck: GantryDeck,
    pub upper_exit_deck: GantryDeck,
    pub jump_decks: Vec<GantryDeck>,
    pub bridge_spans: Vec<GantryBridgeSpan>,
    pub columns: Vec<GantryHexColumn>,
    pub thresholds: Vec<GantryExpanseThreshold>,
    pub routes: Vec<GantryRoutePath>,
    pub arena: FpsArena,
}

impl PartialEq for GantryExpanseCourse {
    fn eq(&self, other: &Self) -> bool {
        self.seed == other.seed
            && self.entry_deck == other.entry_deck
            && self.upper_exit_deck == other.upper_exit_deck
            && self.jump_decks == other.jump_decks
            && self.bridge_spans == other.bridge_spans
            && self.columns == other.columns
            && self.thresholds == other.thresholds
            && self.routes == other.routes
            && self.arena.solids == other.arena.solids
            && self.arena.floor_y == other.arena.floor_y
            && self.arena.floor_half == other.arena.floor_half
    }
}

impl GantryExpanseCourse {
    pub fn generate(seed: u64) -> Self {
        let mirrored = keyed_hash(seed, 0) & 1 == 1;
        let sign = if mirrored { -1.0 } else { 1.0 };
        let entry_z = -34.0 * sign;
        let upper_z = 34.0 * sign;
        let lower_z = -GANTRY_EXPANSE_LENGTH * 0.5 * sign;
        let deck_bottom = GANTRY_EXPANSE_DECK_Y - GANTRY_EXPANSE_PLATFORM_THICKNESS;
        let entry_deck = GantryDeck {
            id: 0,
            kind: GantryDeckKind::Entry,
            center: Vec2::new(-57.5, entry_z),
            half: Vec2::new(6.5, 6.0),
            yaw: 0.0,
            bottom_y: deck_bottom,
            top_y: GANTRY_EXPANSE_DECK_Y,
        };
        let upper_exit_deck = GantryDeck {
            id: 1,
            kind: GantryDeckKind::UpperExit,
            center: Vec2::new(56.5, upper_z),
            half: Vec2::new(7.5, 7.0),
            yaw: 0.0,
            bottom_y: deck_bottom,
            top_y: GANTRY_EXPANSE_DECK_Y,
        };

        let jump_controls = jittered_controls(
            seed,
            10,
            sign,
            &[
                Vec2::new(-51.5, -34.0),
                Vec2::new(-39.0, -34.0),
                Vec2::new(-31.0, -18.0),
                Vec2::new(-14.0, -22.0),
                Vec2::new(-2.0, -7.0),
                Vec2::new(14.0, -12.0),
                Vec2::new(25.0, 5.0),
                Vec2::new(42.0, 3.0),
                Vec2::new(38.0, 20.0),
                Vec2::new(49.0, 27.0),
                Vec2::new(49.0, 34.0),
            ],
        );
        let jump_points = sample_polyline(&jump_controls, 4.35);
        let jump_decks = jump_points
            .iter()
            .enumerate()
            .skip(1)
            .take(jump_points.len().saturating_sub(2))
            .filter(|(_, point)| {
                !entry_deck.contains_xz(**point, 0.1) && !upper_exit_deck.contains_xz(**point, 0.1)
            })
            .map(|(index, point)| {
                let previous = jump_points[index - 1];
                let next = jump_points[(index + 1).min(jump_points.len() - 1)];
                GantryDeck {
                    id: index as u16 + 2,
                    kind: GantryDeckKind::Jump,
                    center: *point,
                    half: Vec2::new(1.75, 1.52),
                    yaw: yaw_toward(previous, next),
                    bottom_y: deck_bottom,
                    top_y: GANTRY_EXPANSE_DECK_Y,
                }
            })
            .collect::<Vec<_>>();

        let bridge_controls = jittered_controls(
            seed,
            100,
            sign,
            &[
                Vec2::new(-51.5, -34.0),
                Vec2::new(-55.0, -8.0),
                Vec2::new(-46.0, 22.0),
                Vec2::new(-25.0, 39.0),
                Vec2::new(4.0, 40.0),
                Vec2::new(26.0, 29.0),
                Vec2::new(19.0, 11.0),
                Vec2::new(43.0, 14.0),
                Vec2::new(49.0, 34.0),
            ],
        );
        let bridge_spans = bridge_controls
            .windows(2)
            .enumerate()
            .map(|(index, pair)| GantryBridgeSpan {
                id: index as u16,
                start: pair[0],
                end: pair[1],
                width: 3.4,
                bottom_y: deck_bottom,
                top_y: GANTRY_EXPANSE_DECK_Y,
            })
            .collect::<Vec<_>>();
        let bridge_points = sample_polyline(&bridge_controls, 3.0);

        let first_fall = (jump_points[2] + jump_points[3]) * 0.5;
        let understory_controls = jittered_controls(
            seed,
            200,
            sign,
            &[
                Vec2::new(first_fall.x, first_fall.y / sign),
                Vec2::new(-47.0, -43.0),
                Vec2::new(-22.0, -42.0),
                Vec2::new(-10.0, -28.0),
                Vec2::new(-22.0, -7.0),
                Vec2::new(-1.0, 5.0),
                Vec2::new(-5.0, 25.0),
                Vec2::new(17.0, 39.0),
                Vec2::new(39.0, 29.0),
                Vec2::new(51.0, 9.0),
                Vec2::new(31.0, -9.0),
                Vec2::new(13.0, -25.0),
                Vec2::new(13.0, -48.0),
            ],
        );
        let understory_points = sample_polyline(&understory_controls, 3.2);

        let jump_path = route_path(
            GantryExpanseRoute::JumpLine,
            GantryExpanseExit::UpperExit,
            0,
            GANTRY_EXPANSE_DECK_Y,
            &jump_points,
        );
        let bridge_path = route_path(
            GantryExpanseRoute::HighBridge,
            GantryExpanseExit::UpperExit,
            1_000,
            GANTRY_EXPANSE_DECK_Y,
            &bridge_points,
        );
        let understory_path = route_path(
            GantryExpanseRoute::UnderstoryRecovery,
            GantryExpanseExit::LowerExit,
            2_000,
            0.0,
            &understory_points,
        );

        let columns = generate_columns(
            seed,
            &jump_points,
            &bridge_points,
            &understory_points,
            entry_deck,
            upper_exit_deck,
        );
        let thresholds = vec![
            GantryExpanseThreshold {
                exit: GantryExpanseExit::Entry,
                center: Vec2::new(-GANTRY_EXPANSE_WIDTH * 0.5, entry_z),
                normal: Vec2::new(-1.0, 0.0),
                width: 8.0,
                floor_y: GANTRY_EXPANSE_DECK_Y,
            },
            GantryExpanseThreshold {
                exit: GantryExpanseExit::UpperExit,
                center: Vec2::new(GANTRY_EXPANSE_WIDTH * 0.5, upper_z),
                normal: Vec2::new(1.0, 0.0),
                width: 8.0,
                floor_y: GANTRY_EXPANSE_DECK_Y,
            },
            GantryExpanseThreshold {
                exit: GantryExpanseExit::LowerExit,
                center: Vec2::new(13.0, lower_z),
                normal: Vec2::new(0.0, -sign),
                width: 7.0,
                floor_y: 0.0,
            },
        ];

        let mut solids = Vec::new();
        solids.push(entry_deck.solid());
        solids.push(upper_exit_deck.solid());
        solids.extend(jump_decks.iter().map(|deck| deck.solid()));
        for span in &bridge_spans {
            append_span_solids(&mut solids, *span);
        }
        solids.extend(columns.iter().map(|column| column.solid()));
        let arena = FpsArena {
            solids,
            floor_y: 0.0,
            floor_half: GANTRY_EXPANSE_WIDTH * 0.5,
        };

        Self {
            seed,
            entry_deck,
            upper_exit_deck,
            jump_decks,
            bridge_spans,
            columns,
            thresholds,
            routes: vec![jump_path, bridge_path, understory_path],
            arena,
        }
    }

    pub fn footprint(&self) -> Vec2 {
        Vec2::new(GANTRY_EXPANSE_WIDTH, GANTRY_EXPANSE_LENGTH)
    }

    pub fn route(&self, route: GantryExpanseRoute) -> &GantryRoutePath {
        self.routes
            .iter()
            .find(|path| path.route == route)
            .expect("expanded gantry contains every route")
    }

    pub fn threshold(&self, exit: GantryExpanseExit) -> GantryExpanseThreshold {
        self.thresholds
            .iter()
            .copied()
            .find(|threshold| threshold.exit == exit)
            .expect("expanded gantry contains every threshold")
    }

    /// Compact deterministic signature useful for snapshot/cache keys without
    /// making consumers reproduce the generator's internal construction order.
    pub fn stable_signature(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        mix_signature(&mut hash, self.seed);
        for path in &self.routes {
            mix_signature(&mut hash, path.route as u64);
            for node in &path.nodes {
                mix_signature(&mut hash, node.id as u64);
                mix_signature(&mut hash, node.position.x.to_bits() as u64);
                mix_signature(&mut hash, node.position.y.to_bits() as u64);
                mix_signature(&mut hash, node.position.z.to_bits() as u64);
            }
        }
        for column in &self.columns {
            mix_signature(&mut hash, column.id as u64);
            mix_signature(&mut hash, column.center.x.to_bits() as u64);
            mix_signature(&mut hash, column.center.y.to_bits() as u64);
            mix_signature(&mut hash, column.top_y.to_bits() as u64);
        }
        hash
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GantryExpanseRunResult {
    pub route: GantryExpanseRoute,
    pub exit: GantryExpanseExit,
    pub ticks: u32,
    pub seconds: f32,
    pub max_feet_y: f32,
}

/// Deterministic controller consumer for the ordered v2 routes. This deliberately
/// has no rendering state and doubles as the reference bot behavior for the lab.
#[derive(Clone, Debug)]
pub struct GantryExpanseRunState {
    pub route: GantryExpanseRoute,
    pub body: FpsBody,
    pub ticks: u32,
    pub waypoint_index: usize,
    pub max_feet_y: f32,
}

impl GantryExpanseRunState {
    pub fn new(
        route: GantryExpanseRoute,
        course: &GantryExpanseCourse,
        config: &FpsConfig,
    ) -> Self {
        let path = course.route(route);
        let start = path.nodes[0].position + Vec3::Y * config.half_height;
        let next = path.nodes.get(1).unwrap_or(&path.nodes[0]).position;
        Self {
            route,
            body: FpsBody::spawned(
                start,
                yaw_toward(Vec2::new(start.x, start.z), Vec2::new(next.x, next.z)),
            ),
            ticks: 0,
            waypoint_index: usize::from(path.nodes.len() > 1),
            max_feet_y: path.nodes[0].position.y,
        }
    }

    pub fn step(
        &mut self,
        course: &GantryExpanseCourse,
        config: &FpsConfig,
    ) -> Option<GantryExpanseRunResult> {
        let path = course.route(self.route);
        let here = Vec2::new(self.body.position.x, self.body.position.z);
        let target_node = &path.nodes[self.waypoint_index.min(path.nodes.len() - 1)];
        let target = Vec2::new(target_node.position.x, target_node.position.z);
        let reach = if self.route == GantryExpanseRoute::JumpLine {
            1.45
        } else {
            1.1
        };
        if here.distance(target) <= reach && self.waypoint_index + 1 < path.nodes.len() {
            self.waypoint_index += 1;
        }
        let target_node = &path.nodes[self.waypoint_index.min(path.nodes.len() - 1)];
        let target = Vec2::new(target_node.position.x, target_node.position.z);
        self.body.yaw = yaw_toward(here, target);
        let jump_pressed = self.route == GantryExpanseRoute::JumpLine
            && self.body.grounded
            && here.distance(target) > 1.25;
        let intent = PlayerIntent {
            movement: Vec2::new(0.0, 1.0),
            jump_pressed,
            sprint_held: self.route == GantryExpanseRoute::JumpLine,
            ..Default::default()
        };
        step_body(&mut self.body, intent, &course.arena, config, FIXED_DT);
        self.ticks += 1;
        let feet = self.body.position.y - config.half_height;
        self.max_feet_y = self.max_feet_y.max(feet);

        let final_node = path.nodes.last().expect("expanded route has nodes");
        let final_xz = Vec2::new(final_node.position.x, final_node.position.z);
        let here = Vec2::new(self.body.position.x, self.body.position.z);
        if self.waypoint_index + 1 == path.nodes.len()
            && here.distance(final_xz) <= 1.35
            && (feet - final_node.position.y).abs() <= 0.3
        {
            Some(GantryExpanseRunResult {
                route: self.route,
                exit: path.exit,
                ticks: self.ticks,
                seconds: self.ticks as f32 * FIXED_DT,
                max_feet_y: self.max_feet_y,
            })
        } else {
            None
        }
    }
}

pub fn simulate_expanse_route(
    route: GantryExpanseRoute,
    course: &GantryExpanseCourse,
    config: &FpsConfig,
    max_ticks: u32,
) -> Option<GantryExpanseRunResult> {
    let mut state = GantryExpanseRunState::new(route, course, config);
    for _ in 0..max_ticks {
        if let Some(result) = state.step(course, config) {
            return Some(result);
        }
    }
    None
}

fn route_path(
    route: GantryExpanseRoute,
    exit: GantryExpanseExit,
    id_base: u16,
    y: f32,
    points: &[Vec2],
) -> GantryRoutePath {
    let last = points.len().saturating_sub(1);
    let nodes = points
        .iter()
        .enumerate()
        .map(|(index, point)| GantryRouteNode {
            id: id_base + index as u16,
            position: Vec3::new(point.x, y, point.y),
            kind: if index == 0 {
                if route == GantryExpanseRoute::UnderstoryRecovery {
                    GantryRouteNodeKind::Understory
                } else {
                    GantryRouteNodeKind::EntryPlatform
                }
            } else if index == last {
                if route == GantryExpanseRoute::UnderstoryRecovery {
                    GantryRouteNodeKind::LowerExit
                } else {
                    GantryRouteNodeKind::ExitPlatform
                }
            } else {
                match route {
                    GantryExpanseRoute::JumpLine => GantryRouteNodeKind::JumpPlatform,
                    GantryExpanseRoute::HighBridge => GantryRouteNodeKind::Bridge,
                    GantryExpanseRoute::UnderstoryRecovery => GantryRouteNodeKind::Understory,
                }
            },
        })
        .collect();
    GantryRoutePath { route, exit, nodes }
}

fn sample_polyline(controls: &[Vec2], spacing: f32) -> Vec<Vec2> {
    let mut points = vec![controls[0]];
    for pair in controls.windows(2) {
        let length = pair[0].distance(pair[1]);
        let steps = (length / spacing).ceil().max(1.0) as usize;
        for step in 1..=steps {
            points.push(pair[0].lerp(pair[1], step as f32 / steps as f32));
        }
    }
    points
}

fn jittered_controls(seed: u64, stream: u64, z_sign: f32, controls: &[Vec2]) -> Vec<Vec2> {
    let last = controls.len().saturating_sub(1);
    controls
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let jitter = if index == 0 || index == last {
                Vec2::ZERO
            } else {
                Vec2::new(
                    signed_jitter(seed, stream + index as u64 * 2) * 2.1,
                    signed_jitter(seed, stream + index as u64 * 2 + 1) * 2.1,
                )
            };
            Vec2::new(point.x + jitter.x, (point.y + jitter.y) * z_sign)
        })
        .collect()
}

fn signed_jitter(seed: u64, key: u64) -> f32 {
    let unit = (keyed_hash(seed, key) >> 40) as f32 / ((1_u64 << 24) - 1) as f32;
    unit * 2.0 - 1.0
}

/// Keyed one-shot finalizer, intentionally not a streaming PRNG. Each generated
/// feature owns a stable key so adding a later feature cannot perturb earlier ones.
fn keyed_hash(seed: u64, key: u64) -> u64 {
    let mut z = seed
        .wrapping_add(key.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(0xD1B5_4A32_D192_ED03);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn generate_columns(
    seed: u64,
    jump: &[Vec2],
    bridge: &[Vec2],
    understory: &[Vec2],
    entry: GantryDeck,
    upper_exit: GantryDeck,
) -> Vec<GantryHexColumn> {
    let mut columns = Vec::new();
    let mut next_id = 0_u16;
    for q in -7_i32..=7 {
        for r in -5_i32..=5 {
            let center = Vec2::new(q as f32 * 8.4, r as f32 * 8.0 + (q & 1) as f32 * 4.0);
            if center.x.abs() > GANTRY_EXPANSE_WIDTH * 0.5 - 3.0
                || center.y.abs() > GANTRY_EXPANSE_LENGTH * 0.5 - 3.0
                || entry.contains_xz(center, 3.4)
                || upper_exit.contains_xz(center, 3.4)
                || [jump, bridge, understory]
                    .iter()
                    .any(|route| route.iter().any(|point| point.distance(center) < 5.4))
            {
                continue;
            }
            let height_roll = keyed_hash(seed, 10_000 + next_id as u64) % 2_600;
            columns.push(GantryHexColumn {
                id: next_id,
                center,
                radius: 2.35,
                bottom_y: 0.0,
                top_y: 54.0 + height_roll as f32 / 100.0,
            });
            next_id += 1;
        }
    }
    columns
}

fn append_span_solids(solids: &mut Vec<Aabb3>, span: GantryBridgeSpan) {
    let length = span.length();
    let pieces = (length / 2.4).ceil().max(1.0) as usize;
    let delta = span.end - span.start;
    let piece_length = length / pieces as f32;
    let direction = delta.normalize_or_zero();
    let half_x = direction.y.abs() * span.width * 0.5 + direction.x.abs() * piece_length * 0.55;
    let half_z = direction.x.abs() * span.width * 0.5 + direction.y.abs() * piece_length * 0.55;
    for index in 0..pieces {
        let t = (index as f32 + 0.5) / pieces as f32;
        let center = span.start.lerp(span.end, t);
        solids.push(Aabb3::from_center_half(
            Vec3::new(center.x, (span.bottom_y + span.top_y) * 0.5, center.y),
            Vec3::new(half_x, (span.top_y - span.bottom_y) * 0.5, half_z),
        ));
    }
}

fn mix_signature(hash: &mut u64, value: u64) {
    *hash ^= value;
    *hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
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

    #[test]
    fn entry_landing_reaches_the_entry_threshold_and_the_first_platform() {
        let course = GantryCourse::authored();
        let landing = course.entry_landing;
        let entry = course.threshold(GantryExit::UnderstoryReturn);

        assert_eq!(landing.top_y, UPPER_DECK_Y);
        assert_eq!(landing.bottom_y, UPPER_DECK_Y - PLATFORM_THICKNESS);
        assert!(
            landing.min_z() <= entry.center.y + 0.01,
            "entry landing must reach the entry threshold at the -Z wall"
        );
        assert!(
            landing.max_z() >= course.platforms[0].min_z() - 0.01,
            "entry landing must be contiguous with the first jump platform"
        );
    }

    #[test]
    fn expanse_is_seeded_large_multidirectional_and_three_ended() {
        let course = GantryExpanseCourse::generate(0xA11C_E5EED);
        assert_eq!(course.footprint(), Vec2::new(128.0, 96.0));
        assert_eq!(course.entry_deck.top_y, 36.0);
        assert!(
            course.columns.len() >= 45,
            "hex field should read as an expanse"
        );
        assert_eq!(course.thresholds.len(), 3);

        let jump = course.route(GantryExpanseRoute::JumpLine);
        assert!(jump.nodes.len() >= 12);
        assert!(jump.heading_changes() >= 4);
        assert!(
            jump.nodes
                .windows(2)
                .any(|pair| (pair[1].position.x - pair[0].position.x).abs() > 0.5)
        );
        assert!(
            jump.nodes
                .windows(2)
                .any(|pair| (pair[1].position.z - pair[0].position.z).abs() > 0.5)
        );

        let entry = course.threshold(GantryExpanseExit::Entry);
        assert!(
            course
                .entry_deck
                .contains_xz(entry.center - entry.normal * 1.0, 0.0)
        );
        assert_eq!(
            course.route(GantryExpanseRoute::JumpLine).exit,
            GantryExpanseExit::UpperExit
        );
        assert_eq!(
            course.route(GantryExpanseRoute::HighBridge).exit,
            GantryExpanseExit::UpperExit
        );
        assert_eq!(
            course.route(GantryExpanseRoute::UnderstoryRecovery).exit,
            GantryExpanseExit::LowerExit
        );
    }

    #[test]
    fn expanse_generation_is_exactly_repeatable_but_varies_by_seed() {
        let a = GantryExpanseCourse::generate(17);
        let b = GantryExpanseCourse::generate(17);
        let c = GantryExpanseCourse::generate(18);
        assert_eq!(a, b);
        assert_eq!(a.stable_signature(), b.stable_signature());
        assert_ne!(a.stable_signature(), c.stable_signature());
    }

    #[test]
    fn expanse_bridge_is_contiguous_between_high_platforms() {
        let course = GantryExpanseCourse::generate(73);
        let first = course
            .bridge_spans
            .first()
            .expect("bridge has a first span");
        let last = course.bridge_spans.last().expect("bridge has a last span");
        assert!(course.entry_deck.contains_xz(first.start, 0.01));
        assert!(course.upper_exit_deck.contains_xz(last.end, 0.01));
        assert!(
            course
                .bridge_spans
                .windows(2)
                .all(|pair| pair[0].end == pair[1].start)
        );
        assert!(course.bridge_spans.iter().all(|span| {
            span.bottom_y == GANTRY_EXPANSE_DECK_Y - GANTRY_EXPANSE_PLATFORM_THICKNESS
                && span.top_y == GANTRY_EXPANSE_DECK_Y
        }));
    }

    #[test]
    fn expanse_routes_have_fast_medium_slow_lengths() {
        let course = GantryExpanseCourse::generate(73);
        let jump = course
            .route(GantryExpanseRoute::JumpLine)
            .horizontal_length();
        let bridge = course
            .route(GantryExpanseRoute::HighBridge)
            .horizontal_length();
        let understory = course
            .route(GantryExpanseRoute::UnderstoryRecovery)
            .horizontal_length();
        assert!(jump < bridge, "jump={jump} bridge={bridge}");
        assert!(
            bridge < understory,
            "bridge={bridge} understory={understory}"
        );
    }

    #[test]
    fn expanse_controller_routes_replay_and_keep_timing_order() {
        let course = GantryExpanseCourse::generate(73);
        let config = FpsConfig::default();
        let mut results = Vec::new();
        for route in GantryExpanseRoute::ALL {
            let first = simulate_expanse_route(route, &course, &config, 7_200)
                .unwrap_or_else(|| panic!("{} route reaches its exit", route.label()));
            let replay = simulate_expanse_route(route, &course, &config, 7_200)
                .expect("route replay reaches its exit");
            assert_eq!(first, replay);
            results.push(first);
        }
        assert_eq!(results[0].exit, GantryExpanseExit::UpperExit);
        assert_eq!(results[1].exit, GantryExpanseExit::UpperExit);
        assert_eq!(results[2].exit, GantryExpanseExit::LowerExit);
        assert!(
            results[0].seconds < results[1].seconds && results[1].seconds < results[2].seconds,
            "route timing should be fast/medium/slow: {:?}",
            results
        );
    }
}
