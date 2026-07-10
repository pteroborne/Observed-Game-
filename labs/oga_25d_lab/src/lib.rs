//! oga_25d_lab -- prove CC0 2.5D sprites and metadata pipeline.

pub mod metadata;

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::{
    app::AppExit,
    asset::AssetPlugin,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};
use bevy_sprite3d::prelude::{Sprite3d, Sprite3dPlugin};
use metadata::SpriteMetadata;
use observed_style::{self as style, MarkerRole};

const ROLE_COUNT: usize = 4;

#[derive(Component)]
pub struct LabEntity;

#[derive(Component)]
pub struct SpriteFallback;

#[derive(Component)]
pub struct BillboardSprite;

#[derive(Component)]
pub struct GuardActor {
    pub clip: String,
    pub frame_timer: Timer,
    pub animation_step: usize,
    pub yaw: f32, // Facing direction in world coordinates
}

#[derive(Component)]
pub struct DebugText;

#[derive(Resource)]
struct LabAssets {
    metadata: HashMap<String, SpriteMetadata>,
    images: HashMap<String, Handle<Image>>,
    guard_layout: Handle<TextureAtlasLayout>,
}

#[derive(Resource)]
struct LabRenderAssets {
    cube: Handle<Mesh>,
    floor: Handle<Mesh>,
    fallback_materials: [Handle<StandardMaterial>; ROLE_COUNT],
    floor_material: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct LabSettings {
    pub billboard_mode: bool,
    pub auto_orbit: bool,
    pub camera_angle: f32,
}

impl Default for LabSettings {
    fn default() -> Self {
        Self {
            billboard_mode: false,
            auto_orbit: true,
            camera_angle: std::f32::consts::FRAC_PI_2,
        }
    }
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

pub struct Oga25dLabPlugin;

impl Plugin for Oga25dLabPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Sprite3dPlugin)
            .init_resource::<ContentMode>()
            .init_resource::<LabSettings>()
            .add_systems(Startup, setup_lab)
            .add_systems(Update, (reset_lab, sync_lab_content).chain())
            .add_systems(
                Update,
                (
                    keyboard_controls,
                    animate_and_orient_actor,
                    camera_orbit_control,
                    update_debug_text,
                    face_billboard_sprites,
                ),
            );
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
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut settings: ResMut<LabSettings>,
) {
    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE")
        && path.contains("billboard")
    {
        settings.billboard_mode = true;
    }

    let derived_dir = workspace_assets_dir().join("oga_25d").join("derived");
    let mut metadata_map = HashMap::new();
    let mut image_map = HashMap::new();

    // Try loading derived metadata
    if let Ok(entries) = std::fs::read_dir(&derived_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let meta_res = SpriteMetadata::load_from_path(&path);
                if let Ok(meta) = meta_res {
                    let name = meta.name.clone();
                    let img_handle =
                        asset_server.load(format!("oga_25d/derived/{}", meta.image_path));
                    image_map.insert(name.clone(), img_handle);
                    metadata_map.insert(name, meta);
                }
            }
        }
    }

    let layout = TextureAtlasLayout::from_grid(UVec2::new(64, 64), 8, 7, None, None);
    let guard_layout = texture_atlases.add(layout);

    commands.insert_resource(LabAssets {
        metadata: metadata_map,
        images: image_map,
        guard_layout,
    });

    let fallback_materials = [
        MarkerRole::You,
        MarkerRole::Rival,
        MarkerRole::Director,
        MarkerRole::Control,
    ]
    .map(|role| {
        let treatment = style::marker(role);
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
        floor: meshes.add(Plane3d::default().mesh().size(20.0, 15.0)),
        fallback_materials,
        floor_material: materials.add(StandardMaterial {
            base_color: floor_treatment.base_color,
            emissive: floor_treatment.emissive,
            ..default()
        }),
    });

    // Spawn Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 3.0, 8.0).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
        Name::new("OGA 25D Lab Camera"),
    ));

    // Lighting
    commands.spawn((
        DirectionalLight {
            illuminance: 3_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.35, 0.0)),
        Name::new("OGA 25D Lab Sun"),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.55, 0.65, 0.85),
        brightness: 450.0,
        ..default()
    });

    // Spawn UI Debug overlay
    commands.spawn((
        DebugText,
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::WHITE),
    ));
}

fn reset_lab(
    keyboard: Res<ButtonInput<KeyCode>>,
    lab_entities: Query<Entity, With<LabEntity>>,
    mut mode: ResMut<ContentMode>,
    mut settings: ResMut<LabSettings>,
    mut commands: Commands,
) {
    if !keyboard.just_pressed(KeyCode::KeyR) {
        return;
    }
    for entity in &lab_entities {
        commands.entity(entity).despawn();
    }
    mode.0 = None;
    *settings = LabSettings::default();
}

fn sync_lab_content(
    assets: Res<LabAssets>,
    images: Res<Assets<Image>>,
    render: Res<LabRenderAssets>,
    mut mode: ResMut<ContentMode>,
    existing: Query<Entity, With<LabEntity>>,
    mut commands: Commands,
) {
    // Check if the primary assets are loaded
    let assets_ready =
        !assets.images.is_empty() && assets.images.values().all(|h| images.get(h).is_some());
    if mode.0 == Some(assets_ready) {
        return;
    }

    for entity in &existing {
        commands.entity(entity).despawn();
    }

    spawn_floor(&mut commands, &render);

    if assets_ready {
        spawn_scenes(&mut commands, &assets);
    } else {
        spawn_fallbacks(&mut commands, &render);
    }

    mode.0 = Some(assets_ready);
}

fn spawn_floor(commands: &mut Commands, render: &LabRenderAssets) {
    commands.spawn((
        LabEntity,
        Mesh3d(render.floor.clone()),
        MeshMaterial3d(render.floor_material.clone()),
        Transform::from_xyz(0.0, -0.02, 0.0),
        Name::new("OGA 25D lab floor"),
    ));
}

fn spawn_scenes(commands: &mut Commands, assets: &LabAssets) {
    println!("Spawning metadata-driven scenes...");

    // 1. Actors Row (z = 0.0)
    // Pacing Guard actor (directional, clips)
    if let (Some(meta), Some(image_handle)) =
        (assets.metadata.get("guard"), assets.images.get("guard"))
    {
        commands.spawn((
            LabEntity,
            GuardActor {
                clip: "walk".to_string(),
                frame_timer: Timer::from_seconds(0.15, TimerMode::Repeating),
                animation_step: 0,
                yaw: 0.0,
            },
            Sprite {
                image: image_handle.clone(),
                texture_atlas: Some(TextureAtlas {
                    layout: assets.guard_layout.clone(),
                    index: 0,
                }),
                ..default()
            },
            Sprite3d {
                pixels_per_metre: meta.pixels_per_metre,
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                pivot: Some(Vec2::new(meta.pivot.0, meta.pivot.1)),
                double_sided: true,
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, 0.0),
            Name::new("Guard Actor"),
        ));
    }

    // 2. Objects Row (z = -2.0)
    let objects = [
        ("keystone_card", -4.5),
        ("exit_access_card", -3.0),
        ("battery_charge", -1.5),
        ("route_cell", 0.0),
        ("repair_token", 1.5),
        ("relay_device", 3.0),
        ("anchor_torch", 4.5),
    ];

    for (name, x) in objects {
        if let (Some(meta), Some(img)) = (assets.metadata.get(name), assets.images.get(name)) {
            commands.spawn((
                LabEntity,
                BillboardSprite,
                Sprite {
                    image: img.clone(),
                    ..default()
                },
                Sprite3d {
                    pixels_per_metre: meta.pixels_per_metre,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    pivot: Some(Vec2::new(meta.pivot.0, meta.pivot.1)),
                    double_sided: true,
                    ..default()
                },
                Transform::from_xyz(x, 0.0, -2.0),
                Name::new(format!("Object {}", name)),
            ));
        }
    }

    // 3. Decorations Row (z = 2.0)
    let decs = [
        ("column", -3.0),
        ("torch_wall", -1.0),
        ("lab_crate", 1.0),
        ("lab_table", 3.0),
    ];

    for (name, x) in decs {
        if let (Some(meta), Some(img)) = (assets.metadata.get(name), assets.images.get(name)) {
            commands.spawn((
                LabEntity,
                BillboardSprite,
                Sprite {
                    image: img.clone(),
                    ..default()
                },
                Sprite3d {
                    pixels_per_metre: meta.pixels_per_metre,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    pivot: Some(Vec2::new(meta.pivot.0, meta.pivot.1)),
                    double_sided: true,
                    ..default()
                },
                Transform::from_xyz(x, 0.0, 2.0),
                Name::new(format!("Decoration {}", name)),
            ));
        }
    }

    // 4. LAB Texture Sample (rendered on a vertical quad shell at z = -4.0)
    if let Some(img) = assets.images.get("lab_wall_tile") {
        commands.spawn((
            LabEntity,
            Sprite {
                image: img.clone(),
                ..default()
            },
            Sprite3d {
                pixels_per_metre: 64.0,
                alpha_mode: AlphaMode::Opaque,
                unlit: false,
                pivot: Some(Vec2::new(0.5, 0.5)),
                double_sided: true,
                ..default()
            },
            Transform::from_xyz(0.0, 1.0, -4.0),
            Name::new("LAB Wall Texture Shell"),
        ));
    }
}

fn spawn_fallbacks(commands: &mut Commands, render: &LabRenderAssets) {
    println!("Spawning fallbacks...");
    let x_positions = [-4.5, -1.5, 1.5, 4.5];
    for (index, &x) in x_positions.iter().enumerate() {
        commands.spawn((
            LabEntity,
            SpriteFallback,
            Mesh3d(render.cube.clone()),
            MeshMaterial3d(render.fallback_materials[index].clone()),
            Transform::from_xyz(x, 0.8, 0.0),
            Name::new(format!("Fallback item {}", index)),
        ));
    }
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
        let to_camera = Vec2::new(
            camera_pos.x - transform.translation.x,
            camera_pos.z - transform.translation.z,
        );
        if to_camera.length_squared() > 0.0001 {
            let yaw = to_camera.x.atan2(to_camera.y);
            transform.rotation = Quat::from_rotation_y(yaw);
        }
    }
}

fn animate_and_orient_actor(
    time: Res<Time>,
    assets: Res<LabAssets>,
    settings: Res<LabSettings>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    mut query: Query<(
        &mut GuardActor,
        &mut Transform,
        &mut Sprite,
        Option<&BillboardSprite>,
    )>,
) {
    let Some(camera_transform) = camera.iter().next() else {
        return;
    };
    let camera_pos = camera_transform.translation();
    let meta = match assets.metadata.get("guard") {
        Some(m) => m,
        None => return,
    };

    for (mut guard, mut transform, mut sprite, billboard) in &mut query {
        // Update timer and animation step
        guard.frame_timer.tick(time.delta());
        if guard.frame_timer.just_finished() {
            guard.animation_step += 1;
        }

        // Slowly pace left/right in the lab to show off motion
        let pace_offset = (time.elapsed_secs() * 0.5).sin() * 3.0;
        transform.translation.x = pace_offset;

        // Determine pacing yaw direction (facing left or right)
        let speed = (time.elapsed_secs() * 0.5).cos();
        if speed > 0.05 {
            guard.yaw = -std::f32::consts::FRAC_PI_2; // facing right
        } else if speed < -0.05 {
            guard.yaw = std::f32::consts::FRAC_PI_2; // facing left
        }

        // If billboard toggle is ON or billboard marker is present on entity
        let is_billboard = settings.billboard_mode || billboard.is_some();

        if is_billboard {
            // Face the camera
            let to_camera = Vec2::new(
                camera_pos.x - transform.translation.x,
                camera_pos.z - transform.translation.z,
            );
            if to_camera.length_squared() > 0.0001 {
                let yaw = to_camera.x.atan2(to_camera.y);
                transform.rotation = Quat::from_rotation_y(yaw);
            }
        } else {
            // Keep fixed world rotation based on actor's yaw
            transform.rotation = Quat::from_rotation_y(guard.yaw);
        }

        // Camera relative direction angle
        let to_camera = Vec2::new(
            camera_pos.x - transform.translation.x,
            camera_pos.z - transform.translation.z,
        );
        let angle = if to_camera.length_squared() > 0.0001 {
            let camera_angle = to_camera.x.atan2(to_camera.y);
            let mut diff = camera_angle - guard.yaw;
            while diff > std::f32::consts::PI {
                diff -= 2.0 * std::f32::consts::PI;
            }
            while diff < -std::f32::consts::PI {
                diff += 2.0 * std::f32::consts::PI;
            }
            diff
        } else {
            0.0
        };

        // Determine direction column
        let count = 8;
        let sector = (angle + std::f32::consts::PI / count as f32) / (2.0 * std::f32::consts::PI);
        let mut index = (sector * count as f32).floor() as i32;
        if index < 0 {
            index += count as i32;
        }
        let dir_index = if is_billboard {
            0
        } else {
            index as usize % count
        };

        // Get step frame index
        let clip_steps = meta.clips.get(&guard.clip).expect("Clips key exists");
        let logical_step = clip_steps[guard.animation_step % clip_steps.len()];
        let final_frame = logical_step * 8 + dir_index;

        if let Some(ref mut atlas) = sprite.texture_atlas {
            atlas.index = final_frame;
        }
    }
}

fn camera_orbit_control(
    time: Res<Time>,
    mut settings: ResMut<LabSettings>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
) {
    // Toggle auto-orbit
    if keyboard.just_pressed(KeyCode::KeyO) {
        settings.auto_orbit = !settings.auto_orbit;
    }

    if settings.auto_orbit {
        settings.camera_angle += 0.4 * time.delta_secs();
    }

    // Manual orbit
    if keyboard.pressed(KeyCode::ArrowLeft) {
        settings.camera_angle -= 2.0 * time.delta_secs();
        settings.auto_orbit = false;
    }
    if keyboard.pressed(KeyCode::ArrowRight) {
        settings.camera_angle += 2.0 * time.delta_secs();
        settings.auto_orbit = false;
    }

    // Position camera on a circle orbit
    let radius = 7.5;
    let x = settings.camera_angle.cos() * radius;
    let z = settings.camera_angle.sin() * radius;

    for mut transform in &mut camera_query {
        *transform = Transform::from_xyz(x, 2.5, z).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y);
    }
}

fn keyboard_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<LabSettings>,
    mut guard_query: Query<&mut GuardActor>,
) {
    // Toggle billboard mode
    if keyboard.just_pressed(KeyCode::KeyB) {
        settings.billboard_mode = !settings.billboard_mode;
    }

    // Change guard clips
    for mut guard in &mut guard_query {
        if keyboard.just_pressed(KeyCode::Digit1) {
            guard.clip = "idle".to_string();
            guard.animation_step = 0;
        } else if keyboard.just_pressed(KeyCode::Digit2) {
            guard.clip = "walk".to_string();
            guard.animation_step = 0;
        } else if keyboard.just_pressed(KeyCode::Digit3) {
            guard.clip = "operate".to_string();
            guard.animation_step = 0;
        } else if keyboard.just_pressed(KeyCode::Digit4) {
            guard.clip = "alert".to_string();
            guard.animation_step = 0;
        } else if keyboard.just_pressed(KeyCode::Digit5) {
            guard.clip = "disrupted".to_string();
            guard.animation_step = 0;
        }
    }
}

fn update_debug_text(
    settings: Res<LabSettings>,
    guard_query: Query<(&GuardActor, &Sprite)>,
    mut text_query: Query<&mut Text, With<DebugText>>,
) {
    let mut debug_str = format!(
        "OGA 25D PIPELINE LAB\n\
         -------------------\n\
         Billboard Mode (B): {}\n\
         Auto Orbit (O): {}\n\
         Camera Orbit: [Arrow Left/Right]\n\
         Reset Lab: [R]\n\n",
        if settings.billboard_mode {
            "ON (front-facing only)"
        } else {
            "OFF (8-way directional)"
        },
        if settings.auto_orbit { "ON" } else { "OFF" }
    );

    if let Some((guard, sprite)) = guard_query.iter().next() {
        let frame_index = sprite.texture_atlas.as_ref().map(|a| a.index).unwrap_or(0);
        let logical_step = frame_index / 8;
        let direction_index = frame_index % 8;
        debug_str.push_str(&format!(
            "Actor Clip: {}\n\
             Logical Step: {}\n\
             Direction Column: {} (angle-driven)\n\
             Atlas Frame Index: {}\n\
             Clips: [1] Idle, [2] Walk, [3] Operate, [4] Alert, [5] Disrupted\n",
            guard.clip, logical_step, direction_index, frame_index
        ));
    } else {
        debug_str.push_str("Actor: Loading...\n");
    }

    for mut text in &mut text_query {
        **text = debug_str.clone();
    }
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<LabCaptureRequest>,
    guard: Query<(), With<GuardActor>>,
    fallbacks: Query<(), With<SpriteFallback>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            let ready = guard.iter().count() > 0 && fallbacks.iter().next().is_none();
            if (ready && elapsed >= 2.0) || elapsed >= 4.0 {
                request.phase = 1;
                request.next_at = elapsed + 0.5;
            }
        }
        1 if elapsed >= request.next_at => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(request.path.clone()));
            request.phase = 2;
            request.next_at = elapsed + 1.5;
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
                        title: "OGA 25D Sprite Pipeline Lab".to_string(),
                        resolution: WindowResolution::new(1280, 720),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(Oga25dLabPlugin);
    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(LabCaptureRequest::new(path))
            .add_systems(Update, capture_progress.after(sync_lab_content));
    }
    app.run();
}

pub fn yaw_toward_camera(sprite: Vec3, camera: Vec3) -> Option<f32> {
    let to_camera = Vec2::new(camera.x - sprite.x, camera.z - sprite.z);
    (to_camera.length_squared() > 0.0001).then(|| to_camera.x.atan2(to_camera.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, input::InputPlugin};

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
        .add_plugins(Oga25dLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
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
        // Since we are running the test headlessly without real assets loaded yet,
        // it should spawn 4 fallbacks (like the original lab)
        assert_eq!(count::<SpriteFallback>(&mut app), 4);

        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::KeyR);
        }
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset(KeyCode::KeyR);
        app.update();

        assert_eq!(count::<SpriteFallback>(&mut app), 4);
        assert_eq!(count::<LabEntity>(&mut app), 5); // 4 fallbacks + 1 floor
    }
}
