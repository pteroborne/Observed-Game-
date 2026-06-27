use bevy::{ecs::system::SystemParam, prelude::*};
use player_input::{PlayerId, PlayerIntent};

use crate::{
    input::HumanInputBuffer,
    simulation::{
        GroundContact, MovementBody, MovementConfig, MovementStep, MovementWorld, PlatformId,
        SupportId, step_body,
    },
};

pub const PLAYER_COUNT: usize = 4;
pub const PLAYERS: [PlayerId; PLAYER_COUNT] = [PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)];

const PLAYER_COLORS: [Color; PLAYER_COUNT] = [
    Color::srgb(0.20, 0.85, 1.0),
    Color::srgb(1.0, 0.38, 0.30),
    Color::srgb(0.55, 0.95, 0.28),
    Color::srgb(0.82, 0.42, 1.0),
];

#[derive(Component)]
pub(crate) struct MovementLabOwned;

#[derive(Component)]
pub(crate) struct MovementLabUiRoot;

#[derive(Component)]
pub(crate) struct PlayerVisual;

#[derive(Component)]
pub(crate) struct PlatformVisual(PlatformId);

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct MovementLabRuntime {
    pub selected_player: PlayerId,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    pub checkpoint_index: usize,
    pub last_event: String,
}

impl Default for MovementLabRuntime {
    fn default() -> Self {
        Self {
            selected_player: PlayerId(0),
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
            checkpoint_index: 0,
            last_event: "Ready on the flat acceleration strip.".to_string(),
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct MovementCounters {
    pub jumps: u32,
    pub landings: u32,
    pub step_ups: u32,
    pub respawns: u32,
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, MovementLabRuntime>,
    counters: Res<'w, MovementCounters>,
    world: Res<'w, MovementWorld>,
    players: Query<
        'w,
        's,
        (
            &'static PlayerId,
            &'static MovementBody,
            &'static PlayerIntent,
        ),
    >,
    owned: Query<'w, 's, (), With<MovementLabOwned>>,
    ui_roots: Query<'w, 's, (), With<MovementLabUiRoot>>,
    text: Single<'w, 's, &'static mut Text, With<DebugText>>,
    panel: Single<'w, 's, &'static mut Visibility, (With<DebugPanel>, Without<HelpText>)>,
    help: Single<'w, 's, &'static mut Visibility, (With<HelpText>, Without<DebugPanel>)>,
}

pub(crate) fn setup_lab(mut commands: Commands, world: Res<MovementWorld>) {
    spawn_course(&mut commands, &world);
    spawn_players(&mut commands);
    spawn_ui(&mut commands);
}

fn spawn_course(commands: &mut Commands, world: &MovementWorld) {
    commands
        .spawn((
            MovementLabOwned,
            Name::new("Movement Course Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (index, segment) in world.segments.iter().enumerate() {
                let delta = segment.end - segment.start;
                let length = delta.length();
                let center = (segment.start + segment.end) * 0.5 - Vec2::Y * 7.0;
                parent.spawn((
                    Name::new(format!("Surface Segment {index}")),
                    Sprite::from_color(Color::srgb(0.12, 0.22, 0.31), Vec2::new(length, 14.0)),
                    Transform {
                        translation: center.extend(-1.0),
                        rotation: Quat::from_rotation_z(delta.y.atan2(delta.x)),
                        ..default()
                    },
                ));
            }

            for (index, solid) in world.solids.iter().enumerate() {
                parent.spawn((
                    Name::new(format!("Stair Block {index}")),
                    Sprite::from_color(Color::srgb(0.17, 0.29, 0.38), solid.half_size * 2.0),
                    Transform::from_translation(solid.center.extend(-1.0)),
                ));
            }

            for platform in &world.platforms {
                parent.spawn((
                    PlatformVisual(platform.id),
                    Name::new("Moving Platform"),
                    Sprite::from_color(Color::srgb(0.95, 0.62, 0.18), platform.half_size * 2.0),
                    Transform::from_translation(platform.center.extend(0.0)),
                ));
            }

            parent.spawn((
                Name::new("Kill Plane Marker"),
                Sprite::from_color(Color::srgba(1.0, 0.18, 0.16, 0.35), Vec2::new(2080.0, 4.0)),
                Transform::from_xyz(0.0, world.bounds_min.y, -1.0),
            ));
        });
}

fn spawn_players(commands: &mut Commands) {
    let spawns = [
        Vec2::new(-820.0, -220.0),
        Vec2::new(-720.0, -220.0),
        Vec2::new(-300.0, -80.0),
        Vec2::new(690.0, 20.0),
    ];
    commands
        .spawn((
            MovementLabOwned,
            Name::new("Movement Players Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (index, player) in PLAYERS.into_iter().enumerate() {
                parent.spawn((
                    player,
                    PlayerIntent::default(),
                    MovementBody::new(spawns[index]),
                    PlayerVisual,
                    Name::new(format!("{} Movement Body", player.label())),
                    Sprite::from_color(PLAYER_COLORS[index], Vec2::new(36.0, 68.0)),
                    Transform::from_translation(spawns[index].extend(2.0)),
                ));
            }
        });
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            MovementLabOwned,
            MovementLabUiRoot,
            Name::new("Movement Lab UI Root"),
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
                    width: px(430),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.25, 0.75, 0.95, 0.65)),
                children![(
                    DebugText,
                    Text::new("Movement diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.72, 0.90, 1.0)),
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
                BorderColor::all(Color::srgba(0.25, 0.75, 0.95, 0.65)),
                children![(
                    Text::new(
                        "MOVEMENT LAB\n\
                         A / D      Walk\n\
                         Shift      Run\n\
                         Space      Jump\n\
                         1–4        Select human-controlled body\n\
                         T          Cycle test checkpoint\n\
                         R          Reset all bodies and platform\n\
                         F1         Toggle debug overlay\n\n\
                         Course: acceleration strip → slope → stairs →\n\
                         upper ledge → moving platform → respawn gap",
                    ),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.82, 0.91, 1.0)),
                )],
            ));
        });
}

pub(crate) fn handle_lab_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<MovementLabRuntime>,
    mut players: Query<(&PlayerId, &mut MovementBody)>,
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
    if keyboard.just_pressed(KeyCode::KeyT) {
        let checkpoints = [
            Vec2::new(-820.0, -220.0),
            Vec2::new(-570.0, -190.0),
            Vec2::new(-175.0, -78.0),
            Vec2::new(250.0, 22.0),
            Vec2::new(445.0, -70.0),
            Vec2::new(445.0, 220.0),
        ];
        runtime.checkpoint_index = (runtime.checkpoint_index + 1) % checkpoints.len();
        let target = checkpoints[runtime.checkpoint_index];
        for (player, mut body) in &mut players {
            if *player == runtime.selected_player {
                body.position = target;
                body.velocity = Vec2::ZERO;
                body.grounded = false;
                body.contact = None;
                runtime.last_event = format!(
                    "{} teleported to checkpoint {}.",
                    player.label(),
                    runtime.checkpoint_index
                );
            }
        }
    }
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<MovementLabRuntime>,
    mut counters: ResMut<MovementCounters>,
    mut world: ResMut<MovementWorld>,
    mut input: ResMut<HumanInputBuffer>,
    mut players: Query<(&mut MovementBody, &mut PlayerIntent)>,
) {
    if !runtime.reset_requested {
        return;
    }

    runtime.reset_requested = false;
    runtime.reset_count += 1;
    runtime.checkpoint_index = 0;
    runtime.last_event = format!(
        "Reset {} restored all authored start states.",
        runtime.reset_count
    );
    *counters = MovementCounters::default();
    *input = HumanInputBuffer::default();
    world.reset_platforms();
    for (mut body, mut intent) in &mut players {
        body.reset();
        *intent = PlayerIntent::default();
    }
}

pub(crate) fn advance_platforms(time: Res<Time<Fixed>>, mut world: ResMut<MovementWorld>) {
    world.advance_platforms(time.delta_secs());
}

pub(crate) fn simulate_players(
    time: Res<Time<Fixed>>,
    config: Res<MovementConfig>,
    world: Res<MovementWorld>,
    mut counters: ResMut<MovementCounters>,
    mut players: Query<(&PlayerIntent, &mut MovementBody)>,
) {
    for (intent, mut body) in &mut players {
        let report = step_body(&mut body, *intent, &world, *config, time.delta_secs());
        count_report(&mut counters, report);
    }
}

fn count_report(counters: &mut MovementCounters, report: MovementStep) {
    counters.jumps += u32::from(report.jumped);
    counters.landings += u32::from(report.landed);
    counters.step_ups += u32::from(report.stepped_up);
    counters.respawns += u32::from(report.respawned);
}

pub(crate) fn update_platform_visuals(
    world: Res<MovementWorld>,
    mut platforms: Query<(&PlatformVisual, &mut Transform)>,
) {
    for (visual, mut transform) in &mut platforms {
        if let Some(platform) = world.platform(visual.0) {
            transform.translation.x = platform.center.x;
            transform.translation.y = platform.center.y;
        }
    }
}

pub(crate) fn present_players(
    runtime: Res<MovementLabRuntime>,
    mut players: Query<(&PlayerId, &MovementBody, &mut Transform, &mut Sprite), With<PlayerVisual>>,
) {
    for (player, body, mut transform, mut sprite) in &mut players {
        transform.translation.x = body.position.x;
        transform.translation.y = body.position.y;
        transform.rotation = Quat::IDENTITY;
        sprite.color = if *player == runtime.selected_player {
            if body.grounded {
                Color::srgb(0.35, 1.0, 0.62)
            } else {
                Color::srgb(0.95, 0.95, 1.0)
            }
        } else {
            PLAYER_COLORS[player.index()]
        };
    }
}

pub(crate) fn draw_debug(
    runtime: Res<MovementLabRuntime>,
    world: Res<MovementWorld>,
    players: Query<(&PlayerId, &MovementBody, &PlayerIntent)>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    for segment in &world.segments {
        gizmos.line_2d(segment.start, segment.end, Color::srgb(0.25, 0.85, 1.0));
    }
    for solid in &world.solids {
        gizmos.rect_2d(
            solid.center,
            solid.half_size * 2.0,
            Color::srgb(0.55, 0.75, 0.9),
        );
    }
    for platform in &world.platforms {
        gizmos.rect_2d(
            platform.center,
            platform.half_size * 2.0,
            Color::srgb(1.0, 0.72, 0.20),
        );
        gizmos.line_2d(
            platform.origin - platform.amplitude,
            platform.origin + platform.amplitude,
            Color::srgba(1.0, 0.72, 0.20, 0.5),
        );
    }

    for (player, body, intent) in &players {
        let selected = *player == runtime.selected_player;
        let color = if selected {
            Color::srgb(0.4, 1.0, 0.65)
        } else {
            Color::srgba(0.7, 0.8, 0.9, 0.45)
        };
        gizmos.rect_2d(body.position, body.half_size * 2.0, color);
        gizmos.line_2d(
            body.position,
            body.position + body.velocity * 0.18,
            Color::srgb(1.0, 0.35, 0.30),
        );
        gizmos.line_2d(
            body.position,
            body.position + intent.movement * 70.0,
            Color::srgb(0.25, 0.65, 1.0),
        );
        if let Some(contact) = body.contact {
            let feet = body.position - Vec2::Y * body.half_size.y;
            gizmos.line_2d(
                feet,
                feet + contact.normal * 55.0,
                Color::srgb(1.0, 0.92, 0.25),
            );
        }
    }
}

pub(crate) fn update_debug_text(mut context: DebugContext) {
    let debug_visibility = if context.runtime.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    **context.panel = debug_visibility;
    **context.help = debug_visibility;

    let selected = context
        .players
        .iter()
        .find(|(player, _, _)| **player == context.runtime.selected_player);
    let Some((player, body, intent)) = selected else {
        return;
    };
    let support = support_label(body.contact);
    let body_count = context.players.iter().count();
    let root_count = context.ui_roots.iter().count();
    let owned_count = context.owned.iter().count();
    let healthy = body_count == PLAYER_COUNT && root_count == 1;
    let mut text = context.text.into_inner();
    **text = format!(
        "MOVEMENT MONITOR  {}\n\
         selected       {} (human)\n\
         position       {:+7.1}, {:+7.1}\n\
         velocity       {:+7.1}, {:+7.1}\n\
         intent X       {:+.2}  sprint:{}  jump:{}\n\
         state          {}\n\
         support        {support}\n\
         coyote         {:>5.3}s\n\
         jump buffer    {:>5.3}s\n\
         body respawns  {}\n\
         platform Y     {:+7.1}\n\n\
         jumps:{} landings:{} steps:{} respawns:{}\n\
         bodies:{body_count} UI:{root_count} owned roots:{owned_count}\n\
         resets:{}  {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        player.label(),
        body.position.x,
        body.position.y,
        body.velocity.x,
        body.velocity.y,
        intent.movement.x,
        bit(intent.sprint_held),
        bit(intent.jump_pressed),
        if body.grounded {
            "GROUNDED"
        } else {
            "AIRBORNE"
        },
        body.coyote_remaining,
        body.jump_buffer_remaining,
        body.respawns,
        context
            .world
            .platforms
            .first()
            .map_or(0.0, |platform| platform.center.y),
        context.counters.jumps,
        context.counters.landings,
        context.counters.step_ups,
        context.counters.respawns,
        context.runtime.reset_count,
        if context.runtime.debug_visible {
            "debug ON"
        } else {
            "debug OFF"
        },
        context.runtime.last_event,
    );
}

fn support_label(contact: Option<GroundContact>) -> String {
    match contact.map(|contact| contact.support) {
        Some(SupportId::Segment(index)) => format!("surface {index}"),
        Some(SupportId::Solid(index)) => format!("stair {index}"),
        Some(SupportId::Platform(id)) => format!("moving platform {}", id.0),
        None => "none".to_string(),
    }
}

fn bit(value: bool) -> char {
    if value { '1' } else { '0' }
}
