use std::collections::HashSet;

use bevy::{
    ecs::system::SystemParam,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use fps_controller_lab::controller::{FIXED_DT, FpsArena, FpsBody, FpsConfig, step_body};
use fps_maze_lab::maze::{GRID_H, GRID_W, TILE_SIZE, Tile};
use observation_lab::model::ROOM_COUNT;
use observed_core::RoomId;
use player_input::PlayerIntent;

use crate::reroute::RerouteMaze;

const ROOM_FILL: Color = Color::srgb(0.20, 0.30, 0.46);
const CORRIDOR_FILL: Color = Color::srgb(0.16, 0.22, 0.30);
const SPINE_FILL: Color = Color::srgb(1.0, 0.78, 0.26);
const PENDING: Color = Color::srgb(1.0, 0.35, 0.85);
const WALLC: Color = Color::srgb(0.42, 0.50, 0.64);
const EXIT_FILL: Color = Color::srgb(0.36, 1.0, 0.58);
const GAZE: Color = Color::srgb(0.95, 0.97, 1.0);

const WALL_HEIGHT: f32 = 3.4;
const MOUSE_SENSITIVITY: f32 = 0.075;
/// How far (in tiles) and how wide the off-camera gate considers "in view".
const VIEW_RANGE_TILES: f32 = 16.0;
const VIEW_HALF_DEG: f32 = 48.0;

#[derive(Component)]
pub(crate) struct RerouteCam;

#[derive(Component)]
pub(crate) struct RerouteUiRoot;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource)]
pub struct RerouteRuntime {
    pub body: FpsBody,
    pub config: FpsConfig,
    pub intent: PlayerIntent,
    pub auto: bool,
    pub top_down: bool,
    pub debug_visible: bool,
    pub reset_count: u32,
    pub camera_override: Option<Transform>,
}

impl Default for RerouteRuntime {
    fn default() -> Self {
        Self {
            body: FpsBody::spawned(Vec3::new(0.0, 0.9, 0.0), 0.0),
            config: FpsConfig::default(),
            intent: PlayerIntent::default(),
            auto: true,
            top_down: false,
            debug_visible: true,
            reset_count: 0,
            camera_override: None,
        }
    }
}

#[derive(Resource)]
pub(crate) struct RerouteCollision(pub FpsArena);

#[derive(Resource)]
pub struct DecohereTimer(pub Timer);

impl Default for DecohereTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(2.0, TimerMode::Repeating))
    }
}

pub fn tile_world(x: usize, y: usize) -> Vec2 {
    Vec2::new(
        (x as f32 - GRID_W as f32 * 0.5 + 0.5) * TILE_SIZE,
        (y as f32 - GRID_H as f32 * 0.5 + 0.5) * TILE_SIZE,
    )
}

fn world_tile(p: Vec2) -> Option<(usize, usize)> {
    let tx = (p.x / TILE_SIZE + GRID_W as f32 * 0.5).floor();
    let ty = (p.y / TILE_SIZE + GRID_H as f32 * 0.5).floor();
    if tx < 0.0 || ty < 0.0 || tx >= GRID_W as f32 || ty >= GRID_H as f32 {
        None
    } else {
        Some((tx as usize, ty as usize))
    }
}

pub fn spawn_player(runtime: &mut RerouteRuntime, maze: &RerouteMaze) {
    let (cx, cy) = maze.rooms[0].center_tile();
    let point = tile_world(cx, cy);
    runtime.body = FpsBody::spawned(
        Vec3::new(
            point.x,
            maze.floor_height(cx, cy) + runtime.config.half_height,
            point.y,
        ),
        0.0,
    );
    runtime.body.grounded = true;
}

pub(crate) fn setup_lab(mut commands: Commands) {
    commands.spawn((
        RerouteCam,
        Camera3d::default(),
        Transform::from_translation(Vec3::new(0.0, 2.5, 0.0)),
        Name::new("Reroute First-Person Camera"),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.95, -0.6, 0.0)),
        Name::new("Reroute Sun"),
    ));
    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            RerouteUiRoot,
            Name::new("Reroute UI Root"),
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
                BackgroundColor(Color::srgba(0.02, 0.01, 0.03, 0.94)),
                BorderColor::all(Color::srgba(1.0, 0.45, 0.85, 0.6)),
                children![(
                    DebugText,
                    Text::new("Reroute diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.9, 0.97)),
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
                BackgroundColor(Color::srgba(0.02, 0.01, 0.03, 0.94)),
                BorderColor::all(Color::srgba(1.0, 0.45, 0.85, 0.6)),
                children![(
                    Text::new(
                        "REROUTING PASSAGES (Phase 26)\n\
                         WASD + mouse     Walk the maze, look around\n\
                         Space            Decohere now\n\
                         P                Toggle auto-decoherence\n\
                         Tab              Top-down map / first-person\n\
                         Esc cursor - R reset - F1 debug\n\n\
                         The maze is live: when a passage is unobserved, the graph\n\
                         rewires and the corridor re-routes to a different room — but\n\
                         only off-camera and never under your feet (magenta = a reroute\n\
                         waiting for you to look away). The room you stand in is frozen.\n\
                         Walk off, look back, and a corridor leads somewhere new.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.92, 0.9, 0.96)),
                )],
            ));
        });
}

/// The tiles the camera currently "sees" (range + FOV cone), the off-camera gate.
fn visible_tiles(maze: &RerouteMaze, runtime: &RerouteRuntime) -> HashSet<(usize, usize)> {
    let mut out = HashSet::new();
    if runtime.top_down {
        // The map view sees everything — reroutes wait until first-person.
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                if maze.rendered_tiles[y * GRID_W + x].is_floor() {
                    out.insert((x, y));
                }
            }
        }
        return out;
    }
    let eye = Vec2::new(runtime.body.position.x, runtime.body.position.z);
    let fwd = Vec2::new(runtime.body.yaw.sin(), -runtime.body.yaw.cos());
    let cos_half = VIEW_HALF_DEG.to_radians().cos();
    let range = VIEW_RANGE_TILES * TILE_SIZE;
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let centre = tile_world(x, y);
            let to = centre - eye;
            let dist = to.length();
            if dist > range {
                continue;
            }
            if dist < TILE_SIZE || to.normalize_or_zero().dot(fwd) >= cos_half {
                out.insert((x, y));
            }
        }
    }
    out
}

pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_motion: Option<Res<AccumulatedMouseMotion>>,
    mut runtime: ResMut<RerouteRuntime>,
    mut maze: ResMut<RerouteMaze>,
    mut collision: ResMut<RerouteCollision>,
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
    runtime.intent = PlayerIntent {
        movement,
        look: Vec2::new(
            axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) + mouse.x * MOUSE_SENSITIVITY,
            axis(KeyCode::ArrowUp, KeyCode::ArrowDown) + mouse.y * MOUSE_SENSITIVITY,
        ),
        jump_pressed: false,
        sprint_held: keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight),
        ..default()
    };

    if keyboard.just_pressed(KeyCode::Space) {
        maze.decohere();
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        runtime.auto = !runtime.auto;
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        runtime.top_down = !runtime.top_down;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        let count = runtime.reset_count + 1;
        let seed = maze.seed;
        maze.reset();
        *runtime = RerouteRuntime::default();
        runtime.reset_count = count;
        spawn_player(&mut runtime, &maze);
        collision.0 = maze.arena(WALL_HEIGHT);
        let _ = seed;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
}

pub(crate) fn simulate(mut runtime: ResMut<RerouteRuntime>, collision: Res<RerouteCollision>) {
    let intent = runtime.intent;
    let config = runtime.config;
    step_body(&mut runtime.body, intent, &collision.0, &config, FIXED_DT);
}

/// Observe the room the player stands in, auto-decohere, and try to reroute any
/// pending passages — but only the ones currently out of view and clear of feet.
pub(crate) fn reconcile(
    time: Res<Time>,
    mut timer: ResMut<DecohereTimer>,
    runtime: Res<RerouteRuntime>,
    mut maze: ResMut<RerouteMaze>,
    mut collision: ResMut<RerouteCollision>,
) {
    let player_tile = world_tile(Vec2::new(runtime.body.position.x, runtime.body.position.z));
    let observed = player_tile.and_then(|(x, y)| match maze.rendered_tiles[y * GRID_W + x] {
        Tile::Room(r) => Some(RoomId(r)),
        _ => None,
    });
    maze.observe(observed);

    if runtime.auto && timer.0.tick(time.delta()).just_finished() {
        maze.decohere();
    }

    let visible = visible_tiles(&maze, &runtime);
    if maze.try_commit(&visible, player_tile) {
        collision.0 = maze.arena(WALL_HEIGHT);
    }
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
    runtime: Res<RerouteRuntime>,
    mut camera: Single<&mut Transform, With<RerouteCam>>,
) {
    if let Some(pose) = runtime.camera_override {
        **camera = pose;
        return;
    }
    if runtime.top_down {
        let target = runtime.body.position;
        **camera = Transform::from_translation(target + Vec3::Y * 60.0)
            .looking_at(target, Vec3::new(0.0, 0.0, -1.0));
        return;
    }
    **camera = Transform::from_translation(runtime.body.eye(&runtime.config))
        .looking_to(runtime.body.look_dir(), Vec3::Y);
}

fn floor_square(gizmos: &mut Gizmos, centre: Vec2, y: f32, color: Color) {
    let h = TILE_SIZE * 0.5;
    let p = |dx: f32, dz: f32| Vec3::new(centre.x + dx, y, centre.y + dz);
    gizmos.linestrip([p(-h, -h), p(h, -h), p(h, h), p(-h, h), p(-h, -h)], color);
}

pub(crate) fn draw_maze(runtime: Res<RerouteRuntime>, maze: Res<RerouteMaze>, mut gizmos: Gizmos) {
    if !runtime.debug_visible {
        return;
    }

    let mut spine_tiles: HashSet<(usize, usize)> = HashSet::new();
    for corridor in &maze.rendered {
        if corridor.spine {
            for &t in &corridor.path {
                spine_tiles.insert(t);
            }
        }
    }
    let affected = maze.affected_tiles();

    let h = TILE_SIZE * 0.5;
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let tile = maze.rendered_tiles[y * GRID_W + x];
            let centre = tile_world(x, y);
            let floor_y = maze.floor_height(x, y);
            match tile {
                Tile::Room(r) => {
                    let color = if r == (ROOM_COUNT as u32 - 1) {
                        EXIT_FILL
                    } else {
                        ROOM_FILL
                    };
                    floor_square(&mut gizmos, centre, floor_y + 0.02, color);
                }
                Tile::Corridor => {
                    let color = if spine_tiles.contains(&(x, y)) {
                        SPINE_FILL
                    } else {
                        CORRIDOR_FILL
                    };
                    floor_square(&mut gizmos, centre, floor_y + 0.02, color);
                }
                Tile::Wall => {}
            }
            // Pending reroute tiles: magenta marker (waiting to swap off-camera).
            if affected.contains(&(x, y)) {
                floor_square(&mut gizmos, centre, floor_y + 0.16, PENDING);
            }
            if tile.is_floor() {
                let edges = [
                    ((0i32, -1i32), Vec2::new(0.0, -h)),
                    ((1, 0), Vec2::new(h, 0.0)),
                    ((0, 1), Vec2::new(0.0, h)),
                    ((-1, 0), Vec2::new(-h, 0.0)),
                ];
                for ((dx, dy), edge) in edges {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    let wall = nx < 0
                        || ny < 0
                        || nx >= GRID_W as i32
                        || ny >= GRID_H as i32
                        || maze.rendered_tiles[ny as usize * GRID_W + nx as usize] == Tile::Wall;
                    if wall {
                        let along = Vec2::new(edge.y, edge.x).normalize_or_zero() * h;
                        let a = centre + edge - along;
                        let b = centre + edge + along;
                        let base_a = Vec3::new(a.x, floor_y, a.y);
                        let base_b = Vec3::new(b.x, floor_y, b.y);
                        gizmos.line(base_a, base_a + Vec3::Y * WALL_HEIGHT, WALLC);
                        gizmos.line(base_b, base_b + Vec3::Y * WALL_HEIGHT, WALLC);
                        gizmos.line(
                            base_a + Vec3::Y * WALL_HEIGHT,
                            base_b + Vec3::Y * WALL_HEIGHT,
                            WALLC,
                        );
                    }
                }
            }
        }
    }

    if !runtime.top_down {
        let feet = runtime.body.position.y - runtime.config.half_height;
        let here = Vec3::new(
            runtime.body.position.x,
            feet + 0.08,
            runtime.body.position.z,
        );
        let f = runtime.body.forward();
        gizmos.line(here, here + f * (TILE_SIZE * 1.5), GAZE);
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, RerouteRuntime>,
    maze: Res<'w, RerouteMaze>,
    cams: Query<'w, 's, (), With<RerouteCam>>,
    ui_roots: Query<'w, 's, (), With<RerouteUiRoot>>,
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

    let maze = &*context.maze;
    let pending = maze.affected_tiles().len();
    let observed = maze
        .observed_rooms()
        .first()
        .map(|r| r.0 as i32)
        .unwrap_or(-1);
    let cams = context.cams.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = cams == 1 && ui_roots == 1 && maze.navigable();

    let mut text = context.text.into_inner();
    **text = format!(
        "REROUTING PASSAGES  {}\n\
         observed room   {}\n\
         pending tiles   {}\n\
         in sync         {}\n\
         commits         {}   deferred {}\n\
         decoherences    {}\n\
         rooms reachable {} / {}\n\
         view            {}   resets {}\n\
         camera {cams}  UI {ui_roots}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        if observed < 0 {
            "— (in a corridor)".to_string()
        } else {
            observed.to_string()
        },
        pending,
        if maze.in_sync() {
            "yes"
        } else {
            "no (reroute waiting)"
        },
        maze.commit_count,
        maze.deferred_count,
        maze.decohere_count,
        maze.reachable_rooms(),
        ROOM_COUNT,
        if context.runtime.top_down {
            "top-down map"
        } else {
            "first-person"
        },
        context.runtime.reset_count,
        maze.last_event,
    );
}
