use bevy::{ecs::system::SystemParam, prelude::*};
use competitive_facility::model::{
    CompetitiveFacility, EXIT_ROOM, MEMBERS_PER_TEAM, PLAYER_COUNT, TEAM_COUNT,
};
use director_lab::model::Role;
use observation_lab::model::{DOOR_COUNT, DoorId, ROOM_COUNT, ROOM_HALF};
use observed_core::RoomId;

use crate::tape::Tape;

const GOLD: Color = Color::srgb(1.0, 0.80, 0.28);
const GREEN: Color = Color::srgb(0.40, 1.0, 0.60);
const CYAN: Color = Color::srgb(0.30, 0.80, 1.0);
const COLLAPSE: Color = Color::srgb(1.0, 0.26, 0.28);
const FOCUS: Color = Color::srgb(0.95, 0.97, 1.0);

const TEAM_COLORS: [Color; TEAM_COUNT] = [
    Color::srgb(0.96, 0.28, 0.34), // 0 crimson
    Color::srgb(0.32, 0.62, 1.0),  // 1 sky
    Color::srgb(0.72, 0.46, 1.0),  // 2 violet
    Color::srgb(1.0, 0.62, 0.20),  // 3 amber
];

const TEAM_OFFSETS: [Vec2; TEAM_COUNT] = [
    Vec2::new(-52.0, 52.0),
    Vec2::new(52.0, 52.0),
    Vec2::new(-52.0, -52.0),
    Vec2::new(52.0, -52.0),
];
const MEMBER_OFFSETS: [Vec2; MEMBERS_PER_TEAM] = [Vec2::new(-13.0, 0.0), Vec2::new(13.0, 0.0)];

// Scrubber timeline geometry (world space, below the schematic map).
const SCRUB_Y: f32 = -560.0;
const SCRUB_X0: f32 = -440.0;
const SCRUB_X1: f32 = 440.0;

#[derive(Component)]
pub(crate) struct ReplayOwned;

#[derive(Component)]
pub(crate) struct ReplayUiRoot;

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
pub struct Spectator {
    /// Playback cursor, in rounds (fractional for smooth play).
    pub cursor: f32,
    pub playing: bool,
    /// Playback rate, in rounds per second.
    pub speed: f32,
    pub debug_visible: bool,
    pub reset_count: u32,
}

impl Default for Spectator {
    fn default() -> Self {
        Self {
            cursor: 0.0,
            playing: true,
            speed: 2.5,
            debug_visible: true,
            reset_count: 0,
        }
    }
}

/// The match state at the current cursor — a pure projection of the tape,
/// recomputed each frame. Rendering reads this, never live entities.
#[derive(Resource)]
pub struct View(pub CompetitiveFacility);

pub(crate) fn setup_lab(mut commands: Commands) {
    commands
        .spawn((
            ReplayOwned,
            Name::new("Match Replay Root"),
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
            ReplayOwned,
            ReplayUiRoot,
            Name::new("Match Replay UI Root"),
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
                    width: px(460),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.94)),
                BorderColor::all(Color::srgba(0.6, 0.8, 1.0, 0.6)),
                children![(
                    DebugText,
                    Text::new("Replay diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.93, 1.0)),
                )],
            ));
            root.spawn((
                HelpText,
                Node {
                    width: px(430),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.94)),
                BorderColor::all(Color::srgba(0.6, 0.8, 1.0, 0.6)),
                children![(
                    Text::new(
                        "MATCH REPLAY / SPECTATOR (integration)\n\
                         Space     Play / pause\n\
                         ← / →     Step one round (pauses)\n\
                         [ / ]     Seek -5 / +5 rounds\n\
                         - / =     Slower / faster playback\n\
                         R         Restart from the beginning\n\
                         F1        Toggle debug\n\n\
                         A full competitive match (observation + spine + competition\n\
                         + director) was recorded as a tape of per-round intents. The\n\
                         spectator view is replayed from it each frame onto the\n\
                         schematic map — seek anywhere and the state, the collapse, and\n\
                         the standings are reproduced exactly. The white ring is the\n\
                         director camera's focus on the current leader.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.92, 0.97)),
                )],
            ));
        });
}

pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    tape: Res<Tape>,
    mut spectator: ResMut<Spectator>,
) {
    let last = tape.len() as f32;

    if keyboard.just_pressed(KeyCode::Space) {
        spectator.playing = !spectator.playing;
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        spectator.cursor = (spectator.cursor.floor() + 1.0).min(last);
        spectator.playing = false;
    }
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        spectator.cursor = (spectator.cursor.floor() - 1.0).max(0.0);
        spectator.playing = false;
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        spectator.cursor = (spectator.cursor + 5.0).min(last);
        spectator.playing = false;
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        spectator.cursor = (spectator.cursor - 5.0).max(0.0);
        spectator.playing = false;
    }
    if keyboard.just_pressed(KeyCode::Equal) {
        spectator.speed = (spectator.speed * 1.5).min(60.0);
    }
    if keyboard.just_pressed(KeyCode::Minus) {
        spectator.speed = (spectator.speed / 1.5).max(0.5);
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        spectator.cursor = 0.0;
        spectator.playing = true;
        spectator.reset_count += 1;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        spectator.debug_visible = !spectator.debug_visible;
    }
}

pub(crate) fn advance_cursor(time: Res<Time>, tape: Res<Tape>, mut spectator: ResMut<Spectator>) {
    if !spectator.playing {
        return;
    }
    let last = tape.len() as f32;
    spectator.cursor = (spectator.cursor + spectator.speed * time.delta_secs()).min(last);
    if spectator.cursor >= last {
        spectator.playing = false;
    }
}

pub(crate) fn update_view(tape: Res<Tape>, spectator: Res<Spectator>, mut view: ResMut<View>) {
    view.0 = tape.replay_to(spectator.cursor.floor() as usize);
}

/// The team the director camera is focused on: the current leader (escaped teams
/// count as fully advanced; ties resolve to the lowest index).
fn leader_index(facility: &CompetitiveFacility) -> usize {
    (0..facility.teams.len())
        .max_by(|&a, &b| {
            facility
                .team_progress(a)
                .total_cmp(&facility.team_progress(b))
                .then(b.cmp(&a))
        })
        .unwrap_or(0)
}

pub(crate) fn present_teams(
    view: Res<View>,
    mut dots: Query<(&MemberDot, &mut Transform, &mut Sprite)>,
) {
    let facility = &view.0;
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
            (Some(_), _) => base.mix(&Color::WHITE, 0.5),
            (None, Role::Director) => base.mix(&Color::srgb(0.08, 0.08, 0.1), 0.7),
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
    spectator: Res<Spectator>,
    tape: Res<Tape>,
    view: Res<View>,
    mut gizmos: Gizmos,
) {
    if !spectator.debug_visible {
        return;
    }
    let facility = &view.0;
    let graph = &facility.structure.graph;

    // --- schematic map (the same projection competitive_facility draws live) ---
    for index in 0..ROOM_COUNT {
        let room = RoomId(index as u32);
        let color = if graph.observed(room) {
            GREEN
        } else {
            Color::srgb(0.22, 0.26, 0.34)
        };
        gizmos.rect_2d(graph.room_center(room), Vec2::splat(ROOM_HALF * 2.0), color);
    }
    gizmos.circle_2d(
        graph.room_center(RoomId(EXIT_ROOM)),
        ROOM_HALF * 0.72,
        GREEN,
    );

    for (a, b) in graph.connections() {
        gizmos.line_2d(
            graph.door_position(a),
            graph.door_position(b),
            connection_color(facility, a),
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

    // Collapse frontier.
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

    // --- director camera focus on the current leader ---
    let leader = leader_index(facility);
    let center = graph.room_center(facility.team_room(leader));
    gizmos.circle_2d(center, ROOM_HALF * 1.05, FOCUS);
    let reach = ROOM_HALF * 1.3;
    gizmos.line_2d(
        center + Vec2::new(-reach, 0.0),
        center + Vec2::new(-reach + 26.0, 0.0),
        FOCUS,
    );
    gizmos.line_2d(
        center + Vec2::new(reach, 0.0),
        center + Vec2::new(reach - 26.0, 0.0),
        FOCUS,
    );

    // --- scrubber timeline ---
    let len = tape.len().max(1) as f32;
    let frac = (spectator.cursor / len).clamp(0.0, 1.0);
    let at = |f: f32| SCRUB_X0 + (SCRUB_X1 - SCRUB_X0) * f.clamp(0.0, 1.0);
    gizmos.line_2d(
        Vec2::new(SCRUB_X0, SCRUB_Y),
        Vec2::new(SCRUB_X1, SCRUB_Y),
        Color::srgba(0.4, 0.45, 0.55, 0.6),
    );
    gizmos.line_2d(
        Vec2::new(SCRUB_X0, SCRUB_Y),
        Vec2::new(at(frac), SCRUB_Y),
        Color::srgb(0.4, 0.8, 1.0),
    );
    let cursor_x = at(frac);
    gizmos.line_2d(
        Vec2::new(cursor_x, SCRUB_Y - 16.0),
        Vec2::new(cursor_x, SCRUB_Y + 16.0),
        FOCUS,
    );
    for (round, _) in &tape.markers {
        let x = at(*round as f32 / len);
        gizmos.line_2d(
            Vec2::new(x, SCRUB_Y - 11.0),
            Vec2::new(x, SCRUB_Y + 11.0),
            GOLD,
        );
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    spectator: Res<'w, Spectator>,
    tape: Res<'w, Tape>,
    view: Res<'w, View>,
    dots: Query<'w, 's, (), With<MemberDot>>,
    ui_roots: Query<'w, 's, (), With<ReplayUiRoot>>,
    text: Single<'w, 's, &'static mut Text, With<DebugText>>,
    panel: Single<'w, 's, &'static mut Visibility, (With<DebugPanel>, Without<HelpText>)>,
    help: Single<'w, 's, &'static mut Visibility, (With<HelpText>, Without<DebugPanel>)>,
}

pub(crate) fn update_debug_text(mut context: DebugContext) {
    let visibility = if context.spectator.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    **context.panel = visibility;
    **context.help = visibility;

    let facility = &context.view.0;
    let cursor = context.spectator.cursor.floor() as usize;
    let len = context.tape.len();

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

    let mut markers = String::new();
    for (round, label) in &context.tape.markers {
        let seen = if *round <= cursor { "•" } else { "·" };
        markers.push_str(&format!("  {seen} r{round}: {label}\n"));
    }

    let dots = context.dots.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = dots == PLAYER_COUNT && ui_roots == 1;

    let mut text = context.text.into_inner();
    **text = format!(
        "MATCH REPLAY  {}\n\
         round          {cursor} / {len}\n\
         transport      {}\n\
         speed          {:.1} rounds/s\n\
         match          {}\n\
         collapse line  {:.0}%\n\
         escaped {}   absorbed {}\n\n\
         standings @ cursor:\n{}\n\
         events:\n{}\n\
         dots {dots}/{PLAYER_COUNT}  UI {ui_roots}   resets {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        if context.spectator.playing {
            "PLAYING"
        } else {
            "PAUSED"
        },
        context.spectator.speed,
        if facility.finished {
            "FINISHED"
        } else {
            "in progress"
        },
        facility.purge_line.max(0.0) * 100.0,
        facility.escaped_count(),
        facility.absorbed_count(),
        standings.trim_end(),
        markers.trim_end(),
        context.spectator.reset_count,
    );
}
