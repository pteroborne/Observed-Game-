use std::collections::HashSet;

use bevy::{
    ecs::system::SystemParam,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use competitive_facility::model::{EXIT_ROOM, TEAM_COUNT};
use director_lab::model::Role;
use fps_maze_lab::maze::{GRID_H, GRID_W, TILE_SIZE, Tile};
use observation_lab::model::ROOM_COUNT;
use player_input::PlayerIntent;

use crate::hybrid::{HybridMatch, HybridTape, LOCAL_TEAM};

const ROOM_FILL: Color = Color::srgb(0.18, 0.27, 0.42);
const CORRIDOR_FILL: Color = Color::srgb(0.15, 0.20, 0.28);
const SPINE_FILL: Color = Color::srgb(1.0, 0.78, 0.26);
const SAFE_FILL: Color = Color::srgb(0.18, 0.90, 0.92);
const TRAP_ACTIVE_FILL: Color = Color::srgb(1.0, 0.16, 0.08);
const TRAP_IDLE_FILL: Color = Color::srgb(1.0, 0.52, 0.10);
const PENDING_FILL: Color = Color::srgb(1.0, 0.30, 0.82);
const COLLAPSE_FILL: Color = Color::srgb(0.55, 0.12, 0.14);
const EXIT_FILL: Color = Color::srgb(0.36, 1.0, 0.58);
const WALL_COLOR: Color = Color::srgb(0.40, 0.48, 0.62);
const TARGET_COLOR: Color = Color::srgb(1.0, 0.92, 0.45);
const TEAM_COLORS: [Color; TEAM_COUNT] = [
    Color::srgb(0.96, 0.28, 0.34),
    Color::srgb(0.32, 0.62, 1.0),
    Color::srgb(0.72, 0.46, 1.0),
    Color::srgb(1.0, 0.62, 0.20),
];
const WALL_HEIGHT: f32 = 3.4;
const MOUSE_SENSITIVITY: f32 = 0.12;

#[derive(Component)]
pub(crate) struct HybridCam;

#[derive(Component)]
pub(crate) struct HybridUiRoot;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Live,
    Replay,
}

#[derive(Resource, Default)]
pub(crate) struct InputIntent(pub PlayerIntent);

#[derive(Resource)]
pub(crate) struct LiveMatch(pub HybridMatch);

#[derive(Resource)]
pub(crate) struct DemoTape(pub HybridTape);

#[derive(Resource)]
pub(crate) struct ActiveView(pub HybridMatch);

#[derive(Resource)]
pub struct HybridRuntime {
    pub mode: Mode,
    pub replay_cursor: f32,
    pub replay_playing: bool,
    pub replay_verified: bool,
    pub top_down: bool,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    pub camera_override: Option<Transform>,
}

impl Default for HybridRuntime {
    fn default() -> Self {
        Self {
            mode: Mode::Live,
            replay_cursor: 0.0,
            replay_playing: false,
            replay_verified: false,
            top_down: false,
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
            camera_override: None,
        }
    }
}

#[derive(Resource)]
pub(crate) struct ResolutionTimer(pub Timer);

impl Default for ResolutionTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.45, TimerMode::Repeating))
    }
}

fn tile_world(x: usize, y: usize) -> Vec2 {
    Vec2::new(
        (x as f32 - GRID_W as f32 * 0.5 + 0.5) * TILE_SIZE,
        (y as f32 - GRID_H as f32 * 0.5 + 0.5) * TILE_SIZE,
    )
}

fn room_world(game: &HybridMatch, room: observed_core::RoomId) -> Vec2 {
    let (x, y) = game.rooms[room.0 as usize].center_tile();
    tile_world(x, y)
}

pub(crate) fn setup_lab(mut commands: Commands) {
    commands.spawn((
        HybridCam,
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.0, 0.0),
        Name::new("Hybrid Match Camera"),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 9_500.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.95, -0.6, 0.0)),
        Name::new("Hybrid Match Sun"),
    ));
    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            HybridUiRoot,
            Name::new("Hybrid Match UI Root"),
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
                    width: px(500),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.012, 0.018, 0.03, 0.95)),
                BorderColor::all(Color::srgba(0.5, 0.85, 1.0, 0.6)),
                children![(
                    DebugText,
                    Text::new("Hybrid match diagnostics starting..."),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.86, 0.94, 1.0)),
                )],
            ));
            root.spawn((
                HelpText,
                Node {
                    width: px(455),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.012, 0.018, 0.03, 0.95)),
                BorderColor::all(Color::srgba(0.5, 0.85, 1.0, 0.6)),
                children![(
                    Text::new(
                        "FIRST-PERSON HYBRID MATCH (Phase 27)\n\
                         WASD + mouse     Walk the concrete maze\n\
                         Shift / Space    Sprint / jump\n\
                         E                Seize control in its room\n\
                         Tab              Tac-map / first-person\n\
                         T                Live / exact demo replay\n\
                         P                Play/pause replay\n\
                         Left/Right [/]   Replay step / seek\n\
                         Esc cursor - R reset - F1 debug\n\n\
                         Enter the next gold-spine room to advance your team. The\n\
                         red pressure gate is the short route; cyan is the longer\n\
                         safe bypass. An active pulse returns you to the checkpoint\n\
                         without removing progress. Watch its rhythm or go around.\n\
                         graph rewires after each round, but changed corridors swap\n\
                         only off-camera and clear of your body (magenta = pending).\n\
                         The match, rendered maze, and first-person pose replay exactly.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.94, 0.98)),
                )],
            ));
        });
}

pub(crate) fn map_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_motion: Option<Res<AccumulatedMouseMotion>>,
    mut intent: ResMut<InputIntent>,
) {
    let axis = |negative: KeyCode, positive: KeyCode| {
        (keyboard.pressed(positive) as i32 - keyboard.pressed(negative) as i32) as f32
    };
    let mouse = mouse_motion
        .map(|motion| motion.delta)
        .unwrap_or(Vec2::ZERO);
    let mut movement = Vec2::new(
        axis(KeyCode::KeyA, KeyCode::KeyD),
        axis(KeyCode::KeyS, KeyCode::KeyW),
    );
    if movement.length_squared() > 1.0 {
        movement = movement.normalize_or_zero();
    }
    intent.0 = PlayerIntent {
        movement,
        look: Vec2::new(
            axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) + mouse.x * MOUSE_SENSITIVITY,
            axis(KeyCode::ArrowUp, KeyCode::ArrowDown) + mouse.y * MOUSE_SENSITIVITY,
        ),
        jump_pressed: keyboard.pressed(KeyCode::Space),
        sprint_held: keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight),
        interact_pressed: keyboard.just_pressed(KeyCode::KeyE),
        ..default()
    };
}

pub(crate) fn simulate_live(
    mut intent: ResMut<InputIntent>,
    runtime: Res<HybridRuntime>,
    mut live: ResMut<LiveMatch>,
) {
    if runtime.mode == Mode::Live {
        live.0.step_player(intent.0, runtime.top_down);
    }
    intent.0.interact_pressed = false;
}

pub(crate) fn resolve_after_local_finish(
    time: Res<Time>,
    runtime: Res<HybridRuntime>,
    mut timer: ResMut<ResolutionTimer>,
    mut live: ResMut<LiveMatch>,
) {
    if runtime.mode != Mode::Live || live.0.competitive.finished {
        timer.0.reset();
        return;
    }
    let local_active = live
        .0
        .competitive
        .team(LOCAL_TEAM)
        .is_some_and(|team| team.active_runner());
    if local_active {
        timer.0.reset();
        return;
    }
    if timer.0.tick(time.delta()).just_finished() {
        live.0.apply_action(crate::hybrid::LocalAction::Wait);
    }
}

pub(crate) fn handle_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    tape: Res<DemoTape>,
    mut runtime: ResMut<HybridRuntime>,
) {
    if keyboard.just_pressed(KeyCode::KeyT) {
        runtime.mode = match runtime.mode {
            Mode::Live => Mode::Replay,
            Mode::Replay => Mode::Live,
        };
        runtime.replay_cursor = 0.0;
        runtime.replay_playing = runtime.mode == Mode::Replay;
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        runtime.top_down = !runtime.top_down;
    }
    if keyboard.just_pressed(KeyCode::KeyP) && runtime.mode == Mode::Replay {
        runtime.replay_playing = !runtime.replay_playing;
    }
    if runtime.mode == Mode::Replay {
        let end = tape.0.len() as f32;
        if keyboard.just_pressed(KeyCode::ArrowRight) {
            runtime.replay_cursor = (runtime.replay_cursor.floor() + 1.0).min(end);
            runtime.replay_playing = false;
        }
        if keyboard.just_pressed(KeyCode::ArrowLeft) {
            runtime.replay_cursor = (runtime.replay_cursor.floor() - 1.0).max(0.0);
            runtime.replay_playing = false;
        }
        if keyboard.just_pressed(KeyCode::BracketRight) {
            runtime.replay_cursor = (runtime.replay_cursor + 5.0).min(end);
            runtime.replay_playing = false;
        }
        if keyboard.just_pressed(KeyCode::BracketLeft) {
            runtime.replay_cursor = (runtime.replay_cursor - 5.0).max(0.0);
            runtime.replay_playing = false;
        }
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
}

pub(crate) fn advance_replay(
    time: Res<Time>,
    tape: Res<DemoTape>,
    mut runtime: ResMut<HybridRuntime>,
) {
    let end = tape.0.len() as f32;
    runtime.replay_cursor = runtime.replay_cursor.min(end);
    if runtime.mode == Mode::Replay && runtime.replay_playing {
        runtime.replay_cursor = (runtime.replay_cursor + time.delta_secs() * 1.5).min(end);
        if runtime.replay_cursor >= end {
            runtime.replay_playing = false;
        }
    }
    runtime.replay_verified = tape
        .0
        .snapshots
        .last()
        .is_some_and(|expected| tape.0.replay_to(tape.0.len()).snapshot() == *expected);
}

pub(crate) fn perform_reset(mut live: ResMut<LiveMatch>, mut runtime: ResMut<HybridRuntime>) {
    if !runtime.reset_requested {
        return;
    }
    let resets = runtime.reset_count + 1;
    let seed = live.0.seed;
    let camera_override = runtime.camera_override.take();
    live.0 = HybridMatch::authored(seed);
    *runtime = HybridRuntime::default();
    runtime.reset_count = resets;
    runtime.camera_override = camera_override;
}

pub(crate) fn update_view(
    runtime: Res<HybridRuntime>,
    live: Res<LiveMatch>,
    tape: Res<DemoTape>,
    mut view: ResMut<ActiveView>,
) {
    view.0 = match runtime.mode {
        Mode::Live => live.0.clone(),
        Mode::Replay => tape.0.replay_to(runtime.replay_cursor.floor() as usize),
    };
}

pub(crate) fn grab_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor) = cursors.single_mut() {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

pub(crate) fn toggle_grab(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    if let Ok(mut cursor) = cursors.single_mut() {
        let grabbed = cursor.grab_mode != CursorGrabMode::None;
        cursor.grab_mode = if grabbed {
            CursorGrabMode::None
        } else {
            CursorGrabMode::Locked
        };
        cursor.visible = grabbed;
    }
}

pub(crate) fn present_camera(
    runtime: Res<HybridRuntime>,
    view: Res<ActiveView>,
    mut camera: Single<&mut Transform, With<HybridCam>>,
) {
    if let Some(pose) = runtime.camera_override {
        **camera = pose;
        return;
    }
    if runtime.top_down || runtime.mode == Mode::Replay {
        let target = view.0.body.position.with_y(0.0);
        **camera = Transform::from_translation(target + Vec3::Y * 62.0)
            .looking_at(target, Vec3::new(0.0, 0.0, -1.0));
    } else {
        **camera = Transform::from_translation(view.0.body.eye(&view.0.config))
            .looking_to(view.0.body.look_dir(), Vec3::Y);
    }
}

fn floor_square(gizmos: &mut Gizmos, centre: Vec2, y: f32, color: Color) {
    let half = TILE_SIZE * 0.5;
    let point = |dx: f32, dz: f32| Vec3::new(centre.x + dx, y, centre.y + dz);
    gizmos.linestrip(
        [
            point(-half, -half),
            point(half, -half),
            point(half, half),
            point(-half, half),
            point(-half, -half),
        ],
        color,
    );
}

pub(crate) fn draw_world(runtime: Res<HybridRuntime>, view: Res<ActiveView>, mut gizmos: Gizmos) {
    if !runtime.debug_visible {
        return;
    }
    let game = &view.0;
    let collapse: HashSet<u32> = game
        .competitive
        .collapse_rooms()
        .into_iter()
        .map(|room| room.0)
        .collect();
    let pending = game.affected_tiles();
    let half = TILE_SIZE * 0.5;

    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let tile = game.maze_tiles[y * GRID_W + x];
            let centre = tile_world(x, y);
            let floor_y = game.floor_height(x, y);
            match tile {
                Tile::Room(room) => {
                    let color = if room == EXIT_ROOM {
                        EXIT_FILL
                    } else if collapse.contains(&room) {
                        COLLAPSE_FILL
                    } else {
                        ROOM_FILL
                    };
                    floor_square(&mut gizmos, centre, floor_y + 0.02, color);
                }
                Tile::Corridor => {
                    let color = if game.trap_tiles.contains(&(x, y)) {
                        if game.trap_active() {
                            TRAP_ACTIVE_FILL
                        } else {
                            TRAP_IDLE_FILL
                        }
                    } else if game.safe_tiles.contains(&(x, y)) {
                        SAFE_FILL
                    } else if game.spine_tiles.contains(&(x, y)) {
                        SPINE_FILL
                    } else {
                        CORRIDOR_FILL
                    };
                    floor_square(&mut gizmos, centre, floor_y + 0.02, color);
                }
                Tile::Wall => {}
            }
            if pending.contains(&(x, y)) {
                floor_square(&mut gizmos, centre, floor_y + 0.14, PENDING_FILL);
            }
            if tile.is_floor() {
                let edges = [
                    ((0i32, -1i32), Vec2::new(0.0, -half)),
                    ((1, 0), Vec2::new(half, 0.0)),
                    ((0, 1), Vec2::new(0.0, half)),
                    ((-1, 0), Vec2::new(-half, 0.0)),
                ];
                for ((dx, dy), edge) in edges {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    let wall = nx < 0
                        || ny < 0
                        || nx >= GRID_W as i32
                        || ny >= GRID_H as i32
                        || game.maze_tiles[ny as usize * GRID_W + nx as usize] == Tile::Wall;
                    if wall {
                        let along = Vec2::new(edge.y, edge.x).normalize_or_zero() * half;
                        let a = centre + edge - along;
                        let b = centre + edge + along;
                        let base_a = Vec3::new(a.x, floor_y, a.y);
                        let base_b = Vec3::new(b.x, floor_y, b.y);
                        gizmos.line(base_a, base_a + Vec3::Y * WALL_HEIGHT, WALL_COLOR);
                        gizmos.line(base_b, base_b + Vec3::Y * WALL_HEIGHT, WALL_COLOR);
                        gizmos.line(
                            base_a + Vec3::Y * WALL_HEIGHT,
                            base_b + Vec3::Y * WALL_HEIGHT,
                            WALL_COLOR,
                        );
                    }
                }
            }
        }
    }

    for (index, team) in game.competitive.teams.iter().enumerate() {
        let room = game.competitive.team_room(index);
        let centre = room_world(game, room);
        let base = TEAM_COLORS[index];
        let color = match (team.placement, team.role) {
            (Some(_), _) => base.mix(&Color::WHITE, 0.5),
            (None, Role::Director) => base.mix(&Color::srgb(0.07, 0.07, 0.1), 0.7),
            _ => base,
        };
        let foot = Vec3::new(centre.x, game.room_floor_height(room) + 0.1, centre.y);
        let height = if team.role == Role::Director {
            1.0
        } else {
            3.0
        };
        gizmos.line(foot, foot + Vec3::Y * height, color);
    }

    if let Some(target) = game.local_target() {
        let centre = room_world(game, target);
        let foot = Vec3::new(centre.x, game.room_floor_height(target) + 0.1, centre.y);
        gizmos.line(foot, foot + Vec3::Y * 4.5, TARGET_COLOR);
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, HybridRuntime>,
    view: Res<'w, ActiveView>,
    tape: Res<'w, DemoTape>,
    cams: Query<'w, 's, (), With<HybridCam>>,
    ui_roots: Query<'w, 's, (), With<HybridUiRoot>>,
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

    let game = &context.view.0;
    let cameras = context.cams.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = cameras == 1
        && ui_roots == 1
        && game.navigable()
        && game.player_on_floor()
        && context.runtime.replay_verified;
    let target = game
        .local_target()
        .map_or_else(|| "exit reached".to_string(), |room| room.0.to_string());
    let player_room = game
        .player_room()
        .map_or_else(|| "corridor".to_string(), |room| room.0.to_string());
    let route_lengths = game.local_route_lengths().map_or_else(
        || "—".to_string(),
        |(risk, safe)| format!("{risk} / {safe}"),
    );

    let mut text = context.text.into_inner();
    **text = format!(
        "FIRST-PERSON HYBRID MATCH  {}\n\
         mode / camera    {:?} / {}\n\
         round            {}   replay {:.0}/{}\n\
         local / player   {} / {}\n\
         target room      {}\n\
         match            {}\n\
         control holder   {}\n\
         escaped {}   absorbed {}\n\
         collapse         {:.0}%\n\
         maze reachable   {} / {}\n\
         player elevation {:.2}m\n\
         risk / safe len {} tiles\n\
         pressure gate    {}  hits {}  stun {}\n\
         reroute          {}  pending {} tiles\n\
         commits {}   deferred {}  feedback {}\n\
         replay exact     {}\n\
         player on floor  {}\n\
         camera {} UI {} resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        context.runtime.mode,
        if context.runtime.top_down || context.runtime.mode == Mode::Replay {
            "TAC-MAP"
        } else {
            "FIRST PERSON"
        },
        game.competitive.round,
        context.runtime.replay_cursor,
        context.tape.0.len(),
        game.local_room().0,
        player_room,
        target,
        if game.competitive.finished {
            "FINISHED"
        } else {
            "racing"
        },
        game.competitive
            .control_holder
            .map_or_else(|| "none".to_string(), |team| team.label()),
        game.competitive.escaped_count(),
        game.competitive.absorbed_count(),
        game.competitive.purge_line.max(0.0) * 100.0,
        game.reachable_rooms(),
        ROOM_COUNT,
        game.body.position.y - game.config.half_height,
        route_lengths,
        if game.trap_active() { "ACTIVE" } else { "idle" },
        game.trap_hits,
        game.trap_cooldown_ticks,
        if game.in_sync() { "in sync" } else { "WAITING" },
        game.affected_tiles().len(),
        game.reroute_commits,
        game.reroute_deferrals,
        game.reroute_feedback_ticks,
        if context.runtime.replay_verified {
            "MATCH"
        } else {
            "MISMATCH"
        },
        if game.player_on_floor() { "yes" } else { "NO" },
        cameras,
        ui_roots,
        context.runtime.reset_count,
        game.last_event,
    );
}

pub(crate) fn configure_capture(runtime: &mut HybridRuntime, tape: &HybridTape) {
    runtime.mode = Mode::Replay;
    runtime.replay_playing = false;
    runtime.replay_cursor = (tape.len() as f32 * 0.6).floor();
    runtime.top_down = true;
    runtime.replay_verified = tape
        .snapshots
        .last()
        .is_some_and(|expected| tape.replay_to(tape.len()).snapshot() == *expected);
    runtime.camera_override =
        Some(Transform::from_xyz(0.0, 72.0, 56.0).looking_at(Vec3::ZERO, Vec3::Y));
}
