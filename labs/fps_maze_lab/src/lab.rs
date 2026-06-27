use bevy::{
    ecs::system::SystemParam,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use constraint_lab::model::ConstraintWorld;
use fps_controller_lab::controller::{FIXED_DT, FpsArena, FpsBody, FpsConfig, step_body};
use observation_lab::model::ROOM_COUNT;
use observed_core::RoomId;
use player_input::PlayerIntent;

use crate::maze::{GRID_H, GRID_W, MazeLayout, TILE_SIZE, Tile};

const ROOM_FILL: Color = Color::srgb(0.20, 0.30, 0.46);
const CORRIDOR_FILL: Color = Color::srgb(0.16, 0.22, 0.30);
const SPINE_FILL: Color = Color::srgb(1.0, 0.78, 0.26);
const WALLC: Color = Color::srgb(0.42, 0.50, 0.64);
const EXIT_FILL: Color = Color::srgb(0.36, 1.0, 0.58);
const GAZE: Color = Color::srgb(0.95, 0.97, 1.0);

const WALL_HEIGHT: f32 = 3.4;
const MOUSE_SENSITIVITY: f32 = 0.075;
const START_ROOM: u32 = 0;
const EXIT_ROOM: u32 = 8;

#[derive(Component)]
pub(crate) struct MazeCam;

#[derive(Component)]
pub(crate) struct MazeUiRoot;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource)]
pub struct MazeRuntime {
    pub world: ConstraintWorld,
    pub seed: u64,
    pub body: FpsBody,
    pub config: FpsConfig,
    pub intent: PlayerIntent,
    pub top_down: bool,
    pub debug_visible: bool,
    pub regen_count: u32,
    pub reset_count: u32,
    pub camera_override: Option<Transform>,
    pub last_event: String,
}

impl Default for MazeRuntime {
    fn default() -> Self {
        let world = ConstraintWorld::authored();
        Self {
            world,
            seed: 1,
            body: FpsBody::spawned(Vec3::new(0.0, 0.9, 0.0), 0.0),
            config: FpsConfig::default(),
            intent: PlayerIntent::default(),
            top_down: false,
            debug_visible: true,
            regen_count: 0,
            reset_count: 0,
            camera_override: None,
            last_event: "Walk the corridors — the graph embedded as real space.".to_string(),
        }
    }
}

#[derive(Resource)]
pub(crate) struct MazeCollision(pub FpsArena);

impl MazeRuntime {
    /// Is the protected spine routed through this corridor (for gold highlight)?
    pub fn corridor_is_spine(&self, door: observation_lab::model::DoorId) -> bool {
        self.world.is_protected(door)
    }
}

fn build_maze(world: &ConstraintWorld, seed: u64) -> MazeLayout {
    MazeLayout::generate(&world.graph, seed)
}

pub(crate) fn setup_lab(mut commands: Commands) {
    commands.spawn((
        MazeCam,
        Camera3d::default(),
        Transform::from_translation(Vec3::new(0.0, 2.5, 0.0)),
        Name::new("Maze First-Person Camera"),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.95, -0.6, 0.0)),
        Name::new("Maze Sun"),
    ));
    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            MazeUiRoot,
            Name::new("Maze UI Root"),
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
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.94)),
                BorderColor::all(Color::srgba(0.5, 0.8, 1.0, 0.6)),
                children![(
                    DebugText,
                    Text::new("Maze diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.93, 1.0)),
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
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.94)),
                BorderColor::all(Color::srgba(0.5, 0.8, 1.0, 0.6)),
                children![(
                    Text::new(
                        "MULTI-LEVEL SPATIAL MAZE\n\
                         WASD + mouse     Walk the maze, look around\n\
                         Shift / Space    Sprint / jump\n\
                         N                Regenerate (new seed)\n\
                         Tab              Top-down map / first-person\n\
                         Esc release cursor - R reset - F1 debug\n\n\
                         The proven room graph embedded in space: nine rooms placed\n\
                         deterministically and every graph connection routed as a real\n\
                         walkable corridor (gold = protected spine) — no portals. The\n\
                         layout is a function of the graph and seed; press N to vary it.\n\
                         Three generated floor bands are joined by climbable stairs.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.93, 0.97)),
                )],
            ));
        });
}

pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_motion: Option<Res<AccumulatedMouseMotion>>,
    mut runtime: ResMut<MazeRuntime>,
    mut maze: ResMut<MazeLayout>,
    mut collision: ResMut<MazeCollision>,
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
        jump_pressed: keyboard.pressed(KeyCode::Space),
        sprint_held: keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight),
        ..default()
    };

    if keyboard.just_pressed(KeyCode::KeyN) {
        runtime.seed = runtime
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        runtime.regen_count += 1;
        *maze = build_maze(&runtime.world, runtime.seed);
        collision.0 = maze.arena(WALL_HEIGHT);
        spawn_player(&mut runtime, &maze);
        runtime.last_event = format!("Regenerated maze (seed {}).", runtime.seed);
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        runtime.top_down = !runtime.top_down;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        let count = runtime.reset_count + 1;
        *runtime = MazeRuntime::default();
        runtime.reset_count = count;
        *maze = build_maze(&runtime.world, runtime.seed);
        collision.0 = maze.arena(WALL_HEIGHT);
        spawn_player(&mut runtime, &maze);
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
}

pub(crate) fn simulate(mut runtime: ResMut<MazeRuntime>, collision: Res<MazeCollision>) {
    let intent = runtime.intent;
    let config = runtime.config;
    step_body(&mut runtime.body, intent, &collision.0, &config, FIXED_DT);
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
    runtime: Res<MazeRuntime>,
    mut camera: Single<&mut Transform, With<MazeCam>>,
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
    let p = |dx: f32, dz: f32| Vec3::new(centre.x + dx, y + 0.02, centre.y + dz);
    gizmos.linestrip([p(-h, -h), p(h, -h), p(h, h), p(-h, h), p(-h, -h)], color);
}

pub(crate) fn draw_maze(runtime: Res<MazeRuntime>, maze: Res<MazeLayout>, mut gizmos: Gizmos) {
    if !runtime.debug_visible {
        return;
    }

    // Spine corridor tiles, for a gold highlight.
    let mut spine_tiles: std::collections::HashSet<(usize, usize)> =
        std::collections::HashSet::new();
    for corridor in &maze.corridors {
        if runtime.corridor_is_spine(corridor.door_a) {
            for &t in &corridor.path {
                spine_tiles.insert(t);
            }
        }
    }

    let h = TILE_SIZE * 0.5;
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let tile = maze.at(x, y);
            let centre = maze.tile_world(x, y);
            let floor_y = maze.floor_height(x, y);
            match tile {
                Tile::Room(r) => {
                    let color = if r == EXIT_ROOM { EXIT_FILL } else { ROOM_FILL };
                    floor_square(&mut gizmos, centre, floor_y, color);
                }
                Tile::Corridor => {
                    let color = if spine_tiles.contains(&(x, y)) {
                        SPINE_FILL
                    } else {
                        CORRIDOR_FILL
                    };
                    floor_square(&mut gizmos, centre, floor_y, color);
                }
                Tile::Wall => {}
            }
            // Walls: a vertical face on each floor→wall (or boundary) edge.
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
                        || maze.at(nx as usize, ny as usize) == Tile::Wall;
                    if wall {
                        // Edge endpoints (perpendicular to the edge direction).
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

    // The gaze: a short line on the floor showing facing.
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
    runtime: Res<'w, MazeRuntime>,
    maze: Res<'w, MazeLayout>,
    cams: Query<'w, 's, (), With<MazeCam>>,
    ui_roots: Query<'w, 's, (), With<MazeUiRoot>>,
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

    let cams = context.cams.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let reachable = context.maze.reachable_rooms();
    let healthy = cams == 1 && ui_roots == 1 && reachable == ROOM_COUNT;

    let mut text = context.text.into_inner();
    **text = format!(
        "SPATIAL MAZE  {}\n\
         seed            {}\n\
         rooms reachable {} / {}\n\
         corridors       {}\n\
         floor levels    {} (max {:.2}m)\n\
         player feet     {:.2}m\n\
         rooms overlap   {}\n\
         view            {}\n\
         regenerations   {}   resets {}\n\
         camera {cams}  UI {ui_roots}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        context.runtime.seed,
        reachable,
        ROOM_COUNT,
        context.maze.corridors.len(),
        crate::maze::LEVEL_COUNT,
        context.maze.max_floor_height(),
        context.runtime.body.position.y - context.runtime.config.half_height,
        if context.maze.rooms_overlap() {
            "YES"
        } else {
            "no"
        },
        if context.runtime.top_down {
            "top-down map"
        } else {
            "first-person"
        },
        context.runtime.regen_count,
        context.runtime.reset_count,
        context.runtime.last_event,
    );
}

/// Build the initial layout resource from the runtime's authored graph.
pub fn initial_maze(runtime: &MazeRuntime) -> MazeLayout {
    build_maze(&runtime.world, runtime.seed)
}

/// Place the player in the start room (called once the layout exists).
pub fn spawn_player(runtime: &mut MazeRuntime, maze: &MazeLayout) {
    let floor = maze.room_floor_height(RoomId(START_ROOM));
    let position = maze.room_world(RoomId(START_ROOM));
    runtime.body = FpsBody::spawned(
        Vec3::new(position.x, floor + runtime.config.half_height, position.y),
        0.0,
    );
    runtime.body.grounded = true;
}

pub fn initial_collision(maze: &MazeLayout) -> MazeCollision {
    MazeCollision(maze.arena(WALL_HEIGHT))
}
