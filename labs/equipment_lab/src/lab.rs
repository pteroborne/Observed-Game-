use bevy::{ecs::system::SystemParam, prelude::*};
use observed_core::PlayerId;

use crate::{
    input::{EquipAction, HumanInput},
    model::{Equipment, EquipmentKind, EquipmentLocation, EquipmentWorld},
};

pub const PLAYER_COUNT: usize = 4;

const PLAYER_COLORS: [Color; PLAYER_COUNT] = [
    Color::srgb(0.30, 0.85, 1.0),
    Color::srgb(1.0, 0.42, 0.34),
    Color::srgb(0.62, 1.0, 0.36),
    Color::srgb(0.86, 0.46, 1.0),
];

const ARENA_MIN: Vec2 = Vec2::new(-560.0, -195.0);
const ARENA_MAX: Vec2 = Vec2::new(580.0, 195.0);
const MOVE_SPEED: f32 = 240.0;

#[derive(Component)]
pub(crate) struct EquipLabOwned;

#[derive(Component)]
pub(crate) struct EquipLabUiRoot;

#[derive(Component)]
pub(crate) struct EquipmentVisual(pub observed_core::EquipmentId);

#[derive(Component)]
pub(crate) struct PlayerVisualE(pub PlayerId);

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct EquipmentLabRuntime {
    pub selected_player: PlayerId,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    pub last_event: String,
}

impl Default for EquipmentLabRuntime {
    fn default() -> Self {
        Self {
            selected_player: PlayerId(0),
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
            last_event: "Showcase ready: battery powers room A; P1 carries the grapple device."
                .to_string(),
        }
    }
}

pub(crate) fn setup_lab(mut commands: Commands, world: Res<EquipmentWorld>) {
    spawn_static_scene(&mut commands, &world);
    spawn_ui(&mut commands);
}

fn spawn_static_scene(commands: &mut Commands, world: &EquipmentWorld) {
    commands
        .spawn((
            EquipLabOwned,
            Name::new("Equipment Scene Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for room in &world.rooms {
                parent.spawn((
                    Name::new(format!("Room {}", room.id.0)),
                    Sprite::from_color(Color::srgb(0.09, 0.13, 0.18), room.half_size * 2.0),
                    Transform::from_translation(room.center.extend(-2.0)),
                ));
            }
            for socket in &world.sockets {
                let color = if socket.provides_power {
                    Color::srgb(0.95, 0.78, 0.25)
                } else {
                    Color::srgb(0.55, 0.70, 1.0)
                };
                parent.spawn((
                    Name::new(format!("Socket {}", socket.id.0)),
                    Sprite::from_color(color.with_alpha(0.5), Vec2::splat(30.0)),
                    Transform::from_translation(socket.position.extend(-1.0)),
                ));
            }
        });
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            EquipLabOwned,
            EquipLabUiRoot,
            Name::new("Equipment Lab UI Root"),
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
                BorderColor::all(Color::srgba(0.95, 0.78, 0.25, 0.6)),
                children![(
                    DebugText,
                    Text::new("Equipment diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.92, 0.75)),
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
                BorderColor::all(Color::srgba(0.95, 0.78, 0.25, 0.6)),
                children![(
                    Text::new(
                        "EQUIPMENT LAB\n\
                         WASD / arrows  Move selected player\n\
                         E              Pick up nearest item\n\
                         G              Drop carried item\n\
                         C              Place: socket if possible, else deploy\n\
                         V              Recover nearest socketed/deployed item\n\
                         F              Hand off to nearest player\n\
                         L              Toggle 'left facility' (drops carried)\n\
                         T              Replace the current room\n\
                         1–4            Select player\n\
                         R              Reset · F1 Toggle debug\n\n\
                         Equipment is a persistent projection: visuals follow\n\
                         logical state and survive carriers leaving and room\n\
                         replacement.",
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
    mut runtime: ResMut<EquipmentLabRuntime>,
) {
    for (key, player) in [
        (KeyCode::Digit1, PlayerId(0)),
        (KeyCode::Digit2, PlayerId(1)),
        (KeyCode::Digit3, PlayerId(2)),
        (KeyCode::Digit4, PlayerId(3)),
    ] {
        if keyboard.just_pressed(key) {
            runtime.selected_player = player;
            runtime.last_event = format!("{} selected.", player.label());
        }
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
}

pub(crate) fn apply_input(
    time: Res<Time>,
    mut world: ResMut<EquipmentWorld>,
    mut input: ResMut<HumanInput>,
    mut runtime: ResMut<EquipmentLabRuntime>,
) {
    let selected = runtime.selected_player;
    let dt = time.delta_secs();

    if let Some(index) = world
        .players
        .iter()
        .position(|player| player.id == selected && player.present)
    {
        let proposed = world.players[index].position + input.movement * MOVE_SPEED * dt;
        world.players[index].position = proposed.clamp(ARENA_MIN, ARENA_MAX);
    }

    let actions = std::mem::take(&mut input.actions);
    for action in actions {
        match action {
            EquipAction::PickUp => {
                world.pick_up(selected);
            }
            EquipAction::Drop => {
                world.drop_carried(selected);
            }
            EquipAction::Place => {
                world.place_carried(selected);
            }
            EquipAction::HandOff => {
                world.hand_off(selected);
            }
            EquipAction::Recover => {
                world.recover(selected);
            }
            EquipAction::ToggleLeave => {
                let present = world.player(selected).map(|p| p.present).unwrap_or(true);
                world.set_player_present(selected, !present);
            }
            EquipAction::ReplaceRoom => {
                let room = world
                    .player(selected)
                    .map(|player| player.position)
                    .and_then(|position| world.room_of(position));
                if let Some(room) = room {
                    world.replace_room(room);
                }
            }
        }
    }

    if let Some(event) = world.recent_events.last() {
        runtime.last_event = event.label();
    }
}

pub(crate) fn tick_power(time: Res<Time>, mut world: ResMut<EquipmentWorld>) {
    world.tick_power(time.delta_secs());
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<EquipmentLabRuntime>,
    mut world: ResMut<EquipmentWorld>,
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

fn equipment_world_position(world: &EquipmentWorld, item: &Equipment) -> Vec2 {
    match item.location {
        EquipmentLocation::Ground { position, .. }
        | EquipmentLocation::Deployed { position, .. } => position,
        EquipmentLocation::Socketed { socket } => world
            .socket(socket)
            .map(|socket| socket.position)
            .unwrap_or(Vec2::ZERO),
        EquipmentLocation::Carried { player } => world
            .player(player)
            .map(|player| player.position + Vec2::new(0.0, 50.0))
            .unwrap_or(Vec2::ZERO),
    }
}

fn equipment_color(item: &Equipment) -> Color {
    let base = match item.kind {
        EquipmentKind::Battery => Color::srgb(0.95, 0.82, 0.25),
        EquipmentKind::StructuralJack => Color::srgb(0.80, 0.55, 0.35),
        EquipmentKind::CableSpool => Color::srgb(0.55, 0.80, 0.70),
        EquipmentKind::DeployableLight => Color::srgb(1.0, 0.95, 0.55),
        EquipmentKind::GrappleDevice => Color::srgb(1.0, 0.55, 0.30),
    };
    if item.kind.needs_power() && !item.powered {
        base.with_alpha(0.30)
    } else {
        base
    }
}

pub(crate) fn sync_equipment_visuals(
    mut commands: Commands,
    world: Res<EquipmentWorld>,
    mut visuals: Query<(Entity, &EquipmentVisual, &mut Transform, &mut Sprite)>,
) {
    let mut seen: Vec<observed_core::EquipmentId> = Vec::new();
    for (entity, visual, mut transform, mut sprite) in &mut visuals {
        match world.equipment(visual.0) {
            Some(item) => {
                let position = equipment_world_position(&world, item);
                transform.translation = position.extend(1.0);
                sprite.color = equipment_color(item);
                seen.push(visual.0);
            }
            None => commands.entity(entity).despawn(),
        }
    }
    for item in &world.equipment {
        if !seen.contains(&item.id) {
            let position = equipment_world_position(&world, item);
            commands.spawn((
                EquipLabOwned,
                EquipmentVisual(item.id),
                Name::new(format!("Equipment {} visual", item.id.0)),
                Sprite::from_color(equipment_color(item), Vec2::new(26.0, 26.0)),
                Transform::from_translation(position.extend(1.0)),
            ));
        }
    }
}

pub(crate) fn sync_player_visuals(
    mut commands: Commands,
    world: Res<EquipmentWorld>,
    runtime: Res<EquipmentLabRuntime>,
    mut visuals: Query<(Entity, &PlayerVisualE, &mut Transform, &mut Sprite)>,
) {
    let mut seen: Vec<PlayerId> = Vec::new();
    for (entity, visual, mut transform, mut sprite) in &mut visuals {
        match world.player(visual.0) {
            Some(player) if player.present => {
                transform.translation = player.position.extend(2.0);
                sprite.color = player_color(visual.0, runtime.selected_player);
                seen.push(visual.0);
            }
            _ => commands.entity(entity).despawn(),
        }
    }
    for player in &world.players {
        if player.present && !seen.contains(&player.id) {
            commands.spawn((
                EquipLabOwned,
                PlayerVisualE(player.id),
                Name::new(format!("{} visual", player.id.label())),
                Sprite::from_color(
                    player_color(player.id, runtime.selected_player),
                    Vec2::new(34.0, 60.0),
                ),
                Transform::from_translation(player.position.extend(2.0)),
            ));
        }
    }
}

fn player_color(player: PlayerId, selected: PlayerId) -> Color {
    let base = PLAYER_COLORS[player.index() % PLAYER_COUNT];
    if player == selected {
        base.mix(&Color::WHITE, 0.35)
    } else {
        base
    }
}

pub(crate) fn draw_debug(
    runtime: Res<EquipmentLabRuntime>,
    world: Res<EquipmentWorld>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    for room in &world.rooms {
        let powered = world.room_powered(room.id);
        let color = if powered {
            Color::srgb(0.35, 0.90, 0.55)
        } else {
            Color::srgb(0.35, 0.45, 0.55)
        };
        gizmos.rect_2d(room.center, room.half_size * 2.0, color);
        gizmos.circle_2d(room.fallback, 9.0, Color::srgba(0.7, 0.7, 0.8, 0.6));
    }

    for socket in &world.sockets {
        let color = if socket.provides_power {
            Color::srgb(0.95, 0.78, 0.25)
        } else {
            Color::srgb(0.55, 0.70, 1.0)
        };
        gizmos.circle_2d(socket.position, 16.0, color);
        if socket.occupied.is_some() {
            gizmos.circle_2d(socket.position, 8.0, Color::WHITE);
        }
    }

    for item in &world.equipment {
        let position = equipment_world_position(&world, item);
        // Ownership line from carrier to the carried item.
        if let EquipmentLocation::Carried { player } = item.location
            && let Some(carrier) = world.player(player)
        {
            gizmos.line_2d(
                carrier.position,
                position,
                Color::srgba(1.0, 0.85, 0.35, 0.8),
            );
        }
        if item.powered {
            gizmos.circle_2d(position, 20.0, Color::srgba(1.0, 0.95, 0.45, 0.7));
        }
    }

    if let Some(selected) = world
        .player(runtime.selected_player)
        .filter(|player| player.present)
    {
        gizmos.circle_2d(selected.position, 40.0, Color::srgb(0.55, 1.0, 0.78));
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, EquipmentLabRuntime>,
    world: Res<'w, EquipmentWorld>,
    equip_visuals: Query<'w, 's, (), With<EquipmentVisual>>,
    player_visuals: Query<'w, 's, (), With<PlayerVisualE>>,
    ui_roots: Query<'w, 's, (), With<EquipLabUiRoot>>,
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
    let selected = context.runtime.selected_player;

    let (mut ground, mut carried, mut socketed, mut deployed) = (0, 0, 0, 0);
    for item in &world.equipment {
        match item.location {
            EquipmentLocation::Ground { .. } => ground += 1,
            EquipmentLocation::Carried { .. } => carried += 1,
            EquipmentLocation::Socketed { .. } => socketed += 1,
            EquipmentLocation::Deployed { .. } => deployed += 1,
        }
    }
    let present_players = world.players.iter().filter(|p| p.present).count();
    let occupied_sockets = world
        .sockets
        .iter()
        .filter(|s| s.occupied.is_some())
        .count();

    let equip_visuals = context.equip_visuals.iter().count();
    let player_visuals = context.player_visuals.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = equip_visuals == world.equipment.len()
        && player_visuals == present_players
        && ui_roots == 1;

    let selected_present = world.player(selected).map(|p| p.present).unwrap_or(false);
    let carrying = match world.carried_by(selected) {
        Some(id) => world
            .equipment(id)
            .map(|item| format!("{} (id {})", item.kind.label(), id.0))
            .unwrap_or_else(|| "nothing".to_string()),
        None => "nothing".to_string(),
    };

    let battery_line = world
        .equipment
        .iter()
        .find(|item| item.kind == EquipmentKind::Battery)
        .map(|item| {
            format!(
                "battery id {}  charge {:>3.0}%  {}",
                item.id.0,
                item.charge * 100.0,
                if item.powered { "SOURCING" } else { "idle" }
            )
        })
        .unwrap_or_else(|| "no battery".to_string());

    let mut rooms = String::new();
    for room in &world.rooms {
        rooms.push_str(&format!(
            "room {}: {}\n",
            room.id.0,
            if world.room_powered(room.id) {
                "POWERED"
            } else {
                "dark"
            }
        ));
    }

    let mut text = context.text.into_inner();
    **text = format!(
        "EQUIPMENT MONITOR  {}\n\
         selected       {} ({})\n\
         carrying       {}\n\
         {}\
         {}\n\
         equipment      {} total — ground:{} carried:{} socketed:{} deployed:{}\n\
         players present {}/{}   sockets occupied {}/{}\n\
         visuals: equip {}/{}  player {}/{}  UI {}\n\
         replacements {}   events {}   resets {}   debug {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        selected.label(),
        if selected_present { "present" } else { "left" },
        carrying,
        rooms,
        battery_line,
        world.equipment.len(),
        ground,
        carried,
        socketed,
        deployed,
        present_players,
        world.players.len(),
        occupied_sockets,
        world.sockets.len(),
        equip_visuals,
        world.equipment.len(),
        player_visuals,
        present_players,
        ui_roots,
        world.room_replacements,
        world.total_events,
        context.runtime.reset_count,
        if context.runtime.debug_visible {
            "ON"
        } else {
            "OFF"
        },
        context.runtime.last_event,
    );
}
