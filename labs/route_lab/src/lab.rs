use bevy::{ecs::system::SystemParam, prelude::*};
use observation_lab::model::{DOOR_COUNT, DoorId, ROOM_COUNT, ROOM_HALF};
use observed_core::{RoomId, TeamId};

use crate::model::{CABLE_CAPACITY, RouteWorld, TEAM_COUNT};

const TEAM_COLORS: [Color; TEAM_COUNT] =
    [Color::srgb(0.35, 0.72, 1.0), Color::srgb(1.0, 0.60, 0.28)];

const CYAN: Color = Color::srgb(0.30, 0.80, 1.0);

#[derive(Component)]
pub(crate) struct RouteOwned;

#[derive(Component)]
pub(crate) struct RouteUiRoot;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct RouteRuntime {
    pub selected_team: TeamId,
    pub selected_door: DoorId,
    pub auto_decohere: bool,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
}

impl Default for RouteRuntime {
    fn default() -> Self {
        Self {
            selected_team: TeamId(0),
            selected_door: DoorId(0),
            auto_decohere: true,
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
        }
    }
}

#[derive(Resource)]
pub struct DecohereTimer(pub Timer);

impl Default for DecohereTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(2.5, TimerMode::Repeating))
    }
}

pub(crate) fn setup_lab(mut commands: Commands, world: Res<RouteWorld>) {
    commands
        .spawn((
            RouteOwned,
            Name::new("Structure Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for index in 0..ROOM_COUNT {
                parent.spawn((
                    Name::new(format!("Room {index}")),
                    Sprite::from_color(
                        Color::srgb(0.07, 0.10, 0.15),
                        Vec2::splat(ROOM_HALF * 2.0 - 6.0),
                    ),
                    Transform::from_translation(
                        world.graph.room_center(RoomId(index as u32)).extend(-2.0),
                    ),
                ));
            }
        });

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            RouteOwned,
            RouteUiRoot,
            Name::new("Route Lab UI Root"),
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
                BorderColor::all(Color::srgba(0.45, 0.8, 1.0, 0.6)),
                children![(
                    DebugText,
                    Text::new("Route diagnostics starting…"),
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
                BorderColor::all(Color::srgba(0.45, 0.8, 1.0, 0.6)),
                children![(
                    Text::new(
                        "PERSISTENT ROUTE LAB\n\
                         Tab     Select next doorway\n\
                         C       Cable the selected doorway (your team)\n\
                         X       Cut the cable on the selected doorway\n\
                         Space   Decohere now · P pause auto-decoherence\n\
                         1–2     Select your team\n\
                         R       Reset · F1 Toggle debug\n\n\
                         Cabled routes (team-coloured) survive decoherence; cyan\n\
                         routes rewire. Cable budget is limited; an opponent can cut\n\
                         your cable to send that route back into the churn.",
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
    mut runtime: ResMut<RouteRuntime>,
    mut world: ResMut<RouteWorld>,
) {
    for (key, team) in [(KeyCode::Digit1, TeamId(0)), (KeyCode::Digit2, TeamId(1))] {
        if keyboard.just_pressed(key) {
            runtime.selected_team = team;
        }
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        runtime.selected_door = DoorId((runtime.selected_door.0 + 1) % DOOR_COUNT as u16);
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        world.deploy_cable(runtime.selected_team, runtime.selected_door);
    }
    if keyboard.just_pressed(KeyCode::KeyX) {
        world.cut_on(runtime.selected_team, runtime.selected_door);
    }
    if keyboard.just_pressed(KeyCode::Space) {
        world.decohere();
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        runtime.auto_decohere = !runtime.auto_decohere;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
}

pub(crate) fn auto_decohere(
    time: Res<Time>,
    runtime: Res<RouteRuntime>,
    mut timer: ResMut<DecohereTimer>,
    mut world: ResMut<RouteWorld>,
) {
    if runtime.auto_decohere && timer.0.tick(time.delta()).just_finished() {
        world.decohere();
    }
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<RouteRuntime>,
    mut world: ResMut<RouteWorld>,
    mut timer: ResMut<DecohereTimer>,
) {
    if !runtime.reset_requested {
        return;
    }
    runtime.reset_requested = false;
    runtime.reset_count += 1;
    world.reset();
    timer.0.reset();
}

fn team_color(world: &RouteWorld, team: TeamId) -> Color {
    world
        .teams
        .iter()
        .position(|t| *t == team)
        .map(|i| TEAM_COLORS[i % TEAM_COUNT])
        .unwrap_or(CYAN)
}

pub(crate) fn draw_debug(runtime: Res<RouteRuntime>, world: Res<RouteWorld>, mut gizmos: Gizmos) {
    if !runtime.debug_visible {
        return;
    }

    for index in 0..ROOM_COUNT {
        let room = RoomId(index as u32);
        gizmos.rect_2d(
            world.graph.room_center(room),
            Vec2::splat(ROOM_HALF * 2.0),
            Color::srgb(0.22, 0.30, 0.40),
        );
    }

    // Connections: cabled routes in the owner's colour, free routes cyan.
    for (a, b) in world.graph.connections() {
        let color = match world.cable_on(a) {
            Some(cable) => team_color(&world, cable.owner),
            None => CYAN,
        };
        let pa = world.graph.door_position(a);
        let pb = world.graph.door_position(b);
        gizmos.line_2d(pa, pb, color);
        if world.cable_on(a).is_some() {
            // Emphasize a cabled route with a parallel strand.
            let offset = Vec2::new(0.0, 4.0);
            gizmos.line_2d(pa + offset, pb + offset, color);
        }
    }

    // Doorway markers.
    for index in 0..DOOR_COUNT {
        let door = DoorId(index as u16);
        let position = world.graph.door_position(door);
        if world.graph.is_sealed(door) {
            gizmos.circle_2d(position, 5.0, Color::srgba(0.5, 0.55, 0.6, 0.7));
        } else if let Some(cable) = world.cable_on(door) {
            gizmos.circle_2d(position, 6.0, team_color(&world, cable.owner));
        } else {
            gizmos.circle_2d(position, 4.0, CYAN);
        }
    }

    // The selected doorway.
    let selected = world.graph.door_position(runtime.selected_door);
    gizmos.circle_2d(selected, 14.0, team_color(&world, runtime.selected_team));
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, RouteRuntime>,
    world: Res<'w, RouteWorld>,
    rooms: Query<'w, 's, (), With<Sprite>>,
    ui_roots: Query<'w, 's, (), With<RouteUiRoot>>,
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

    let mut budgets = String::new();
    for team in &world.teams {
        budgets.push_str(&format!(
            "{:<8} {}/{} cable\n",
            team.label(),
            world.budget_of(*team),
            CABLE_CAPACITY
        ));
    }

    let door = world.graph.door(context.runtime.selected_door);
    let selected = format!("R{} {}", door.room.0, door.side.label());

    let rooms = context.rooms.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = rooms == ROOM_COUNT && ui_roots == 1;

    let mut text = context.text.into_inner();
    **text = format!(
        "PERSISTENT ROUTE MONITOR  {}\n\
         decoherence events  {}\n\
         cabled routes       {}\n\
         contests (cuts)     {}\n\
         {}\
         selected            {} ({})  doorway {}\n\
         rooms {rooms}  UI {ui_roots}   resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        world.decohere_count,
        world.cables.len(),
        world.contests,
        budgets,
        context.runtime.selected_team.label(),
        if context.runtime.auto_decohere {
            "auto"
        } else {
            "paused"
        },
        selected,
        context.runtime.reset_count,
        world.last_event,
    );
}
