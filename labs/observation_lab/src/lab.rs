use bevy::{ecs::system::SystemParam, prelude::*};
use observed_core::RoomId;

use crate::model::{
    DOOR_COUNT, DoorId, ObservationWorld, PLAYER_COUNT, ROOM_COUNT, ROOM_HALF, Side,
};

const PLAYER_COLORS: [Color; PLAYER_COUNT] = [
    Color::srgb(0.30, 0.85, 1.0),
    Color::srgb(1.0, 0.42, 0.34),
    Color::srgb(0.62, 1.0, 0.36),
    Color::srgb(0.86, 0.46, 1.0),
];

const PLAYER_OFFSETS: [Vec2; PLAYER_COUNT] = [
    Vec2::new(-32.0, -32.0),
    Vec2::new(32.0, -32.0),
    Vec2::new(-32.0, 32.0),
    Vec2::new(32.0, 32.0),
];

#[derive(Component)]
pub(crate) struct ObsOwned;

#[derive(Component)]
pub(crate) struct ObsUiRoot;

#[derive(Component)]
pub(crate) struct PlayerDot(pub usize);

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct ObservationRuntime {
    pub selected_player: usize,
    pub debug_visible: bool,
    pub auto_decohere: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    pub last_event: String,
}

impl Default for ObservationRuntime {
    fn default() -> Self {
        Self {
            selected_player: 0,
            debug_visible: true,
            auto_decohere: true,
            reset_requested: false,
            reset_count: 0,
            last_event: "Watch a room to freeze its doors; look away and they rewire.".to_string(),
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

pub(crate) fn setup_lab(mut commands: Commands, world: Res<ObservationWorld>) {
    commands
        .spawn((
            ObsOwned,
            Name::new("Structure Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for index in 0..ROOM_COUNT {
                let room = RoomId(index as u32);
                parent.spawn((
                    Name::new(format!("Room {index}")),
                    Sprite::from_color(
                        Color::srgb(0.07, 0.10, 0.15),
                        Vec2::splat(ROOM_HALF * 2.0 - 6.0),
                    ),
                    Transform::from_translation(world.room_center(room).extend(-2.0)),
                ));
            }
        });

    commands
        .spawn((
            ObsOwned,
            Name::new("Observers Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (index, color) in PLAYER_COLORS.into_iter().enumerate() {
                parent.spawn((
                    PlayerDot(index),
                    Name::new(format!("Observer {}", index + 1)),
                    Sprite::from_color(color, Vec2::splat(30.0)),
                    Transform::from_translation(Vec3::new(0.0, 0.0, 5.0)),
                ));
            }
        });

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            ObsOwned,
            ObsUiRoot,
            Name::new("Observation Lab UI Root"),
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
                BorderColor::all(Color::srgba(0.45, 0.85, 1.0, 0.6)),
                children![(
                    DebugText,
                    Text::new("Observation diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.80, 0.94, 1.0)),
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
                BorderColor::all(Color::srgba(0.45, 0.85, 1.0, 0.6)),
                children![(
                    Text::new(
                        "OBSERVATION LAB\n\
                         WASD / arrows  Step the selected observer through a door\n\
                         Space          Decohere now (rewire unobserved doors)\n\
                         P              Pause / resume auto-decoherence\n\
                         1–4            Select an observer\n\
                         R              Reset · F1 Toggle debug\n\n\
                         Green doors are observed and frozen; cyan doors are\n\
                         unobserved and rewire on decoherence; grey ticks are\n\
                         sealed walls. Where a door leads depends on what is watched.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.86, 0.92, 0.97)),
                )],
            ));
        });
}

pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<ObservationRuntime>,
    mut world: ResMut<ObservationWorld>,
) {
    for (key, index) in [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
    ] {
        if keyboard.just_pressed(key) {
            runtime.selected_player = index;
        }
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        runtime.auto_decohere = !runtime.auto_decohere;
    }
    if keyboard.just_pressed(KeyCode::Space) {
        world.decohere();
        runtime.last_event = format!(
            "Decohered: {} doors rewired, {} locked by observation.",
            world.rewires_last, world.locked_last
        );
    }

    for (keys, side) in [
        ([KeyCode::KeyW, KeyCode::ArrowUp], Side::North),
        ([KeyCode::KeyD, KeyCode::ArrowRight], Side::East),
        ([KeyCode::KeyS, KeyCode::ArrowDown], Side::South),
        ([KeyCode::KeyA, KeyCode::ArrowLeft], Side::West),
    ] {
        if keys.iter().any(|key| keyboard.just_pressed(*key)) {
            let player = runtime.selected_player;
            if world.traverse(player, side) {
                runtime.last_event = format!(
                    "Observer {} stepped {} into room {}.",
                    player + 1,
                    side.label(),
                    world.players[player].0
                );
            } else {
                runtime.last_event = format!(
                    "Observer {} hit a sealed wall ({}).",
                    player + 1,
                    side.label()
                );
            }
        }
    }
}

pub(crate) fn auto_decohere(
    time: Res<Time>,
    runtime: Res<ObservationRuntime>,
    mut timer: ResMut<DecohereTimer>,
    mut world: ResMut<ObservationWorld>,
) {
    if !runtime.auto_decohere {
        return;
    }
    if timer.0.tick(time.delta()).just_finished() {
        world.decohere();
    }
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<ObservationRuntime>,
    mut world: ResMut<ObservationWorld>,
    mut timer: ResMut<DecohereTimer>,
) {
    if !runtime.reset_requested {
        return;
    }
    runtime.reset_requested = false;
    runtime.reset_count += 1;
    runtime.last_event = format!(
        "Reset {} restored the authored structure.",
        runtime.reset_count
    );
    world.reset();
    timer.0.reset();
}

pub(crate) fn present_players(
    runtime: Res<ObservationRuntime>,
    world: Res<ObservationWorld>,
    mut dots: Query<(&PlayerDot, &mut Transform, &mut Sprite)>,
) {
    for (dot, mut transform, mut sprite) in &mut dots {
        if let Some(&room) = world.players.get(dot.0) {
            let position = world.room_center(room) + PLAYER_OFFSETS[dot.0 % PLAYER_COUNT];
            transform.translation.x = position.x;
            transform.translation.y = position.y;
            let base = PLAYER_COLORS[dot.0 % PLAYER_COUNT];
            sprite.color = if dot.0 == runtime.selected_player {
                base.mix(&Color::WHITE, 0.4)
            } else {
                base
            };
        }
    }
}

pub(crate) fn draw_debug(
    runtime: Res<ObservationRuntime>,
    world: Res<ObservationWorld>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    // Rooms: brighter border when observed.
    for index in 0..ROOM_COUNT {
        let room = RoomId(index as u32);
        let color = if world.observed(room) {
            Color::srgb(0.45, 0.95, 0.70)
        } else {
            Color::srgb(0.22, 0.30, 0.40)
        };
        gizmos.rect_2d(world.room_center(room), Vec2::splat(ROOM_HALF * 2.0), color);
    }

    // Sealed walls: a tick at the door; doorway markers coloured by pin state.
    for index in 0..DOOR_COUNT {
        let door = DoorId(index as u16);
        let position = world.door_position(door);
        if world.is_sealed(door) {
            gizmos.circle_2d(position, 5.0, Color::srgba(0.5, 0.55, 0.6, 0.7));
        } else {
            let color = if world.is_pinned(door) {
                Color::srgb(0.40, 1.0, 0.60)
            } else {
                Color::srgb(0.30, 0.80, 1.0)
            };
            gizmos.circle_2d(position, 4.0, color);
        }
    }

    // Connections: green when observed/frozen, cyan when free/mutable.
    for (a, b) in world.connections() {
        let color = if world.is_pinned(a) {
            Color::srgb(0.40, 1.0, 0.60)
        } else {
            Color::srgb(0.30, 0.80, 1.0)
        };
        gizmos.line_2d(world.door_position(a), world.door_position(b), color);
    }

    // Selected observer ring.
    if let Some(&room) = world.players.get(runtime.selected_player) {
        let position = world.room_center(room) + PLAYER_OFFSETS[runtime.selected_player];
        gizmos.circle_2d(position, 22.0, Color::srgb(0.7, 1.0, 0.85));
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, ObservationRuntime>,
    world: Res<'w, ObservationWorld>,
    dots: Query<'w, 's, (), With<PlayerDot>>,
    ui_roots: Query<'w, 's, (), With<ObsUiRoot>>,
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
    let observed: Vec<u32> = (0..ROOM_COUNT as u32)
        .filter(|i| world.observed(RoomId(*i)))
        .collect();
    let free = world.free_door_count();
    let connections = world.connections().len();

    let player_rooms: Vec<String> = world.players.iter().map(|r| format!("R{}", r.0)).collect();

    let dots = context.dots.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = dots == PLAYER_COUNT && ui_roots == 1;

    let mut text = context.text.into_inner();
    **text = format!(
        "OBSERVATION MONITOR  {}\n\
         decoherence events  {}\n\
         last rewired doors  {}\n\
         last locked doors   {}\n\
         free (mutable) doors {} / {}\n\
         active passages     {}\n\
         observed rooms      {:?}\n\
         observers in        {}\n\
         selected            observer {} (room {})\n\
         auto-decohere       {}\n\
         observers {dots}  UI {ui_roots}   resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        world.decoherence_count,
        world.rewires_last,
        world.locked_last,
        free,
        DOOR_COUNT,
        connections,
        observed,
        player_rooms.join(" "),
        context.runtime.selected_player + 1,
        world.players[context.runtime.selected_player].0,
        if context.runtime.auto_decohere {
            "on"
        } else {
            "paused"
        },
        context.runtime.reset_count,
        context.runtime.last_event,
    );
}
