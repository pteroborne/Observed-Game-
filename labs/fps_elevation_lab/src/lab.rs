//! First-person showcase for the elevation controller: walk the test course —
//! a staircase onto a raised platform, plus a too-tall block — and watch the feet
//! height / grounded state and a `[PASS]` health line that lights once you've climbed.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use fps_controller_lab::controller::{FIXED_DT, FpsBody, FpsConfig};
use player_input::PlayerIntent;

use crate::climb::{ElevArena, STEP_COUNT, step_body_elev};

const MOUSE_SENS: f32 = 0.06;

#[derive(Component)]
pub struct ElevCam;

#[derive(Component)]
pub struct ElevUiRoot;

#[derive(Component)]
pub(crate) struct DebugText;

#[derive(Resource)]
pub struct ElevRuntime {
    pub body: FpsBody,
    pub arena: ElevArena,
    pub config: FpsConfig,
    pub intent: PlayerIntent,
    pub max_feet: f32,
    pub reset_count: u32,
    pub reset_requested: bool,
    /// Capture/demo: drive the body forward automatically.
    pub auto_walk: bool,
}

impl Default for ElevRuntime {
    fn default() -> Self {
        let config = FpsConfig::default();
        Self {
            body: spawn_body(&config),
            arena: ElevArena::authored(),
            config,
            intent: PlayerIntent::default(),
            max_feet: 0.0,
            reset_count: 0,
            reset_requested: false,
            auto_walk: false,
        }
    }
}

fn spawn_body(config: &FpsConfig) -> FpsBody {
    // Stand on the floor before the stairs, facing +X (toward them).
    FpsBody::spawned(
        Vec3::new(0.0, config.half_height, 0.0),
        std::f32::consts::FRAC_PI_2,
    )
}

pub fn setup_lab(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        ElevCam,
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.0, 0.0),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 7_500.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.6, 0.0)),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.7, 0.74, 0.85),
        brightness: 700.0,
        ..default()
    });

    let runtime = ElevRuntime::default();

    // Floor.
    let floor_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.25, 0.30),
        perceptual_roughness: 0.95,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(
            runtime.arena.floor_half * 2.0,
            runtime.arena.floor_half * 2.0,
        ))),
        MeshMaterial3d(floor_material),
        Transform::from_xyz(0.0, runtime.arena.floor_y, 0.0),
    ));

    // Course solids, coloured by role: perimeter walls, stairs, platform, tall block.
    let wall_mat = materials.add(StandardMaterial::from_color(Color::srgb(0.34, 0.38, 0.46)));
    let step_mat = materials.add(StandardMaterial::from_color(Color::srgb(0.40, 0.62, 0.95)));
    let platform_mat = materials.add(StandardMaterial::from_color(Color::srgb(0.36, 0.85, 0.55)));
    let block_mat = materials.add(StandardMaterial::from_color(Color::srgb(0.9, 0.32, 0.3)));
    for (index, solid) in runtime.arena.solids.iter().enumerate() {
        let size = solid.max - solid.min;
        let centre = (solid.min + solid.max) * 0.5;
        let material = if index < 4 {
            wall_mat.clone()
        } else if index < 4 + STEP_COUNT {
            step_mat.clone()
        } else if index < 4 + STEP_COUNT + 1 {
            platform_mat.clone()
        } else {
            block_mat.clone()
        };
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
            MeshMaterial3d(material),
            Transform::from_translation(centre),
        ));
    }

    commands.insert_resource(runtime);
    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands.spawn((
        ElevUiRoot,
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.04, 0.07, 0.86)),
        children![(
            DebugText,
            Text::new("Elevation"),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.95, 1.0)),
        )],
    ));
}

pub fn grab_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor) = cursors.single_mut() {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

pub fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Option<Res<AccumulatedMouseMotion>>,
    mut runtime: ResMut<ElevRuntime>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
    if runtime.auto_walk {
        runtime.intent = PlayerIntent {
            movement: Vec2::new(0.0, 1.0),
            ..default()
        };
        return;
    }
    let axis = |negative: KeyCode, positive: KeyCode| {
        (keyboard.pressed(positive) as i32 - keyboard.pressed(negative) as i32) as f32
    };
    let delta = mouse.map(|m| m.delta).unwrap_or(Vec2::ZERO);
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
            axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) + delta.x * MOUSE_SENS,
            axis(KeyCode::ArrowUp, KeyCode::ArrowDown) + delta.y * MOUSE_SENS,
        ),
        jump_pressed: keyboard.pressed(KeyCode::Space),
        sprint_held: keyboard.pressed(KeyCode::ShiftLeft),
        ..default()
    };
}

pub fn simulate(mut runtime: ResMut<ElevRuntime>) {
    if runtime.reset_requested {
        return;
    }
    let intent = runtime.intent;
    let arena = runtime.arena.clone();
    let config = runtime.config;
    step_body_elev(&mut runtime.body, intent, &arena, &config, FIXED_DT);
    let feet = runtime.body.position.y - runtime.config.half_height;
    runtime.max_feet = runtime.max_feet.max(feet);
}

pub fn perform_reset(mut runtime: ResMut<ElevRuntime>) {
    if !runtime.reset_requested {
        return;
    }
    let resets = runtime.reset_count + 1;
    let config = runtime.config;
    runtime.body = spawn_body(&config);
    runtime.max_feet = 0.0;
    runtime.reset_requested = false;
    runtime.reset_count = resets;
}

pub fn present_camera(runtime: Res<ElevRuntime>, mut camera: Query<&mut Transform, With<ElevCam>>) {
    if let Ok(mut transform) = camera.single_mut() {
        *transform = Transform::from_translation(runtime.body.eye(&runtime.config))
            .looking_to(runtime.body.look_dir(), Vec3::Y);
    }
}

pub fn update_debug_text(runtime: Res<ElevRuntime>, mut query: Query<&mut Text, With<DebugText>>) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    let feet = runtime.body.position.y - runtime.config.half_height;
    let platform_top = runtime.arena.platform_top();
    let climbed = runtime.max_feet >= platform_top - 0.05;
    **text = format!(
        "ELEVATION CONTROLLER  {}\n\
         feet height   {:.2}\n\
         max reached   {:.2} / platform {:.2}\n\
         grounded      {}\n\
         resets        {}\n\n\
         WASD + mouse move | Shift sprint | Space jump | R reset",
        if climbed { "[PASS]" } else { "[ ... ]" },
        feet,
        runtime.max_feet,
        platform_top,
        runtime.body.grounded,
        runtime.reset_count,
    );
}
