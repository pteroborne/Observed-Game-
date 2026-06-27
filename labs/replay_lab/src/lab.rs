use bevy::{ecs::system::SystemParam, prelude::*};
use competition_lab::model::{CompetitionWorld, TEAM_COUNT};
use observed_core::TeamId;

use crate::tape::Tape;

const TEAM_COLORS: [Color; TEAM_COUNT] = [
    Color::srgb(0.30, 0.75, 1.0),
    Color::srgb(1.0, 0.55, 0.28),
    Color::srgb(0.62, 1.0, 0.40),
];

const START_X: f32 = -540.0;
const EXIT_X: f32 = 470.0;
const LANE_GAP: f32 = 150.0;
const SCRUB_Y: f32 = -290.0;

fn lane_y(index: usize) -> f32 {
    ((TEAM_COUNT as f32 - 1.0) * 0.5 - index as f32) * LANE_GAP
}

fn track_x(progress: f32) -> f32 {
    START_X + (EXIT_X - START_X) * progress.clamp(0.0, 1.0)
}

fn scrub_x(fraction: f32) -> f32 {
    START_X + (EXIT_X - START_X) * fraction.clamp(0.0, 1.0)
}

#[derive(Component)]
pub(crate) struct ReplayOwned;

#[derive(Component)]
pub(crate) struct ReplayUiRoot;

#[derive(Component)]
pub(crate) struct Racer(pub TeamId);

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct Spectator {
    pub cursor: f32,
    pub playing: bool,
    pub speed: f32,
    pub debug_visible: bool,
    pub reset_count: u32,
}

impl Default for Spectator {
    fn default() -> Self {
        Self {
            cursor: 0.0,
            playing: true,
            speed: 30.0,
            debug_visible: true,
            reset_count: 0,
        }
    }
}

/// The simulation state at the current cursor — a pure projection of the tape,
/// recomputed each frame. Rendering reads this, never live entities.
#[derive(Resource)]
pub struct View(pub CompetitionWorld);

pub(crate) fn setup_lab(mut commands: Commands, view: Res<View>) {
    commands
        .spawn((
            ReplayOwned,
            Name::new("Replay Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (index, team) in view.0.teams.iter().enumerate() {
                parent.spawn((
                    Racer(team.id),
                    Name::new(format!("{} racer", team.id.label())),
                    Sprite::from_color(TEAM_COLORS[index % TEAM_COUNT], Vec2::new(32.0, 56.0)),
                    Transform::from_translation(Vec3::new(START_X, lane_y(index), 5.0)),
                ));
            }
        });

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            ReplayOwned,
            ReplayUiRoot,
            Name::new("Replay Lab UI Root"),
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
                    width: px(420),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.6, 0.8, 1.0, 0.6)),
                children![(
                    Text::new(
                        "REPLAY / SPECTATOR LAB\n\
                         Space     Play / pause\n\
                         ← / →     Step one tick (pauses)\n\
                         [ / ]     Seek -10 / +10 ticks\n\
                         - / =     Slower / faster playback\n\
                         R         Restart from the beginning\n\
                         F1        Toggle debug\n\n\
                         A finished competition match was recorded as a tape of\n\
                         per-tick inputs. The view is replayed from it each frame —\n\
                         seek anywhere and the state is reproduced exactly.",
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
        spectator.cursor = (spectator.cursor + 10.0).min(last);
        spectator.playing = false;
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        spectator.cursor = (spectator.cursor - 10.0).max(0.0);
        spectator.playing = false;
    }
    if keyboard.just_pressed(KeyCode::Equal) {
        spectator.speed = (spectator.speed * 1.5).min(480.0);
    }
    if keyboard.just_pressed(KeyCode::Minus) {
        spectator.speed = (spectator.speed / 1.5).max(2.0);
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

pub(crate) fn present_racers(
    view: Res<View>,
    mut racers: Query<(&Racer, &mut Transform, &mut Sprite)>,
) {
    for (racer, mut transform, mut sprite) in &mut racers {
        if let Some((index, team)) = view
            .0
            .teams
            .iter()
            .enumerate()
            .find(|(_, team)| team.id == racer.0)
        {
            transform.translation.x = track_x(team.progress);
            transform.translation.y = lane_y(index);
            let base = TEAM_COLORS[index % TEAM_COUNT];
            sprite.color = if team.eliminated {
                base.mix(&Color::BLACK, 0.55)
            } else if view.0.control_holder == Some(team.id) {
                base.mix(&Color::WHITE, 0.4)
            } else {
                base
            };
        }
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

    for (index, team) in view.0.teams.iter().enumerate() {
        let y = lane_y(index);
        let base = TEAM_COLORS[index % TEAM_COUNT];
        gizmos.line_2d(
            Vec2::new(START_X, y),
            Vec2::new(EXIT_X, y),
            Color::srgba(0.4, 0.45, 0.55, 0.4),
        );
        gizmos.line_2d(
            Vec2::new(START_X, y),
            Vec2::new(track_x(team.progress), y),
            base,
        );
        if view.0.control_holder == Some(team.id) {
            gizmos.circle_2d(
                Vec2::new(track_x(team.progress), y),
                38.0,
                Color::srgb(1.0, 0.95, 0.5),
            );
        }
    }

    // Exit gate + capacity slots.
    let top = lane_y(0) + 60.0;
    let bottom = lane_y(TEAM_COUNT - 1) - 60.0;
    gizmos.line_2d(
        Vec2::new(EXIT_X, top),
        Vec2::new(EXIT_X, bottom),
        Color::srgb(0.4, 1.0, 0.6),
    );
    let claimed = view.0.exit_capacity - view.0.slots_remaining;
    for slot in 0..view.0.exit_capacity {
        let y = 60.0 - slot as f32 * 40.0;
        let color = if slot < claimed {
            Color::srgb(1.0, 0.84, 0.3)
        } else {
            Color::srgba(0.4, 1.0, 0.6, 0.35)
        };
        gizmos.rect_2d(Vec2::new(EXIT_X + 56.0, y), Vec2::splat(26.0), color);
    }

    // Scrubber timeline.
    let len = tape.len().max(1) as f32;
    let fraction = (spectator.cursor / len).clamp(0.0, 1.0);
    gizmos.line_2d(
        Vec2::new(START_X, SCRUB_Y),
        Vec2::new(EXIT_X, SCRUB_Y),
        Color::srgba(0.4, 0.45, 0.55, 0.6),
    );
    gizmos.line_2d(
        Vec2::new(START_X, SCRUB_Y),
        Vec2::new(scrub_x(fraction), SCRUB_Y),
        Color::srgb(0.4, 0.8, 1.0),
    );
    let cursor_x = scrub_x(fraction);
    gizmos.line_2d(
        Vec2::new(cursor_x, SCRUB_Y - 16.0),
        Vec2::new(cursor_x, SCRUB_Y + 16.0),
        Color::srgb(0.95, 0.97, 1.0),
    );
    for (tick, _) in &tape.markers {
        let x = scrub_x(*tick as f32 / len);
        gizmos.line_2d(
            Vec2::new(x, SCRUB_Y - 10.0),
            Vec2::new(x, SCRUB_Y + 10.0),
            Color::srgb(1.0, 0.84, 0.3),
        );
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    spectator: Res<'w, Spectator>,
    tape: Res<'w, Tape>,
    view: Res<'w, View>,
    racers: Query<'w, 's, (), With<Racer>>,
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

    let world = &context.view.0;
    let cursor = context.spectator.cursor.floor() as usize;
    let len = context.tape.len();

    let mut standings = String::new();
    for id in world.standings() {
        if let Some(team) = world.team(id) {
            standings.push_str(&format!("{:<8} {}\n", team.id.label(), team.status()));
        }
    }

    let mut markers = String::new();
    for (tick, label) in &context.tape.markers {
        markers.push_str(&format!("t{tick}: {label}\n"));
    }

    let racers = context.racers.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = racers == TEAM_COUNT && ui_roots == 1;

    let mut text = context.text.into_inner();
    **text = format!(
        "REPLAY MONITOR  {}\n\
         tick           {cursor} / {len}\n\
         transport      {}\n\
         speed          {:.0} ticks/s\n\
         match status   {}\n\n\
         standings @ cursor:\n\
         {}\n\
         events:\n\
         {}\n\
         racers {racers}  UI {ui_roots}   resets {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        if context.spectator.playing {
            "PLAYING"
        } else {
            "PAUSED"
        },
        context.spectator.speed,
        if world.finished {
            "FINISHED"
        } else {
            "in progress"
        },
        standings.trim_end(),
        markers.trim_end(),
        context.spectator.reset_count,
    );
}
