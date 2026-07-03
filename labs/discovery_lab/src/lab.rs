use bevy::{ecs::system::SystemParam, prelude::*};
use observed_style as style;

use crate::model::{
    DiscoveryWorld, KnowledgeSource, REQUIRED_KEYSTONES, REQUIRED_POWER, ROOM_COUNT, RoomType,
    identity_for_room_type,
};

/// Spacing between room tiles in the 3×3 schematic.
const SPACING: f32 = 160.0;
const TILE: f32 = 120.0;

const UNKNOWN: Color = Color::srgb(0.12, 0.15, 0.20);
const RED: Color = Color::srgb(1.0, 0.36, 0.34);

fn type_color(t: RoomType) -> Color {
    style::door_identity(identity_for_room_type(t)).base_color
}

/// The world-space centre of a room tile in the 3×3 schematic.
pub(crate) fn room_center(room: usize) -> Vec2 {
    let col = (room % 3) as f32;
    let row = (room / 3) as f32;
    Vec2::new((col - 1.0) * SPACING, (1.0 - row) * SPACING)
}

#[derive(Component)]
pub(crate) struct DiscOwned;

#[derive(Component)]
pub(crate) struct DiscUiRoot;

#[derive(Component)]
pub(crate) struct RoomTile(pub usize);

#[derive(Component)]
pub(crate) struct RoomGlyph(pub usize);

#[derive(Component)]
pub(crate) struct DoorReadFrame(pub usize);

#[derive(Component)]
pub(crate) struct DoorReadGlyph(pub usize);

#[derive(Component)]
pub(crate) struct DoorReadBleed(pub usize);

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct DiscoveryRuntime {
    pub debug_visible: bool,
    pub auto_explore: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
}

impl Default for DiscoveryRuntime {
    fn default() -> Self {
        Self {
            debug_visible: true,
            auto_explore: true,
            reset_requested: false,
            reset_count: 0,
        }
    }
}

#[derive(Resource)]
pub struct ShiftTimer(pub Timer);

impl Default for ShiftTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(1.1, TimerMode::Repeating))
    }
}

pub(crate) fn setup_lab(mut commands: Commands) {
    commands
        .spawn((
            DiscOwned,
            Name::new("Facility Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for room in 0..ROOM_COUNT {
                let center = room_center(room);
                parent
                    .spawn((
                        RoomTile(room),
                        Name::new(format!("Room {room}")),
                        Sprite::from_color(UNKNOWN, Vec2::splat(TILE)),
                        Transform::from_translation(center.extend(0.0)),
                    ))
                    .with_children(|tile| {
                        tile.spawn((
                            DoorReadBleed(room),
                            Sprite::from_color(Color::NONE, Vec2::splat(TILE + 34.0)),
                            Transform::from_xyz(0.0, 0.0, -0.2),
                        ));
                        tile.spawn((
                            DoorReadFrame(room),
                            Sprite::from_color(UNKNOWN, Vec2::new(TILE + 24.0, 12.0)),
                            Transform::from_xyz(0.0, TILE * 0.5 + 13.0, 1.2),
                        ));
                        tile.spawn((
                            DoorReadGlyph(room),
                            Text2d::new(" "),
                            TextFont {
                                font_size: 24.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.02, 0.03, 0.04)),
                            Transform::from_xyz(0.0, TILE * 0.5 + 12.0, 1.5),
                        ));
                        tile.spawn((
                            RoomGlyph(room),
                            Text2d::new("?"),
                            TextFont {
                                font_size: 46.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.85, 0.9, 1.0)),
                            Transform::from_xyz(0.0, 0.0, 1.0),
                        ));
                    });
            }
        });

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            DiscOwned,
            DiscUiRoot,
            Name::new("Discovery Lab UI Root"),
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
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(1.0, 0.80, 0.28, 0.6)),
                children![(
                    DebugText,
                    Text::new("Discovery diagnostics starting..."),
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
                    width: px(430),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(1.0, 0.80, 0.28, 0.6)),
                children![(
                    Text::new(
                        "DISCOVERY LAB\n\
                         1-9    Visit a room (reveal + harvest its type)\n\
                         Space  Shift unobserved rooms now\n\
                         X      Try the gated exit\n\
                         A      Auto-explore on/off\n\
                         C      Toggle the solvability constraint\n\
                         R      Reset | F1 Toggle debug\n\n\
                         K=Keystone Vault  P=Power Cache  R=Reactor(+2)\n\
                         C=Control  S=Survey  N=Sensor(reveal nearby)\n\
                         E=False exit read  !=Decoy exposed  .=Dead-end\n\
                         ?=undiscovered\n\n\
                         Tile fill is team-map knowledge; the top doorframe\n\
                         glyph and glow are the current threshold read.\n\
                         The exit is locked until you collect the keystones.\n\
                         Rooms shift their type when unobserved. A Decoy\n\
                         advertises an exit signal until you reach it.\n\
                         With the constraint ON the objective is always\n\
                         solvable; turn it OFF and a keystone can strand.",
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
    mut runtime: ResMut<DiscoveryRuntime>,
    mut world: ResMut<DiscoveryWorld>,
) {
    const DIGITS: [KeyCode; ROOM_COUNT] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (room, key) in DIGITS.iter().enumerate() {
        if keyboard.just_pressed(*key) {
            world.visit(room);
        }
    }
    if keyboard.just_pressed(KeyCode::Space) {
        world.shift();
    }
    if keyboard.just_pressed(KeyCode::KeyX) {
        world.escape();
    }
    if keyboard.just_pressed(KeyCode::KeyA) {
        runtime.auto_explore = !runtime.auto_explore;
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        world.constraint_enabled = !world.constraint_enabled;
        world.recompute_solvability();
        world.last_event = if world.constraint_enabled {
            "Constraint ON - keystones can't strand; always solvable.".to_string()
        } else {
            "Constraint OFF - keystones may strand on spent rooms.".to_string()
        };
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
}

pub(crate) fn auto_explore(
    time: Res<Time>,
    runtime: Res<DiscoveryRuntime>,
    mut timer: ResMut<ShiftTimer>,
    mut world: ResMut<DiscoveryWorld>,
) {
    if !runtime.auto_explore || world.escaped {
        return;
    }
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    if world.gate_open() {
        world.escape();
    } else if let Some(room) = world.next_unharvested() {
        world.visit(room);
        world.shift();
    }
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<DiscoveryRuntime>,
    mut world: ResMut<DiscoveryWorld>,
    mut timer: ResMut<ShiftTimer>,
) {
    if !runtime.reset_requested {
        return;
    }
    runtime.reset_requested = false;
    runtime.reset_count += 1;
    world.reset();
    timer.0.reset();
}

pub(crate) fn present_rooms(
    world: Res<DiscoveryWorld>,
    mut tiles: Query<(&RoomTile, &mut Sprite)>,
    mut glyphs: Query<(&RoomGlyph, &mut Text2d, &mut TextColor)>,
) {
    for (tile, mut sprite) in &mut tiles {
        let color = match world.known[tile.0] {
            None => UNKNOWN,
            Some(t) => {
                let base = type_color(t);
                if world.harvested[tile.0] {
                    base.mix(&Color::BLACK, 0.55) // spent rooms dim
                } else {
                    base
                }
            }
        };
        sprite.color = color;
    }
    for (glyph, mut text, mut color) in &mut glyphs {
        let (s, c) = match world.known[glyph.0] {
            None => ("?".to_string(), Color::srgb(0.55, 0.6, 0.7)),
            Some(t) => (t.glyph().to_string(), Color::srgb(0.05, 0.06, 0.09)),
        };
        text.0 = s;
        color.0 = c;
    }
}

pub(crate) fn present_door_frames(
    world: Res<DiscoveryWorld>,
    mut frames: Query<(&DoorReadFrame, &mut Sprite)>,
) {
    for (frame, mut sprite) in &mut frames {
        let Some(role) = world.door_read(frame.0) else {
            continue;
        };
        let treatment = style::door_identity(role);
        let alpha = if world.harvested[frame.0] { 0.55 } else { 0.94 };
        sprite.color = treatment.base_color.with_alpha(alpha);
    }
}

pub(crate) fn present_door_glyphs(
    world: Res<DiscoveryWorld>,
    mut glyphs: Query<(&DoorReadGlyph, &mut Text2d, &mut TextColor)>,
) {
    for (glyph, mut text, mut color) in &mut glyphs {
        let Some(role) = world.door_read(glyph.0) else {
            continue;
        };
        text.0 = role.glyph().to_string();
        color.0 = Color::srgb(0.02, 0.03, 0.04);
    }
}

pub(crate) fn present_door_bleeds(
    world: Res<DiscoveryWorld>,
    mut bleeds: Query<(&DoorReadBleed, &mut Sprite)>,
) {
    for (bleed, mut sprite) in &mut bleeds {
        let Some(role) = world.door_read(bleed.0) else {
            continue;
        };
        let treatment = style::door_identity(role);
        let alpha = if world.at == Some(bleed.0) {
            0.30
        } else if world.harvested[bleed.0] {
            0.08
        } else {
            0.16
        };
        sprite.color = Color::from(treatment.emissive).with_alpha(alpha);
    }
}

pub(crate) fn draw_debug(
    runtime: Res<DiscoveryRuntime>,
    world: Res<DiscoveryWorld>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }
    // Outline every room; ring the one currently observed.
    for room in 0..ROOM_COUNT {
        gizmos.rect_2d(
            room_center(room),
            Vec2::splat(TILE + 8.0),
            Color::srgb(0.2, 0.25, 0.32),
        );
    }
    if let Some(at) = world.at {
        gizmos.rect_2d(
            room_center(at),
            Vec2::splat(TILE + 18.0),
            Color::srgb(0.7, 1.0, 0.85),
        );
    }
    // The gate sits below the grid: gold when open, red when locked.
    let gate = Vec2::new(0.0, -SPACING - 120.0);
    gizmos.rect_2d(
        gate,
        Vec2::new(TILE, 44.0),
        if world.gate_open() {
            style::marker(style::MarkerRole::Exit).base_color
        } else {
            RED
        },
    );
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, DiscoveryRuntime>,
    world: Res<'w, DiscoveryWorld>,
    tiles: Query<'w, 's, (), With<RoomTile>>,
    frames: Query<'w, 's, (), With<DoorReadFrame>>,
    ui_roots: Query<'w, 's, (), With<DiscUiRoot>>,
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
    let tiles = context.tiles.iter().count();
    let frames = context.frames.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = tiles == ROOM_COUNT && frames == ROOM_COUNT && ui_roots == 1;
    let discovered = world.team_map_known_count();
    let sensor_known = world
        .known_source
        .iter()
        .filter(|source| **source == Some(KnowledgeSource::Sensor))
        .count();

    let mut text = context.text.into_inner();
    **text = format!(
        "DISCOVERY MONITOR   {}\n\
         objective           {}\n\
         keystones           {} / {}\n\
         power               {} / {}\n\
         gate                {}\n\
         still solvable      {}\n\
         collectable vaults  {}   caches {}\n\
         rooms harvested     {} / {}\n\
         rooms discovered    {} / {}\n\
         sensor map entries  {}   updates {}\n\
         decoy lies resolved {}\n\
         shifts / visits     {} / {}\n\
         constraint          {}\n\
         stabilised (Control){}\n\
         tiles {tiles}  reads {frames}  UI {ui_roots}   resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        if world.escaped {
            "ESCAPED"
        } else {
            "in progress"
        },
        world.keystones,
        REQUIRED_KEYSTONES,
        world.power,
        REQUIRED_POWER,
        if world.gate_open() { "OPEN" } else { "LOCKED" },
        if world.solvable {
            "yes"
        } else {
            "NO - run lost"
        },
        world.collectable_keystones(),
        world.collectable_power(),
        world.harvested.iter().filter(|h| **h).count(),
        ROOM_COUNT,
        discovered,
        ROOM_COUNT,
        sensor_known,
        world.sensor_map_updates,
        world.decoy_lies_resolved,
        world.shift_count,
        world.visit_count,
        if world.constraint_enabled {
            "ON"
        } else {
            "OFF"
        },
        if world.stabilized { " yes" } else { " no" },
        context.runtime.reset_count,
        world.last_event,
    );
}
