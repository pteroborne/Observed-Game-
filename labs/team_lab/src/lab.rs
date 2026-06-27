use bevy::{ecs::system::SystemParam, prelude::*};
use observed_core::PlayerId;

use crate::{
    input::HumanInput,
    model::{ItemId, StationId, StationKind, TeamIntent, TeamWorld},
};

const TEAM_COLORS: [Color; 2] = [Color::srgb(0.30, 0.75, 1.0), Color::srgb(1.0, 0.55, 0.28)];

/// Present when capturing an evidence screenshot: agents are frozen so the
/// authored showcase stays legible while the machine keeps making progress.
#[derive(Resource)]
pub(crate) struct FreezeAgents;

#[derive(Component)]
pub(crate) struct TeamLabOwned;

#[derive(Component)]
pub(crate) struct TeamLabUiRoot;

#[derive(Component)]
pub(crate) struct PlayerDot(pub PlayerId);

#[derive(Component)]
pub(crate) struct ItemDot(pub ItemId);

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct TeamLabRuntime {
    pub selected_player: PlayerId,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    pub last_event: String,
}

impl Default for TeamLabRuntime {
    fn default() -> Self {
        Self {
            selected_player: PlayerId(0),
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
            last_event: "Showcase ready: Team 2 operates the machine; Team 1 is split.".to_string(),
        }
    }
}

pub(crate) fn setup_lab(mut commands: Commands, world: Res<TeamWorld>) {
    spawn_scene(&mut commands, &world);
    spawn_ui(&mut commands);
}

fn spawn_scene(commands: &mut Commands, world: &TeamWorld) {
    commands
        .spawn((
            TeamLabOwned,
            Name::new("Team Scene Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for zone in &world.zones {
                let color = if zone.name == "RALLY" {
                    Color::srgb(0.10, 0.16, 0.14)
                } else {
                    Color::srgb(0.08, 0.11, 0.16)
                };
                parent.spawn((
                    Name::new(format!("Zone {}", zone.name)),
                    Sprite::from_color(color, zone.half_size * 2.0),
                    Transform::from_translation(zone.center.extend(-3.0)),
                ));
            }
            for station in &world.stations {
                parent.spawn((
                    Name::new(format!("Station {}", station.id.0)),
                    Sprite::from_color(
                        station_color(station.kind).with_alpha(0.35),
                        Vec2::splat(36.0),
                    ),
                    Transform::from_translation(station.position.extend(-1.0)),
                ));
            }
            for item in &world.items {
                parent.spawn((
                    ItemDot(item.id),
                    Name::new(format!("Item {}", item.id.0)),
                    Sprite::from_color(Color::srgb(0.85, 0.85, 0.5), Vec2::splat(18.0)),
                    Transform::from_translation(item.position.extend(1.0)),
                ));
            }
            for player in &world.players {
                parent.spawn((
                    PlayerDot(player.id),
                    Name::new(format!("{} dot", player.id.label())),
                    Sprite::from_color(TEAM_COLORS[player.team.index() % 2], Vec2::new(28.0, 48.0)),
                    Transform::from_translation(player.position.extend(2.0)),
                ));
            }
        });
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            TeamLabOwned,
            TeamLabUiRoot,
            Name::new("Team Lab UI Root"),
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
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.45, 0.80, 1.0, 0.6)),
                children![(
                    DebugText,
                    Text::new("Team diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.80, 0.92, 1.0)),
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
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.45, 0.80, 1.0, 0.6)),
                children![(
                    Text::new(
                        "TEAM LAB\n\
                         WASD / arrows  Move selected player\n\
                         E              Use nearest station in reach\n\
                         Q              Release current station\n\
                         F              Grab nearest item\n\
                         G              Drop carried item\n\
                         1–4            Select player (others are bots)\n\
                         R              Reset · F1 Toggle debug\n\n\
                         Two teams of two share narrow passages (cap 1), a\n\
                         climb point (cap 3), and a machine (needs 2). Contention\n\
                         resolves by player id; cohesion tracks who is together.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.86, 0.90, 0.95)),
                )],
            ));
        });
}

pub(crate) fn handle_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<TeamLabRuntime>,
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

pub(crate) fn simulate(
    time: Res<Time>,
    frozen: Option<Res<FreezeAgents>>,
    input: Res<HumanInput>,
    mut runtime: ResMut<TeamLabRuntime>,
    mut world: ResMut<TeamWorld>,
) {
    let dt = time.delta_secs();
    let intents: Vec<(PlayerId, TeamIntent)> = if frozen.is_some() {
        Vec::new()
    } else {
        let elapsed = time.elapsed_secs();
        let selected = runtime.selected_player;
        world
            .players
            .iter()
            .map(|player| {
                let intent = if player.id == selected {
                    human_intent(&world, player.id, &input)
                } else {
                    bot_intent(&world, player.id, player.position, elapsed)
                };
                (player.id, intent)
            })
            .collect()
    };

    world.tick(&intents, dt);

    if let Some(event) = world.recent_events.last() {
        runtime.last_event = event.label();
    }
}

fn human_intent(world: &TeamWorld, player: PlayerId, input: &HumanInput) -> TeamIntent {
    let position = world.player(player).map(|p| p.position).unwrap_or_default();
    TeamIntent {
        movement: input.movement,
        use_station: input
            .use_pressed
            .then(|| nearest_station(world, position))
            .flatten(),
        release_station: input.release_pressed,
        grab_item: input
            .grab_pressed
            .then(|| nearest_item(world, position))
            .flatten(),
        drop_item: input.drop_pressed,
    }
}

fn bot_intent(world: &TeamWorld, player: PlayerId, position: Vec2, elapsed: f32) -> TeamIntent {
    if world.stations.is_empty() {
        return TeamIntent::default();
    }
    // Cycle targets over time so bots converge on shared stations and contend.
    let index = ((elapsed / 4.0) as usize + player.index()) % world.stations.len();
    let target = &world.stations[index];
    let to_target = target.position - position;
    if target.in_range(position) {
        TeamIntent {
            use_station: Some(target.id),
            ..default()
        }
    } else {
        TeamIntent {
            movement: to_target.normalize_or_zero(),
            ..default()
        }
    }
}

fn nearest_station(world: &TeamWorld, position: Vec2) -> Option<StationId> {
    world
        .stations
        .iter()
        .filter(|station| station.in_range(position))
        .min_by(|a, b| {
            a.position
                .distance(position)
                .total_cmp(&b.position.distance(position))
        })
        .map(|station| station.id)
}

fn nearest_item(world: &TeamWorld, position: Vec2) -> Option<ItemId> {
    world
        .items
        .iter()
        .filter(|item| item.holder.is_none() && item.position.distance(position) <= item.radius)
        .min_by(|a, b| {
            a.position
                .distance(position)
                .total_cmp(&b.position.distance(position))
        })
        .map(|item| item.id)
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<TeamLabRuntime>,
    mut world: ResMut<TeamWorld>,
    mut input: ResMut<HumanInput>,
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
    world.reset();
    *input = HumanInput::default();
}

pub(crate) fn present_players(
    runtime: Res<TeamLabRuntime>,
    world: Res<TeamWorld>,
    mut dots: Query<(&PlayerDot, &mut Transform, &mut Sprite)>,
) {
    for (dot, mut transform, mut sprite) in &mut dots {
        if let Some(player) = world.player(dot.0) {
            transform.translation.x = player.position.x;
            transform.translation.y = player.position.y;
            let base = TEAM_COLORS[player.team.index() % 2];
            sprite.color = if dot.0 == runtime.selected_player {
                base.mix(&Color::WHITE, 0.4)
            } else {
                base
            };
        }
    }
}

pub(crate) fn present_items(
    world: Res<TeamWorld>,
    mut dots: Query<(&ItemDot, &mut Transform, &mut Sprite)>,
) {
    for (dot, mut transform, mut sprite) in &mut dots {
        if let Some(item) = world.item(dot.0) {
            let position = match item.holder.and_then(|holder| world.player(holder)) {
                Some(holder) => holder.position + Vec2::new(0.0, 36.0),
                None => item.position,
            };
            transform.translation.x = position.x;
            transform.translation.y = position.y;
            sprite.color = if item.holder.is_some() {
                Color::srgb(1.0, 0.95, 0.5)
            } else {
                Color::srgb(0.7, 0.7, 0.45)
            };
        }
    }
}

fn station_color(kind: StationKind) -> Color {
    match kind {
        StationKind::NarrowPassage => Color::srgb(0.95, 0.42, 0.42),
        StationKind::ClimbPoint => Color::srgb(0.42, 0.92, 0.55),
        StationKind::Machine => Color::srgb(0.95, 0.85, 0.32),
    }
}

pub(crate) fn draw_debug(runtime: Res<TeamLabRuntime>, world: Res<TeamWorld>, mut gizmos: Gizmos) {
    if !runtime.debug_visible {
        return;
    }

    for zone in &world.zones {
        gizmos.rect_2d(
            zone.center,
            zone.half_size * 2.0,
            Color::srgb(0.25, 0.35, 0.45),
        );
    }

    for station in &world.stations {
        let color = station_color(station.kind);
        gizmos.circle_2d(station.position, station.radius, color);
        // Capacity dots above the station, filled for current occupants.
        for slot in 0..station.capacity {
            let filled = slot < station.occupants.len();
            let x = station.position.x - (station.capacity as f32 - 1.0) * 7.0 + slot as f32 * 14.0;
            let point = Vec2::new(x, station.position.y + station.radius + 14.0);
            gizmos.circle_2d(
                point,
                5.0,
                if filled {
                    color
                } else {
                    color.with_alpha(0.25)
                },
            );
        }
        if station.kind == StationKind::Machine && station.progress > 0.0 {
            gizmos.circle_2d(
                station.position,
                station.radius * station.progress.clamp(0.05, 1.0),
                Color::srgb(1.0, 0.95, 0.5),
            );
        }
    }

    for item in &world.items {
        if item.holder.is_none() {
            gizmos.circle_2d(
                item.position,
                item.radius,
                Color::srgba(0.85, 0.85, 0.5, 0.5),
            );
        }
    }

    for player in &world.players {
        // Line to the station this player occupies.
        if let Some(station) = player.occupying.and_then(|id| world.station(id)) {
            gizmos.line_2d(
                player.position,
                station.position,
                Color::srgba(0.9, 0.95, 1.0, 0.5),
            );
        }
        // Line to a carried item.
        if let Some(item) = player.carrying.and_then(|id| world.item(id)) {
            gizmos.line_2d(
                player.position,
                item.position,
                Color::srgba(1.0, 0.95, 0.5, 0.7),
            );
        }
        if player.id == runtime.selected_player {
            gizmos.circle_2d(player.position, 34.0, Color::srgb(0.6, 1.0, 0.8));
        }
    }

    // Cohesion line per team: green together, red apart.
    for team in &world.teams {
        let members: Vec<Vec2> = world
            .team_members(*team)
            .map(|player| player.position)
            .collect();
        if members.len() == 2 {
            let color = if world.cohesive(*team) {
                Color::srgb(0.35, 1.0, 0.5)
            } else {
                Color::srgba(1.0, 0.4, 0.4, 0.7)
            };
            gizmos.line_2d(members[0], members[1], color);
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, TeamLabRuntime>,
    world: Res<'w, TeamWorld>,
    player_dots: Query<'w, 's, (), With<PlayerDot>>,
    item_dots: Query<'w, 's, (), With<ItemDot>>,
    ui_roots: Query<'w, 's, (), With<TeamLabUiRoot>>,
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

    let player_dots = context.player_dots.iter().count();
    let item_dots = context.item_dots.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy =
        player_dots == world.players.len() && item_dots == world.items.len() && ui_roots == 1;

    let mut teams_text = String::new();
    for team in &world.teams {
        let zone = world
            .team_members(*team)
            .map(|player| world.zone_of(player.position))
            .next()
            .flatten();
        let zone_name = zone
            .and_then(|id| world.zones.iter().find(|z| z.id == id))
            .map(|z| z.name)
            .unwrap_or("split");
        teams_text.push_str(&format!(
            "{}: {}{}\n",
            team.label(),
            if world.cohesive(*team) {
                "TOGETHER"
            } else {
                "apart"
            },
            if world.cohesive(*team) {
                format!(" in {zone_name}")
            } else {
                String::new()
            }
        ));
    }

    let mut stations_text = String::new();
    for station in &world.stations {
        let extra = if station.kind == StationKind::Machine {
            format!(
                " prog {:>3.0}% acts {}",
                station.progress * 100.0,
                station.activations
            )
        } else {
            String::new()
        };
        stations_text.push_str(&format!(
            "s{} {:<14} {}/{}{}\n",
            station.id.0,
            station.kind.label(),
            station.occupants.len(),
            station.capacity,
            extra
        ));
    }

    let selected = context.runtime.selected_player;
    let selected_line = match world.player(selected) {
        Some(player) => format!(
            "{} {} occupy:{} carry:{}",
            player.id.label(),
            player.team.label(),
            player
                .occupying
                .map(|s| format!("s{}", s.0))
                .unwrap_or_else(|| "-".to_string()),
            player
                .carrying
                .map(|i| format!("i{}", i.0))
                .unwrap_or_else(|| "-".to_string()),
        ),
        None => "no selection".to_string(),
    };

    let mut text = context.text.into_inner();
    **text = format!(
        "TEAM MONITOR  {}\n\
         selected  {}\n\
         {}\
         reunions {}  separations {}  denials {}\n\
         {}\
         players {}  dots {}/{}  items {}/{}  UI {}\n\
         resets {}  debug {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        selected_line,
        teams_text,
        world.reunions,
        world.separations,
        world.denials,
        stations_text,
        world.players.len(),
        player_dots,
        world.players.len(),
        item_dots,
        world.items.len(),
        ui_roots,
        context.runtime.reset_count,
        if context.runtime.debug_visible {
            "ON"
        } else {
            "OFF"
        },
        context.runtime.last_event,
    );
}
