use bevy::{ecs::system::SystemParam, prelude::*};
use competition_lab::model::RaceAction;
use director_lab::model::Role;
use observation_lab::model::{DOOR_COUNT, DoorId, ROOM_COUNT, ROOM_HALF};
use observed_core::{RoomId, TeamId};

use crate::model::{CompetitiveFacility, EXIT_ROOM, MEMBERS_PER_TEAM, PLAYER_COUNT, TEAM_COUNT};

const GOLD: Color = Color::srgb(1.0, 0.80, 0.28);
const GREEN: Color = Color::srgb(0.40, 1.0, 0.60);
const CYAN: Color = Color::srgb(0.30, 0.80, 1.0);
const COLLAPSE: Color = Color::srgb(1.0, 0.26, 0.28);

/// One distinct colour per team (avoiding the structure's gold / green / cyan).
const TEAM_COLORS: [Color; TEAM_COUNT] = [
    Color::srgb(0.96, 0.28, 0.34), // 0 crimson
    Color::srgb(0.32, 0.62, 1.0),  // 1 sky
    Color::srgb(0.72, 0.46, 1.0),  // 2 violet
    Color::srgb(1.0, 0.62, 0.20),  // 3 amber
];

/// Cluster offset for each team within a room, then each member within the team.
const TEAM_OFFSETS: [Vec2; TEAM_COUNT] = [
    Vec2::new(-52.0, 52.0),
    Vec2::new(52.0, 52.0),
    Vec2::new(-52.0, -52.0),
    Vec2::new(52.0, -52.0),
];
const MEMBER_OFFSETS: [Vec2; MEMBERS_PER_TEAM] = [Vec2::new(-13.0, 0.0), Vec2::new(13.0, 0.0)];

#[derive(Component)]
pub(crate) struct CompOwned;

#[derive(Component)]
pub(crate) struct CompUiRoot;

#[derive(Component)]
pub(crate) struct MemberDot {
    team: usize,
    member: usize,
}

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct CompRuntime {
    pub selected_team: usize,
    pub running: bool,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
}

impl Default for CompRuntime {
    fn default() -> Self {
        Self {
            selected_team: 0,
            running: true,
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
        }
    }
}

#[derive(Resource)]
pub struct RoundTimer(pub Timer);

impl Default for RoundTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.9, TimerMode::Repeating))
    }
}

pub(crate) fn setup_lab(mut commands: Commands) {
    commands
        .spawn((
            CompOwned,
            Name::new("Competitive Facility Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (team, &color) in TEAM_COLORS.iter().enumerate() {
                for member in 0..MEMBERS_PER_TEAM {
                    parent.spawn((
                        MemberDot { team, member },
                        Name::new(format!("Team {} member {}", team + 1, member + 1)),
                        Sprite::from_color(color, Vec2::splat(22.0)),
                        Transform::from_translation(Vec3::new(0.0, 0.0, 5.0)),
                    ));
                }
            }
        });

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            CompOwned,
            CompUiRoot,
            Name::new("Competitive Facility UI Root"),
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
                    width: px(470),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.01, 0.03, 0.94)),
                BorderColor::all(Color::srgba(1.0, 0.4, 0.4, 0.6)),
                children![(
                    DebugText,
                    Text::new("Match diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.92, 0.86)),
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
                BackgroundColor(Color::srgba(0.02, 0.01, 0.03, 0.94)),
                BorderColor::all(Color::srgba(1.0, 0.4, 0.4, 0.6)),
                children![(
                    Text::new(
                        "COMPETITIVE FACILITY (integration)\n\
                         Space   Pause / resume the match\n\
                         Enter   Advance one round\n\
                         Z       Selected team seizes the control\n\
                         X       Selected team (if absorbed) scrambles\n\
                         1–4     Select a team · R reset · F1 debug\n\n\
                         Four teams race the shifting facility along the gold spine\n\
                         to two capacity-limited exits. The red collapse line chases\n\
                         the leader; teams that fall a gap behind are absorbed into\n\
                         the facility and speed it up. The leader never gets caught —\n\
                         the match resolves: some escape, the rest are taken.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.92, 0.95)),
                )],
            ));
        });
}

pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<CompRuntime>,
    mut facility: ResMut<CompetitiveFacility>,
) {
    for (key, team) in [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
    ] {
        if keyboard.just_pressed(key) {
            runtime.selected_team = team;
        }
    }
    if keyboard.just_pressed(KeyCode::Space) {
        runtime.running = !runtime.running;
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        facility.advance_round(&[]);
    }
    if keyboard.just_pressed(KeyCode::KeyZ) {
        let seizer = TeamId(runtime.selected_team as u8);
        facility.advance_round(&[(seizer, RaceAction::Seize)]);
    }
    if keyboard.just_pressed(KeyCode::KeyX) {
        let id = TeamId(runtime.selected_team as u8);
        facility.scramble(id);
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
}

pub(crate) fn simulate(
    time: Res<Time>,
    runtime: Res<CompRuntime>,
    mut timer: ResMut<RoundTimer>,
    mut facility: ResMut<CompetitiveFacility>,
) {
    if !runtime.running || facility.finished {
        return;
    }
    if timer.0.tick(time.delta()).just_finished() {
        facility.advance_round(&[]);
    }
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<CompRuntime>,
    mut facility: ResMut<CompetitiveFacility>,
    mut timer: ResMut<RoundTimer>,
) {
    if !runtime.reset_requested {
        return;
    }
    runtime.reset_requested = false;
    runtime.reset_count += 1;
    facility.reset();
    timer.0.reset();
}

pub(crate) fn present_teams(
    runtime: Res<CompRuntime>,
    facility: Res<CompetitiveFacility>,
    mut dots: Query<(&MemberDot, &mut Transform, &mut Sprite)>,
) {
    for (dot, mut transform, mut sprite) in &mut dots {
        let team = &facility.teams[dot.team];
        let room = facility.team_room(dot.team);
        let position = facility.structure.graph.room_center(room)
            + TEAM_OFFSETS[dot.team]
            + MEMBER_OFFSETS[dot.member % MEMBERS_PER_TEAM];
        transform.translation.x = position.x;
        transform.translation.y = position.y;

        let base = TEAM_COLORS[dot.team];
        sprite.color = match (team.placement, team.role) {
            (Some(_), _) => base.mix(&Color::WHITE, 0.5), // escaped — bright
            (None, Role::Director) => base.mix(&Color::srgb(0.08, 0.08, 0.1), 0.7), // absorbed — dim
            (None, Role::Runner) if dot.team == runtime.selected_team => {
                base.mix(&Color::WHITE, 0.3)
            }
            _ => base,
        };
    }
}

fn connection_color(facility: &CompetitiveFacility, door: DoorId) -> Color {
    if facility.structure.is_protected(door) {
        GOLD
    } else if facility.structure.graph.is_pinned(door) {
        GREEN
    } else {
        CYAN
    }
}

pub(crate) fn draw_debug(
    runtime: Res<CompRuntime>,
    facility: Res<CompetitiveFacility>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }
    let graph = &facility.structure.graph;

    for index in 0..ROOM_COUNT {
        let room = RoomId(index as u32);
        let color = if graph.observed(room) {
            GREEN
        } else {
            Color::srgb(0.22, 0.26, 0.34)
        };
        gizmos.rect_2d(graph.room_center(room), Vec2::splat(ROOM_HALF * 2.0), color);
    }

    // Exit room highlight.
    gizmos.circle_2d(
        graph.room_center(RoomId(EXIT_ROOM)),
        ROOM_HALF * 0.72,
        GREEN,
    );

    // Connections, coloured by what holds them.
    for (a, b) in graph.connections() {
        gizmos.line_2d(
            graph.door_position(a),
            graph.door_position(b),
            connection_color(&facility, a),
        );
    }
    for index in 0..DOOR_COUNT {
        let door = DoorId(index as u16);
        if graph.is_sealed(door) {
            gizmos.circle_2d(
                graph.door_position(door),
                4.0,
                Color::srgba(0.5, 0.55, 0.6, 0.6),
            );
        }
    }

    // The collapse: spine rooms it has swallowed get a red ring; its frontier a
    // brighter, larger one.
    let frontier = facility.collapse_frontier();
    for room in facility.collapse_rooms() {
        let big = Some(room) == frontier;
        gizmos.circle_2d(
            graph.room_center(room),
            if big {
                ROOM_HALF * 0.92
            } else {
                ROOM_HALF * 0.62
            },
            if big {
                COLLAPSE
            } else {
                Color::srgba(1.0, 0.26, 0.28, 0.45)
            },
        );
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, CompRuntime>,
    facility: Res<'w, CompetitiveFacility>,
    dots: Query<'w, 's, (), With<MemberDot>>,
    ui_roots: Query<'w, 's, (), With<CompUiRoot>>,
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

    let facility = &*context.facility;

    let dots = context.dots.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = dots == PLAYER_COUNT && ui_roots == 1;

    // Per-team standings line.
    let mut standings = String::new();
    for &id in &facility.standings() {
        let index = facility.teams.iter().position(|t| t.id == id).unwrap();
        let team = &facility.teams[index];
        let status = match (team.placement, team.role) {
            (Some(_), _) | (None, Role::Director) => team.status(),
            (None, Role::Runner) => format!("{:.0}%", facility.team_progress(index) * 100.0),
        };
        standings.push_str(&format!("  {}  {}\n", id.label(), status));
    }

    let mut text = context.text.into_inner();
    **text = format!(
        "COMPETITIVE FACILITY  {}\n\
         match           {}\n\
         exits left      {} / {}\n\
         collapse line   {:.0}%\n\
         control held    {}\n\
         escaped {}   absorbed {}\n\
         exit reachable  {}\n\
         round {}   decoheres {}   resets {}\n\
         dots {dots}/{PLAYER_COUNT}  UI {ui_roots}\n\
         standings:\n{}\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        if facility.finished { "OVER" } else { "running" },
        facility.slots_remaining,
        facility.exit_capacity,
        facility.purge_line.max(0.0) * 100.0,
        match facility.control_holder {
            Some(id) => id.label(),
            None => "—".to_string(),
        },
        facility.escaped_count(),
        facility.absorbed_count(),
        if facility.connected() { "yes" } else { "NO" },
        facility.round,
        facility.decohere_count,
        context.runtime.reset_count,
        standings,
        facility.last_event,
    );
}
