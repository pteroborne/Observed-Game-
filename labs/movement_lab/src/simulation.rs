use bevy::prelude::*;
use player_input::PlayerIntent;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformId(pub u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportId {
    Segment(usize),
    Solid(usize),
    Platform(PlatformId),
}

#[derive(Clone, Copy, Debug)]
pub struct GroundContact {
    pub support: SupportId,
    pub normal: Vec2,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct MovementBody {
    pub position: Vec2,
    pub velocity: Vec2,
    pub half_size: Vec2,
    pub grounded: bool,
    pub contact: Option<GroundContact>,
    pub coyote_remaining: f32,
    pub jump_buffer_remaining: f32,
    pub spawn_position: Vec2,
    pub respawns: u32,
}

impl MovementBody {
    pub fn new(spawn_position: Vec2) -> Self {
        Self {
            position: spawn_position,
            velocity: Vec2::ZERO,
            half_size: Vec2::new(18.0, 34.0),
            grounded: false,
            contact: None,
            coyote_remaining: 0.0,
            jump_buffer_remaining: 0.0,
            spawn_position,
            respawns: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.spawn_position);
    }

    pub fn respawn(&mut self) {
        let spawn_position = self.spawn_position;
        let respawns = self.respawns + 1;
        *self = Self {
            respawns,
            ..Self::new(spawn_position)
        };
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct MovementConfig {
    pub walk_speed: f32,
    pub run_speed: f32,
    pub ground_acceleration: f32,
    pub ground_deceleration: f32,
    pub air_acceleration: f32,
    pub gravity: f32,
    pub max_fall_speed: f32,
    pub jump_speed: f32,
    pub coyote_time: f32,
    pub jump_buffer_time: f32,
    pub step_height: f32,
    pub ground_snap_distance: f32,
    pub substep_distance: f32,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            walk_speed: 175.0,
            run_speed: 285.0,
            ground_acceleration: 1250.0,
            ground_deceleration: 1550.0,
            air_acceleration: 520.0,
            gravity: 1250.0,
            max_fall_speed: 680.0,
            jump_speed: 455.0,
            coyote_time: 0.11,
            jump_buffer_time: 0.12,
            step_height: 24.0,
            ground_snap_distance: 8.0,
            substep_distance: 7.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceSegment {
    pub start: Vec2,
    pub end: Vec2,
}

impl SurfaceSegment {
    pub fn height_at(self, x: f32) -> Option<f32> {
        let min_x = self.start.x.min(self.end.x);
        let max_x = self.start.x.max(self.end.x);
        if x < min_x || x > max_x {
            return None;
        }

        let width = self.end.x - self.start.x;
        if width.abs() < f32::EPSILON {
            return None;
        }
        let t = (x - self.start.x) / width;
        Some(self.start.y + (self.end.y - self.start.y) * t)
    }

    pub fn normal(self) -> Vec2 {
        let tangent = (self.end - self.start).normalize_or_zero();
        Vec2::new(-tangent.y, tangent.x).normalize_or_zero()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SolidRect {
    pub center: Vec2,
    pub half_size: Vec2,
}

impl SolidRect {
    pub fn min(self) -> Vec2 {
        self.center - self.half_size
    }

    pub fn max(self) -> Vec2 {
        self.center + self.half_size
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MovingPlatform {
    pub id: PlatformId,
    pub origin: Vec2,
    pub center: Vec2,
    pub previous_center: Vec2,
    pub half_size: Vec2,
    pub amplitude: Vec2,
    pub angular_speed: f32,
    pub phase: f32,
}

impl MovingPlatform {
    pub fn rect(self) -> SolidRect {
        SolidRect {
            center: self.center,
            half_size: self.half_size,
        }
    }

    pub fn delta(self) -> Vec2 {
        self.center - self.previous_center
    }
}

#[derive(Resource, Clone, Debug)]
pub struct MovementWorld {
    pub segments: Vec<SurfaceSegment>,
    pub solids: Vec<SolidRect>,
    pub platforms: Vec<MovingPlatform>,
    pub elapsed: f32,
    pub bounds_min: Vec2,
    pub bounds_max: Vec2,
}

impl MovementWorld {
    pub fn authored_course() -> Self {
        let mut solids = Vec::new();
        for step in 0..5 {
            let width = 40.0;
            let height = 20.0 * (step as f32 + 1.0);
            let min_x = -150.0 + step as f32 * width;
            solids.push(SolidRect {
                center: Vec2::new(min_x + width * 0.5, -120.0 + height * 0.5),
                half_size: Vec2::new(width * 0.5, height * 0.5),
            });
        }

        Self {
            segments: vec![
                SurfaceSegment {
                    start: Vec2::new(-900.0, -260.0),
                    end: Vec2::new(-620.0, -260.0),
                },
                SurfaceSegment {
                    start: Vec2::new(-620.0, -260.0),
                    end: Vec2::new(-380.0, -120.0),
                },
                SurfaceSegment {
                    start: Vec2::new(-380.0, -120.0),
                    end: Vec2::new(-150.0, -120.0),
                },
                SurfaceSegment {
                    start: Vec2::new(50.0, -20.0),
                    end: Vec2::new(330.0, -20.0),
                },
                SurfaceSegment {
                    start: Vec2::new(560.0, -20.0),
                    end: Vec2::new(900.0, -20.0),
                },
            ],
            solids,
            platforms: vec![MovingPlatform {
                id: PlatformId(0),
                origin: Vec2::new(445.0, -135.0),
                center: Vec2::new(445.0, -135.0),
                previous_center: Vec2::new(445.0, -135.0),
                half_size: Vec2::new(72.0, 12.0),
                amplitude: Vec2::new(0.0, 92.0),
                angular_speed: 1.15,
                phase: 0.0,
            }],
            elapsed: 0.0,
            bounds_min: Vec2::new(-1040.0, -620.0),
            bounds_max: Vec2::new(1040.0, 620.0),
        }
    }

    pub fn advance_platforms(&mut self, dt: f32) {
        self.elapsed += dt;
        for platform in &mut self.platforms {
            platform.previous_center = platform.center;
            let wave = (self.elapsed * platform.angular_speed + platform.phase).sin();
            platform.center = platform.origin + platform.amplitude * wave;
        }
    }

    pub fn reset_platforms(&mut self) {
        self.elapsed = 0.0;
        for platform in &mut self.platforms {
            platform.center = platform.origin;
            platform.previous_center = platform.origin;
        }
    }

    pub fn platform(&self, id: PlatformId) -> Option<&MovingPlatform> {
        self.platforms.iter().find(|platform| platform.id == id)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MovementStep {
    pub jumped: bool,
    pub landed: bool,
    pub stepped_up: bool,
    pub respawned: bool,
}

#[derive(Clone, Copy, Debug)]
struct SupportCandidate {
    height: f32,
    normal: Vec2,
    support: SupportId,
}

pub fn step_body(
    body: &mut MovementBody,
    intent: PlayerIntent,
    world: &MovementWorld,
    config: MovementConfig,
    dt: f32,
) -> MovementStep {
    let mut report = MovementStep::default();
    if out_of_bounds(body.position, world) {
        body.respawn();
        report.respawned = true;
        return report;
    }

    if body.grounded
        && let Some(GroundContact {
            support: SupportId::Platform(id),
            ..
        }) = body.contact
        && let Some(platform) = world.platform(id)
    {
        body.position += platform.delta();
    }

    if body.grounded {
        body.coyote_remaining = config.coyote_time;
    } else {
        body.coyote_remaining = (body.coyote_remaining - dt).max(0.0);
    }
    if intent.jump_pressed {
        body.jump_buffer_remaining = config.jump_buffer_time;
    } else {
        body.jump_buffer_remaining = (body.jump_buffer_remaining - dt).max(0.0);
    }

    let target_speed = if intent.sprint_held {
        config.run_speed
    } else {
        config.walk_speed
    };
    let target_velocity = intent.movement.x * target_speed;
    let acceleration = if body.grounded {
        if intent.movement.x.abs() > 0.01 {
            config.ground_acceleration
        } else {
            config.ground_deceleration
        }
    } else {
        config.air_acceleration
    };
    body.velocity.x = approach(body.velocity.x, target_velocity, acceleration * dt);

    if body.jump_buffer_remaining > 0.0 && body.coyote_remaining > 0.0 {
        execute_jump(body, config);
        report.jumped = true;
    }

    if !body.grounded {
        body.velocity.y = (body.velocity.y - config.gravity * dt).max(-config.max_fall_speed);
    }

    let was_grounded = body.grounded;
    let displacement = body.velocity * dt;
    let substeps = (displacement.abs().max_element() / config.substep_distance)
        .ceil()
        .max(1.0) as usize;
    let substep = displacement / substeps as f32;

    for _ in 0..substeps {
        report.stepped_up |= move_horizontal(body, world, config, substep.x);
        report.landed |= move_vertical(body, world, substep.y);
    }

    if !report.jumped && body.velocity.y <= 0.0 {
        let snapped = snap_to_ground(body, world, config, was_grounded || body.grounded);
        report.landed |= snapped && !was_grounded;
    }

    if body.grounded && body.jump_buffer_remaining > 0.0 && !report.jumped {
        execute_jump(body, config);
        report.jumped = true;
    }

    if out_of_bounds(body.position, world) {
        body.respawn();
        report.respawned = true;
    }
    report
}

fn execute_jump(body: &mut MovementBody, config: MovementConfig) {
    body.velocity.y = config.jump_speed;
    body.grounded = false;
    body.contact = None;
    body.coyote_remaining = 0.0;
    body.jump_buffer_remaining = 0.0;
}

fn move_horizontal(
    body: &mut MovementBody,
    world: &MovementWorld,
    config: MovementConfig,
    delta_x: f32,
) -> bool {
    if delta_x.abs() < f32::EPSILON {
        return false;
    }

    let proposed_x = body.position.x + delta_x;
    let feet = body.position.y - body.half_size.y;
    let mut stepped_up = false;
    let mut blocked_x: Option<f32> = None;

    for (index, rect) in collision_rects(world) {
        let min = rect.min();
        let max = rect.max();
        let vertical_overlap = body.position.y + body.half_size.y > min.y + 0.1
            && body.position.y - body.half_size.y < max.y - 0.1;
        let horizontal_overlap =
            proposed_x + body.half_size.x > min.x && proposed_x - body.half_size.x < max.x;
        if !vertical_overlap || !horizontal_overlap {
            continue;
        }

        let rise = max.y - feet;
        if body.velocity.y <= 0.0 && rise >= -0.5 && rise <= config.step_height {
            body.position.x = proposed_x;
            body.position.y = max.y + body.half_size.y;
            body.velocity.y = 0.0;
            body.grounded = true;
            body.contact = Some(GroundContact {
                support: index,
                normal: Vec2::Y,
            });
            stepped_up = true;
            continue;
        }

        let resolved = if delta_x > 0.0 {
            min.x - body.half_size.x
        } else {
            max.x + body.half_size.x
        };
        blocked_x = Some(match blocked_x {
            Some(current) if delta_x > 0.0 => current.min(resolved),
            Some(current) => current.max(resolved),
            None => resolved,
        });
    }

    if !stepped_up {
        body.position.x = blocked_x.unwrap_or(proposed_x);
        if blocked_x.is_some() {
            body.velocity.x = 0.0;
        }
    }
    stepped_up
}

fn move_vertical(body: &mut MovementBody, world: &MovementWorld, delta_y: f32) -> bool {
    if delta_y.abs() < f32::EPSILON {
        return false;
    }

    let previous_feet = body.position.y - body.half_size.y;
    let proposed_y = body.position.y + delta_y;
    if delta_y < 0.0 {
        let proposed_feet = proposed_y - body.half_size.y;
        if let Some(candidate) =
            support_at(world, body.position.x, previous_feet + 0.5).filter(|candidate| {
                proposed_feet <= candidate.height && previous_feet >= candidate.height - 0.5
            })
        {
            body.position.y = candidate.height + body.half_size.y;
            body.velocity.y = 0.0;
            let landed = !body.grounded;
            body.grounded = true;
            body.contact = Some(GroundContact {
                support: candidate.support,
                normal: candidate.normal,
            });
            return landed;
        }
    } else {
        let previous_head = body.position.y + body.half_size.y;
        let proposed_head = proposed_y + body.half_size.y;
        for (_, rect) in collision_rects(world) {
            let min = rect.min();
            let max = rect.max();
            let overlaps_x = body.position.x + body.half_size.x > min.x
                && body.position.x - body.half_size.x < max.x;
            if overlaps_x && previous_head <= min.y && proposed_head >= min.y {
                body.position.y = min.y - body.half_size.y;
                body.velocity.y = 0.0;
                return false;
            }
        }
    }

    body.position.y = proposed_y;
    if delta_y.abs() > 0.0 {
        body.grounded = false;
        body.contact = None;
    }
    false
}

fn snap_to_ground(
    body: &mut MovementBody,
    world: &MovementWorld,
    config: MovementConfig,
    allow_step: bool,
) -> bool {
    let feet = body.position.y - body.half_size.y;
    let max_up = if allow_step { config.step_height } else { 0.5 };
    let Some(candidate) = support_at(world, body.position.x, feet + max_up) else {
        body.grounded = false;
        body.contact = None;
        return false;
    };

    let difference = candidate.height - feet;
    if difference < -config.ground_snap_distance || difference > max_up {
        body.grounded = false;
        body.contact = None;
        return false;
    }

    let landed = !body.grounded;
    body.position.y = candidate.height + body.half_size.y;
    body.velocity.y = 0.0;
    body.grounded = true;
    body.contact = Some(GroundContact {
        support: candidate.support,
        normal: candidate.normal,
    });
    landed
}

fn support_at(world: &MovementWorld, x: f32, max_height: f32) -> Option<SupportCandidate> {
    let mut best: Option<SupportCandidate> = None;
    for (index, segment) in world.segments.iter().copied().enumerate() {
        if let Some(height) = segment.height_at(x).filter(|height| *height <= max_height) {
            choose_higher(
                &mut best,
                SupportCandidate {
                    height,
                    normal: segment.normal(),
                    support: SupportId::Segment(index),
                },
            );
        }
    }
    for (index, rect) in world.solids.iter().copied().enumerate() {
        if x >= rect.min().x && x <= rect.max().x && rect.max().y <= max_height {
            choose_higher(
                &mut best,
                SupportCandidate {
                    height: rect.max().y,
                    normal: Vec2::Y,
                    support: SupportId::Solid(index),
                },
            );
        }
    }
    for platform in &world.platforms {
        let rect = platform.rect();
        if x >= rect.min().x && x <= rect.max().x && rect.max().y <= max_height {
            choose_higher(
                &mut best,
                SupportCandidate {
                    height: rect.max().y,
                    normal: Vec2::Y,
                    support: SupportId::Platform(platform.id),
                },
            );
        }
    }
    best
}

fn choose_higher(best: &mut Option<SupportCandidate>, candidate: SupportCandidate) {
    if best.is_none_or(|current| candidate.height > current.height) {
        *best = Some(candidate);
    }
}

fn collision_rects(world: &MovementWorld) -> impl Iterator<Item = (SupportId, SolidRect)> + '_ {
    world
        .solids
        .iter()
        .copied()
        .enumerate()
        .map(|(index, rect)| (SupportId::Solid(index), rect))
        .chain(
            world
                .platforms
                .iter()
                .map(|platform| (SupportId::Platform(platform.id), platform.rect())),
        )
}

fn out_of_bounds(position: Vec2, world: &MovementWorld) -> bool {
    position.x < world.bounds_min.x
        || position.y < world.bounds_min.y
        || position.x > world.bounds_max.x
        || position.y > world.bounds_max.y
}

fn approach(current: f32, target: f32, max_delta: f32) -> f32 {
    if current < target {
        (current + max_delta).min(target)
    } else {
        (current - max_delta).max(target)
    }
}
