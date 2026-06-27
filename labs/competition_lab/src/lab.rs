use bevy::{ecs::system::SystemParam, prelude::*};
use observed_core::TeamId;

use crate::model::{CompetitionWorld, RaceAction, TEAM_COUNT};

const TEAM_COLORS: [Color; TEAM_COUNT] = [
    Color::srgb(0.30, 0.75, 1.0),
    Color::srgb(1.0, 0.55, 0.28),
    Color::srgb(0.62, 1.0, 0.40),
];

const START_X: f32 = -520.0;
const EXIT_X: f32 = 470.0;
const LANE_GAP: f32 = 170.0;

fn lane_y(index: usize) -> f32 {
    (1.0 - index as f32) * LANE_GAP
}

fn racer_x(progress: f32) -> f32 {
    START_X + (EXIT_X - START_X) * progress.clamp(0.0, 1.0)
}

#[derive(Component)]
pub(crate) struct CompOwned;

#[derive(Component)]
pub(crate) struct CompUiRoot;

#[derive(Component)]
pub(crate) struct Racer(pub TeamId);

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct CompetitionRuntime {
    pub selected_team: TeamId,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
}

impl Default for CompetitionRuntime {
    fn default() -> Self {
        Self {
            selected_team: TeamId(0),
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
        }
    }
}

pub(crate) fn setup_lab(mut commands: Commands, world: Res<CompetitionWorld>) {
    commands
        .spawn((
            CompOwned,
            Name::new("Race Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (index, team) in world.teams.iter().enumerate() {
                parent.spawn((
                    Racer(team.id),
                    Name::new(format!("{} racer", team.id.label())),
                    Sprite::from_color(TEAM_COLORS[index % TEAM_COUNT], Vec2::new(34.0, 60.0)),
                    Transform::from_translation(Vec3::new(START_X, lane_y(index), 5.0)),
                ));
            }
        });

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            CompOwned,
            CompUiRoot,
            Name::new("Competition Lab UI Root"),
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
                BorderColor::all(Color::srgba(0.5, 0.85, 1.0, 0.6)),
                children![(
                    DebugText,
                    Text::new("Competition diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.84, 0.93, 1.0)),
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
                BorderColor::all(Color::srgba(0.5, 0.85, 1.0, 0.6)),
                children![(
                    Text::new(
                        "COMPETITION LAB\n\
                         Hold Space   Seize the shared control (your team)\n\
                         1–3          Select your team\n\
                         R            Reset the match · F1 Toggle debug\n\n\
                         Three teams race; only two exits. Holding the shared\n\
                         control speeds your team — the only way to interfere is\n\
                         to win the shared advantage, never to harm opponents.\n\
                         Reach an exit before they fill or be locked out.",
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
    mut runtime: ResMut<CompetitionRuntime>,
) {
    for (key, team) in [
        (KeyCode::Digit1, TeamId(0)),
        (KeyCode::Digit2, TeamId(1)),
        (KeyCode::Digit3, TeamId(2)),
    ] {
        if keyboard.just_pressed(key) {
            runtime.selected_team = team;
        }
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
    keyboard: Res<ButtonInput<KeyCode>>,
    runtime: Res<CompetitionRuntime>,
    mut world: ResMut<CompetitionWorld>,
) {
    if world.finished {
        return;
    }
    let seize_held = keyboard.pressed(KeyCode::Space);
    let tick = world.tick_count;
    let holder = world.control_holder;
    let selected = runtime.selected_team;

    let intents: Vec<(TeamId, RaceAction)> = world
        .teams
        .iter()
        .filter(|team| team.racing())
        .map(|team| {
            let action = if team.id == selected {
                if seize_held {
                    RaceAction::Seize
                } else {
                    RaceAction::Advance
                }
            } else {
                bot_action(team.id, holder, tick)
            };
            (team.id, action)
        })
        .collect();

    world.tick(&intents, time.delta_secs());
}

/// Bots periodically contest the control (staggered, deterministic) when they do
/// not already hold it.
fn bot_action(team: TeamId, holder: Option<TeamId>, tick: u32) -> RaceAction {
    let phase = (tick + team.0 as u32 * 47) % 150;
    if phase < 8 && holder != Some(team) {
        RaceAction::Seize
    } else {
        RaceAction::Advance
    }
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<CompetitionRuntime>,
    mut world: ResMut<CompetitionWorld>,
) {
    if !runtime.reset_requested {
        return;
    }
    runtime.reset_requested = false;
    runtime.reset_count += 1;
    world.reset();
}

pub(crate) fn present_racers(
    world: Res<CompetitionWorld>,
    mut racers: Query<(&Racer, &mut Transform, &mut Sprite)>,
) {
    for (racer, mut transform, mut sprite) in &mut racers {
        if let Some((index, team)) = world
            .teams
            .iter()
            .enumerate()
            .find(|(_, team)| team.id == racer.0)
        {
            transform.translation.x = racer_x(team.progress);
            transform.translation.y = lane_y(index);
            let base = TEAM_COLORS[index % TEAM_COUNT];
            sprite.color = if team.eliminated {
                base.mix(&Color::BLACK, 0.55)
            } else if world.control_holder == Some(team.id) {
                base.mix(&Color::WHITE, 0.4)
            } else {
                base
            };
        }
    }
}

pub(crate) fn draw_debug(
    runtime: Res<CompetitionRuntime>,
    world: Res<CompetitionWorld>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    for (index, team) in world.teams.iter().enumerate() {
        let y = lane_y(index);
        let base = TEAM_COLORS[index % TEAM_COUNT];
        // Lane baseline and progress fill.
        gizmos.line_2d(
            Vec2::new(START_X, y),
            Vec2::new(EXIT_X, y),
            Color::srgba(0.4, 0.45, 0.55, 0.4),
        );
        gizmos.line_2d(
            Vec2::new(START_X, y),
            Vec2::new(racer_x(team.progress), y),
            base,
        );
        if world.control_holder == Some(team.id) {
            gizmos.circle_2d(
                Vec2::new(racer_x(team.progress), y),
                40.0,
                Color::srgb(1.0, 0.95, 0.5),
            );
        }
        // Lane selector ring.
        if team.id == runtime.selected_team {
            gizmos.circle_2d(
                Vec2::new(START_X - 36.0, y),
                14.0,
                Color::srgb(0.7, 1.0, 0.85),
            );
        }
    }

    // Exit gate and capacity slots.
    gizmos.line_2d(
        Vec2::new(EXIT_X, lane_y(0) + 70.0),
        Vec2::new(EXIT_X, lane_y(TEAM_COUNT - 1) - 70.0),
        Color::srgb(0.4, 1.0, 0.6),
    );
    let claimed = world.exit_capacity - world.slots_remaining;
    for slot in 0..world.exit_capacity {
        let y = 70.0 - slot as f32 * 40.0;
        let color = if slot < claimed {
            Color::srgb(1.0, 0.84, 0.3)
        } else {
            Color::srgba(0.4, 1.0, 0.6, 0.35)
        };
        gizmos.rect_2d(Vec2::new(EXIT_X + 60.0, y), Vec2::splat(28.0), color);
    }

    // Shared control marker, coloured to its holder.
    let control_color = world
        .control_holder
        .and_then(|t| world.teams.iter().position(|team| team.id == t))
        .map(|i| TEAM_COLORS[i % TEAM_COUNT])
        .unwrap_or(Color::srgb(0.5, 0.5, 0.55));
    gizmos.circle_2d(Vec2::new(0.0, 300.0), 18.0, control_color);
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, CompetitionRuntime>,
    world: Res<'w, CompetitionWorld>,
    racers: Query<'w, 's, (), With<Racer>>,
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

    let world = &*context.world;

    let mut standings = String::new();
    for id in world.standings() {
        if let Some(team) = world.team(id) {
            let marker = if context.runtime.selected_team == id {
                "*"
            } else {
                " "
            };
            standings.push_str(&format!(
                "{marker}{:<8} {}\n",
                team.id.label(),
                team.status()
            ));
        }
    }

    let control = world
        .control_holder
        .map(|t| t.label())
        .unwrap_or_else(|| "none".to_string());
    let winner = world
        .winner
        .map(|t| t.label())
        .unwrap_or_else(|| "—".to_string());

    let racers = context.racers.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = racers == TEAM_COUNT && ui_roots == 1;

    let mut text = context.text.into_inner();
    **text = format!(
        "COMPETITION MONITOR  {}\n\
         status        {}\n\
         exits         {} / {} slots free\n\
         control held  {}\n\
         winner        {}\n\
         tick          {}\n\n\
         {}\n\
         racers {racers}  UI {ui_roots}   resets {}\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        if world.finished { "FINISHED" } else { "racing" },
        world.slots_remaining,
        world.exit_capacity,
        control,
        winner,
        world.tick_count,
        standings.trim_end(),
        context.runtime.reset_count,
        world.last_event,
    );
}
