//! Hex tile lab — Arc L Phase 89: walk the TrenchBroom-authored tiles.
//!
//! Loads `assets/tiles/manifest.ron`, renders each validated tile's convex
//! hulls, and drives the SHARED `observed_traversal` controller through them
//! first-person — including the two-level ramp prefab whose full-level ascent
//! is this phase's gate.
//!
//! Keys: WASD walk · mouse look · Shift sprint · Space jump · [ / ] cycle
//! tiles · R respawn · C collider view · F1 overlay. `OBSERVED2_CAPTURE=<dir>`
//! records one still per curated family representative (the Phase 91 library
//! is ~750 tiles) plus a scripted first-person diagonal-ramp ascent, writes
//! `manifest.json` (with the measured height gain), and exits.

mod capture;

use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::MouseMotion;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PresentMode, PrimaryWindow, WindowResolution};
use observed_authoring::{Manifest, TilePrototype};
use observed_style::SurfaceRole;
use observed_traversal::rapier_controller::{RapierTraversalScene, step_character};
use observed_traversal::{FpsBody, FpsConfig};
use player_input::PlayerIntent;
use rapier3d::prelude::{SharedShape, Vector as RapierVector};

use crate::capture::{CaptureRun, capture_progress};

#[derive(Component)]
struct TileVisual;

#[derive(Component)]
struct LabStatus;

#[derive(Component)]
struct EyeCamera;

#[derive(Resource)]
struct LabState {
    tiles: Vec<TilePrototype>,
    current: usize,
    scene: RapierTraversalScene,
    body: FpsBody,
    config: FpsConfig,
    collider_view: bool,
    overlay: bool,
    look: Vec2,
    /// Capture script drives the body forward instead of the keyboard.
    scripted_walk: bool,
    dirty: bool,
}

/// Outward plan direction (world x, z) of a lateral face's midpoint.
fn face_plan_dir(face: observed_hex::HexFace) -> Vec2 {
    let [a, b] = observed_hex::face_edge(face);
    Vec2::new((a.0 + b.0) as f32 * 0.5, (a.1 + b.1) as f32 * 0.5).normalize()
}

/// The first lateral door in face order, if any.
fn first_lateral_door(tile: &TilePrototype) -> Option<observed_hex::HexFace> {
    observed_hex::HexFace::LATERAL
        .into_iter()
        .find(|&face| tile.signature.port(face) == observed_hex::PortClass::Door)
}

/// Yaw that faces the plan direction `f` (forward is `(sin yaw, 0, -cos yaw)`).
fn yaw_toward(f: Vec2) -> f32 {
    f.x.atan2(-f.y)
}

/// Per-tile spawn: feet position + yaw + pitch. Ramps spawn just inside their
/// entrance door facing the climb; the shaft family spawns on its low ledge;
/// everything else spawns inside its first doorway facing the interior.
fn spawn_pose(tile: &TilePrototype) -> (Vec3, f32, f32) {
    match tile.key.archetype.as_str() {
        // Feet on the ramp surface just inside the entrance (the slope is
        // already ~0.9 m high there — spawning lower embeds the capsule);
        // pitched up so the view reads the full climb ahead.
        "ramp" => {
            let door = first_lateral_door(tile).expect("ramp has an entrance door");
            let dir = face_plan_dir(door);
            (
                Vec3::new(dir.x * 6.3, 0.95, dir.y * 6.3),
                yaw_toward(-dir),
                0.42,
            )
        }
        "shaft" | "shaft_top" | "shaft_bottom" | "shaft_landing" => {
            (Vec3::new(-2.0, 2.0, 0.0), std::f32::consts::FRAC_PI_2, 0.25)
        }
        _ => match first_lateral_door(tile) {
            Some(door) => {
                let dir = face_plan_dir(door);
                (
                    Vec3::new(dir.x * 5.2, 0.5, dir.y * 5.2),
                    yaw_toward(-dir),
                    0.0,
                )
            }
            None => (Vec3::new(-4.5, 0.5, 0.0), std::f32::consts::FRAC_PI_2, 0.0),
        },
    }
}

impl LabState {
    fn load() -> Self {
        // Workspace root when launched via `cargo run`; fall back to the
        // manifest-relative path under `cargo test` (crate-local CWD).
        let mut base = std::path::PathBuf::from("assets/tiles");
        if !base.exists() {
            base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles");
        }
        let manifest = Manifest::load(&base.join("manifest.ron")).expect("manifest loads");
        let tiles = manifest
            .load_tiles(&base)
            .expect("all manifest tiles validate");
        assert!(!tiles.is_empty(), "manifest has tiles");
        let mut state = Self {
            scene: RapierTraversalScene::from_arena_spec(&tiles[0].arena_spec()),
            tiles,
            current: 0,
            body: FpsBody::spawned(Vec3::ZERO, 0.0),
            config: FpsConfig::default(),
            collider_view: false,
            overlay: true,
            look: Vec2::ZERO,
            scripted_walk: false,
            dirty: true,
        };
        state.switch(0);
        state
    }

    fn switch(&mut self, index: usize) {
        self.current = index % self.tiles.len();
        let tile = &self.tiles[self.current];
        self.scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
        let (feet, yaw, pitch) = spawn_pose(tile);
        self.body = FpsBody::spawned(feet + Vec3::Y * self.config.half_height, yaw);
        self.body.pitch = pitch;
        self.dirty = true;
    }

    fn respawn(&mut self) {
        self.switch(self.current);
    }

    fn tile(&self) -> &TilePrototype {
        &self.tiles[self.current]
    }

    fn feet_height(&self) -> f32 {
        self.body.position.y - self.config.half_height
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.008, 0.012, 0.024)))
        .insert_resource(LabState::load())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Hex Tile Lab".to_string(),
                resolution: WindowResolution::new(1500, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, step_body)
        .add_systems(
            Update,
            (handle_input, rebuild_visuals, sync_camera, update_status).chain(),
        );
    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRun::new(path))
            .add_systems(Update, capture_progress.after(sync_camera));
    }
    app.run();
}

fn setup(mut commands: Commands, mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    commands
        .spawn((
            EyeCamera,
            Camera3d::default(),
            Transform::from_xyz(0.0, 1.6, 0.0),
            Name::new("Tile lab eye"),
        ))
        .with_children(|eye| {
            // Debug headlamp: keeps first-person capture frames legible in
            // authored interiors before the real light staging (Phase 91/92).
            eye.spawn((
                PointLight {
                    intensity: 260_000.0,
                    range: 18.0,
                    color: Color::srgb(0.85, 0.92, 1.0),
                    shadows_enabled: false,
                    ..default()
                },
                Name::new("Tile lab headlamp"),
            ));
        });
    commands.spawn((
        DirectionalLight {
            illuminance: 2_800.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.0, 0.6, 0.0)),
        Name::new("Tile lab key light"),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.55, 0.65, 0.85),
        brightness: 420.0,
        ..default()
    });
    commands.spawn((
        LabStatus,
        Text::new(""),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgb(0.88, 0.94, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(16.0),
            ..default()
        },
    ));
    if std::env::var("OBSERVED2_CAPTURE").is_err()
        && let Ok(mut cursor) = cursors.single_mut()
    {
        cursor.grab_mode = CursorGrabMode::Confined;
        cursor.visible = false;
    }
}

fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut motions: MessageReader<MouseMotion>,
    mut state: ResMut<LabState>,
) {
    let mut look = Vec2::ZERO;
    for motion in motions.read() {
        look += motion.delta;
    }
    state.look = look * 0.06;

    if keyboard.just_pressed(KeyCode::BracketRight) {
        let next = (state.current + 1) % state.tiles.len();
        state.switch(next);
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        let next = (state.current + state.tiles.len() - 1) % state.tiles.len();
        state.switch(next);
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        state.respawn();
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        state.collider_view = !state.collider_view;
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        state.overlay = !state.overlay;
    }
}

fn intent_from_keys(keyboard: &ButtonInput<KeyCode>, look: Vec2) -> PlayerIntent {
    let mut movement = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        movement.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        movement.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        movement.x += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        movement.x -= 1.0;
    }
    PlayerIntent {
        movement,
        look,
        jump_pressed: keyboard.just_pressed(KeyCode::Space),
        sprint_held: keyboard.pressed(KeyCode::ShiftLeft),
        ..PlayerIntent::default()
    }
}

fn step_body(keyboard: Res<ButtonInput<KeyCode>>, mut state: ResMut<LabState>) {
    let intent = if state.scripted_walk {
        PlayerIntent {
            movement: Vec2::new(0.0, 1.0),
            ..PlayerIntent::default()
        }
    } else {
        intent_from_keys(&keyboard, state.look)
    };
    state.look = Vec2::ZERO;
    let LabState {
        scene,
        body,
        config,
        ..
    } = &mut *state;
    step_character(scene, body, intent, config, 1.0 / 60.0);
    // Fell out of the tile (open shaft, doorway ledge): respawn.
    if state.body.position.y < -12.0 {
        state.respawn();
    }
}

fn sync_camera(state: Res<LabState>, mut camera: Query<&mut Transform, With<EyeCamera>>) {
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    let eye = state.body.position + Vec3::Y * (state.config.eye_height - state.config.half_height);
    transform.translation = eye;
    transform.rotation =
        Quat::from_rotation_y(-state.body.yaw) * Quat::from_rotation_x(state.body.pitch);
}

fn hull_mesh(hull: &[Vec3]) -> Option<Mesh> {
    let points: Vec<_> = hull
        .iter()
        .map(|v| RapierVector::new(v.x, v.y, v.z))
        .collect();
    let shape = SharedShape::convex_hull(&points)?;
    let (vertices, indices) = shape.as_convex_polyhedron()?.to_trimesh();
    let positions: Vec<[f32; 3]> = vertices.iter().map(|p| [p.x, p.y, p.z]).collect();
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices.into_iter().flatten().collect()))
    .with_duplicated_vertices()
    .with_computed_flat_normals();
    Some(mesh)
}

fn rebuild_visuals(
    mut commands: Commands,
    mut state: ResMut<LabState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    visuals: Query<Entity, With<TileVisual>>,
) {
    if !state.dirty {
        return;
    }
    for entity in &visuals {
        commands.entity(entity).despawn();
    }
    let treatment = observed_style::surface(match state.tile().key.archetype.as_str() {
        archetype if archetype.starts_with("shaft") => SurfaceRole::WellshaftStone,
        "ramp" => SurfaceRole::GantryDeck,
        _ => SurfaceRole::Plain,
    });
    let material = if state.collider_view {
        materials.add(StandardMaterial {
            base_color: treatment
                .edge
                .unwrap_or(treatment.base_color)
                .with_alpha(0.35),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        })
    } else {
        materials.add(StandardMaterial {
            base_color: treatment.base_color,
            emissive: treatment.emissive * 0.30,
            perceptual_roughness: 0.92,
            ..default()
        })
    };
    for hull in state.tile().hulls.clone() {
        let Some(mesh) = hull_mesh(&hull) else {
            continue;
        };
        commands.spawn((
            TileVisual,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Name::new("tile hull"),
        ));
    }
    // One practical lamp per level so authored interiors actually read.
    for level in 0..state.tile().levels {
        commands.spawn((
            TileVisual,
            PointLight {
                intensity: 1_800_000.0,
                range: 32.0,
                color: Color::srgb(1.0, 0.86, 0.62),
                shadows_enabled: true,
                ..default()
            },
            Transform::from_xyz(
                0.0,
                f32::from(level) * observed_hex::TILE_LEVEL_HEIGHT
                    + observed_hex::TILE_LEVEL_HEIGHT
                    - 1.6,
                0.0,
            ),
            Name::new("tile practical"),
        ));
    }
    state.dirty = false;
}

fn update_status(state: Res<LabState>, mut status: Query<&mut Text, With<LabStatus>>) {
    let Ok(mut text) = status.single_mut() else {
        return;
    };
    if !state.overlay {
        **text = String::new();
        return;
    }
    let tile = state.tile();
    let ports: Vec<String> = observed_hex::HexFace::ALL
        .into_iter()
        .filter_map(|face| {
            let class = tile.signature.port(face);
            (class != observed_hex::PortClass::Sealed).then(|| format!("{face:?}:{class:?}"))
        })
        .collect();
    **text = format!(
        "HEX TILE LAB — Arc L Phase 89\ntile {}/{} | {} / {} / v{} | levels {} | ports [{}] | hulls {}\nfeet {:.2} m | grounded {}\n\nWASD+mouse walk | Shift sprint | Space jump | [ ] cycle | R respawn | C colliders | F1 overlay",
        state.current + 1,
        state.tiles.len(),
        tile.key.archetype,
        tile.key.register,
        tile.key.variant,
        tile.levels,
        ports.join(", "),
        tile.hulls.len(),
        state.feet_height(),
        state.body.grounded,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_tiles_load_and_every_spawn_pose_is_inside() {
        let state = LabState::load();
        assert!(state.tiles.len() >= 4);
        for tile in &state.tiles {
            let (feet, _, _) = spawn_pose(tile);
            assert!(feet.length() < 8.0, "spawn stays near the cell center");
        }
    }

    #[test]
    fn switching_tiles_resets_the_body_to_the_tile_spawn() {
        let mut state = LabState::load();
        let ramp = state
            .tiles
            .iter()
            .position(|tile| tile.key.archetype == "ramp")
            .expect("ramp tile present");
        state.switch(ramp);
        let (feet, yaw, _) = spawn_pose(state.tile());
        assert_eq!(
            state.body.position,
            feet + Vec3::Y * state.config.half_height
        );
        assert_eq!(state.body.yaw, yaw);
    }
}
