use bevy::{ecs::system::SystemParam, prelude::*};
use observed_core::RoomId;
use observed_observation::{DOOR_COUNT, DoorId, PLAYER_COUNT, ROOM_COUNT, ROOM_HALF, Side};

use crate::model::ConstraintWorld;

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

const GOLD: Color = Color::srgb(1.0, 0.80, 0.28);
const GREEN: Color = Color::srgb(0.40, 1.0, 0.60);
const CYAN: Color = Color::srgb(0.30, 0.80, 1.0);
const RED: Color = Color::srgb(1.0, 0.36, 0.34);

#[derive(Component)]
pub(crate) struct ConOwned;

#[derive(Component)]
pub(crate) struct ConUiRoot;

#[derive(Component)]
pub(crate) struct PlayerDot(pub usize);

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct ConstraintRuntime {
    pub selected_player: usize,
    pub debug_visible: bool,
    pub auto_decohere: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
}

impl Default for ConstraintRuntime {
    fn default() -> Self {
        Self {
            selected_player: 0,
            debug_visible: true,
            auto_decohere: true,
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

pub(crate) fn setup_lab(mut commands: Commands, world: Res<ConstraintWorld>) {
    commands
        .spawn((
            ConOwned,
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

    commands
        .spawn((
            ConOwned,
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
            ConOwned,
            ConUiRoot,
            Name::new("Constraint Lab UI Root"),
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
                BorderColor::all(Color::srgba(1.0, 0.80, 0.28, 0.6)),
                children![(
                    DebugText,
                    Text::new("Constraint diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.94, 0.80)),
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
                BorderColor::all(Color::srgba(1.0, 0.80, 0.28, 0.6)),
                children![(
                    Text::new(
                        "CONSTRAINT LAB\n\
                         WASD / arrows  Step the selected observer through a door\n\
                         Space          Decohere now\n\
                         T              Toggle route protection (the spine)\n\
                         P              Pause / resume auto-decoherence\n\
                         1–4            Select an observer\n\
                         R              Reset · F1 Toggle debug\n\n\
                         Gold routes are the persistent spine; with protection on\n\
                         the structure rewires but stays fully connected. Toggle it\n\
                         off and watch a decoherence isolate a room (red).",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.92, 0.90, 0.84)),
                )],
            ));
        });
}

pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<ConstraintRuntime>,
    mut world: ResMut<ConstraintWorld>,
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
    if keyboard.just_pressed(KeyCode::KeyT) {
        world.toggle_protection();
    }
    if keyboard.just_pressed(KeyCode::Space) {
        world.decohere();
    }

    for (keys, side) in [
        ([KeyCode::KeyW, KeyCode::ArrowUp], Side::North),
        ([KeyCode::KeyD, KeyCode::ArrowRight], Side::East),
        ([KeyCode::KeyS, KeyCode::ArrowDown], Side::South),
        ([KeyCode::KeyA, KeyCode::ArrowLeft], Side::West),
    ] {
        if keys.iter().any(|key| keyboard.just_pressed(*key)) {
            world.traverse(runtime.selected_player, side);
        }
    }
}

pub(crate) fn auto_decohere(
    time: Res<Time>,
    runtime: Res<ConstraintRuntime>,
    mut timer: ResMut<DecohereTimer>,
    mut world: ResMut<ConstraintWorld>,
) {
    if runtime.auto_decohere && timer.0.tick(time.delta()).just_finished() {
        world.decohere();
    }
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<ConstraintRuntime>,
    mut world: ResMut<ConstraintWorld>,
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

pub(crate) fn present_players(
    runtime: Res<ConstraintRuntime>,
    world: Res<ConstraintWorld>,
    mut dots: Query<(&PlayerDot, &mut Transform, &mut Sprite)>,
) {
    for (dot, mut transform, mut sprite) in &mut dots {
        if let Some(&room) = world.graph.players.get(dot.0) {
            let position = world.graph.room_center(room) + PLAYER_OFFSETS[dot.0 % PLAYER_COUNT];
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

fn door_color(world: &ConstraintWorld, door: DoorId) -> Color {
    if world.protection_enabled && world.is_protected(door) {
        GOLD
    } else if world.graph.is_pinned(door) {
        GREEN
    } else {
        CYAN
    }
}

pub(crate) fn draw_debug(
    runtime: Res<ConstraintRuntime>,
    world: Res<ConstraintWorld>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    for (index, is_reachable) in world.reachable_set().into_iter().enumerate() {
        let room = RoomId(index as u32);
        let color = if !is_reachable {
            RED
        } else if world.graph.observed(room) {
            GREEN
        } else {
            Color::srgb(0.22, 0.30, 0.40)
        };
        gizmos.rect_2d(
            world.graph.room_center(room),
            Vec2::splat(ROOM_HALF * 2.0),
            color,
        );
    }

    for index in 0..DOOR_COUNT {
        let door = DoorId(index as u16);
        let position = world.graph.door_position(door);
        if world.graph.is_sealed(door) {
            gizmos.circle_2d(position, 5.0, Color::srgba(0.5, 0.55, 0.6, 0.7));
        } else {
            gizmos.circle_2d(position, 4.0, door_color(&world, door));
        }
    }

    for (a, b) in world.graph.connections() {
        gizmos.line_2d(
            world.graph.door_position(a),
            world.graph.door_position(b),
            door_color(&world, a),
        );
    }

    if let Some(&room) = world.graph.players.get(runtime.selected_player) {
        let position = world.graph.room_center(room) + PLAYER_OFFSETS[runtime.selected_player];
        gizmos.circle_2d(position, 22.0, Color::srgb(0.7, 1.0, 0.85));
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, ConstraintRuntime>,
    world: Res<'w, ConstraintWorld>,
    dots: Query<'w, 's, (), With<PlayerDot>>,
    ui_roots: Query<'w, 's, (), With<ConUiRoot>>,
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
    let protected = (0..DOOR_COUNT)
        .filter(|i| world.is_protected(DoorId(*i as u16)))
        .count();
    let free = (0..DOOR_COUNT)
        .filter(|i| !world.is_frozen(DoorId(*i as u16)))
        .count();
    let observed: Vec<u32> = (0..ROOM_COUNT as u32)
        .filter(|i| world.graph.observed(RoomId(*i)))
        .collect();

    let dots = context.dots.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = dots == PLAYER_COUNT && ui_roots == 1;

    let mut text = context.text.into_inner();
    **text = format!(
        "CONSTRAINT MONITOR  {}\n\
         connectivity        {} ({}/{})\n\
         route protection    {}\n\
         protected (spine)   {} doors\n\
         decoherence events  {}\n\
         last rewired doors  {}\n\
         free (mutable) doors {} / {}\n\
         observed rooms      {:?}\n\
         selected            observer {} (room {})\n\
         auto-decohere       {}\n\
         observers {dots}  UI {ui_roots}   resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        if world.connected {
            "CONNECTED"
        } else {
            "DISCONNECTED"
        },
        world.reachable,
        ROOM_COUNT,
        if world.protection_enabled {
            "ON"
        } else {
            "OFF"
        },
        protected,
        world.decohere_count,
        world.last_rewired,
        free,
        DOOR_COUNT,
        observed,
        context.runtime.selected_player + 1,
        world.graph.players[context.runtime.selected_player].0,
        if context.runtime.auto_decohere {
            "on"
        } else {
            "paused"
        },
        context.runtime.reset_count,
        world.last_event,
    );
}
