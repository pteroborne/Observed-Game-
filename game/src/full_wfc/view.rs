use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use observed_facility::full_wfc::{
    CellCoord, ModuleFace, ModulePlacement, ModuleSpace, ThresholdKey,
};
use observed_style::{MarkerRole, ObservedState, SurfaceRole};

use super::sim::{CELL_SIZE, FullWfcRuntime, LEVEL_HEIGHT};
use crate::GameState;
use crate::view::components::{GameCam, GameSun, MENU_SUN_ILLUMINANCE};

const WALL_HEIGHT: f32 = 4.55;
const WALL_THICKNESS: f32 = 0.18;
const FLOOR_THICKNESS: f32 = 0.16;
const SHAFT_OPENING: f32 = 4.2;

#[derive(Component)]
pub(super) struct FullWfcCell(CellCoord);

#[derive(Component)]
pub(super) struct FullWfcHud;

#[derive(Component)]
pub(super) struct CandleLight;

#[derive(Component)]
pub(super) struct ThresholdSignal(ThresholdKey);

#[derive(Resource)]
pub(super) struct FullWfcVisualAssets {
    cube: Handle<Mesh>,
    room: Handle<StandardMaterial>,
    hall: Handle<StandardMaterial>,
    wall: Handle<StandardMaterial>,
    ceiling: Handle<StandardMaterial>,
    threshold_unobserved: Handle<StandardMaterial>,
    threshold_observed: Handle<StandardMaterial>,
    threshold_sealed: Handle<StandardMaterial>,
    exit: Handle<StandardMaterial>,
}

pub(super) fn setup_view(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut runtime: ResMut<FullWfcRuntime>,
    camera: Query<Entity, With<GameCam>>,
    mut sun: Query<&mut DirectionalLight, With<GameSun>>,
) {
    let assets = FullWfcVisualAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        room: material(&mut materials, observed_style::surface(SurfaceRole::Plain)),
        hall: material(
            &mut materials,
            observed_style::surface(SurfaceRole::GantryDeck),
        ),
        wall: material(&mut materials, observed_style::surface(SurfaceRole::Wall)),
        ceiling: material(
            &mut materials,
            observed_style::surface(SurfaceRole::Ceiling),
        ),
        threshold_unobserved: material(
            &mut materials,
            observed_style::observed_modulate(
                observed_style::surface(SurfaceRole::SafeBypass),
                ObservedState::Unobserved,
            ),
        ),
        threshold_observed: material(
            &mut materials,
            observed_style::observed_modulate(
                observed_style::surface(SurfaceRole::Spine),
                ObservedState::Frozen,
            ),
        ),
        threshold_sealed: material(&mut materials, observed_style::marker(MarkerRole::Collapse)),
        exit: material(&mut materials, observed_style::marker(MarkerRole::Exit)),
    };

    for placement in runtime.world.placements.values() {
        spawn_cell(&mut commands, &assets, placement, runtime.world.exit());
    }
    runtime.pending_visual_changes.clear();

    if let Ok(camera) = camera.single() {
        commands.entity(camera).insert((
            Hdr,
            Bloom {
                intensity: 0.09,
                ..Bloom::NATURAL
            },
            DistanceFog {
                color: Color::srgb(0.006, 0.012, 0.025),
                falloff: FogFalloff::Linear {
                    start: 16.0,
                    end: 42.0,
                },
                ..default()
            },
        ));
    }
    for mut light in &mut sun {
        light.illuminance = 0.0;
    }
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.20, 0.30, 0.48),
        brightness: 55.0,
        ..default()
    });

    let candle_color = observed_style::marker(MarkerRole::NextRoom).base_color;
    commands.spawn((
        CandleLight,
        DespawnOnExit(GameState::FullWfc),
        PointLight {
            color: candle_color,
            intensity: 220.0,
            range: 9.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::default(),
        Name::new("A* proximity candle"),
    ));
    spawn_hud(&mut commands);
    commands.insert_resource(assets);
}

fn material(
    materials: &mut Assets<StandardMaterial>,
    treatment: observed_style::Treatment,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: treatment.base_color,
        emissive: treatment.emissive,
        metallic: 0.08,
        perceptual_roughness: 0.78,
        ..default()
    })
}

fn spawn_cell(
    commands: &mut Commands,
    assets: &FullWfcVisualAssets,
    placement: &ModulePlacement,
    exit: CellCoord,
) {
    if placement.space == ModuleSpace::Void {
        return;
    }
    let center = Vec3::new(
        f32::from(placement.coord.x) * CELL_SIZE,
        f32::from(placement.coord.level) * LEVEL_HEIGHT,
        f32::from(placement.coord.z) * CELL_SIZE,
    );
    commands
        .spawn((
            FullWfcCell(placement.coord),
            DespawnOnExit(GameState::FullWfc),
            Transform::from_translation(center),
            Visibility::default(),
            Name::new(format!(
                "Full WFC {:?} {:?} {:?}",
                placement.space, placement.archetype, placement.coord
            )),
        ))
        .with_children(|cell| {
            let floor_material = if placement.space == ModuleSpace::Room {
                assets.room.clone()
            } else {
                assets.hall.clone()
            };
            spawn_deck(
                cell,
                assets,
                floor_material,
                placement.is_open(ModuleFace::Down),
                0.0,
                false,
            );
            spawn_deck(
                cell,
                assets,
                assets.ceiling.clone(),
                placement.is_open(ModuleFace::Up),
                WALL_HEIGHT,
                true,
            );

            for face in [
                ModuleFace::East,
                ModuleFace::West,
                ModuleFace::South,
                ModuleFace::North,
            ] {
                if !placement.is_open(face) {
                    spawn_wall(cell, assets, face);
                }
                if placement.space == ModuleSpace::Room {
                    spawn_threshold_frame(
                        cell,
                        assets,
                        ThresholdKey {
                            room: placement.coord,
                            face,
                        },
                    );
                }
            }
            if placement.space == ModuleSpace::Room {
                for face in [ModuleFace::Up, ModuleFace::Down] {
                    spawn_vertical_threshold(
                        cell,
                        assets,
                        ThresholdKey {
                            room: placement.coord,
                            face,
                        },
                    );
                }
            }
            if placement.coord == exit {
                spawn_exit_beacon(cell, assets);
            } else if placement.space == ModuleSpace::Room {
                cell.spawn((
                    PointLight {
                        color: observed_style::surface(SurfaceRole::SafeBypass)
                            .edge
                            .unwrap_or(Color::WHITE),
                        intensity: 65.0,
                        range: 8.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.0, WALL_HEIGHT - 0.35, 0.0),
                ));
            }
        });
}

fn spawn_deck(
    parent: &mut ChildSpawnerCommands,
    assets: &FullWfcVisualAssets,
    material: Handle<StandardMaterial>,
    has_hole: bool,
    y: f32,
    ceiling: bool,
) {
    let thickness = FLOOR_THICKNESS;
    let y = if ceiling {
        y + thickness * 0.5
    } else {
        y - thickness * 0.5
    };
    if !has_hole {
        spawn_cube(
            parent,
            assets,
            material,
            Vec3::new(
                CELL_SIZE - WALL_THICKNESS,
                thickness,
                CELL_SIZE - WALL_THICKNESS,
            ),
            Vec3::new(0.0, y, 0.0),
        );
        return;
    }
    let side = (CELL_SIZE - SHAFT_OPENING) * 0.5;
    for x in [-1.0, 1.0] {
        spawn_cube(
            parent,
            assets,
            material.clone(),
            Vec3::new(side, thickness, CELL_SIZE),
            Vec3::new(x * (SHAFT_OPENING + side) * 0.5, y, 0.0),
        );
    }
    for z in [-1.0, 1.0] {
        spawn_cube(
            parent,
            assets,
            material.clone(),
            Vec3::new(SHAFT_OPENING, thickness, side),
            Vec3::new(0.0, y, z * (SHAFT_OPENING + side) * 0.5),
        );
    }
}

fn spawn_wall(parent: &mut ChildSpawnerCommands, assets: &FullWfcVisualAssets, face: ModuleFace) {
    let (size, position) = match face {
        ModuleFace::East => (
            Vec3::new(WALL_THICKNESS, WALL_HEIGHT, CELL_SIZE),
            Vec3::new(
                CELL_SIZE * 0.5 - WALL_THICKNESS * 0.5,
                WALL_HEIGHT * 0.5,
                0.0,
            ),
        ),
        ModuleFace::West => (
            Vec3::new(WALL_THICKNESS, WALL_HEIGHT, CELL_SIZE),
            Vec3::new(
                -CELL_SIZE * 0.5 + WALL_THICKNESS * 0.5,
                WALL_HEIGHT * 0.5,
                0.0,
            ),
        ),
        ModuleFace::South => (
            Vec3::new(CELL_SIZE, WALL_HEIGHT, WALL_THICKNESS),
            Vec3::new(
                0.0,
                WALL_HEIGHT * 0.5,
                CELL_SIZE * 0.5 - WALL_THICKNESS * 0.5,
            ),
        ),
        ModuleFace::North => (
            Vec3::new(CELL_SIZE, WALL_HEIGHT, WALL_THICKNESS),
            Vec3::new(
                0.0,
                WALL_HEIGHT * 0.5,
                -CELL_SIZE * 0.5 + WALL_THICKNESS * 0.5,
            ),
        ),
        ModuleFace::Up | ModuleFace::Down => return,
    };
    spawn_cube(parent, assets, assets.wall.clone(), size, position);
}

fn spawn_threshold_frame(
    parent: &mut ChildSpawnerCommands,
    assets: &FullWfcVisualAssets,
    key: ThresholdKey,
) {
    let face = key.face;
    let boundary = CELL_SIZE * 0.5 - WALL_THICKNESS * 1.2;
    let (lintel_size, lintel_position, post_size, post_positions) = match face {
        ModuleFace::East | ModuleFace::West => {
            let x = if face == ModuleFace::East {
                boundary
            } else {
                -boundary
            };
            (
                Vec3::new(0.12, 0.14, 4.2),
                Vec3::new(x, 3.35, 0.0),
                Vec3::new(0.12, 3.35, 0.14),
                [Vec3::new(x, 1.67, -2.1), Vec3::new(x, 1.67, 2.1)],
            )
        }
        ModuleFace::South | ModuleFace::North => {
            let z = if face == ModuleFace::South {
                boundary
            } else {
                -boundary
            };
            (
                Vec3::new(4.2, 0.14, 0.12),
                Vec3::new(0.0, 3.35, z),
                Vec3::new(0.14, 3.35, 0.12),
                [Vec3::new(-2.1, 1.67, z), Vec3::new(2.1, 1.67, z)],
            )
        }
        ModuleFace::Up | ModuleFace::Down => return,
    };
    parent.spawn((
        ThresholdSignal(key),
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.threshold_unobserved.clone()),
        Transform::from_translation(lintel_position).with_scale(lintel_size),
        Name::new("unobserved threshold frame"),
    ));
    for position in post_positions {
        parent.spawn((
            ThresholdSignal(key),
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.threshold_unobserved.clone()),
            Transform::from_translation(position).with_scale(post_size),
        ));
    }
}

fn spawn_vertical_threshold(
    parent: &mut ChildSpawnerCommands,
    assets: &FullWfcVisualAssets,
    key: ThresholdKey,
) {
    let y = if key.face == ModuleFace::Up {
        WALL_HEIGHT - 0.08
    } else {
        0.08
    };
    parent.spawn((
        ThresholdSignal(key),
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.threshold_unobserved.clone()),
        Transform::from_xyz(0.0, y, 0.0).with_scale(Vec3::new(SHAFT_OPENING, 0.08, 0.12)),
        Name::new("vertical threshold signal"),
    ));
}

fn spawn_exit_beacon(parent: &mut ChildSpawnerCommands, assets: &FullWfcVisualAssets) {
    spawn_cube(
        parent,
        assets,
        assets.exit.clone(),
        Vec3::new(1.1, 3.6, 1.1),
        Vec3::new(0.0, 1.8, 0.0),
    );
    parent.spawn((
        PointLight {
            color: observed_style::marker(MarkerRole::Exit).base_color,
            intensity: 2_200.0,
            range: 18.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 2.5, 0.0),
        Name::new("exit beacon light"),
    ));
}

fn spawn_cube(
    parent: &mut ChildSpawnerCommands,
    assets: &FullWfcVisualAssets,
    material: Handle<StandardMaterial>,
    size: Vec3,
    position: Vec3,
) {
    parent.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(position).with_scale(size),
    ));
}

fn spawn_hud(commands: &mut Commands) {
    commands
        .spawn((
            DespawnOnExit(GameState::FullWfc),
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(18)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(30),
            Name::new("Full WFC HUD root"),
        ))
        .with_children(|root| {
            root.spawn((
                FullWfcHud,
                Text::new("Full WFC initializing"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.95, 1.0)),
                Node {
                    width: px(470),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.008, 0.018, 0.035, 0.9)),
                BorderColor::all(Color::srgba(0.35, 0.9, 1.0, 0.6)),
            ));
            root.spawn((
                Text::new(
                    "FULL WFC EXPERIMENT\nWASD move | Shift sprint\nMouse / arrows look\nSpace climb up | Ctrl climb down\nEsc return to menu\n\nCandle: A* travel-cost proximity\nCyan frames: mutable threshold\nGold frames: observed / frozen\nRed frames: competing exit path sealed",
                ),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.70, 0.78, 0.9)),
                Node {
                    width: px(330),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.008, 0.018, 0.035, 0.88)),
                BorderColor::all(Color::srgba(0.35, 0.9, 1.0, 0.45)),
            ));
            root.spawn((
                Text::new("+"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(50),
                    top: percent(48),
                    ..default()
                },
            ));
        });
}

pub(super) fn sync_changed_geometry(
    mut commands: Commands,
    mut runtime: ResMut<FullWfcRuntime>,
    assets: Res<FullWfcVisualAssets>,
    existing: Query<(Entity, &FullWfcCell)>,
) {
    if runtime.pending_visual_changes.is_empty() {
        return;
    }
    let changed = std::mem::take(&mut runtime.pending_visual_changes);
    for (entity, cell) in &existing {
        if changed.contains(&cell.0) {
            commands.entity(entity).despawn();
        }
    }
    for coord in changed {
        if let Some(placement) = runtime.world.placement(coord) {
            spawn_cell(&mut commands, &assets, placement, runtime.world.exit());
        }
    }
}

pub(super) fn sync_threshold_signals(
    runtime: Res<FullWfcRuntime>,
    assets: Res<FullWfcVisualAssets>,
    mut signals: Query<(&ThresholdSignal, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let observed = &runtime.world.observation.visible_thresholds;
    for (signal, mut material) in &mut signals {
        let sealed = terminal_face_is_reserved(&runtime, signal.0);
        material.0 = if sealed {
            assets.threshold_sealed.clone()
        } else if observed.contains(&signal.0) {
            assets.threshold_observed.clone()
        } else {
            assets.threshold_unobserved.clone()
        };
    }
}

fn terminal_face_is_reserved(runtime: &FullWfcRuntime, threshold: ThresholdKey) -> bool {
    if threshold.room == runtime.world.exit() {
        return runtime.world.reserved_exit_faces.contains(&threshold.face);
    }
    runtime
        .world
        .config
        .neighbor(threshold.room, threshold.face)
        .is_some_and(|next| {
            next == runtime.world.exit()
                && runtime
                    .world
                    .reserved_exit_faces
                    .contains(&threshold.face.opposite())
        })
}

pub(super) fn sync_camera_and_candle(
    runtime: Res<FullWfcRuntime>,
    mut camera: Query<&mut Transform, (With<GameCam>, Without<CandleLight>)>,
    mut candle: Query<(&mut Transform, &mut PointLight), With<CandleLight>>,
) {
    let rotation = Quat::from_euler(EulerRot::YXZ, runtime.player.yaw, runtime.player.pitch, 0.0);
    if let Ok(mut transform) = camera.single_mut() {
        transform.translation = runtime.player.position;
        transform.rotation = rotation;
    }
    let proximity = runtime.world.candle_proximity(runtime.player.cell);
    if let Ok((mut transform, mut light)) = candle.single_mut() {
        let forward = rotation * Vec3::NEG_Z;
        let right = rotation * Vec3::X;
        transform.translation =
            runtime.player.position + forward * 0.35 + right * 0.28 - Vec3::Y * 0.28;
        light.intensity = 180.0 + proximity.powf(1.6) * 2_300.0;
        light.range = 8.0 + proximity * 11.0;
    }
}

pub(super) fn sync_hud(runtime: Res<FullWfcRuntime>, mut hud: Query<&mut Text, With<FullWfcHud>>) {
    if !runtime.is_changed() {
        return;
    }
    let Ok(mut text) = hud.single_mut() else {
        return;
    };
    let proximity = runtime.world.candle_proximity(runtime.player.cell);
    let observed = runtime
        .world
        .observation
        .visible_thresholds
        .iter()
        .next()
        .map_or("none".to_string(), |key| format!("{:?}", key.face));
    let exit_rule = runtime
        .world
        .exit_claim
        .as_ref()
        .map_or("exit unclaimed".to_string(), |claim| {
            format!("exit chain claimed by {}", claim.owner.label())
        });
    let space = runtime.world.placements[&runtime.player.cell].space;
    **text = format!(
        "GENERATION {}  |  pulse in {:.1}s\nlevel {}  |  {:?} ({}, {})  |  candle {:.0}%\nobserved threshold: {}  |  {}\n{}{}",
        runtime.world.generation,
        runtime.ticks_until_pulse as f32 / 60.0,
        runtime.player.cell.level,
        space,
        runtime.player.cell.x,
        runtime.player.cell.z,
        proximity * 100.0,
        observed,
        exit_rule,
        runtime.status,
        if runtime.escaped {
            "\n\nEXIT REACHED - Esc returns to the menu"
        } else {
            ""
        }
    );
}

pub(super) fn clear_view(
    mut commands: Commands,
    camera: Query<Entity, With<GameCam>>,
    mut sun: Query<&mut DirectionalLight, With<GameSun>>,
) {
    if let Ok(camera) = camera.single() {
        commands
            .entity(camera)
            .remove::<(Hdr, Bloom, DistanceFog)>();
    }
    for mut light in &mut sun {
        light.illuminance = MENU_SUN_ILLUMINANCE;
    }
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.7, 0.74, 0.85),
        brightness: 900.0,
        ..default()
    });
    commands.remove_resource::<FullWfcVisualAssets>();
}
