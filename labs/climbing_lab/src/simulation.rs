use bevy::prelude::*;
use observed_core::PlayerIntent;

/// Half-extents of every climbing body. Bodies are uniform grey-boxes so the
/// transitions between traversal modes stay the focus.
pub const BODY_HALF: Vec2 = Vec2::new(18.0, 34.0);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LadderId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LedgeId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GrappleId(pub u16);

/// The discrete traversal mode a body is currently in. Climbing is modelled as
/// authored, explicit modes rather than detecting arbitrary climbable surfaces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClimbMode {
    Free {
        grounded: bool,
    },
    Ladder {
        ladder: LadderId,
    },
    LedgeHang {
        ledge: LedgeId,
        hand_x: f32,
    },
    Grapple {
        from: GrappleId,
        to: GrappleId,
        t: f32,
    },
}

impl ClimbMode {
    pub fn label(self) -> &'static str {
        match self {
            ClimbMode::Free { grounded: true } => "FREE · GROUNDED",
            ClimbMode::Free { grounded: false } => "FREE · AIRBORNE",
            ClimbMode::Ladder { .. } => "LADDER",
            ClimbMode::LedgeHang { .. } => "LEDGE-HANG",
            ClimbMode::Grapple { .. } => "GRAPPLE",
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct ClimbBody {
    pub position: Vec2,
    pub velocity: Vec2,
    pub half_size: Vec2,
    pub mode: ClimbMode,
    pub spawn_position: Vec2,
    pub spawn_mode: ClimbMode,
    pub respawns: u32,
}

impl ClimbBody {
    pub fn new(spawn_position: Vec2, spawn_mode: ClimbMode) -> Self {
        Self {
            position: spawn_position,
            velocity: Vec2::ZERO,
            half_size: BODY_HALF,
            mode: spawn_mode,
            spawn_position,
            spawn_mode,
            respawns: 0,
        }
    }

    /// Restore the authored start state without touching the respawn counter.
    pub fn reset(&mut self) {
        self.position = self.spawn_position;
        self.velocity = Vec2::ZERO;
        self.mode = self.spawn_mode;
    }

    /// Return to the authored start state after leaving the world bounds.
    pub fn respawn(&mut self) {
        self.respawns += 1;
        self.position = self.spawn_position;
        self.velocity = Vec2::ZERO;
        self.mode = self.spawn_mode;
    }

    pub fn grounded(self) -> bool {
        matches!(self.mode, ClimbMode::Free { grounded: true })
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct ClimbConfig {
    pub move_speed: f32,
    pub ground_acceleration: f32,
    pub air_acceleration: f32,
    pub gravity: f32,
    pub max_fall_speed: f32,
    pub jump_speed: f32,
    pub climb_speed: f32,
    pub shimmy_speed: f32,
    pub grapple_speed: f32,
    pub grab_reach: f32,
    pub socket_radius: f32,
    pub detach_push: f32,
}

impl Default for ClimbConfig {
    fn default() -> Self {
        Self {
            move_speed: 210.0,
            ground_acceleration: 1400.0,
            air_acceleration: 620.0,
            gravity: 1300.0,
            max_fall_speed: 720.0,
            jump_speed: 470.0,
            climb_speed: 165.0,
            shimmy_speed: 130.0,
            grapple_speed: 205.0,
            grab_reach: 26.0,
            socket_radius: 52.0,
            detach_push: 150.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ClimbSolid {
    pub center: Vec2,
    pub half_size: Vec2,
}

impl ClimbSolid {
    pub fn min(self) -> Vec2 {
        self.center - self.half_size
    }

    pub fn max(self) -> Vec2 {
        self.center + self.half_size
    }
}

/// An authored vertical ladder spanning `bottom_y..top_y`. The top aligns with a
/// platform surface so a climber steps off cleanly.
#[derive(Clone, Copy, Debug)]
pub struct Ladder {
    pub id: LadderId,
    pub center_x: f32,
    pub half_width: f32,
    pub bottom_y: f32,
    pub top_y: f32,
}

/// An authored grabbable horizontal edge (the lip of a floating platform). A
/// body hangs from `edge_y` and can pull up onto it, drop, or shimmy along
/// `x_min..x_max`.
#[derive(Clone, Copy, Debug)]
pub struct Ledge {
    pub id: LedgeId,
    pub edge_y: f32,
    pub x_min: f32,
    pub x_max: f32,
}

/// An authored grapple anchor. Traversal is a straight point-to-point move to
/// `target`; there is no rope simulation.
#[derive(Clone, Copy, Debug)]
pub struct GrappleSocket {
    pub id: GrappleId,
    pub position: Vec2,
    pub target: Option<GrappleId>,
}

#[derive(Resource, Clone, Debug)]
pub struct ClimbWorld {
    pub solids: Vec<ClimbSolid>,
    pub ladders: Vec<Ladder>,
    pub ledges: Vec<Ledge>,
    pub sockets: Vec<GrappleSocket>,
    pub bounds_min: Vec2,
    pub bounds_max: Vec2,
}

impl ClimbWorld {
    pub fn authored_course() -> Self {
        Self {
            solids: vec![
                // Left tower: ground, ladder-top platform.
                solid(-600.0, -300.0, 180.0, 30.0),
                solid(-600.0, -40.0, 150.0, 16.0),
                // Centre: base floor and the floating ledge bar.
                solid(-180.0, -300.0, 150.0, 30.0),
                solid(-180.0, -60.0, 110.0, 12.0),
                // Right: base floor, grapple destination platform, launch pad.
                solid(320.0, -300.0, 170.0, 30.0),
                solid(560.0, -40.0, 150.0, 16.0),
                solid(140.0, -150.0, 80.0, 14.0),
            ],
            ladders: vec![Ladder {
                id: LadderId(0),
                center_x: -600.0,
                half_width: 22.0,
                bottom_y: -270.0,
                top_y: -24.0,
            }],
            ledges: vec![Ledge {
                id: LedgeId(0),
                edge_y: -48.0,
                x_min: -280.0,
                x_max: -80.0,
            }],
            sockets: vec![
                GrappleSocket {
                    id: GrappleId(0),
                    position: Vec2::new(180.0, -60.0),
                    target: Some(GrappleId(1)),
                },
                GrappleSocket {
                    id: GrappleId(1),
                    position: Vec2::new(520.0, 30.0),
                    target: None,
                },
            ],
            bounds_min: Vec2::new(-1200.0, -700.0),
            bounds_max: Vec2::new(1200.0, 700.0),
        }
    }

    pub fn ladder(&self, id: LadderId) -> Option<&Ladder> {
        self.ladders.iter().find(|ladder| ladder.id == id)
    }

    pub fn ledge(&self, id: LedgeId) -> Option<&Ledge> {
        self.ledges.iter().find(|ledge| ledge.id == id)
    }

    pub fn socket(&self, id: GrappleId) -> Option<&GrappleSocket> {
        self.sockets.iter().find(|socket| socket.id == id)
    }
}

fn solid(cx: f32, cy: f32, hx: f32, hy: f32) -> ClimbSolid {
    ClimbSolid {
        center: Vec2::new(cx, cy),
        half_size: Vec2::new(hx, hy),
    }
}

/// Transitions observed during a single step, used for counters, debug text,
/// and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClimbStep {
    pub grabbed_ladder: bool,
    pub left_ladder: bool,
    pub grabbed_ledge: bool,
    pub pulled_up: bool,
    pub dropped: bool,
    pub started_grapple: bool,
    pub finished_grapple: bool,
    pub respawned: bool,
}

pub fn step_body(
    body: &mut ClimbBody,
    intent: PlayerIntent,
    world: &ClimbWorld,
    config: ClimbConfig,
    dt: f32,
) -> ClimbStep {
    let mut report = ClimbStep::default();

    if out_of_bounds(body.position, world) {
        body.respawn();
        report.respawned = true;
        return report;
    }

    match body.mode {
        ClimbMode::Free { .. } => step_free(body, intent, world, config, dt, &mut report),
        ClimbMode::Ladder { .. } => step_ladder(body, intent, world, config, dt, &mut report),
        ClimbMode::LedgeHang { .. } => step_hang(body, intent, world, config, dt, &mut report),
        ClimbMode::Grapple { .. } => step_grapple(body, world, config, dt, &mut report),
    }

    if out_of_bounds(body.position, world) {
        body.respawn();
        report.respawned = true;
    }
    report
}

fn step_free(
    body: &mut ClimbBody,
    intent: PlayerIntent,
    world: &ClimbWorld,
    config: ClimbConfig,
    dt: f32,
    report: &mut ClimbStep,
) {
    let grounded = body.grounded();
    let target_vx = intent.movement.x * config.move_speed;
    let acceleration = if grounded {
        config.ground_acceleration
    } else {
        config.air_acceleration
    };
    body.velocity.x = approach(body.velocity.x, target_vx, acceleration * dt);

    if grounded && intent.jump_pressed {
        body.velocity.y = config.jump_speed;
    } else if grounded {
        body.velocity.y = 0.0;
    } else {
        body.velocity.y = (body.velocity.y - config.gravity * dt).max(-config.max_fall_speed);
    }

    let prev_hand_top = body.position.y + body.half_size.y;
    let prev_feet = body.position.y - body.half_size.y;
    body.position.x += body.velocity.x * dt;
    body.position.y += body.velocity.y * dt;
    let new_hand_top = body.position.y + body.half_size.y;
    let new_feet = body.position.y - body.half_size.y;

    // 1. Grab an authored ledge if the hands sweep across its edge.
    for ledge in &world.ledges {
        let within_x = body.position.x >= ledge.x_min - config.grab_reach
            && body.position.x <= ledge.x_max + config.grab_reach;
        let crossed = crosses(prev_hand_top, new_hand_top, ledge.edge_y);
        if within_x && crossed {
            let hand_x = body.position.x.clamp(ledge.x_min, ledge.x_max);
            attach_hang(body, ledge, hand_x);
            report.grabbed_ledge = true;
            return;
        }
    }

    // 2. Land on the top of a solid.
    let mut now_grounded = false;
    for solid in &world.solids {
        let top = solid.max().y;
        let overlap_x = body.position.x + body.half_size.x > solid.min().x
            && body.position.x - body.half_size.x < solid.max().x;
        if overlap_x && body.velocity.y <= 0.0 && prev_feet >= top - 0.5 && new_feet <= top {
            body.position.y = top + body.half_size.y;
            body.velocity.y = 0.0;
            now_grounded = true;
            break;
        }
    }
    if !now_grounded && grounded && supported(body, world) {
        body.velocity.y = 0.0;
        now_grounded = true;
    }
    body.mode = ClimbMode::Free {
        grounded: now_grounded,
    };

    // 3. Attach to an overlapping ladder when the player asks to climb.
    if (intent.climb_pressed || intent.movement.y.abs() > 0.5)
        && let Some(ladder) = overlapping_ladder(body, world)
    {
        body.position.x = ladder.center_x;
        body.velocity = Vec2::ZERO;
        body.mode = ClimbMode::Ladder { ladder: ladder.id };
        report.grabbed_ladder = true;
        return;
    }

    // 4. Launch from a grapple socket that has an authored target.
    if intent.climb_pressed
        && let Some(socket) = nearest_socket_with_target(body, world, config)
        && let Some(to) = socket.target
    {
        body.velocity = Vec2::ZERO;
        body.mode = ClimbMode::Grapple {
            from: socket.id,
            to,
            t: 0.0,
        };
        report.started_grapple = true;
    }
}

fn step_ladder(
    body: &mut ClimbBody,
    intent: PlayerIntent,
    world: &ClimbWorld,
    config: ClimbConfig,
    dt: f32,
    report: &mut ClimbStep,
) {
    let ClimbMode::Ladder { ladder: ladder_id } = body.mode else {
        return;
    };
    let Some(ladder) = world.ladder(ladder_id).copied() else {
        body.mode = ClimbMode::Free { grounded: false };
        return;
    };

    if intent.jump_pressed {
        body.velocity = Vec2::new(
            intent.movement.x * config.detach_push,
            config.jump_speed * 0.6,
        );
        body.mode = ClimbMode::Free { grounded: false };
        report.left_ladder = true;
        return;
    }

    body.position.x = ladder.center_x;
    body.velocity = Vec2::new(0.0, intent.movement.y * config.climb_speed);
    body.position.y += body.velocity.y * dt;

    let feet = body.position.y - body.half_size.y;
    if feet >= ladder.top_y {
        body.position.y = ladder.top_y + body.half_size.y;
        body.velocity = Vec2::ZERO;
        body.mode = ClimbMode::Free { grounded: true };
        report.left_ladder = true;
    } else if feet <= ladder.bottom_y {
        body.position.y = ladder.bottom_y + body.half_size.y;
        body.velocity = Vec2::ZERO;
        body.mode = ClimbMode::Free { grounded: true };
        report.left_ladder = true;
    }
}

fn step_hang(
    body: &mut ClimbBody,
    intent: PlayerIntent,
    world: &ClimbWorld,
    config: ClimbConfig,
    dt: f32,
    report: &mut ClimbStep,
) {
    let ClimbMode::LedgeHang {
        ledge: ledge_id,
        hand_x,
    } = body.mode
    else {
        return;
    };
    let Some(ledge) = world.ledge(ledge_id).copied() else {
        body.mode = ClimbMode::Free { grounded: false };
        return;
    };

    if intent.movement.y > 0.5 || intent.climb_pressed {
        body.position.x = hand_x.clamp(ledge.x_min, ledge.x_max);
        body.position.y = ledge.edge_y + body.half_size.y;
        body.velocity = Vec2::ZERO;
        body.mode = ClimbMode::Free { grounded: true };
        report.pulled_up = true;
        return;
    }

    if intent.movement.y < -0.5 || intent.jump_pressed {
        body.velocity = Vec2::new(0.0, -config.detach_push * 0.4);
        body.mode = ClimbMode::Free { grounded: false };
        report.dropped = true;
        return;
    }

    let new_hand =
        (hand_x + intent.movement.x * config.shimmy_speed * dt).clamp(ledge.x_min, ledge.x_max);
    body.position.x = new_hand;
    body.position.y = ledge.edge_y - body.half_size.y;
    body.velocity = Vec2::ZERO;
    body.mode = ClimbMode::LedgeHang {
        ledge: ledge_id,
        hand_x: new_hand,
    };
}

fn step_grapple(
    body: &mut ClimbBody,
    world: &ClimbWorld,
    config: ClimbConfig,
    dt: f32,
    report: &mut ClimbStep,
) {
    let ClimbMode::Grapple { from, to, t } = body.mode else {
        return;
    };
    let (Some(start), Some(end)) = (world.socket(from), world.socket(to)) else {
        body.mode = ClimbMode::Free { grounded: false };
        return;
    };

    let distance = start.position.distance(end.position).max(1.0);
    let new_t = (t + config.grapple_speed * dt / distance).min(1.0);
    body.position = start.position.lerp(end.position, new_t);
    body.velocity = (end.position - start.position).normalize_or_zero() * config.grapple_speed;

    if new_t >= 1.0 {
        body.position = end.position;
        body.velocity = Vec2::ZERO;
        body.mode = ClimbMode::Free { grounded: false };
        report.finished_grapple = true;
    } else {
        body.mode = ClimbMode::Grapple { from, to, t: new_t };
    }
}

fn attach_hang(body: &mut ClimbBody, ledge: &Ledge, hand_x: f32) {
    body.position.x = hand_x;
    body.position.y = ledge.edge_y - body.half_size.y;
    body.velocity = Vec2::ZERO;
    body.mode = ClimbMode::LedgeHang {
        ledge: ledge.id,
        hand_x,
    };
}

fn overlapping_ladder<'a>(body: &ClimbBody, world: &'a ClimbWorld) -> Option<&'a Ladder> {
    world.ladders.iter().find(|ladder| {
        (body.position.x - ladder.center_x).abs() <= ladder.half_width + body.half_size.x
            && body.position.y + body.half_size.y > ladder.bottom_y
            && body.position.y - body.half_size.y < ladder.top_y
    })
}

fn nearest_socket_with_target<'a>(
    body: &ClimbBody,
    world: &'a ClimbWorld,
    config: ClimbConfig,
) -> Option<&'a GrappleSocket> {
    world
        .sockets
        .iter()
        .filter(|socket| socket.target.is_some())
        .filter(|socket| socket.position.distance(body.position) <= config.socket_radius)
        .min_by(|a, b| {
            a.position
                .distance(body.position)
                .total_cmp(&b.position.distance(body.position))
        })
}

fn supported(body: &ClimbBody, world: &ClimbWorld) -> bool {
    let feet = body.position.y - body.half_size.y;
    world.solids.iter().any(|solid| {
        let overlap_x = body.position.x + body.half_size.x > solid.min().x
            && body.position.x - body.half_size.x < solid.max().x;
        overlap_x && (feet - solid.max().y).abs() < 1.5
    })
}

fn crosses(previous: f32, current: f32, threshold: f32) -> bool {
    (previous - threshold).signum() != (current - threshold).signum()
        || (current - threshold).abs() < 1.0
}

fn out_of_bounds(position: Vec2, world: &ClimbWorld) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn test_world() -> ClimbWorld {
        ClimbWorld {
            solids: vec![
                solid(0.0, -300.0, 300.0, 30.0), // wide ground, top at -270
                solid(0.0, 40.0, 120.0, 10.0),   // floating bar, top at 50
            ],
            ladders: vec![Ladder {
                id: LadderId(0),
                center_x: 0.0,
                half_width: 20.0,
                bottom_y: -270.0,
                top_y: 50.0,
            }],
            ledges: vec![Ledge {
                id: LedgeId(0),
                edge_y: 50.0,
                x_min: -120.0,
                x_max: 120.0,
            }],
            sockets: vec![
                GrappleSocket {
                    id: GrappleId(0),
                    position: Vec2::new(-100.0, 0.0),
                    target: Some(GrappleId(1)),
                },
                GrappleSocket {
                    id: GrappleId(1),
                    position: Vec2::new(300.0, 0.0),
                    target: None,
                },
            ],
            bounds_min: Vec2::new(-1000.0, -1000.0),
            bounds_max: Vec2::new(1000.0, 1000.0),
        }
    }

    fn free_body(x: f32, y: f32, grounded: bool) -> ClimbBody {
        ClimbBody::new(Vec2::new(x, y), ClimbMode::Free { grounded })
    }

    #[test]
    fn airborne_body_falls_and_lands_grounded() {
        let world = test_world();
        let config = ClimbConfig::default();
        let mut body = free_body(0.0, -150.0, false);

        let mut landed = false;
        for _ in 0..240 {
            step_body(&mut body, PlayerIntent::default(), &world, config, DT);
            if body.grounded() {
                landed = true;
                break;
            }
        }

        assert!(landed);
        assert!((body.position.y - (-270.0 + BODY_HALF.y)).abs() < 0.5);
        assert_eq!(body.velocity.y, 0.0);
    }

    #[test]
    fn walking_off_a_platform_becomes_airborne() {
        let world = test_world();
        let config = ClimbConfig::default();
        let mut body = free_body(280.0, -270.0 + BODY_HALF.y, true);

        for _ in 0..60 {
            step_body(
                &mut body,
                PlayerIntent {
                    movement: Vec2::X,
                    ..default()
                },
                &world,
                config,
                DT,
            );
        }

        assert!(!body.grounded());
        assert!(body.position.x > 300.0);
    }

    #[test]
    fn jump_leaves_the_ground() {
        let world = test_world();
        let config = ClimbConfig::default();
        let mut body = free_body(0.0, -270.0 + BODY_HALF.y, true);

        step_body(
            &mut body,
            PlayerIntent {
                jump_pressed: true,
                ..default()
            },
            &world,
            config,
            DT,
        );

        assert!(!body.grounded());
        assert!(body.velocity.y > 0.0);
    }

    #[test]
    fn climb_intent_attaches_to_ladder_and_reaches_the_top() {
        let world = test_world();
        let config = ClimbConfig::default();
        let mut body = free_body(0.0, -270.0 + BODY_HALF.y, true);

        let report = step_body(
            &mut body,
            PlayerIntent {
                climb_pressed: true,
                ..default()
            },
            &world,
            config,
            DT,
        );
        assert!(report.grabbed_ladder);
        assert!(matches!(body.mode, ClimbMode::Ladder { .. }));

        let mut topped = false;
        for _ in 0..600 {
            let report = step_body(
                &mut body,
                PlayerIntent {
                    movement: Vec2::Y,
                    ..default()
                },
                &world,
                config,
                DT,
            );
            if report.left_ladder {
                topped = true;
                break;
            }
        }
        assert!(topped);
        assert!(body.grounded());
        assert!((body.position.y - (50.0 + BODY_HALF.y)).abs() < 0.5);
    }

    #[test]
    fn jumping_off_a_ladder_returns_to_free_movement() {
        let world = test_world();
        let config = ClimbConfig::default();
        let mut body = ClimbBody::new(
            Vec2::new(0.0, -100.0),
            ClimbMode::Ladder {
                ladder: LadderId(0),
            },
        );

        let report = step_body(
            &mut body,
            PlayerIntent {
                jump_pressed: true,
                ..default()
            },
            &world,
            config,
            DT,
        );

        assert!(report.left_ladder);
        assert!(matches!(body.mode, ClimbMode::Free { grounded: false }));
        assert!(body.velocity.y > 0.0);
    }

    #[test]
    fn rising_into_a_ledge_grabs_and_hangs() {
        let world = test_world();
        let config = ClimbConfig::default();
        // Start below the bar with enough upward velocity for the hands to sweep
        // past the edge before gravity turns the jump around.
        let mut body = free_body(0.0, 50.0 - 2.0 * BODY_HALF.y, false);
        body.velocity.y = 380.0;

        let mut grabbed = false;
        for _ in 0..30 {
            let report = step_body(&mut body, PlayerIntent::default(), &world, config, DT);
            if report.grabbed_ledge {
                grabbed = true;
                break;
            }
        }

        assert!(grabbed);
        assert!(matches!(body.mode, ClimbMode::LedgeHang { .. }));
        assert!((body.position.y + body.half_size.y - 50.0).abs() < 0.5);
    }

    #[test]
    fn hang_pulls_up_onto_the_surface() {
        let world = test_world();
        let config = ClimbConfig::default();
        let mut body = ClimbBody::new(
            Vec2::new(0.0, 50.0 - BODY_HALF.y),
            ClimbMode::LedgeHang {
                ledge: LedgeId(0),
                hand_x: 0.0,
            },
        );

        let report = step_body(
            &mut body,
            PlayerIntent {
                movement: Vec2::Y,
                ..default()
            },
            &world,
            config,
            DT,
        );

        assert!(report.pulled_up);
        assert!(body.grounded());
        assert!((body.position.y - (50.0 + BODY_HALF.y)).abs() < 0.5);
    }

    #[test]
    fn hang_drops_back_into_free_fall() {
        let world = test_world();
        let config = ClimbConfig::default();
        let mut body = ClimbBody::new(
            Vec2::new(0.0, 50.0 - BODY_HALF.y),
            ClimbMode::LedgeHang {
                ledge: LedgeId(0),
                hand_x: 0.0,
            },
        );

        let report = step_body(
            &mut body,
            PlayerIntent {
                movement: Vec2::new(0.0, -1.0),
                ..default()
            },
            &world,
            config,
            DT,
        );

        assert!(report.dropped);
        assert!(matches!(body.mode, ClimbMode::Free { grounded: false }));
        assert!(body.velocity.y < 0.0);
    }

    #[test]
    fn shimmy_moves_along_the_edge_and_clamps_at_the_end() {
        let world = test_world();
        let config = ClimbConfig::default();
        let mut body = ClimbBody::new(
            Vec2::new(0.0, 50.0 - BODY_HALF.y),
            ClimbMode::LedgeHang {
                ledge: LedgeId(0),
                hand_x: 0.0,
            },
        );

        for _ in 0..600 {
            step_body(
                &mut body,
                PlayerIntent {
                    movement: Vec2::X,
                    ..default()
                },
                &world,
                config,
                DT,
            );
        }

        let ClimbMode::LedgeHang { hand_x, .. } = body.mode else {
            panic!("expected to remain hanging");
        };
        assert!((hand_x - 120.0).abs() < 0.5);
        assert!((body.position.x - 120.0).abs() < 0.5);
    }

    #[test]
    fn grapple_traverses_from_socket_to_socket() {
        let world = test_world();
        let config = ClimbConfig::default();
        let mut body = free_body(-100.0, 0.0, false);

        let report = step_body(
            &mut body,
            PlayerIntent {
                climb_pressed: true,
                ..default()
            },
            &world,
            config,
            DT,
        );
        assert!(report.started_grapple);
        assert!(matches!(body.mode, ClimbMode::Grapple { .. }));

        let mut finished = false;
        for _ in 0..600 {
            let report = step_body(&mut body, PlayerIntent::default(), &world, config, DT);
            if report.finished_grapple {
                finished = true;
                break;
            }
        }
        assert!(finished);
        assert!((body.position - Vec2::new(300.0, 0.0)).length() < 0.5);
    }

    #[test]
    fn leaving_bounds_respawns_to_authored_state() {
        let world = test_world();
        let config = ClimbConfig::default();
        let mut body = free_body(0.0, -270.0 + BODY_HALF.y, true);
        body.position = Vec2::new(0.0, -2000.0);

        let report = step_body(&mut body, PlayerIntent::default(), &world, config, DT);

        assert!(report.respawned);
        assert_eq!(body.position, body.spawn_position);
        assert!(body.grounded());
        assert_eq!(body.respawns, 1);
    }
}
