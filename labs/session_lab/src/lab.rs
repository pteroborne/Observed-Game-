use bevy::{ecs::system::SystemParam, prelude::*};

use crate::model::{AccountId, ConnectionState, Region, SessionLabWorld, SessionPhase, TEAM_COUNT};

const ACCOUNT_COUNT: usize = 6;
const TEAM_X: [f32; TEAM_COUNT] = [-260.0, 260.0];
const TEAM_COLOR: [Color; TEAM_COUNT] =
    [Color::srgb(0.25, 0.82, 1.0), Color::srgb(1.0, 0.62, 0.25)];
const QUEUE_X: f32 = -570.0;

#[derive(Component)]
pub(crate) struct SessionOwned;

#[derive(Component)]
pub(crate) struct SessionUiRoot;

#[derive(Component)]
pub(crate) struct AccountDot(pub AccountId);

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Debug)]
pub struct SessionRuntime {
    pub selected: AccountId,
    pub auto_run: bool,
    pub step_requested: bool,
    pub toggle_ready_requested: bool,
    pub disconnect_requested: bool,
    pub reconnect_requested: bool,
    pub finish_requested: bool,
    pub reset_requested: bool,
    pub debug_visible: bool,
    auto_timer: Timer,
}

impl Default for SessionRuntime {
    fn default() -> Self {
        Self {
            selected: AccountId(0),
            auto_run: true,
            step_requested: false,
            toggle_ready_requested: false,
            disconnect_requested: false,
            reconnect_requested: false,
            finish_requested: false,
            reset_requested: false,
            debug_visible: true,
            auto_timer: Timer::from_seconds(0.7, TimerMode::Repeating),
        }
    }
}

pub(crate) fn setup_lab(mut commands: Commands) {
    commands
        .spawn((
            SessionOwned,
            Name::new("Session Formation World"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (label, position, color) in [
                (
                    "MATCH QUEUE",
                    Vec2::new(QUEUE_X, 205.0),
                    Color::srgb(0.70, 0.80, 0.90),
                ),
                ("TEAM 1", Vec2::new(TEAM_X[0], 205.0), TEAM_COLOR[0]),
                ("TEAM 2", Vec2::new(TEAM_X[1], 205.0), TEAM_COLOR[1]),
            ] {
                parent.spawn((
                    Text2d::new(label),
                    TextFont {
                        font_size: 17.0,
                        ..default()
                    },
                    TextColor(color),
                    Transform::from_xyz(position.x, position.y, 7.0),
                ));
            }
            parent.spawn((
                Name::new("Team One Lobby"),
                Sprite::from_color(
                    Color::srgba(0.12, 0.24, 0.34, 0.65),
                    Vec2::new(320.0, 390.0),
                ),
                Transform::from_xyz(TEAM_X[0], -15.0, -4.0),
            ));
            parent.spawn((
                Name::new("Team Two Lobby"),
                Sprite::from_color(
                    Color::srgba(0.34, 0.21, 0.10, 0.65),
                    Vec2::new(320.0, 390.0),
                ),
                Transform::from_xyz(TEAM_X[1], -15.0, -4.0),
            ));
            parent.spawn((
                Name::new("Queue Lane"),
                Sprite::from_color(
                    Color::srgba(0.12, 0.16, 0.22, 0.75),
                    Vec2::new(150.0, 390.0),
                ),
                Transform::from_xyz(QUEUE_X, -15.0, -5.0),
            ));
            for account in 0..ACCOUNT_COUNT {
                parent.spawn((
                    AccountDot(AccountId(account as u16)),
                    Name::new(format!("Account {}", account + 1)),
                    Sprite::from_color(Color::srgb(0.55, 0.62, 0.70), Vec2::splat(34.0)),
                    Transform::from_xyz(QUEUE_X, 120.0 - account as f32 * 55.0, 5.0),
                ));
            }
            for (index, label) in [
                "QUEUE", "LOBBY", "COUNT", "MATCH", "RECON", "POST", "CLOSED",
            ]
            .iter()
            .enumerate()
            {
                parent.spawn((
                    Text2d::new(*label),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.90, 0.95)),
                    Transform::from_xyz(-450.0 + index as f32 * 150.0, -285.0, 8.0),
                ));
            }
        });
    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            SessionOwned,
            SessionUiRoot,
            Name::new("Session Lab UI Root"),
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
                    width: px(485),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.96)),
                BorderColor::all(Color::srgba(0.4, 0.82, 1.0, 0.65)),
                children![(
                    DebugText,
                    Text::new("Session diagnostics starting..."),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.84, 0.94, 1.0)),
                )],
            ));
            root.spawn((
                HelpText,
                Node {
                    width: px(455),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.96)),
                BorderColor::all(Color::srgba(1.0, 0.62, 0.28, 0.65)),
                children![(
                    Text::new(
                        "PHASE 17 - SESSION FORMATION\n\
                         Space   Advance scripted lifecycle\n\
                         A       Toggle automatic lifecycle\n\
                         1-6     Select account\n\
                         T       Toggle selected ready state\n\
                         D / C   Disconnect / reconnect selected\n\
                         F       Finish active match\n\
                         R       Reset / F1 Toggle debug\n\n\
                         Compatible tickets form a four-seat lobby. Ratings are\n\
                         balanced into two teams; accounts receive stable PlayerId\n\
                         seats. All connected players must ready before countdown.\n\
                         Disconnect cancels launch or pauses the match; reconnect\n\
                         preserves identity, while host migration is deterministic.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.92, 0.88)),
                )],
            ));
        });
}

pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<SessionRuntime>,
) {
    for (key, account) in [
        (KeyCode::Digit1, AccountId(0)),
        (KeyCode::Digit2, AccountId(1)),
        (KeyCode::Digit3, AccountId(2)),
        (KeyCode::Digit4, AccountId(3)),
        (KeyCode::Digit5, AccountId(4)),
        (KeyCode::Digit6, AccountId(5)),
    ] {
        if keyboard.just_pressed(key) {
            runtime.selected = account;
        }
    }
    if keyboard.just_pressed(KeyCode::Space) {
        runtime.auto_run = false;
        runtime.step_requested = true;
    }
    if keyboard.just_pressed(KeyCode::KeyA) {
        runtime.auto_run = !runtime.auto_run;
    }
    if keyboard.just_pressed(KeyCode::KeyT) {
        runtime.toggle_ready_requested = true;
    }
    if keyboard.just_pressed(KeyCode::KeyD) {
        runtime.disconnect_requested = true;
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        runtime.reconnect_requested = true;
    }
    if keyboard.just_pressed(KeyCode::KeyF) {
        runtime.finish_requested = true;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<SessionRuntime>,
    mut world: ResMut<SessionLabWorld>,
) {
    if !runtime.reset_requested {
        return;
    }
    runtime.reset_requested = false;
    runtime.step_requested = false;
    runtime.toggle_ready_requested = false;
    runtime.disconnect_requested = false;
    runtime.reconnect_requested = false;
    runtime.finish_requested = false;
    runtime.auto_run = false;
    runtime.auto_timer.reset();
    world.reset();
}

pub(crate) fn simulate(
    time: Res<Time>,
    mut runtime: ResMut<SessionRuntime>,
    mut world: ResMut<SessionLabWorld>,
) {
    if runtime.toggle_ready_requested {
        runtime.toggle_ready_requested = false;
        if let Some(session) = &mut world.session
            && let Some(participant) = session.participant(runtime.selected)
        {
            session.set_ready(runtime.selected, !participant.ready);
        }
    }
    if runtime.disconnect_requested {
        runtime.disconnect_requested = false;
        if let Some(session) = &mut world.session {
            session.disconnect(runtime.selected);
        }
    }
    if runtime.reconnect_requested {
        runtime.reconnect_requested = false;
        if let Some(session) = &mut world.session {
            session.reconnect(runtime.selected);
        }
    }
    if runtime.finish_requested {
        runtime.finish_requested = false;
        if let Some(session) = &mut world.session {
            session.finish_match();
        }
    }

    let auto_step = runtime.auto_run && runtime.auto_timer.tick(time.delta()).just_finished();
    if runtime.step_requested || auto_step {
        runtime.step_requested = false;
        world.advance_demo();
        if world.demo_step > 13 {
            runtime.auto_run = false;
        }
    }
}

pub(crate) fn present_accounts(
    runtime: Res<SessionRuntime>,
    world: Res<SessionLabWorld>,
    mut dots: Query<(&AccountDot, &mut Transform, &mut Sprite)>,
) {
    for (dot, mut transform, mut sprite) in &mut dots {
        let queued_index = world
            .matchmaker
            .queue
            .iter()
            .position(|ticket| ticket.account == dot.0);
        let participant = world
            .session
            .as_ref()
            .and_then(|session| session.participant(dot.0));

        if let Some(participant) = participant {
            let members: Vec<_> = world
                .session
                .as_ref()
                .unwrap()
                .participants
                .iter()
                .filter(|member| member.team == participant.team)
                .collect();
            let slot = members
                .iter()
                .position(|member| member.account == participant.account)
                .unwrap_or(0);
            transform.translation.x = TEAM_X[participant.team.index()];
            transform.translation.y = 80.0 - slot as f32 * 130.0;
            let base = TEAM_COLOR[participant.team.index()];
            sprite.color = match (participant.connection, participant.ready) {
                (ConnectionState::Disconnected, _) => Color::srgb(0.38, 0.18, 0.18),
                (ConnectionState::Connected, true) => base.mix(&Color::WHITE, 0.35),
                (ConnectionState::Connected, false) => base.mix(&Color::BLACK, 0.18),
            };
        } else if let Some(index) = queued_index {
            transform.translation.x = QUEUE_X;
            transform.translation.y = 125.0 - index as f32 * 65.0;
            let ticket = world
                .matchmaker
                .queue
                .iter()
                .find(|ticket| ticket.account == dot.0)
                .unwrap();
            sprite.color =
                if ticket.region == Region::West && ticket.build == crate::model::CURRENT_BUILD {
                    Color::srgb(0.58, 0.68, 0.78)
                } else {
                    Color::srgb(0.46, 0.38, 0.58)
                };
        } else {
            transform.translation = Vec3::new(0.0, -400.0, 0.0);
        }

        if dot.0 == runtime.selected {
            sprite.color = sprite.color.mix(&Color::WHITE, 0.25);
        }
    }
}

pub(crate) fn draw_debug(
    runtime: Res<SessionRuntime>,
    world: Res<SessionLabWorld>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    gizmos.rect_2d(
        Vec2::new(QUEUE_X, -15.0),
        Vec2::new(150.0, 390.0),
        Color::srgb(0.40, 0.52, 0.64),
    );
    for team in 0..TEAM_COUNT {
        gizmos.rect_2d(
            Vec2::new(TEAM_X[team], -15.0),
            Vec2::new(320.0, 390.0),
            TEAM_COLOR[team],
        );
    }

    if let Some(session) = &world.session {
        for participant in &session.participants {
            let members: Vec<_> = session
                .participants
                .iter()
                .filter(|member| member.team == participant.team)
                .collect();
            let slot = members
                .iter()
                .position(|member| member.account == participant.account)
                .unwrap_or(0);
            let position = Vec2::new(TEAM_X[participant.team.index()], 80.0 - slot as f32 * 130.0);
            if session.host == Some(participant.account) {
                gizmos.circle_2d(position, 30.0, Color::srgb(1.0, 0.92, 0.35));
            }
            if participant.account == runtime.selected {
                gizmos.circle_2d(position, 39.0, Color::WHITE);
            }
            if participant.connection == ConnectionState::Disconnected {
                gizmos.line_2d(
                    position + Vec2::splat(-18.0),
                    position + Vec2::splat(18.0),
                    Color::srgb(1.0, 0.2, 0.2),
                );
                gizmos.line_2d(
                    position + Vec2::new(-18.0, 18.0),
                    position + Vec2::new(18.0, -18.0),
                    Color::srgb(1.0, 0.2, 0.2),
                );
            }
        }
    }

    let phases = [
        "QUEUE", "LOBBY", "COUNT", "MATCH", "RECON", "POST", "CLOSED",
    ];
    let active = world
        .session
        .as_ref()
        .map(|session| match session.phase {
            SessionPhase::Lobby => 1,
            SessionPhase::Countdown { .. } => 2,
            SessionPhase::InMatch { .. } => 3,
            SessionPhase::ReconnectGrace { .. } => 4,
            SessionPhase::PostMatch { .. } => 5,
            SessionPhase::Closed { .. } => 6,
        })
        .unwrap_or(0);
    let start_x = -450.0;
    for (index, _) in phases.iter().enumerate() {
        let x = start_x + index as f32 * 150.0;
        let color = if index == active {
            Color::srgb(0.35, 1.0, 0.68)
        } else if index < active {
            Color::srgb(0.35, 0.68, 0.88)
        } else {
            Color::srgba(0.45, 0.50, 0.55, 0.4)
        };
        gizmos.rect_2d(Vec2::new(x, -285.0), Vec2::new(105.0, 28.0), color);
        if index + 1 < phases.len() {
            gizmos.line_2d(
                Vec2::new(x + 52.0, -285.0),
                Vec2::new(x + 98.0, -285.0),
                color,
            );
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, SessionRuntime>,
    world: Res<'w, SessionLabWorld>,
    account_dots: Query<'w, 's, (), With<AccountDot>>,
    ui_roots: Query<'w, 's, (), With<SessionUiRoot>>,
    text: Single<'w, 's, &'static mut Text, With<DebugText>>,
    panel: Single<'w, 's, &'static mut Visibility, (With<DebugPanel>, Without<HelpText>)>,
    help: Single<'w, 's, &'static mut Visibility, (With<HelpText>, Without<DebugPanel>)>,
}

pub(crate) fn update_debug_text(mut context: DebugContext) {
    let visibility = if context.runtime.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    **context.panel = visibility;
    **context.help = visibility;

    let dots = context.account_dots.iter().count();
    let ui = context.ui_roots.iter().count();
    let healthy = dots == ACCOUNT_COUNT && ui == 1;
    let world = &*context.world;

    let queue = if world.matchmaker.queue.is_empty() {
        "empty".to_string()
    } else {
        world
            .matchmaker
            .queue
            .iter()
            .map(|ticket| {
                format!(
                    "{} {} r{} b{:x}",
                    ticket.account.label(),
                    ticket.region.label(),
                    ticket.rating,
                    ticket.build & 0xffff
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let (session_line, roster, events) = if let Some(session) = &world.session {
        let detail = match session.phase {
            SessionPhase::Countdown { remaining } => {
                format!("{} ({remaining})", session.phase.label())
            }
            SessionPhase::InMatch { frame } => format!("{} frame {frame}", session.phase.label()),
            SessionPhase::ReconnectGrace { remaining, .. } => {
                format!("{} ({remaining})", session.phase.label())
            }
            SessionPhase::PostMatch { remaining, .. } => {
                format!("{} ({remaining})", session.phase.label())
            }
            SessionPhase::Closed { reason } => format!("{}: {reason}", session.phase.label()),
            SessionPhase::Lobby => session.phase.label().to_string(),
        };
        let host = session
            .host
            .map(AccountId::label)
            .unwrap_or_else(|| "none".to_string());
        let manifest = session
            .launch
            .as_ref()
            .map(|launch| {
                format!(
                    "VALID {} lockstep {:08x}",
                    launch.valid(),
                    launch.lockstep_session
                )
            })
            .unwrap_or_else(|| "not emitted".to_string());
        let session_line = format!(
            "{} {}  host {}  match {}\n\
             teams {} / {}  migrations {} reconnects {}\n\
             launch {}",
            session.id.label(),
            detail,
            host,
            session.match_number,
            session.team_rating(observed_core::TeamId(0)),
            session.team_rating(observed_core::TeamId(1)),
            session.host_migrations,
            session.reconnects,
            manifest,
        );
        let roster = session
            .participants
            .iter()
            .map(|participant| {
                let selected = if participant.account == context.runtime.selected {
                    "*"
                } else {
                    " "
                };
                let roster_state = match participant.connection {
                    ConnectionState::Disconnected => "OFFLINE",
                    ConnectionState::Connected => match session.phase {
                        SessionPhase::Lobby | SessionPhase::Countdown { .. } => {
                            if participant.ready { "RDY" } else { "WAIT" }
                        }
                        SessionPhase::InMatch { .. } => "PLAY",
                        SessionPhase::ReconnectGrace { .. } => "HOLD",
                        SessionPhase::PostMatch { .. } => "POST",
                        SessionPhase::Closed { .. } => "CLOSED",
                    },
                };
                format!(
                    "{selected}{} P{} {} r{} {:<7}",
                    participant.account.label(),
                    participant.player.0 + 1,
                    participant.team.label(),
                    participant.rating,
                    roster_state,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let events = session
            .recent_events
            .iter()
            .rev()
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        (session_line, roster, events)
    } else {
        (
            "no active session".to_string(),
            "roster pending".to_string(),
            world.matchmaker.last_event.clone(),
        )
    };

    let mut text = context.text.into_inner();
    **text = format!(
        "SESSION MONITOR  {}\n\
         auto          {}\n\
         demo step     {}\n\
         formed        {}\n\
         queue         {}\n{}\n\n\
         {}\n\n\
         {}\n\n\
         recent\n{}\n\n\
         account dots {}/{}  UI {}  resets {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        if context.runtime.auto_run {
            "RUN"
        } else {
            "PAUSED"
        },
        world.demo_step,
        world.matchmaker.formed_sessions,
        world.matchmaker.queue.len(),
        queue,
        session_line,
        roster,
        events,
        dots,
        ACCOUNT_COUNT,
        ui,
        world.reset_count,
    );
}
