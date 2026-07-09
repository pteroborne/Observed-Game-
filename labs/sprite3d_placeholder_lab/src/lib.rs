//! sprite3d_placeholder_lab -- prove CC0 2.5D placeholder sprites in a 3D Bevy scene.
//!
//! The lab uses `bevy_sprite3d` only as a presentation layer: image handles are loaded
//! from the shared `observed_assets` slots, sprites spawn only once their `Image` is
//! available, and missing/not-yet-loaded images fall back to procedural meshes.

use std::path::PathBuf;

use bevy::{
    app::AppExit,
    asset::AssetPlugin,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};
use bevy_sprite3d::prelude::{Sprite3d, Sprite3dPlugin};
use observed_style::{self as style, MarkerRole};

const ACTOR_PIXELS_PER_METRE: f32 = 64.0;
const DEVICE_PIXELS_PER_METRE: f32 = 80.0;
const ROLE_COUNT: usize = 4;

#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub struct PlaceholderVisual {
    pub role: PlaceholderRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaceholderRole {
    Runner,
    Rival,
    Guardian,
    ControlDevice,
}

impl PlaceholderRole {
    const ALL: [Self; ROLE_COUNT] = [
        Self::Runner,
        Self::Rival,
        Self::Guardian,
        Self::ControlDevice,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Runner => "Runner",
            Self::Rival => "Rival",
            Self::Guardian => "Guardian",
            Self::ControlDevice => "Control device",
        }
    }

    fn marker_role(self) -> MarkerRole {
        match self {
            Self::Runner => MarkerRole::You,
            Self::Rival => MarkerRole::Rival,
            Self::Guardian => MarkerRole::Director,
            Self::ControlDevice => MarkerRole::Control,
        }
    }

    fn x(self) -> f32 {
        match self {
            Self::Runner => -4.5,
            Self::Rival => -1.5,
            Self::Guardian => 1.5,
            Self::ControlDevice => 4.5,
        }
    }
}

#[derive(Component)]
pub struct LabEntity;

#[derive(Component)]
pub struct SpriteFallback;

#[derive(Component)]
pub struct BillboardSprite;

#[derive(Resource)]
struct SpriteLabAssets {
    runner: Handle<Image>,
    rival: Handle<Image>,
    guardian: Handle<Image>,
    control_device: Handle<Image>,
}

impl SpriteLabAssets {
    fn load(asset_server: &AssetServer) -> Self {
        Self {
            runner: asset_server.load(observed_assets::RUNNER_STAND.path),
            rival: asset_server.load(observed_assets::RIVAL_STAND.path),
            guardian: asset_server.load(observed_assets::GUARDIAN_STAND.path),
            control_device: asset_server.load(observed_assets::CONTROL_DEVICE.path),
        }
    }

    fn handle(&self, role: PlaceholderRole) -> &Handle<Image> {
        match role {
            PlaceholderRole::Runner => &self.runner,
            PlaceholderRole::Rival => &self.rival,
            PlaceholderRole::Guardian => &self.guardian,
            PlaceholderRole::ControlDevice => &self.control_device,
        }
    }
}

#[derive(Resource)]
struct LabRenderAssets {
    cube: Handle<Mesh>,
    floor: Handle<Mesh>,
    fallback_materials: [Handle<StandardMaterial>; ROLE_COUNT],
    floor_material: Handle<StandardMaterial>,
}

#[derive(Default, Resource)]
struct ContentMode(Option<bool>);

#[derive(Resource)]
struct LabCaptureRequest {
    path: String,
    phase: u8,
    next_at: f32,
}

impl LabCaptureRequest {
    fn new(path: String) -> Self {
        Self {
            path,
            phase: 0,
            next_at: 0.0,
        }
    }
}

pub struct Sprite3dPlaceholderLabPlugin;

impl Plugin for Sprite3dPlaceholderLabPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Sprite3dPlugin)
            .init_resource::<ContentMode>()
            .add_systems(Startup, setup_lab)
            .add_systems(Update, (reset_lab, sync_lab_content).chain())
            .add_systems(Update, face_billboard_sprites);
    }
}

fn workspace_assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|labs| labs.parent())
        .expect("lab crate lives under workspace/labs")
        .join("assets")
}

fn setup_lab(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(SpriteLabAssets::load(&asset_server));

    let fallback_materials = PlaceholderRole::ALL.map(|role| {
        let treatment = style::marker(role.marker_role());
        materials.add(StandardMaterial {
            base_color: treatment.base_color,
            emissive: treatment.emissive,
            unlit: true,
            ..default()
        })
    });
    let floor_treatment = style::surface(style::SurfaceRole::Plain);
    commands.insert_resource(LabRenderAssets {
        cube: meshes.add(Cuboid::new(0.6, 1.6, 0.25)),
        floor: meshes.add(Plane3d::default().mesh().size(12.0, 8.0)),
        fallback_materials,
        floor_material: materials.add(StandardMaterial {
            base_color: floor_treatment.base_color,
            emissive: floor_treatment.emissive,
            ..default()
        }),
    });

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.5, 7.5).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
        Name::new("Sprite3D Lab Camera"),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 3_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.35, 0.0)),
        Name::new("Sprite3D Lab Sun"),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.55, 0.65, 0.85),
        brightness: 450.0,
        ..default()
    });
}

fn reset_lab(
    keyboard: Res<ButtonInput<KeyCode>>,
    lab_entities: Query<Entity, With<LabEntity>>,
    mut mode: ResMut<ContentMode>,
    mut commands: Commands,
) {
    if !keyboard.just_pressed(KeyCode::KeyR) {
        return;
    }
    for entity in &lab_entities {
        commands.entity(entity).despawn();
    }
    mode.0 = None;
}

fn sync_lab_content(
    sprites: Res<SpriteLabAssets>,
    images: Res<Assets<Image>>,
    render: Res<LabRenderAssets>,
    mut mode: ResMut<ContentMode>,
    existing: Query<Entity, With<LabEntity>>,
    mut commands: Commands,
) {
    let sprites_ready = placeholders_ready(&sprites, &images);
    if mode.0 == Some(sprites_ready) {
        return;
    }

    for entity in &existing {
        commands.entity(entity).despawn();
    }
    spawn_floor(&mut commands, &render);
    for (index, role) in PlaceholderRole::ALL.into_iter().enumerate() {
        if sprites_ready {
            spawn_sprite(&mut commands, &sprites, role);
        } else {
            spawn_fallback(&mut commands, &render, role, index);
        }
    }
    mode.0 = Some(sprites_ready);
}

fn spawn_floor(commands: &mut Commands, render: &LabRenderAssets) {
    commands.spawn((
        LabEntity,
        Mesh3d(render.floor.clone()),
        MeshMaterial3d(render.floor_material.clone()),
        Transform::from_xyz(0.0, -0.02, 0.0),
        Name::new("Sprite3D placeholder lab floor"),
    ));
}

fn spawn_sprite(commands: &mut Commands, sprites: &SpriteLabAssets, role: PlaceholderRole) {
    let treatment = style::marker(role.marker_role());
    let pixels_per_metre = if role == PlaceholderRole::ControlDevice {
        DEVICE_PIXELS_PER_METRE
    } else {
        ACTOR_PIXELS_PER_METRE
    };
    commands.spawn((
        LabEntity,
        PlaceholderVisual { role },
        BillboardSprite,
        Sprite {
            image: sprites.handle(role).clone(),
            ..default()
        },
        Sprite3d {
            pixels_per_metre,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            emissive: treatment.emissive,
            pivot: Some(Vec2::new(0.5, 0.0)),
            double_sided: true,
            ..default()
        },
        Transform::from_xyz(role.x(), 0.0, 0.0),
        Name::new(format!("{} sprite placeholder", role.label())),
    ));
}

fn spawn_fallback(
    commands: &mut Commands,
    render: &LabRenderAssets,
    role: PlaceholderRole,
    material_index: usize,
) {
    commands.spawn((
        LabEntity,
        PlaceholderVisual { role },
        SpriteFallback,
        Mesh3d(render.cube.clone()),
        MeshMaterial3d(render.fallback_materials[material_index].clone()),
        Transform::from_xyz(role.x(), 0.8, 0.0),
        Name::new(format!("{} procedural fallback", role.label())),
    ));
}

fn placeholders_ready(sprites: &SpriteLabAssets, images: &Assets<Image>) -> bool {
    PlaceholderRole::ALL
        .into_iter()
        .all(|role| sprite_ready(images, sprites.handle(role)))
}

pub fn sprite_ready(images: &Assets<Image>, image: &Handle<Image>) -> bool {
    images.get(image).is_some()
}

pub fn yaw_toward_camera(sprite: Vec3, camera: Vec3) -> Option<f32> {
    let to_camera = Vec2::new(camera.x - sprite.x, camera.z - sprite.z);
    (to_camera.length_squared() > 0.0001).then(|| to_camera.x.atan2(to_camera.y))
}

fn face_billboard_sprites(
    camera: Query<&GlobalTransform, (With<Camera3d>, Without<BillboardSprite>)>,
    mut sprites: Query<&mut Transform, With<BillboardSprite>>,
) {
    let Some(camera_transform) = camera.iter().next() else {
        return;
    };
    let camera_pos = camera_transform.translation();
    for mut transform in &mut sprites {
        if let Some(yaw) = yaw_toward_camera(transform.translation, camera_pos) {
            transform.rotation = Quat::from_rotation_y(yaw);
        }
    }
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<LabCaptureRequest>,
    visuals: Query<(), With<PlaceholderVisual>>,
    fallbacks: Query<(), With<SpriteFallback>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            let sprites_ready =
                visuals.iter().count() == ROLE_COUNT && fallbacks.iter().next().is_none();
            if sprites_ready || elapsed >= 2.0 {
                request.phase = 1;
                request.next_at = elapsed + 0.25;
            }
        }
        1 if elapsed >= request.next_at => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(request.path.clone()));
            request.phase = 2;
            request.next_at = elapsed + 0.8;
        }
        2 if elapsed >= request.next_at => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.02, 0.03, 0.05)))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: workspace_assets_dir().to_string_lossy().into_owned(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Sprite3D Placeholder Lab".to_string(),
                        resolution: WindowResolution::new(1280, 720),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(Sprite3dPlaceholderLabPlugin);
    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(LabCaptureRequest::new(path))
            .add_systems(Update, capture_progress.after(sync_lab_content));
    }
    app.run();
}

#[cfg(test)]
mod tests {
    use bevy::{asset::AssetPlugin, input::InputPlugin};

    use super::*;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: workspace_assets_dir().to_string_lossy().into_owned(),
                ..default()
            },
            InputPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Image>()
        .init_asset::<TextureAtlasLayout>()
        .add_plugins(Sprite3dPlaceholderLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn readiness_depends_on_loaded_images_not_just_handles() {
        let mut images = Assets::<Image>::default();
        let handle = Handle::<Image>::default();
        assert!(!sprite_ready(&images, &handle));

        let loaded = images.add(Image::default());
        assert!(sprite_ready(&images, &loaded));
    }

    #[test]
    fn yaw_faces_the_camera_on_the_xz_plane() {
        let yaw = yaw_toward_camera(Vec3::ZERO, Vec3::new(1.0, 5.0, 0.0)).unwrap();
        assert!((yaw - std::f32::consts::FRAC_PI_2).abs() < 0.0001);

        let yaw = yaw_toward_camera(Vec3::ZERO, Vec3::new(0.0, 5.0, -1.0)).unwrap();
        assert!((yaw - std::f32::consts::PI).abs() < 0.0001);

        assert!(yaw_toward_camera(Vec3::ZERO, Vec3::ZERO).is_none());
    }

    #[test]
    fn lab_uses_fallbacks_until_images_are_ready_and_reset_cleans_them() {
        let mut app = test_app();
        assert_eq!(count::<PlaceholderVisual>(&mut app), ROLE_COUNT);
        assert_eq!(count::<SpriteFallback>(&mut app), ROLE_COUNT);

        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::KeyR);
        }
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset(KeyCode::KeyR);
        app.update();

        assert_eq!(count::<PlaceholderVisual>(&mut app), ROLE_COUNT);
        assert_eq!(count::<LabEntity>(&mut app), ROLE_COUNT + 1);
    }
}
