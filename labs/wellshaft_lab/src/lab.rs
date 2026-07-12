//! Interactive first-person showcase for the wellshaft prototype, driven by the
//! **production** `observed_traversal` controller so the traversal feel and the
//! stair-collision transfer directly to the game. Walk around the hexagonal
//! pillar: every landing has an outward threshold bridge, and visible tread
//! heights are the controller's real collision heights.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use observed_traversal::{FpsArena, FpsBody, FpsConfig, step_body};
use player_input::PlayerIntent;

use crate::shaft::{self, Part};

const MOUSE_SENS: f32 = 0.06;
const FIXED_DT: f32 = 1.0 / 60.0;

#[derive(Component)]
pub struct ShaftCam;

#[derive(Component)]
pub struct ShaftGeometry;

#[derive(Component)]
pub struct ShaftUiRoot;

#[derive(Component)]
pub struct DebugText;

#[derive(Resource)]
pub struct ShaftRuntime {
    pub body: FpsBody,
    pub arena: FpsArena,
    pub config: FpsConfig,
    pub intent: PlayerIntent,
    pub min_feet: f32,
    pub reset_requested: bool,
    pub reset_count: u32,
}

fn spawn_body(config: &FpsConfig) -> FpsBody {
    let xz = shaft::spawn_xz();
    let next = shaft::landing_center(shaft::LEVELS - 2);
    let facing = next - xz;
    let yaw = facing.x.atan2(-facing.y);
    FpsBody::spawned(
        Vec3::new(xz.x, shaft::spawn_height() + config.half_height, xz.y),
        yaw,
    )
}

impl Default for ShaftRuntime {
    fn default() -> Self {
        let config = FpsConfig::default();
        let parts = shaft::build();
        let arena = FpsArena {
            solids: shaft::solids(&parts),
            floor_y: 0.0,
            floor_half: shaft::HALF,
        };
        let body = spawn_body(&config);
        Self {
            min_feet: body.position.y - config.half_height,
            body,
            arena,
            config,
            intent: PlayerIntent::default(),
            reset_requested: false,
            reset_count: 0,
        }
    }
}

pub fn setup_lab(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Silo register: near-black ambient, warm practicals do the lighting.
    commands.spawn((
        ShaftCam,
        Camera3d::default(),
        Hdr,
        DistanceFog {
            color: Color::srgb(0.006, 0.006, 0.008),
            falloff: FogFalloff::Linear {
                start: 16.0,
                end: 46.0,
            },
            ..default()
        },
        Transform::from_xyz(0.0, 2.0, 0.0),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.30, 0.34, 0.42),
        brightness: 12.0,
        ..default()
    });

    let concrete = materials.add(StandardMaterial {
        base_color: Color::srgb(0.19, 0.185, 0.18),
        perceptual_roughness: 0.95,
        ..default()
    });
    let step_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 0.195, 0.185),
        emissive: LinearRgba::rgb(0.11, 0.045, 0.012),
        perceptual_roughness: 0.9,
        ..default()
    });
    let door_mat = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(0.1, 1.6, 2.2),
        ..default()
    });
    let lamp_mat = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(6.0, 3.2, 1.1),
        ..default()
    });
    // Guard rails read as a faint warm-lit edge line so the walk-off boundary is
    // legible in the buried dark without competing with the practicals.
    let guard_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.15, 0.14),
        emissive: LinearRgba::rgb(0.22, 0.12, 0.05),
        perceptual_roughness: 0.85,
        ..default()
    });

    let parts = shaft::build();
    for p in &parts {
        let mesh = if p.part == Part::Pillar {
            meshes.add(
                Cylinder::new(shaft::PILLAR_RADIUS, p.half.y * 2.0)
                    .mesh()
                    .resolution(6),
            )
        } else {
            meshes.add(Cuboid::new(p.half.x * 2.0, p.half.y * 2.0, p.half.z * 2.0))
        };
        let material = match p.part {
            Part::Floor | Part::Pillar => concrete.clone(),
            Part::Landing(_) | Part::Bridge(_) | Part::Step => step_mat.clone(),
            Part::Guard => guard_mat.clone(),
            Part::Doorway(_) => door_mat.clone(),
            Part::Lamp(_) => lamp_mat.clone(),
        };
        commands.spawn((
            ShaftGeometry,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(p.center).with_rotation(Quat::from_rotation_y(p.yaw)),
        ));
        // A warm point light at each practical.
        if let Part::Lamp(_) = p.part {
            commands.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.6, 0.28),
                    intensity: 170_000.0,
                    range: 6.5,
                    shadows_enabled: matches!(p.part, Part::Lamp(level) if level + 1 == shaft::LEVELS),
                    ..default()
                },
                Transform::from_translation(p.center),
            ));
        }
    }

    commands.insert_resource(ShaftRuntime::default());
    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands.spawn((
        ShaftUiRoot,
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
            Text::new("Wellshaft"),
            TextFont {
                font_size: 15.0,
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
    mut runtime: ResMut<ShaftRuntime>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
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

pub fn simulate(mut runtime: ResMut<ShaftRuntime>) {
    if runtime.reset_requested {
        return;
    }
    let intent = runtime.intent;
    let arena = runtime.arena.clone();
    let config = runtime.config;
    step_body(&mut runtime.body, intent, &arena, &config, FIXED_DT);
    let feet = runtime.body.position.y - runtime.config.half_height;
    runtime.min_feet = runtime.min_feet.min(feet);
}

pub fn perform_reset(mut runtime: ResMut<ShaftRuntime>) {
    if !runtime.reset_requested {
        return;
    }
    let config = runtime.config;
    runtime.body = spawn_body(&config);
    runtime.min_feet = runtime.body.position.y - config.half_height;
    runtime.reset_requested = false;
    runtime.reset_count += 1;
}

pub(crate) fn present_camera(
    runtime: Res<ShaftRuntime>,
    capture: Option<Res<crate::CaptureRun>>,
    mut camera: Query<&mut Transform, With<ShaftCam>>,
) {
    if capture.is_some() {
        return;
    }
    if let Ok(mut transform) = camera.single_mut() {
        *transform = Transform::from_translation(runtime.body.eye(&runtime.config))
            .looking_to(runtime.body.look_dir(), Vec3::Y);
    }
}

pub fn update_debug_text(runtime: Res<ShaftRuntime>, mut query: Query<&mut Text, With<DebugText>>) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    let feet = runtime.body.position.y - runtime.config.half_height;
    let reached_bottom = runtime.min_feet <= 0.2;
    **text = format!(
        "HEX WELLSHAFT PROTOTYPE  {}\n\
         feet height   {:.2}\n\
         lowest reached {:.2} / floor 0.00\n\
         grounded      {}\n\
         landings {}  bridges {}  directions 6\n\
         resets        {}\n\n\
         LEGEND cyan = threshold | amber = practical\n\
         WASD + mouse | Shift sprint | Space jump | R reset",
        if reached_bottom {
            "[DESCENDED]"
        } else {
            "[ ... ]"
        },
        feet,
        runtime.min_feet,
        runtime.body.grounded,
        shaft::LEVELS,
        shaft::LEVELS,
        runtime.reset_count,
    );
}
