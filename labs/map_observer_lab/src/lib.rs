use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PresentMode, WindowResolution};
use std::path::PathBuf;

use observed_facility::full_wfc::ModuleArchetype;
use observed_facility::map_spec::RoomRole;
use observed_game::GameState;
use observed_game::full_wfc::sim::FullWfcRuntime;
use observed_match::full_wfc::StructureRole;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CameraMode {
    FreeFly,
    TopDownMap,
    TopDownRoom,
}

impl CameraMode {
    fn label(self) -> &'static str {
        match self {
            CameraMode::FreeFly => "FREE-FLY OBSERVER",
            CameraMode::TopDownMap => "TOP-DOWN (Whole Map)",
            CameraMode::TopDownRoom => "TOP-DOWN (Room / Cell)",
        }
    }
}

#[derive(Resource)]
struct ObserverState {
    yaw: f32,
    pitch: f32,
    translation: Vec3,
    initialized: bool,
    speed: f32,
    last_status: String,
    mode: CameraMode,
    snap_coord: Option<Vec3>,
    glow_enabled: bool,
}

impl Default for ObserverState {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: -0.4,
            translation: Vec3::new(0.0, 30.0, 50.0),
            initialized: false,
            speed: 25.0,
            last_status:
                "Fly around using WASD. Shift: speed boost. Right-click drag to look around."
                    .to_string(),
            mode: CameraMode::FreeFly,
            snap_coord: None,
            glow_enabled: true,
        }
    }
}

#[derive(Component)]
struct ObserverHudText;

pub struct MapObserverLabPlugin;

impl Plugin for MapObserverLabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ObserverState>()
            .add_systems(Startup, (autostart_match, setup_hud))
            .add_systems(Update, (hud_sync, draw_debug_grid_gizmos))
            .add_systems(PostUpdate, observer_fly_control);
    }
}

fn autostart_match(mut next: ResMut<NextState<GameState>>) {
    next.set(GameState::FullWfc);
}

fn setup_hud(mut commands: Commands) {
    commands.spawn((
        ObserverHudText,
        Text::new(""),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.24, 0.82, 1.0)), // Cyan neon glow color
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            left: Val::Px(16.0),
            ..default()
        },
    ));
}

fn hud_sync(
    state: Res<ObserverState>,
    runtime: Option<Res<FullWfcRuntime>>,
    mut query: Query<&mut Text, With<ObserverHudText>>,
) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };

    let seed_str = runtime.as_ref().map_or("N/A".to_string(), |r| {
        format!("{:#018x}", r.match_state.facility.seed)
    });

    let pos = state.translation;
    let glow_status = if state.glow_enabled {
        "ENABLED (G to toggle)"
    } else {
        "DISABLED (G to toggle)"
    };

    text.0 = format!(
        "=== OBSERVED 2: 3D MAP OBSERVER ===\n\
         Seed: {}\n\
         Camera Mode: {}\n\
         Edge Glow: {}\n\
         Position: X={:.1} Y={:.1} Z={:.1}\n\
         Speed: {:.1} m/s (Shift to Sprint)\n\
         \n\
         --- Snapping Controls ---\n\
         [1] Teleport to START room\n\
         [2] Teleport to WELLSHAFT tower\n\
         [3] Teleport to GANTRY course\n\
         [4] Teleport to CLIMB shaft\n\
         [5] Teleport to KEYSTONE chamber\n\
         [6] Teleport to EXIT portal\n\
         \n\
         --- Camera Mode Controls ---\n\
         [Tab] or [F] -> Toggle FREE-FLY / TOP-DOWN\n\
         [M] or [0]   -> Snap TOP-DOWN Whole Map view\n\
         [T]          -> Snap TOP-DOWN focused Room view\n\
         [G]          -> Toggle Wall/Floor Edge Glow\n\
         \n\
         --- Inspection Controls ---\n\
         [C] or [Enter] -> Take Screenshot\n\
         [Space/E] -> Fly/Zoom UP | [Q/Ctrl] -> Fly/Zoom DOWN\n\
         \n\
         Status: {}\n",
        seed_str,
        state.mode.label(),
        glow_status,
        pos.x,
        pos.y,
        pos.z,
        state.speed,
        state.last_status
    );
}

#[allow(clippy::too_many_arguments)]
fn observer_fly_control(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse: Option<Res<bevy::input::mouse::AccumulatedMouseMotion>>,
    mut state: ResMut<ObserverState>,
    runtime: Option<Res<FullWfcRuntime>>,
    mut camera: Query<&mut Transform, (With<Camera3d>, Without<DirectionalLight>)>,
    mut commands: Commands,
) {
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };

    let dt = time.delta_secs();

    // 1. Setup Initial Spawn position if not initialized
    if !state.initialized {
        let start_pos = runtime
            .as_ref()
            .and_then(|r| find_room_position(r, RoomRole::Start));
        if let Some(pos) = start_pos {
            state.translation = pos + Vec3::new(0.0, 10.0, 15.0);
            state.snap_coord = Some(pos);
            state.initialized = true;
        }
    }

    // 2. Snapping/Teleport Controls
    let snap_key = |key: KeyCode| keyboard.just_pressed(key);

    if let Some(runtime) = runtime.as_ref() {
        if snap_key(KeyCode::Digit1) {
            if let Some(pos) = find_room_position(runtime, RoomRole::Start) {
                state.snap_coord = Some(pos);
                if state.mode == CameraMode::TopDownRoom {
                    state.translation = Vec3::new(pos.x, pos.y + 35.0, pos.z);
                } else if state.mode == CameraMode::FreeFly {
                    state.translation = pos + Vec3::new(0.0, 2.0, 0.0);
                    state.yaw = 0.0;
                    state.pitch = 0.0;
                }
                state.last_status = "Teleported focus to START room.".to_string();
            }
        } else if snap_key(KeyCode::Digit2) {
            if let Some(pos) = find_cell_position(runtime, ModuleArchetype::Shaft) {
                state.snap_coord = Some(pos);
                if state.mode == CameraMode::TopDownRoom {
                    state.translation = Vec3::new(pos.x, pos.y + 35.0, pos.z);
                } else if state.mode == CameraMode::FreeFly {
                    state.translation = pos + Vec3::new(0.0, 15.0, 10.0);
                    state.yaw = std::f32::consts::PI;
                    state.pitch = -0.6;
                }
                state.last_status = "Teleported focus to WELLSHAFT tower.".to_string();
            } else {
                state.last_status =
                    "No Wellshaft (Shaft) found on this seed WFC layout.".to_string();
            }
        } else if snap_key(KeyCode::Digit3) {
            if let Some(pos) = find_cell_position(runtime, ModuleArchetype::Gantry) {
                state.snap_coord = Some(pos);
                if state.mode == CameraMode::TopDownRoom {
                    state.translation = Vec3::new(pos.x, pos.y + 35.0, pos.z);
                } else if state.mode == CameraMode::FreeFly {
                    state.translation = pos + Vec3::new(0.0, 6.0, -15.0);
                    state.yaw = 0.0;
                    state.pitch = 0.0;
                }
                state.last_status = "Teleported focus to GANTRY course starting point.".to_string();
            } else {
                state.last_status = "No Gantry found on this seed WFC layout.".to_string();
            }
        } else if snap_key(KeyCode::Digit4) {
            if let Some(pos) = find_cell_position(runtime, ModuleArchetype::Climb) {
                state.snap_coord = Some(pos);
                if state.mode == CameraMode::TopDownRoom {
                    state.translation = Vec3::new(pos.x, pos.y + 35.0, pos.z);
                } else if state.mode == CameraMode::FreeFly {
                    state.translation = pos + Vec3::new(0.0, 1.0, 8.0);
                    state.yaw = std::f32::consts::PI;
                    state.pitch = 0.3;
                }
                state.last_status = "Teleported focus to CLIMB shaft.".to_string();
            } else {
                state.last_status = "No Climb shaft found on this seed WFC layout.".to_string();
            }
        } else if snap_key(KeyCode::Digit5) {
            if let Some(pos) = find_room_position(runtime, RoomRole::Keystone) {
                state.snap_coord = Some(pos);
                if state.mode == CameraMode::TopDownRoom {
                    state.translation = Vec3::new(pos.x, pos.y + 35.0, pos.z);
                } else if state.mode == CameraMode::FreeFly {
                    state.translation = pos + Vec3::new(0.0, 2.0, 12.0);
                    state.yaw = std::f32::consts::PI;
                    state.pitch = -0.1;
                }
                state.last_status = "Teleported focus to KEYSTONE chamber.".to_string();
            } else {
                state.last_status = "No Keystone room found.".to_string();
            }
        } else if snap_key(KeyCode::Digit6) {
            if let Some(pos) = find_room_position(runtime, RoomRole::Exit) {
                state.snap_coord = Some(pos);
                if state.mode == CameraMode::TopDownRoom {
                    state.translation = Vec3::new(pos.x, pos.y + 35.0, pos.z);
                } else if state.mode == CameraMode::FreeFly {
                    state.translation = pos + Vec3::new(0.0, 2.0, 12.0);
                    state.yaw = std::f32::consts::PI;
                    state.pitch = -0.1;
                }
                state.last_status = "Teleported focus to EXIT portal.".to_string();
            } else {
                state.last_status = "No Exit room found.".to_string();
            }
        }
    }

    // 3. Camera Mode & Glow switching
    if snap_key(KeyCode::Tab) || snap_key(KeyCode::KeyF) {
        state.mode = match state.mode {
            CameraMode::FreeFly => CameraMode::TopDownMap,
            CameraMode::TopDownMap | CameraMode::TopDownRoom => CameraMode::FreeFly,
        };
        if state.mode == CameraMode::TopDownMap {
            let config = runtime.as_ref().map(|r| r.match_state.facility.config);
            if let Some(c) = config {
                let cx = (c.cols as f32 - 1.0) * 0.5 * observed_match::full_wfc::CELL_SIZE;
                let cz = (c.rows as f32 - 1.0) * 0.5 * observed_match::full_wfc::CELL_SIZE;
                state.translation = Vec3::new(cx, 110.0, cz);
            }
        }
        state.last_status = format!("Switched to camera mode: {:?}", state.mode);
    } else if snap_key(KeyCode::Digit0) || snap_key(KeyCode::KeyM) {
        state.mode = CameraMode::TopDownMap;
        if let Some(runtime) = runtime.as_ref() {
            let cols = runtime.match_state.facility.config.cols;
            let rows = runtime.match_state.facility.config.rows;
            let cx = (cols as f32 - 1.0) * 0.5 * observed_match::full_wfc::CELL_SIZE;
            let cz = (rows as f32 - 1.0) * 0.5 * observed_match::full_wfc::CELL_SIZE;
            state.translation = Vec3::new(cx, 110.0, cz);
        }
        state.last_status = "Switched to Top-Down Map overview.".to_string();
    } else if snap_key(KeyCode::KeyT) {
        if let Some(target) = state.snap_coord {
            state.mode = CameraMode::TopDownRoom;
            state.translation = Vec3::new(target.x, target.y + 35.0, target.z);
            state.last_status = "Switched to Top-Down Room view.".to_string();
        } else {
            state.last_status = "Cannot snap: no room focused. Press [1-6] first.".to_string();
        }
    } else if snap_key(KeyCode::KeyG) {
        state.glow_enabled = !state.glow_enabled;
        state.last_status = format!(
            "Edge glow toggled to: {}",
            if state.glow_enabled {
                "ENABLED"
            } else {
                "DISABLED"
            }
        );
    }

    // 4. Keyboard Look / Mouse Rotation Controls (FreeFly mode only)
    if state.mode == CameraMode::FreeFly {
        let look_speed = 1.8;
        let look_key = |neg: KeyCode, pos: KeyCode| {
            (keyboard.pressed(pos) as i32 - keyboard.pressed(neg) as i32) as f32
        };
        state.yaw += look_key(KeyCode::ArrowLeft, KeyCode::ArrowRight) * dt * look_speed;
        state.pitch = (state.pitch
            + look_key(KeyCode::ArrowDown, KeyCode::ArrowUp) * dt * look_speed * 0.8)
            .clamp(-1.48, 1.48);

        if mouse_buttons.pressed(MouseButton::Right) {
            let delta = mouse.map(|m| m.delta).unwrap_or(Vec2::ZERO);
            state.yaw -= delta.x * 0.003;
            state.pitch = (state.pitch - delta.y * 0.003).clamp(-1.48, 1.48);
        }
    }

    let (sy, cy) = state.yaw.sin_cos();
    let (sp, cp) = state.pitch.sin_cos();
    let look_dir = Vec3::new(sy * cp, sp, -cy * cp).normalize_or(Vec3::NEG_Z);

    // 5. Fly & Pan Movement controls
    let speed = if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
        state.speed * 4.0
    } else {
        state.speed
    };

    let axis = |neg: KeyCode, pos: KeyCode| {
        (keyboard.pressed(pos) as i32 - keyboard.pressed(neg) as i32) as f32
    };

    let mut move_dir = Vec3::ZERO;
    match state.mode {
        CameraMode::FreeFly => {
            let forward = Vec3::new(look_dir.x, 0.0, look_dir.z).normalize_or(Vec3::NEG_Z);
            let right = forward.cross(Vec3::Y).normalize_or(Vec3::X);
            move_dir = right * axis(KeyCode::KeyA, KeyCode::KeyD)
                + forward * axis(KeyCode::KeyS, KeyCode::KeyW);

            let vertical = (keyboard.pressed(KeyCode::Space) as i32
                + keyboard.pressed(KeyCode::KeyE) as i32
                - keyboard.pressed(KeyCode::ControlLeft) as i32
                - keyboard.pressed(KeyCode::ControlRight) as i32
                - keyboard.pressed(KeyCode::KeyQ) as i32) as f32;
            move_dir += Vec3::Y * vertical;
        }
        CameraMode::TopDownMap | CameraMode::TopDownRoom => {
            // Scroll on XZ plane: A/D scroll X, W/S scroll Z
            move_dir.x += axis(KeyCode::KeyA, KeyCode::KeyD);
            move_dir.z -= axis(KeyCode::KeyW, KeyCode::KeyS);

            // Vertical controls perform zoom
            let vertical = (keyboard.pressed(KeyCode::Space) as i32
                + keyboard.pressed(KeyCode::KeyE) as i32
                - keyboard.pressed(KeyCode::ControlLeft) as i32
                - keyboard.pressed(KeyCode::ControlRight) as i32
                - keyboard.pressed(KeyCode::KeyQ) as i32) as f32;
            move_dir += Vec3::Y * vertical;
        }
    }

    if move_dir.length_squared() > 0.0 {
        state.translation += move_dir.normalize() * speed * dt;
    }

    // 6. Apply final camera transformation based on mode
    match state.mode {
        CameraMode::FreeFly => {
            *transform =
                Transform::from_translation(state.translation).looking_to(look_dir, Vec3::Y);
        }
        CameraMode::TopDownMap | CameraMode::TopDownRoom => {
            // Look straight down (NEG_Y), with screen-up oriented toward negative Z
            *transform =
                Transform::from_translation(state.translation).looking_to(Vec3::NEG_Y, Vec3::NEG_Z);
        }
    }

    // 7. Screenshot Capture controls
    if keyboard.just_pressed(KeyCode::KeyC) || keyboard.just_pressed(KeyCode::Enter) {
        let dir = PathBuf::from("docs/evidence/map_observer");
        let _ = std::fs::create_dir_all(&dir);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("map_inspect_{}.png", timestamp));
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()));
        state.last_status = format!("Screenshot saved to {:?}", path);
    }
}

fn draw_debug_grid_gizmos(
    state: Res<ObserverState>,
    runtime: Option<Res<FullWfcRuntime>>,
    mut gizmos: Gizmos,
) {
    let Some(runtime) = runtime.as_ref() else {
        return;
    };
    if !state.glow_enabled {
        return;
    }

    for piece in &runtime.match_state.geometry.pieces {
        match piece.role {
            StructureRole::Wall => {
                // Neon cyan edge glow for wall outlines
                draw_wireframe_box(
                    &mut gizmos,
                    piece.center,
                    piece.half,
                    Color::srgb(0.0, 0.8, 1.0),
                );
            }
            StructureRole::Feature => {
                // Neon magenta/pink for features/machinery
                draw_wireframe_box(
                    &mut gizmos,
                    piece.center,
                    piece.half,
                    Color::srgb(1.0, 0.0, 0.8),
                );
            }
            StructureRole::Floor => {
                // Dim green outline establishing floors/walkways
                draw_wireframe_box(
                    &mut gizmos,
                    piece.center,
                    piece.half,
                    Color::srgb(0.1, 0.35, 0.1),
                );
            }
            _ => {}
        }
    }
}

fn draw_wireframe_box(gizmos: &mut Gizmos, center: Vec3, half: Vec3, color: Color) {
    let x = half.x;
    let y = half.y;
    let z = half.z;

    // The 8 corners
    let c000 = center + Vec3::new(-x, -y, -z);
    let c100 = center + Vec3::new(x, -y, -z);
    let c010 = center + Vec3::new(-x, y, -z);
    let c110 = center + Vec3::new(x, y, -z);
    let c001 = center + Vec3::new(-x, -y, z);
    let c101 = center + Vec3::new(x, -y, z);
    let c011 = center + Vec3::new(-x, y, z);
    let c111 = center + Vec3::new(x, y, z);

    // 12 edges
    // Bottom face
    gizmos.line(c000, c100, color);
    gizmos.line(c100, c101, color);
    gizmos.line(c101, c001, color);
    gizmos.line(c001, c000, color);

    // Top face
    gizmos.line(c010, c110, color);
    gizmos.line(c110, c111, color);
    gizmos.line(c111, c011, color);
    gizmos.line(c011, c010, color);

    // Pillars
    gizmos.line(c000, c010, color);
    gizmos.line(c100, c110, color);
    gizmos.line(c101, c111, color);
    gizmos.line(c001, c011, color);
}

fn find_cell_position(runtime: &FullWfcRuntime, archetype: ModuleArchetype) -> Option<Vec3> {
    for placement in runtime.match_state.facility.placements.values() {
        if placement.archetype == archetype {
            return Some(Vec3::new(
                placement.coord.x as f32 * observed_match::full_wfc::CELL_SIZE,
                placement.coord.level as f32 * observed_match::full_wfc::LEVEL_HEIGHT,
                placement.coord.z as f32 * observed_match::full_wfc::CELL_SIZE,
            ));
        }
    }
    None
}

fn find_room_position(runtime: &FullWfcRuntime, role: RoomRole) -> Option<Vec3> {
    for room in runtime.match_state.facility.rooms.values() {
        if room.role == role {
            return Some(Vec3::new(
                room.coord.x as f32 * observed_match::full_wfc::CELL_SIZE,
                room.coord.level as f32 * observed_match::full_wfc::LEVEL_HEIGHT,
                room.coord.z as f32 * observed_match::full_wfc::CELL_SIZE,
            ));
        }
    }
    None
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.008, 0.012, 0.022)))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .parent()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .join("assets")
                        .to_string_lossy()
                        .into_owned(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Observed 2 — 3D Map Observer Lab".to_string(),
                        resolution: WindowResolution::new(1440, 900),
                        present_mode: PresentMode::AutoVsync,
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(observed_game::ObservedGamePlugin)
        .add_plugins(MapObserverLabPlugin)
        .run();
}
