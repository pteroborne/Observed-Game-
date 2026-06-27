use bevy::{ecs::system::SystemParam, prelude::*};
use observed_core::{PlayerId, TeamId};

use crate::model::{
    DirectorHazardAction, HazardWorld, HazardZoneId, PLAYER_COUNT, PlayerHazardIntent,
    ROUTE_LENGTH, TEAM_COUNT, ZONE_COUNT,
};

const TEAM_COLORS: [Color; TEAM_COUNT] =
    [Color::srgb(0.28, 0.78, 1.0), Color::srgb(1.0, 0.66, 0.28)];
const ZONE_CENTERS: [f32; ZONE_COUNT] = [-360.0, 0.0, 360.0];
const TRACK_START: f32 = -510.0;
const TRACK_END: f32 = 510.0;
const LANE_Y: [f32; TEAM_COUNT] = [115.0, -115.0];

fn progress_x(progress: u8) -> f32 {
    TRACK_START + (TRACK_END - TRACK_START) * progress as f32 / ROUTE_LENGTH as f32
}

#[derive(Component)]
pub(crate) struct HazardOwned;

#[derive(Component)]
pub(crate) struct HazardUiRoot;

#[derive(Component)]
pub(crate) struct PlayerDot(pub PlayerId);

#[derive(Component)]
pub(crate) struct TeamMarker(pub TeamId);

#[derive(Component)]
pub(crate) struct HazardField;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct HazardRuntime {
    pub selected_player: PlayerId,
    pub staged_intents: [PlayerHazardIntent; PLAYER_COUNT],
    pub director_action: DirectorHazardAction,
    pub step_requested: bool,
    pub reset_requested: bool,
    pub debug_visible: bool,
    pub reset_count: u32,
}

impl Default for HazardRuntime {
    fn default() -> Self {
        Self {
            selected_player: PlayerId(0),
            staged_intents: [
                PlayerHazardIntent::VentA,
                PlayerHazardIntent::VentB,
                PlayerHazardIntent::Advance,
                PlayerHazardIntent::Advance,
            ],
            director_action: DirectorHazardAction::Hold,
            step_requested: false,
            reset_requested: false,
            debug_visible: true,
            reset_count: 0,
        }
    }
}

pub(crate) fn setup_lab(mut commands: Commands) {
    commands
        .spawn((
            HazardOwned,
            Name::new("Hazard Lab World"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (index, center) in ZONE_CENTERS.iter().enumerate() {
                parent.spawn((
                    Name::new(format!("Zone {}", index + 1)),
                    Sprite::from_color(
                        Color::srgba(0.12, 0.18, 0.23, 0.55),
                        Vec2::new(310.0, 410.0),
                    ),
                    Transform::from_xyz(*center, 0.0, -5.0),
                ));
            }

            parent.spawn((
                HazardField,
                Name::new("Active Pressure Front"),
                Sprite::from_color(Color::srgba(1.0, 0.18, 0.12, 0.18), Vec2::new(310.0, 410.0)),
                Transform::from_xyz(ZONE_CENTERS[0], 0.0, -3.0),
            ));

            for team in 0..TEAM_COUNT {
                parent.spawn((
                    TeamMarker(TeamId(team as u8)),
                    Name::new(format!("{} progress", TeamId(team as u8).label())),
                    Sprite::from_color(TEAM_COLORS[team], Vec2::new(34.0, 58.0)),
                    Transform::from_xyz(TRACK_START, LANE_Y[team], 4.0),
                ));
            }

            for player in 0..PLAYER_COUNT {
                let team = player / 2;
                let offset = if player % 2 == 0 { -22.0 } else { 22.0 };
                parent.spawn((
                    PlayerDot(PlayerId(player as u16)),
                    Name::new(format!(
                        "{} hazard operator",
                        PlayerId(player as u16).label()
                    )),
                    Sprite::from_color(TEAM_COLORS[team], Vec2::splat(18.0)),
                    Transform::from_xyz(TRACK_START + offset, LANE_Y[team] + 44.0, 6.0),
                ));
            }
        });

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            HazardOwned,
            HazardUiRoot,
            Name::new("Hazard Lab UI Root"),
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
                BackgroundColor(Color::srgba(0.015, 0.025, 0.035, 0.95)),
                BorderColor::all(Color::srgba(0.45, 0.85, 1.0, 0.65)),
                children![(
                    DebugText,
                    Text::new("Hazard diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.84, 0.94, 1.0)),
                )],
            ));
            root.spawn((
                HelpText,
                Node {
                    width: px(440),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.015, 0.025, 0.035, 0.95)),
                BorderColor::all(Color::srgba(1.0, 0.55, 0.3, 0.65)),
                children![(
                    Text::new(
                        "PHASE 15 - COOPERATIVE HAZARD\n\
                         1-4      Select player\n\
                         A        Stage ADVANCE\n\
                         Q / E    Stage VENT A / VENT B\n\
                         W        Stage WAIT\n\
                         Z / X / C  Director steers INTAKE / CORE / SPINE\n\
                         Space    Resolve one round\n\
                         R        Reset / F1 Toggle debug\n\n\
                         Both relief valves need distinct operators in the active\n\
                         zone. Operators may come from different teams. If either\n\
                         valve is empty, the pressure front stalls teams there;\n\
                         it never damages players or removes earned progress.",
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
    mut runtime: ResMut<HazardRuntime>,
) {
    for (key, player) in [
        (KeyCode::Digit1, PlayerId(0)),
        (KeyCode::Digit2, PlayerId(1)),
        (KeyCode::Digit3, PlayerId(2)),
        (KeyCode::Digit4, PlayerId(3)),
    ] {
        if keyboard.just_pressed(key) {
            runtime.selected_player = player;
        }
    }

    let staged = if keyboard.just_pressed(KeyCode::KeyA) {
        Some(PlayerHazardIntent::Advance)
    } else if keyboard.just_pressed(KeyCode::KeyQ) {
        Some(PlayerHazardIntent::VentA)
    } else if keyboard.just_pressed(KeyCode::KeyE) {
        Some(PlayerHazardIntent::VentB)
    } else if keyboard.just_pressed(KeyCode::KeyW) {
        Some(PlayerHazardIntent::Wait)
    } else {
        None
    };
    if let Some(intent) = staged {
        let index = runtime.selected_player.0 as usize;
        if let Some(slot) = runtime.staged_intents.get_mut(index) {
            *slot = intent;
        }
    }

    for (key, zone) in [
        (KeyCode::KeyZ, HazardZoneId(0)),
        (KeyCode::KeyX, HazardZoneId(1)),
        (KeyCode::KeyC, HazardZoneId(2)),
    ] {
        if keyboard.just_pressed(key) {
            runtime.director_action = DirectorHazardAction::Steer(zone);
        }
    }
    if keyboard.just_pressed(KeyCode::Space) {
        runtime.step_requested = true;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
}

pub(crate) fn perform_reset(mut runtime: ResMut<HazardRuntime>, mut world: ResMut<HazardWorld>) {
    if !runtime.reset_requested {
        return;
    }
    let debug_visible = runtime.debug_visible;
    let reset_count = runtime.reset_count + 1;
    *runtime = HazardRuntime {
        debug_visible,
        reset_count,
        ..default()
    };
    world.reset();
}

pub(crate) fn simulate(mut runtime: ResMut<HazardRuntime>, mut world: ResMut<HazardWorld>) {
    if !runtime.step_requested {
        return;
    }
    runtime.step_requested = false;
    let intents = world
        .players
        .iter()
        .map(|player| {
            (
                player.id,
                runtime
                    .staged_intents
                    .get(player.id.0 as usize)
                    .copied()
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    world.resolve_round(&intents, runtime.director_action);
    runtime.director_action = DirectorHazardAction::Hold;
}

type TeamMarkerQuery<'w, 's> = Query<
    'w,
    's,
    (&'static TeamMarker, &'static mut Transform),
    (Without<PlayerDot>, Without<HazardField>),
>;
type PlayerPresentationQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static PlayerDot,
        &'static mut Transform,
        &'static mut Sprite,
    ),
    (Without<TeamMarker>, Without<HazardField>),
>;
type HazardPresentation<'w, 's> = Single<
    'w,
    's,
    (&'static mut Transform, &'static mut Sprite),
    (With<HazardField>, Without<TeamMarker>, Without<PlayerDot>),
>;

#[derive(SystemParam)]
pub(crate) struct PresentationContext<'w, 's> {
    team_markers: TeamMarkerQuery<'w, 's>,
    players: PlayerPresentationQuery<'w, 's>,
    hazard: HazardPresentation<'w, 's>,
}

pub(crate) fn present(
    runtime: Res<HazardRuntime>,
    world: Res<HazardWorld>,
    mut context: PresentationContext,
) {
    for (marker, mut transform) in &mut context.team_markers {
        if let Some(team) = world.team(marker.0) {
            transform.translation.x = progress_x(team.progress);
        }
    }

    for (dot, mut transform, mut sprite) in &mut context.players {
        let Some(player) = world.player(dot.0) else {
            continue;
        };
        let Some(team) = world.team(player.team) else {
            continue;
        };
        let team_index = player.team.index();
        let member_offset = if dot.0.0 % 2 == 0 { -22.0 } else { 22.0 };
        transform.translation.x = progress_x(team.progress) + member_offset;
        transform.translation.y = LANE_Y[team_index] + 44.0;
        let intent = runtime
            .staged_intents
            .get(dot.0.0 as usize)
            .copied()
            .unwrap_or_default();
        let base = match intent {
            PlayerHazardIntent::Advance => TEAM_COLORS[team_index],
            PlayerHazardIntent::VentA => Color::srgb(0.35, 1.0, 0.92),
            PlayerHazardIntent::VentB => Color::srgb(1.0, 0.45, 0.92),
            PlayerHazardIntent::Wait => Color::srgb(0.45, 0.48, 0.52),
        };
        sprite.color = if dot.0 == runtime.selected_player {
            base.mix(&Color::WHITE, 0.4)
        } else {
            base
        };
    }

    let (mut hazard_transform, mut hazard_sprite) = context.hazard.into_inner();
    hazard_transform.translation.x = ZONE_CENTERS[world.hazard.zone.0 as usize];
    hazard_sprite.color = if world.hazard.contained_last_round {
        Color::srgba(0.2, 1.0, 0.65, 0.16)
    } else {
        Color::srgba(1.0, 0.15, 0.10, 0.10 + world.hazard.pressure as f32 * 0.07)
    };
}

pub(crate) fn draw_debug(runtime: Res<HazardRuntime>, world: Res<HazardWorld>, mut gizmos: Gizmos) {
    if !runtime.debug_visible {
        return;
    }

    for (index, center) in ZONE_CENTERS.iter().enumerate() {
        let active = world.hazard.zone == HazardZoneId(index as u8);
        gizmos.rect_2d(
            Vec2::new(*center, 0.0),
            Vec2::new(310.0, 410.0),
            if active {
                if world.hazard.contained_last_round {
                    Color::srgb(0.25, 1.0, 0.62)
                } else {
                    Color::srgb(1.0, 0.28, 0.18)
                }
            } else {
                Color::srgba(0.35, 0.55, 0.68, 0.5)
            },
        );
    }

    for (index, team) in world.teams.iter().enumerate() {
        gizmos.line_2d(
            Vec2::new(TRACK_START, LANE_Y[index]),
            Vec2::new(TRACK_END, LANE_Y[index]),
            Color::srgba(0.5, 0.58, 0.64, 0.45),
        );
        gizmos.line_2d(
            Vec2::new(TRACK_START, LANE_Y[index]),
            Vec2::new(progress_x(team.progress), LANE_Y[index]),
            TEAM_COLORS[index],
        );
    }

    let hazard_x = ZONE_CENTERS[world.hazard.zone.0 as usize];
    let valve_a = Vec2::new(hazard_x - 48.0, 258.0);
    let valve_b = Vec2::new(hazard_x + 48.0, 258.0);
    gizmos.circle_2d(valve_a, 18.0, Color::srgb(0.35, 1.0, 0.92));
    gizmos.circle_2d(valve_b, 18.0, Color::srgb(1.0, 0.45, 0.92));

    for player in &world.players {
        let Some(team) = world.team(player.team) else {
            continue;
        };
        let player_position = Vec2::new(
            progress_x(team.progress) + if player.id.0 % 2 == 0 { -22.0 } else { 22.0 },
            LANE_Y[player.team.index()] + 44.0,
        );
        match runtime
            .staged_intents
            .get(player.id.0 as usize)
            .copied()
            .unwrap_or_default()
        {
            PlayerHazardIntent::VentA => {
                gizmos.line_2d(player_position, valve_a, Color::srgba(0.35, 1.0, 0.92, 0.7))
            }
            PlayerHazardIntent::VentB => {
                gizmos.line_2d(player_position, valve_b, Color::srgba(1.0, 0.45, 0.92, 0.7))
            }
            PlayerHazardIntent::Advance | PlayerHazardIntent::Wait => {}
        }
        if player.id == runtime.selected_player {
            gizmos.circle_2d(player_position, 19.0, Color::WHITE);
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, HazardRuntime>,
    world: Res<'w, HazardWorld>,
    player_dots: Query<'w, 's, (), With<PlayerDot>>,
    team_markers: Query<'w, 's, (), With<TeamMarker>>,
    hazard_fields: Query<'w, 's, (), With<HazardField>>,
    ui_roots: Query<'w, 's, (), With<HazardUiRoot>>,
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
    let team_markers = context.team_markers.iter().count();
    let hazard_fields = context.hazard_fields.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = player_dots == PLAYER_COUNT
        && team_markers == TEAM_COUNT
        && hazard_fields == 1
        && ui_roots == 1;

    let zone_name = world
        .zone(world.hazard.zone)
        .map(|zone| zone.name)
        .unwrap_or("UNKNOWN");
    let venters = |players: &[PlayerId]| {
        if players.is_empty() {
            "-".to_string()
        } else {
            players
                .iter()
                .map(|player| player.label())
                .collect::<Vec<_>>()
                .join(", ")
        }
    };

    let mut staged = String::new();
    for player in &world.players {
        let marker = if player.id == context.runtime.selected_player {
            "*"
        } else {
            " "
        };
        let intent = context
            .runtime
            .staged_intents
            .get(player.id.0 as usize)
            .copied()
            .unwrap_or_default();
        staged.push_str(&format!(
            "{marker}{} {:<8} {}\n",
            player.id.label(),
            player.team.label(),
            intent.label()
        ));
    }

    let mut teams = String::new();
    for team in &world.teams {
        let team_zone = world
            .team_zone(team.id)
            .and_then(|id| world.zone(id))
            .map(|zone| zone.name)
            .unwrap_or("-");
        let status = team
            .completed_round
            .map(|round| format!("COMPLETE r{round}"))
            .unwrap_or_else(|| format!("{}/{}", team.progress, ROUTE_LENGTH));
        teams.push_str(&format!(
            "{} {:<6} progress {:<12} delays {}\n",
            team.id.label(),
            team_zone,
            status,
            team.delay_rounds
        ));
    }

    let mut text = context.text.into_inner();
    **text = format!(
        "HAZARD MONITOR  {}\n\
         round          {}\n\
         pressure front {}  level {}/3\n\
         last state     {}\n\
         director last  {}\n\
         director next  {}\n\
         steers         {}\n\
         vent A         {}\n\
         vent B         {}\n\
         vents / pulses {} / {}\n\n\
         {}\
         \n{}\
         dots {}/{}  markers {}/{}  hazard {}  UI {}\n\
         resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        world.round,
        zone_name,
        world.hazard.pressure,
        if world.hazard.contained_last_round {
            "CONTAINED"
        } else {
            "PULSING"
        },
        world.last_director_action.label(&world.zones),
        context.runtime.director_action.label(&world.zones),
        world.hazard.steer_count,
        venters(&world.last_vent_a),
        venters(&world.last_vent_b),
        world.vent_cycles,
        world.hazard.pulse_count,
        staged,
        teams,
        player_dots,
        PLAYER_COUNT,
        team_markers,
        TEAM_COUNT,
        hazard_fields,
        ui_roots,
        context.runtime.reset_count,
        world.last_event,
    );
}
