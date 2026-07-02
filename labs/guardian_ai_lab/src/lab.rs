use bevy::{ecs::system::SystemParam, prelude::*};
use observed_core::RoomId;
use observed_observation::{DOOR_COUNT, DoorId, ObservationWorld, ROOM_COUNT, ROOM_HALF, Side};
use std::collections::HashSet;

use crate::model::{
    Actor, Guardian, GuardianState, SimpleRng, find_shortest_path, tick_guardian,
    visible_rooms_from_view,
};

const PLAYER_COLOR: Color = Color::srgb(0.30, 0.85, 1.0); // Blue
const BOT_COLOR: Color = Color::srgb(0.75, 0.40, 0.95); // Purple/Violet
const GUARDIAN_ACTIVE_COLOR: Color = Color::srgb(1.0, 0.20, 0.60); // Bright pink/magenta
const GUARDIAN_FROZEN_COLOR: Color = Color::srgb(0.70, 0.70, 0.80); // Grey/Stone
const ANCHOR_COLOR: Color = Color::srgb(1.0, 0.60, 0.10); // Warm orange/yellow

#[derive(Component)]
pub struct LabOwned;

#[derive(Component)]
pub struct LabUiRoot;

#[derive(Component)]
pub struct PlayerArrow {
    pub actor_id: usize,
}

#[derive(Component)]
pub struct GuardianDot;

#[derive(Component)]
pub struct DebugText;

#[derive(Component)]
pub struct HelpText;

#[derive(Component)]
pub struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct GuardianRuntime {
    pub actors: Vec<Actor>,
    pub anchors: HashSet<RoomId>,
    pub debug_visible: bool,
    pub auto_move: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    pub last_event: String,
    pub bot_move_timer: Timer,
}

impl Default for GuardianRuntime {
    fn default() -> Self {
        Self {
            actors: vec![
                Actor {
                    id: 0,
                    room: RoomId(0), // Human starts top-left
                    facing: Side::East,
                    is_bot: false,
                    touch_count: 0,
                    is_teleported: false,
                },
                Actor {
                    id: 1,
                    room: RoomId(2), // Bot 1 starts top-right
                    facing: Side::South,
                    is_bot: true,
                    touch_count: 0,
                    is_teleported: false,
                },
                Actor {
                    id: 2,
                    room: RoomId(6), // Bot 2 starts bottom-left
                    facing: Side::North,
                    is_bot: true,
                    touch_count: 0,
                    is_teleported: false,
                },
                Actor {
                    id: 3,
                    room: RoomId(4), // Bot 3 starts center
                    facing: Side::West,
                    is_bot: true,
                    touch_count: 0,
                    is_teleported: false,
                },
            ],
            anchors: HashSet::new(),
            debug_visible: true,
            auto_move: true,
            reset_requested: false,
            reset_count: 0,
            last_event: "Guide the guardian: unobserved moves, observed freezes, anchor resets."
                .to_string(),
            bot_move_timer: Timer::from_seconds(1.5, TimerMode::Repeating),
        }
    }
}

#[derive(Resource)]
pub struct MoveTimer(pub Timer);

impl Default for MoveTimer {
    fn default() -> Self {
        // Guardian moves every 1.2 seconds if active
        Self(Timer::from_seconds(1.2, TimerMode::Repeating))
    }
}

pub fn setup_lab(mut commands: Commands, world: Res<ObservationWorld>) {
    commands
        .spawn((
            LabOwned,
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

    // 4 Player / Bot arrows
    for idx in 0..4 {
        let (color, name) = if idx == 0 {
            (PLAYER_COLOR, "Player Arrow".to_string())
        } else {
            (BOT_COLOR, format!("NPC Bot {idx} Arrow"))
        };

        commands.spawn((
            LabOwned,
            PlayerArrow { actor_id: idx },
            Name::new(name),
            Sprite::from_color(color, Vec2::new(24.0, 24.0)),
            Transform::default(),
        ));
    }

    // Guardian dot
    commands.spawn((
        LabOwned,
        GuardianDot,
        Name::new("Weeping Angel Guardian"),
        Sprite::from_color(GUARDIAN_ACTIVE_COLOR, Vec2::splat(22.0)),
        Transform::default(),
    ));

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            LabOwned,
            LabUiRoot,
            Name::new("Guardian Lab UI Root"),
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
                    width: px(450),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.45, 0.85, 1.0, 0.6)),
                children![(
                    DebugText,
                    Text::new("Guardian diagnostics starting…"),
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
                    width: px(440),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.45, 0.85, 1.0, 0.6)),
                children![(
                    Text::new(
                        "GUARDIAN AI LAB\n\
                         WASD            Move player room-by-room\n\
                         Arrow keys      Change look direction (North/East/South/West)\n\
                         T               Drop / pick up anchor torch in current room\n\
                         Space           Decohere now (rewires unobserved pathways)\n\
                         P               Pause / resume guardian auto-movement\n\
                         R               Reset lab · F1 Toggle debug display\n\n\
                         Green outline = player's direct line-of-sight.\n\
                         Red dotted line = guardian's routing path.\n\
                         Guardian freezes under player gaze or anchor torch light.\n\
                         Anchor torch banishes guardian to a random room in 30s.",
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

pub fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<GuardianRuntime>,
    mut world: ResMut<ObservationWorld>,
) {
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        runtime.auto_move = !runtime.auto_move;
    }
    if keyboard.just_pressed(KeyCode::Space) {
        world.decohere();
        runtime.last_event = format!(
            "Decohered: {} pathways rewired, {} locked by observation.",
            world.rewires_last, world.locked_last
        );
    }

    // Toggle anchor in the current player room
    if keyboard.just_pressed(KeyCode::KeyT) {
        let room = runtime.actors[0].room;
        if runtime.anchors.contains(&room) {
            runtime.anchors.remove(&room);
            runtime.last_event = format!("Picked up anchor torch from room {}.", room.0);
        } else {
            runtime.anchors.insert(room);
            runtime.last_event = format!("Dropped anchor torch in room {}.", room.0);
        }
    }

    // Facing change (Player 0)
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        runtime.actors[0].facing = Side::North;
    } else if keyboard.just_pressed(KeyCode::ArrowRight) {
        runtime.actors[0].facing = Side::East;
    } else if keyboard.just_pressed(KeyCode::ArrowDown) {
        runtime.actors[0].facing = Side::South;
    } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
        runtime.actors[0].facing = Side::West;
    }

    // Player movement
    for (keys, side) in [
        ([KeyCode::KeyW], Side::North),
        ([KeyCode::KeyD], Side::East),
        ([KeyCode::KeyS], Side::South),
        ([KeyCode::KeyA], Side::West),
    ] {
        if keys.iter().any(|key| keyboard.just_pressed(*key)) {
            let room = runtime.actors[0].room;
            let door = world.door_id(room, side);
            if !world.is_sealed(door) {
                let partner = world.partner(door);
                let dest = world.door(partner).room;
                runtime.actors[0].room = dest;
                runtime.actors[0].is_teleported = false;
                runtime.last_event =
                    format!("Player stepped {} into room {}.", side.label(), dest.0);
            } else {
                runtime.last_event = format!("Player hit a sealed wall ({}).", side.label());
            }
        }
    }
}

pub fn update_guardian(
    time: Res<Time>,
    mut runtime: ResMut<GuardianRuntime>,
    mut timer: ResMut<MoveTimer>,
    mut guardian: ResMut<Guardian>,
    mut rng: ResMut<SimpleRng>,
    world: Res<ObservationWorld>,
) {
    let dt = time.delta_secs();

    // 1. Tick Bot movements
    runtime.bot_move_timer.tick(time.delta());
    if runtime.bot_move_timer.just_finished() {
        for idx in 1..runtime.actors.len() {
            // bot index
            runtime.actors[idx].is_teleported = false;
            let bot_room = runtime.actors[idx].room;

            let mut neighbors = Vec::new();
            for side in Side::ALL {
                let door = world.door_id(bot_room, side);
                if !world.is_sealed(door) {
                    let partner = world.partner(door);
                    let dest = world.door(partner).room;
                    neighbors.push((side, dest));
                }
            }
            if !neighbors.is_empty() {
                let choice = (rng.next_u64() % neighbors.len() as u64) as usize;
                let (chosen_side, chosen_dest) = neighbors[choice];
                runtime.actors[idx].room = chosen_dest;
                runtime.actors[idx].facing = chosen_side; // Face direction of move
            }
        }
    }

    // 2. Check if guardian auto-move tick has finished
    let mut step_tick = false;
    if runtime.auto_move && timer.0.tick(time.delta()).just_finished() {
        step_tick = true;
    }

    // 3. Tick guardian AI
    let anchors = runtime.anchors.clone();
    let (state, teleport, caught_idx) = tick_guardian(
        &world,
        &mut guardian,
        &mut runtime.actors,
        &anchors,
        &mut rng,
        dt,
        step_tick,
    );

    let mut event = None;
    if let Some(caught) = caught_idx {
        let name = if caught == 0 {
            "Player".to_string()
        } else {
            format!("NPC Bot {caught}")
        };
        event = Some(format!(
            "TOUCHED! Guardian teleported {name} to room {}.",
            runtime.actors[caught].room.0
        ));
    } else if let Some(target) = teleport {
        event = Some(format!(
            "Guardian was banished by anchor light to room {}.",
            target.0
        ));
    } else if state == GuardianState::Active && step_tick {
        event = Some(format!("Guardian stepped to room {}.", guardian.room.0));
    }

    if let Some(msg) = event {
        runtime.last_event = msg;
    }
}

#[allow(clippy::type_complexity)]
pub fn present_entities(
    runtime: Res<GuardianRuntime>,
    guardian: Res<Guardian>,
    world: Res<ObservationWorld>,
    mut player_q: Query<(&mut Transform, &PlayerArrow), (With<PlayerArrow>, Without<GuardianDot>)>,
    mut guardian_q: Query<(&mut Transform, &mut Sprite), (With<GuardianDot>, Without<PlayerArrow>)>,
) {
    // Players/Bots positions and rotations
    for (mut transform, arrow) in player_q.iter_mut() {
        if let Some(actor) = runtime.actors.iter().find(|a| a.id == arrow.actor_id) {
            let center = world.room_center(actor.room);
            transform.translation.x = center.x;
            transform.translation.y = center.y;
            transform.translation.z = 6.0;

            let angle = match actor.facing {
                Side::North => 0.0,
                Side::West => std::f32::consts::FRAC_PI_2,
                Side::South => std::f32::consts::PI,
                Side::East => -std::f32::consts::FRAC_PI_2,
            };
            transform.rotation = Quat::from_rotation_z(angle);
        }
    }

    // Guardian position and coloring
    if let Ok((mut transform, mut sprite)) = guardian_q.single_mut() {
        let center = world.room_center(guardian.room);
        transform.translation.x = center.x;
        transform.translation.y = center.y;
        transform.translation.z = 5.0;

        let mut seen_by_any = false;
        for actor in runtime.actors.iter() {
            if visible_rooms_from_view(&world, actor.room, actor.facing).contains(&guardian.room) {
                seen_by_any = true;
                break;
            }
        }
        let seen_by_anchor = runtime.anchors.contains(&guardian.room);

        sprite.color = if seen_by_any {
            GUARDIAN_FROZEN_COLOR
        } else if seen_by_anchor {
            GUARDIAN_FROZEN_COLOR.mix(&Color::srgb(1.0, 0.5, 0.0), 0.3)
        } else {
            GUARDIAN_ACTIVE_COLOR
        };
    }
}

pub fn draw_debug(
    runtime: Res<GuardianRuntime>,
    guardian: Res<Guardian>,
    world: Res<ObservationWorld>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    // Union of visible rooms for all actors
    let mut seen_rooms = HashSet::new();
    for actor in runtime.actors.iter() {
        seen_rooms.extend(visible_rooms_from_view(&world, actor.room, actor.facing));
    }

    // 1. Rooms layout
    for index in 0..ROOM_COUNT {
        let room = RoomId(index as u32);
        let is_observed = seen_rooms.contains(&room);

        let color = if is_observed {
            Color::srgb(0.40, 1.0, 0.60) // Direct light-of-sight (green)
        } else if runtime.anchors.contains(&room) {
            Color::srgb(1.0, 0.60, 0.10) // Anchored (orange)
        } else {
            Color::srgb(0.22, 0.30, 0.40) // Regular
        };
        gizmos.rect_2d(world.room_center(room), Vec2::splat(ROOM_HALF * 2.0), color);
    }

    // 2. Doorways & Connections
    for index in 0..DOOR_COUNT {
        let door = DoorId(index as u16);
        let position = world.door_position(door);
        if world.is_sealed(door) {
            gizmos.circle_2d(position, 5.0, Color::srgba(0.5, 0.55, 0.6, 0.7));
        }
    }

    for (a, b) in world.connections() {
        let is_p = world.is_pinned(a);
        let color = if is_p {
            Color::srgba(0.40, 1.0, 0.60, 0.5)
        } else {
            Color::srgba(0.30, 0.80, 1.0, 0.5)
        };
        gizmos.line_2d(world.door_position(a), world.door_position(b), color);
    }

    // 3. Draw Guardian planned shortest path to closest actor
    let mut best_path: Option<Vec<RoomId>> = None;
    for actor in runtime.actors.iter() {
        if let Some(path) = find_shortest_path(&world, guardian.room, actor.room)
            && best_path
                .as_ref()
                .is_none_or(|best| path.len() < best.len())
        {
            best_path = Some(path);
        }
    }

    if let Some(path) = best_path {
        for window in path.windows(2) {
            let start = world.room_center(window[0]);
            let end = world.room_center(window[1]);
            gizmos.line_2d(start, end, Color::srgb(1.0, 0.1, 0.1));
        }
    }

    // 4. Draw Anchor Torches dropped inside rooms
    for room in &runtime.anchors {
        let center = world.room_center(*room);
        gizmos.circle_2d(center, 12.0, ANCHOR_COLOR);
        gizmos.circle_2d(center, 8.0, Color::WHITE);
    }
}

#[derive(SystemParam)]
pub struct DebugContext<'w, 's> {
    pub runtime: ResMut<'w, GuardianRuntime>,
    pub guardian: ResMut<'w, Guardian>,
    pub world: ResMut<'w, ObservationWorld>,
    pub ui_roots: Query<'w, 's, (), With<LabUiRoot>>,
    pub text: Single<'w, 's, &'static mut Text, With<DebugText>>,
    pub panel: Single<'w, 's, &'static mut Visibility, (With<DebugPanel>, Without<HelpText>)>,
    pub help: Single<'w, 's, &'static mut Visibility, (With<HelpText>, Without<DebugPanel>)>,
}

pub fn update_debug_text(mut context: DebugContext, mut timer: ResMut<MoveTimer>) {
    // Reset requested
    if context.runtime.reset_requested {
        context.runtime.reset_requested = false;
        context.runtime.reset_count += 1;
        context.runtime.actors = GuardianRuntime::default().actors;
        context.runtime.anchors.clear();
        context.runtime.last_event = format!(
            "Reset {} restored initial state.",
            context.runtime.reset_count
        );
        context.guardian.room = RoomId(8);
        context.guardian.anchor_timer = 30.0;
        context.world.reset();
        timer.0.reset();
    }

    let visibility = if context.runtime.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    **context.panel = visibility;
    **context.help = visibility;

    let world_ref = &*context.world;
    let mut seen_rooms = HashSet::new();
    for actor in context.runtime.actors.iter() {
        seen_rooms.extend(visible_rooms_from_view(world_ref, actor.room, actor.facing));
    }

    let is_seen_by_player = seen_rooms.contains(&context.guardian.room);
    let is_seen_by_anchor = context.runtime.anchors.contains(&context.guardian.room);

    let guardian_status = if is_seen_by_player {
        "FROZEN (Observed by Actor)".to_string()
    } else if is_seen_by_anchor {
        format!(
            "FROZEN (Observed by Anchor: {:.1}s reset)",
            context.guardian.anchor_timer
        )
    } else {
        "ACTIVE (Unobserved, moving)".to_string()
    };

    let mut best_path: Option<Vec<RoomId>> = None;
    for actor in context.runtime.actors.iter() {
        if let Some(path) = find_shortest_path(world_ref, context.guardian.room, actor.room)
            && best_path
                .as_ref()
                .is_none_or(|best| path.len() < best.len())
        {
            best_path = Some(path);
        }
    }
    let path_len = best_path.map(|p| p.len()).unwrap_or(0);

    let ui_roots = context.ui_roots.iter().count();
    let healthy = ui_roots == 1;

    let mut actor_diagnostics = String::new();
    for actor in context.runtime.actors.iter() {
        let role = if actor.id == 0 {
            "Player".to_string()
        } else {
            format!("Bot {}", actor.id)
        };
        let observer_status = if visible_rooms_from_view(world_ref, actor.room, actor.facing)
            .contains(&context.guardian.room)
        {
            "FREEZING"
        } else {
            "LOOKING AWAY"
        };
        actor_diagnostics.push_str(&format!(
            "  {:8} R{} {:?} ({}) touched {}\n",
            role, actor.room.0, actor.facing, observer_status, actor.touch_count
        ));
    }

    let mut text = context.text.into_inner();
    **text = format!(
        "GUARDIAN MONITOR  {}\n\
         guardian room       R{}\n\
         guardian status     {}\n\
         anchor rooms        {:?}\n\
         closest path length  {} rooms\n\
         auto-move active    {}\n\
         resets              {}\n\n\
         ACTORS:\n\
         {}\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        context.guardian.room.0,
        guardian_status,
        context
            .runtime
            .anchors
            .iter()
            .map(|r| r.0)
            .collect::<Vec<_>>(),
        path_len,
        context.runtime.auto_move,
        context.runtime.reset_count,
        actor_diagnostics,
        context.runtime.last_event
    );
}
