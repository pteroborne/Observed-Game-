use bevy::{ecs::system::SystemParam, prelude::*};
use observed_core::{PlayerId, PlayerIntent};

use crate::{
    input::HumanInput,
    simulation::{
        ClimbBody, ClimbConfig, ClimbMode, ClimbStep, ClimbWorld, GrappleId, LadderId, step_body,
    },
};

pub const PLAYER_COUNT: usize = 4;

const PLAYER_COLORS: [Color; PLAYER_COUNT] = [
    Color::srgb(0.30, 0.85, 1.0),
    Color::srgb(1.0, 0.42, 0.34),
    Color::srgb(0.62, 1.0, 0.36),
    Color::srgb(0.86, 0.46, 1.0),
];

#[derive(Component)]
pub(crate) struct ClimbLabOwned;

#[derive(Component)]
pub(crate) struct ClimbLabUiRoot;

#[derive(Component)]
pub(crate) struct BodyVisual;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct ClimbLabRuntime {
    pub selected_player: PlayerId,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    pub last_event: String,
}

impl Default for ClimbLabRuntime {
    fn default() -> Self {
        Self {
            selected_player: PlayerId(0),
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
            last_event: "Showcase ready: ladder, ledge-hang, and grapple in view.".to_string(),
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct ClimbCounters {
    pub ladder_grabs: u32,
    pub ledge_grabs: u32,
    pub pull_ups: u32,
    pub drops: u32,
    pub grapples: u32,
    pub respawns: u32,
}

/// Authored start state: each body demonstrates a distinct traversal mode so the
/// lab is legible the instant it boots.
fn authored_spawns() -> [(PlayerId, Vec2, ClimbMode); PLAYER_COUNT] {
    [
        (
            PlayerId(0),
            Vec2::new(-600.0, -150.0),
            ClimbMode::Ladder {
                ladder: LadderId(0),
            },
        ),
        (
            PlayerId(1),
            Vec2::new(-180.0, -82.0),
            ClimbMode::LedgeHang {
                ledge: crate::simulation::LedgeId(0),
                hand_x: -180.0,
            },
        ),
        (
            PlayerId(2),
            Vec2::new(180.0, -60.0),
            ClimbMode::Grapple {
                from: GrappleId(0),
                to: GrappleId(1),
                t: 0.0,
            },
        ),
        (
            PlayerId(3),
            Vec2::new(320.0, -236.0),
            ClimbMode::Free { grounded: true },
        ),
    ]
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, ClimbLabRuntime>,
    counters: Res<'w, ClimbCounters>,
    bodies: Query<'w, 's, (&'static PlayerId, &'static ClimbBody, &'static PlayerIntent)>,
    owned: Query<'w, 's, (), With<ClimbLabOwned>>,
    ui_roots: Query<'w, 's, (), With<ClimbLabUiRoot>>,
    text: Single<'w, 's, &'static mut Text, With<DebugText>>,
    panel: Single<'w, 's, &'static mut Visibility, (With<DebugPanel>, Without<HelpText>)>,
    help: Single<'w, 's, &'static mut Visibility, (With<HelpText>, Without<DebugPanel>)>,
}

pub(crate) fn setup_lab(mut commands: Commands, world: Res<ClimbWorld>) {
    spawn_course(&mut commands, &world);
    spawn_bodies(&mut commands);
    spawn_ui(&mut commands);
}

fn spawn_course(commands: &mut Commands, world: &ClimbWorld) {
    commands
        .spawn((
            ClimbLabOwned,
            Name::new("Climbing Course Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (index, solid) in world.solids.iter().enumerate() {
                parent.spawn((
                    Name::new(format!("Solid {index}")),
                    Sprite::from_color(Color::srgb(0.16, 0.27, 0.36), solid.half_size * 2.0),
                    Transform::from_translation(solid.center.extend(-1.0)),
                ));
            }

            for ladder in &world.ladders {
                let height = ladder.top_y - ladder.bottom_y;
                let center = Vec2::new(ladder.center_x, (ladder.top_y + ladder.bottom_y) * 0.5);
                parent.spawn((
                    Name::new(format!("Ladder {}", ladder.id.0)),
                    Sprite::from_color(
                        Color::srgba(0.28, 0.85, 0.55, 0.28),
                        Vec2::new(ladder.half_width * 2.0, height),
                    ),
                    Transform::from_translation(center.extend(-0.5)),
                ));
            }

            for socket in &world.sockets {
                parent.spawn((
                    Name::new(format!("Grapple Socket {}", socket.id.0)),
                    Sprite::from_color(Color::srgb(1.0, 0.66, 0.22), Vec2::splat(16.0)),
                    Transform::from_translation(socket.position.extend(-0.5)),
                ));
            }

            parent.spawn((
                Name::new("Kill Plane Marker"),
                Sprite::from_color(Color::srgba(1.0, 0.18, 0.16, 0.35), Vec2::new(2400.0, 4.0)),
                Transform::from_xyz(0.0, world.bounds_min.y, -1.0),
            ));
        });
}

fn spawn_bodies(commands: &mut Commands) {
    commands
        .spawn((
            ClimbLabOwned,
            Name::new("Climbing Bodies Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (player, position, mode) in authored_spawns() {
                parent.spawn((
                    player,
                    PlayerIntent::default(),
                    ClimbBody::new(position, mode),
                    BodyVisual,
                    Name::new(format!("{} Climbing Body", player.label())),
                    Sprite::from_color(PLAYER_COLORS[player.index()], Vec2::new(36.0, 68.0)),
                    Transform::from_translation(position.extend(2.0)),
                ));
            }
        });
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            ClimbLabOwned,
            ClimbLabUiRoot,
            Name::new("Climbing Lab UI Root"),
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(16)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(20),
        ))
        .with_children(|root| {
            root.spawn((
                DebugPanel,
                Node {
                    width: px(440),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.30, 0.85, 0.62, 0.65)),
                children![(
                    DebugText,
                    Text::new("Climbing diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.75, 0.97, 0.85)),
                )],
            ));
            root.spawn((
                HelpText,
                Node {
                    width: px(420),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.30, 0.85, 0.62, 0.65)),
                children![(
                    Text::new(
                        "CLIMBING LAB\n\
                         A / D        Move\n\
                         W / S        Climb up / down · pull up / drop\n\
                         Space        Jump / detach\n\
                         Q            Grab ladder · launch grapple\n\
                         1–4          Select body\n\
                         R            Reset showcase\n\
                         F1           Toggle debug overlay\n\n\
                         P1 ladder · P2 ledge-hang · P3 grapple · P4 free\n\
                         Climbable markers are explicit: ladders, ledges,\n\
                         and grapple sockets only.",
                    ),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.84, 0.95, 0.90)),
                )],
            ));
        });
}

pub(crate) fn handle_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<ClimbLabRuntime>,
) {
    for (key, player) in [
        (KeyCode::Digit1, PlayerId(0)),
        (KeyCode::Digit2, PlayerId(1)),
        (KeyCode::Digit3, PlayerId(2)),
        (KeyCode::Digit4, PlayerId(3)),
    ] {
        if keyboard.just_pressed(key) {
            runtime.selected_player = player;
            runtime.last_event = format!("{} is now human-controlled.", player.label());
        }
    }

    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<ClimbLabRuntime>,
    mut counters: ResMut<ClimbCounters>,
    mut input: ResMut<HumanInput>,
    mut bodies: Query<(&mut ClimbBody, &mut PlayerIntent)>,
) {
    if !runtime.reset_requested {
        return;
    }

    runtime.reset_requested = false;
    runtime.reset_count += 1;
    runtime.last_event = format!(
        "Reset {} restored the authored showcase.",
        runtime.reset_count
    );
    *counters = ClimbCounters::default();
    *input = HumanInput::default();
    for (mut body, mut intent) in &mut bodies {
        body.reset();
        *intent = PlayerIntent::default();
    }
}

pub(crate) fn simulate_bodies(
    time: Res<Time<Fixed>>,
    config: Res<ClimbConfig>,
    world: Res<ClimbWorld>,
    mut counters: ResMut<ClimbCounters>,
    mut bodies: Query<(&PlayerIntent, &mut ClimbBody)>,
) {
    let dt = time.delta_secs();
    for (intent, mut body) in &mut bodies {
        let report = step_body(&mut body, *intent, &world, *config, dt);
        accumulate(&mut counters, report);
    }
}

fn accumulate(counters: &mut ClimbCounters, report: ClimbStep) {
    counters.ladder_grabs += u32::from(report.grabbed_ladder);
    counters.ledge_grabs += u32::from(report.grabbed_ledge);
    counters.pull_ups += u32::from(report.pulled_up);
    counters.drops += u32::from(report.dropped);
    counters.grapples += u32::from(report.finished_grapple);
    counters.respawns += u32::from(report.respawned);
}

pub(crate) fn present_bodies(
    runtime: Res<ClimbLabRuntime>,
    mut bodies: Query<(&PlayerId, &ClimbBody, &mut Transform, &mut Sprite), With<BodyVisual>>,
) {
    for (player, body, mut transform, mut sprite) in &mut bodies {
        transform.translation.x = body.position.x;
        transform.translation.y = body.position.y;
        let base = mode_color(body.mode);
        sprite.color = if *player == runtime.selected_player {
            base.mix(&Color::WHITE, 0.35)
        } else {
            base
        };
    }
}

fn mode_color(mode: ClimbMode) -> Color {
    match mode {
        ClimbMode::Free { grounded: true } => Color::srgb(0.40, 1.0, 0.62),
        ClimbMode::Free { grounded: false } => Color::srgb(0.86, 0.92, 1.0),
        ClimbMode::Ladder { .. } => Color::srgb(0.30, 0.85, 1.0),
        ClimbMode::LedgeHang { .. } => Color::srgb(0.96, 0.50, 1.0),
        ClimbMode::Grapple { .. } => Color::srgb(1.0, 0.68, 0.24),
    }
}

pub(crate) fn draw_debug(
    runtime: Res<ClimbLabRuntime>,
    world: Res<ClimbWorld>,
    bodies: Query<(&PlayerId, &ClimbBody, &PlayerIntent)>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    for solid in &world.solids {
        gizmos.rect_2d(
            solid.center,
            solid.half_size * 2.0,
            Color::srgb(0.45, 0.62, 0.78),
        );
    }
    for ladder in &world.ladders {
        let height = ladder.top_y - ladder.bottom_y;
        let center = Vec2::new(ladder.center_x, (ladder.top_y + ladder.bottom_y) * 0.5);
        gizmos.rect_2d(
            center,
            Vec2::new(ladder.half_width * 2.0, height),
            Color::srgb(0.32, 0.95, 0.60),
        );
        gizmos.line_2d(
            Vec2::new(ladder.center_x, ladder.bottom_y),
            Vec2::new(ladder.center_x, ladder.top_y),
            Color::srgb(0.32, 0.95, 0.60),
        );
    }
    for ledge in &world.ledges {
        gizmos.line_2d(
            Vec2::new(ledge.x_min, ledge.edge_y),
            Vec2::new(ledge.x_max, ledge.edge_y),
            Color::srgb(0.96, 0.45, 1.0),
        );
    }
    for socket in &world.sockets {
        gizmos.circle_2d(socket.position, 12.0, Color::srgb(1.0, 0.66, 0.22));
        if let Some(target) = socket.target.and_then(|id| world.socket(id)) {
            gizmos.line_2d(
                socket.position,
                target.position,
                Color::srgba(1.0, 0.66, 0.22, 0.55),
            );
        }
    }

    for (player, body, intent) in &bodies {
        let selected = *player == runtime.selected_player;
        let color = if selected {
            Color::srgb(0.55, 1.0, 0.78)
        } else {
            Color::srgba(0.7, 0.85, 0.95, 0.45)
        };
        gizmos.rect_2d(body.position, body.half_size * 2.0, color);
        gizmos.line_2d(
            body.position,
            body.position + body.velocity * 0.16,
            Color::srgb(1.0, 0.38, 0.32),
        );
        gizmos.line_2d(
            body.position,
            body.position + intent.movement * 60.0,
            Color::srgb(0.30, 0.70, 1.0),
        );
        draw_attach_point(&mut gizmos, &world, body);
    }
}

fn draw_attach_point(gizmos: &mut Gizmos, world: &ClimbWorld, body: &ClimbBody) {
    match body.mode {
        ClimbMode::LedgeHang { ledge, hand_x } => {
            if let Some(ledge) = world.ledge(ledge) {
                gizmos.circle_2d(
                    Vec2::new(hand_x, ledge.edge_y),
                    7.0,
                    Color::srgb(1.0, 0.92, 0.40),
                );
            }
        }
        ClimbMode::Grapple { from, to, .. } => {
            if let (Some(start), Some(end)) = (world.socket(from), world.socket(to)) {
                gizmos.line_2d(start.position, body.position, Color::srgb(1.0, 0.92, 0.40));
                gizmos.line_2d(
                    body.position,
                    end.position,
                    Color::srgba(1.0, 0.92, 0.40, 0.4),
                );
            }
        }
        ClimbMode::Ladder { ladder } => {
            if let Some(ladder) = world.ladder(ladder) {
                gizmos.circle_2d(
                    Vec2::new(ladder.center_x, body.position.y),
                    7.0,
                    Color::srgb(1.0, 0.92, 0.40),
                );
            }
        }
        ClimbMode::Free { .. } => {}
    }
}

pub(crate) fn update_debug_text(mut context: DebugContext) {
    let visibility = if context.runtime.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    **context.panel = visibility;
    **context.help = visibility;

    let selected = context
        .bodies
        .iter()
        .find(|(player, _, _)| **player == context.runtime.selected_player);
    let Some((player, body, intent)) = selected else {
        return;
    };

    let body_count = context.bodies.iter().count();
    let root_count = context.ui_roots.iter().count();
    let owned_count = context.owned.iter().count();
    let healthy = body_count == PLAYER_COUNT && root_count == 1;
    let mut text = context.text.into_inner();
    **text = format!(
        "CLIMBING MONITOR  {}\n\
         selected       {} (human)\n\
         mode           {}\n\
         position       {:+7.1}, {:+7.1}\n\
         velocity       {:+7.1}, {:+7.1}\n\
         intent         {:+.2}, {:+.2}  jump:{} climb:{}\n\
         respawns       {}\n\n\
         ladder grabs:{}  ledge grabs:{}\n\
         pull-ups:{}  drops:{}  grapples:{}  respawns:{}\n\
         bodies:{body_count} UI:{root_count} owned roots:{owned_count}\n\
         resets:{}  debug {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        player.label(),
        body.mode.label(),
        body.position.x,
        body.position.y,
        body.velocity.x,
        body.velocity.y,
        intent.movement.x,
        intent.movement.y,
        bit(intent.jump_pressed),
        bit(intent.climb_pressed),
        body.respawns,
        context.counters.ladder_grabs,
        context.counters.ledge_grabs,
        context.counters.pull_ups,
        context.counters.drops,
        context.counters.grapples,
        context.counters.respawns,
        context.runtime.reset_count,
        if context.runtime.debug_visible {
            "ON"
        } else {
            "OFF"
        },
        context.runtime.last_event,
    );
}

fn bit(value: bool) -> char {
    if value { '1' } else { '0' }
}
