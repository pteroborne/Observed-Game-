//! A resettable, raw-Rapier proof scene for the promoted kinematic capsule
//! controller. The visible cuboids and the collision scene are built from the same
//! `StructuralCollider` list; `R` replaces the entire physics scene without respawning
//! presentation entities.
//!
//! Controls: `WASD` move, `Q`/`E` turn, `Space` jump, `Shift` sprint, `R` reset.

use bevy::{
    prelude::*,
    window::{PresentMode, WindowResolution},
};
use observed_traversal::{
    FpsBody, FpsConfig,
    rapier_controller::{RapierTraversalScene, StructuralCollider, step_character},
};
use player_input::PlayerIntent;

const FLOOR_HALF: f32 = 12.0;

#[derive(Component)]
struct PlayerCamera;

#[derive(Component)]
struct PlayerVisual;

#[derive(Component)]
struct LabHud;

#[derive(Resource)]
struct Stage {
    scene: RapierTraversalScene,
    body: FpsBody,
    config: FpsConfig,
    reset_count: u32,
}

impl Stage {
    fn fresh(reset_count: u32) -> Self {
        let config = FpsConfig::default();
        Self {
            scene: RapierTraversalScene::from_primitives(0.0, FLOOR_HALF, &structure()),
            body: FpsBody::spawned(Vec3::new(-7.0, config.half_height, 7.0), 0.0),
            config,
            reset_count,
        }
    }

    fn reset(&mut self) {
        *self = Self::fresh(self.reset_count + 1);
    }
}

impl Default for Stage {
    fn default() -> Self {
        Self::fresh(0)
    }
}

#[derive(Resource, Default)]
struct LabIntent(PlayerIntent);

pub struct RapierControllerLabPlugin;

impl Plugin for RapierControllerLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(60.0))
            .init_resource::<Stage>()
            .init_resource::<LabIntent>()
            .add_systems(Startup, setup)
            .add_systems(FixedUpdate, simulate)
            .add_systems(
                Update,
                (sample_intent, reset, sync_view, update_hud).chain(),
            );
    }
}

pub fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.012, 0.018, 0.03)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Raw Rapier Controller Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(RapierControllerLabPlugin)
        .run();
}

/// The authored structural source. It contains perimeter walls, a yawed wall, and a
/// low platform to demonstrate the controller's collision, rotation, and autostep.
fn structure() -> Vec<StructuralCollider> {
    vec![
        StructuralCollider::axis_aligned(
            Vec3::new(0.0, 2.0, -FLOOR_HALF),
            Vec3::new(FLOOR_HALF, 2.0, 0.35),
        ),
        StructuralCollider::axis_aligned(
            Vec3::new(0.0, 2.0, FLOOR_HALF),
            Vec3::new(FLOOR_HALF, 2.0, 0.35),
        ),
        StructuralCollider::axis_aligned(
            Vec3::new(-FLOOR_HALF, 2.0, 0.0),
            Vec3::new(0.35, 2.0, FLOOR_HALF),
        ),
        StructuralCollider::axis_aligned(
            Vec3::new(FLOOR_HALF, 2.0, 0.0),
            Vec3::new(0.35, 2.0, FLOOR_HALF),
        ),
        StructuralCollider {
            center: Vec3::new(-0.5, 1.6, 0.5),
            half: Vec3::new(5.2, 1.6, 0.3),
            yaw: 0.62,
        },
        StructuralCollider::axis_aligned(Vec3::new(5.0, 0.28, -4.5), Vec3::new(1.8, 0.28, 1.8)),
    ]
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stage: Res<Stage>,
) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.35, 0.45, 0.7),
        brightness: 250.0,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            illuminance: 7_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(7.0, 11.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let floor_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.025, 0.045, 0.075),
        perceptual_roughness: 0.94,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(FLOOR_HALF * 2.0, 0.2, FLOOR_HALF * 2.0))),
        MeshMaterial3d(floor_material),
        Transform::from_xyz(0.0, -0.1, 0.0),
        Name::new("Rapier lab floor"),
    ));

    let wall_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.22, 0.34),
        emissive: LinearRgba::rgb(0.0, 0.18, 0.32),
        perceptual_roughness: 0.7,
        ..default()
    });
    for collider in structure() {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(
                collider.half.x * 2.0,
                collider.half.y * 2.0,
                collider.half.z * 2.0,
            ))),
            MeshMaterial3d(wall_material.clone()),
            Transform::from_translation(collider.center)
                .with_rotation(Quat::from_rotation_y(collider.yaw)),
            Name::new("Rapier structural collider"),
        ));
    }

    let player_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.62, 0.12),
        emissive: LinearRgba::rgb(0.45, 0.12, 0.0),
        ..default()
    });
    commands.spawn((
        PlayerVisual,
        Mesh3d(meshes.add(Capsule3d::new(
            stage.config.radius,
            (stage.config.half_height - stage.config.radius) * 2.0,
        ))),
        MeshMaterial3d(player_material),
        Transform::from_translation(stage.body.position),
        Name::new("Rapier capsule"),
    ));
    commands.spawn((
        PlayerCamera,
        Camera3d::default(),
        Transform::from_xyz(-7.0, 1.6, 7.0).looking_to(Vec3::NEG_Z, Vec3::Y),
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.88)),
        children![(
            LabHud,
            Text::new("Rapier controller lab"),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::srgb(0.82, 0.94, 1.0)),
        )],
    ));
}

/// Input only produces the shared abstract intent. The fixed controller never reads a
/// keyboard, so this exact stage can be driven by a bot or a deterministic tape.
fn sample_intent(keys: Res<ButtonInput<KeyCode>>, mut intent: ResMut<LabIntent>) {
    let movement = Vec2::new(
        (keys.pressed(KeyCode::KeyD) as i8 - keys.pressed(KeyCode::KeyA) as i8) as f32,
        (keys.pressed(KeyCode::KeyW) as i8 - keys.pressed(KeyCode::KeyS) as i8) as f32,
    );
    intent.0 = PlayerIntent {
        movement,
        look: Vec2::new(
            (keys.pressed(KeyCode::KeyE) as i8 - keys.pressed(KeyCode::KeyQ) as i8) as f32 * 4.0,
            0.0,
        ),
        jump_pressed: keys.just_pressed(KeyCode::Space),
        sprint_held: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
        ..default()
    };
}

fn simulate(mut stage: ResMut<Stage>, intent: Res<LabIntent>) {
    let Stage {
        scene,
        body,
        config,
        ..
    } = stage.as_mut();
    step_character(scene, body, intent.0, config, 1.0 / 60.0);
}

fn reset(keys: Res<ButtonInput<KeyCode>>, mut stage: ResMut<Stage>) {
    if keys.just_pressed(KeyCode::KeyR) {
        stage.reset();
    }
}

fn sync_view(
    stage: Res<Stage>,
    mut player: Query<&mut Transform, (With<PlayerVisual>, Without<PlayerCamera>)>,
    mut camera: Query<&mut Transform, (With<PlayerCamera>, Without<PlayerVisual>)>,
) {
    if let Ok(mut transform) = player.single_mut() {
        transform.translation = stage.body.position;
        transform.rotation = Quat::from_rotation_y(stage.body.yaw);
    }
    if let Ok(mut transform) = camera.single_mut() {
        transform.translation = stage.body.position + Vec3::Y * 0.55;
        transform.look_to(stage.body.forward(), Vec3::Y);
    }
}

fn update_hud(stage: Res<Stage>, mut hud: Query<&mut Text, With<LabHud>>) {
    let Ok(mut text) = hud.single_mut() else {
        return;
    };
    let body = &stage.body;
    **text = format!(
        "RAPIER KINEMATIC CAPSULE  |  reset {}\n\
         WASD move  Q/E turn  Space jump  Shift sprint  R reset\n\
         position ({:.2}, {:.2}, {:.2})  grounded={}\n\
         structural colliders {} (four bounds, one yawed wall, one low autostep platform)",
        stage.reset_count,
        body.position.x,
        body.position.y,
        body.position.z,
        body.grounded,
        stage.scene.collider_count() - 1,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_rebuilds_the_raw_rapier_scene_at_the_spawn() {
        let mut stage = Stage::default();
        stage.body.position.x += 5.0;
        stage.reset();

        assert_eq!(stage.reset_count, 1);
        assert_eq!(
            stage.body.position,
            Vec3::new(-7.0, stage.config.half_height, 7.0)
        );
        assert_eq!(stage.scene.collider_count(), structure().len() + 1);
    }

    #[test]
    fn scripted_intents_are_bit_identical_after_a_reset() {
        let mut a = Stage::default();
        let mut b = Stage::default();
        for tick in 0..300 {
            let intent = PlayerIntent {
                movement: Vec2::new(if tick % 90 < 45 { 0.4 } else { -0.4 }, 1.0),
                jump_pressed: tick == 100,
                sprint_held: tick > 180,
                ..default()
            };
            step_character(&a.scene, &mut a.body, intent, &a.config, 1.0 / 60.0);
            step_character(&b.scene, &mut b.body, intent, &b.config, 1.0 / 60.0);
        }
        assert_eq!(a.body, b.body);
    }
}
