use bevy::{ecs::system::SystemParam, prelude::*};
use observed_core::TeamId;

use crate::model::{DirectorWorld, RUNNER_COUNT, Role};

const TEAM_COLORS: [Color; RUNNER_COUNT] = [
    Color::srgb(0.30, 0.80, 1.0),
    Color::srgb(1.0, 0.80, 0.30),
    Color::srgb(0.55, 1.0, 0.45),
    Color::srgb(0.80, 0.55, 1.0),
];

const ABSORBED_COLOR: Color = Color::srgb(0.70, 0.20, 0.22);
const COLLAPSE_COLOR: Color = Color::srgb(0.95, 0.25, 0.25);

const START_X: f32 = -540.0;
const EXIT_X: f32 = 470.0;
const LANE_GAP: f32 = 120.0;

fn lane_y(index: usize) -> f32 {
    ((RUNNER_COUNT as f32 - 1.0) * 0.5 - index as f32) * LANE_GAP
}

fn track_x(progress: f32) -> f32 {
    START_X + (EXIT_X - START_X) * progress.clamp(0.0, 1.0)
}

#[derive(Component)]
pub(crate) struct DirOwned;

#[derive(Component)]
pub(crate) struct DirUiRoot;

#[derive(Component)]
pub(crate) struct Racer(pub TeamId);

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct DirectorRuntime {
    pub selected_team: TeamId,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
}

impl Default for DirectorRuntime {
    fn default() -> Self {
        Self {
            selected_team: TeamId(0),
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
        }
    }
}

pub(crate) fn setup_lab(mut commands: Commands, world: Res<DirectorWorld>) {
    commands
        .spawn((
            DirOwned,
            Name::new("Run Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (index, team) in world.teams.iter().enumerate() {
                parent.spawn((
                    Racer(team.id),
                    Name::new(format!("{} racer", team.id.label())),
                    Sprite::from_color(TEAM_COLORS[index % RUNNER_COUNT], Vec2::new(30.0, 52.0)),
                    Transform::from_translation(Vec3::new(START_X, lane_y(index), 5.0)),
                ));
            }
        });

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            DirOwned,
            DirUiRoot,
            Name::new("Director Lab UI Root"),
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
                BackgroundColor(Color::srgba(0.03, 0.01, 0.02, 0.94)),
                BorderColor::all(Color::srgba(1.0, 0.4, 0.4, 0.6)),
                children![(
                    DebugText,
                    Text::new("Director diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.86, 0.82)),
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
                BackgroundColor(Color::srgba(0.03, 0.01, 0.02, 0.94)),
                BorderColor::all(Color::srgba(1.0, 0.4, 0.4, 0.6)),
                children![(
                    Text::new(
                        "FACILITY DIRECTOR LAB\n\
                         Space   Sprint (your runner) / Scramble (once absorbed)\n\
                         1–4     Select a team\n\
                         R       Reset the run · F1 Toggle debug\n\n\
                         Teams flee toward two exits while the collapse line chases\n\
                         the leader. Fall behind it and you are absorbed into the\n\
                         facility director — each absorbed team makes the collapse\n\
                         faster. The director never harms you; it only closes in.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.90, 0.90)),
                )],
            ));
        });
}

pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<DirectorRuntime>,
) {
    for (key, team) in [
        (KeyCode::Digit1, TeamId(0)),
        (KeyCode::Digit2, TeamId(1)),
        (KeyCode::Digit3, TeamId(2)),
        (KeyCode::Digit4, TeamId(3)),
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
    runtime: Res<DirectorRuntime>,
    mut world: ResMut<DirectorWorld>,
) {
    if world.finished {
        return;
    }
    let selected = runtime.selected_team;
    let role = world
        .team(selected)
        .map(|team| (team.active_runner(), team.role));

    let mut boosts = Vec::new();
    if let Some((active_runner, role)) = role {
        if active_runner && keyboard.pressed(KeyCode::Space) {
            boosts.push(selected);
        } else if role == Role::Director && keyboard.just_pressed(KeyCode::Space) {
            world.scramble(selected);
        }
    }

    world.tick(&boosts, time.delta_secs());
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<DirectorRuntime>,
    mut world: ResMut<DirectorWorld>,
) {
    if !runtime.reset_requested {
        return;
    }
    runtime.reset_requested = false;
    runtime.reset_count += 1;
    world.reset();
}

pub(crate) fn present_racers(
    runtime: Res<DirectorRuntime>,
    world: Res<DirectorWorld>,
    mut racers: Query<(&Racer, &mut Transform, &mut Sprite)>,
) {
    for (racer, mut transform, mut sprite) in &mut racers {
        if let Some((index, team)) = world
            .teams
            .iter()
            .enumerate()
            .find(|(_, team)| team.id == racer.0)
        {
            transform.translation.x = track_x(team.progress);
            transform.translation.y = lane_y(index);
            let base = TEAM_COLORS[index % RUNNER_COUNT];
            sprite.color = if team.role == Role::Director {
                ABSORBED_COLOR
            } else if team.id == runtime.selected_team {
                base.mix(&Color::WHITE, 0.4)
            } else {
                base
            };
        }
    }
}

pub(crate) fn draw_debug(
    runtime: Res<DirectorRuntime>,
    world: Res<DirectorWorld>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    let top = lane_y(0) + 60.0;
    let bottom = lane_y(RUNNER_COUNT - 1) - 60.0;

    for (index, team) in world.teams.iter().enumerate() {
        let y = lane_y(index);
        let base = TEAM_COLORS[index % RUNNER_COUNT];
        gizmos.line_2d(
            Vec2::new(START_X, y),
            Vec2::new(EXIT_X, y),
            Color::srgba(0.4, 0.4, 0.5, 0.4),
        );
        let fill = if team.role == Role::Director {
            ABSORBED_COLOR
        } else {
            base
        };
        gizmos.line_2d(
            Vec2::new(START_X, y),
            Vec2::new(track_x(team.progress), y),
            fill,
        );
        if team.id == runtime.selected_team {
            gizmos.circle_2d(
                Vec2::new(START_X - 34.0, y),
                13.0,
                Color::srgb(0.8, 1.0, 0.85),
            );
        }
    }

    // The collapse line chasing the runners (everything left of it is taken).
    let purge_x = track_x(world.purge_line);
    gizmos.line_2d(
        Vec2::new(purge_x, top),
        Vec2::new(purge_x, bottom),
        COLLAPSE_COLOR,
    );
    gizmos.line_2d(
        Vec2::new(purge_x - 6.0, top),
        Vec2::new(purge_x - 6.0, bottom),
        Color::srgba(0.95, 0.25, 0.25, 0.4),
    );

    // Exit gate and capacity slots.
    gizmos.line_2d(
        Vec2::new(EXIT_X, top),
        Vec2::new(EXIT_X, bottom),
        Color::srgb(0.4, 1.0, 0.6),
    );
    let claimed = world.exit_capacity - world.slots_remaining;
    for slot in 0..world.exit_capacity {
        let y = 60.0 - slot as f32 * 38.0;
        let color = if slot < claimed {
            Color::srgb(1.0, 0.84, 0.3)
        } else {
            Color::srgba(0.4, 1.0, 0.6, 0.35)
        };
        gizmos.rect_2d(Vec2::new(EXIT_X + 56.0, y), Vec2::splat(26.0), color);
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, DirectorRuntime>,
    world: Res<'w, DirectorWorld>,
    racers: Query<'w, 's, (), With<Racer>>,
    ui_roots: Query<'w, 's, (), With<DirUiRoot>>,
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

    let racers = context.racers.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = racers == RUNNER_COUNT && ui_roots == 1;

    let mut text = context.text.into_inner();
    **text = format!(
        "FACILITY DIRECTOR  {}\n\
         status         {}\n\
         director taken {} / {}\n\
         collapse line  {:.0}%\n\
         collapse rate  {:.2}/s\n\
         exits free     {} / {}\n\
         interventions  {}\n\
         tick           {}\n\n\
         {}\n\
         racers {racers}  UI {ui_roots}   resets {}\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        if world.finished {
            "RUN OVER"
        } else {
            "fleeing"
        },
        world.director_strength(),
        RUNNER_COUNT,
        world.purge_line.clamp(0.0, 1.0) * 100.0,
        world.purge_rate(),
        world.slots_remaining,
        world.exit_capacity,
        world.director_actions,
        world.tick_count,
        standings.trim_end(),
        context.runtime.reset_count,
        world.last_event,
    );
}
