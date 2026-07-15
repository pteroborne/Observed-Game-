use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use observed_facility::full_wfc::{
    CellCoord, ModuleFace, ModulePlacement, ModuleSpace, ThresholdKey,
};
use observed_match::full_wfc::{
    CELL_SIZE, LEVEL_HEIGHT, SHAFT_OPENING, StructurePiece, StructureRole, WALL_HEIGHT,
    WALL_THICKNESS,
};
use observed_style::{MarkerRole, ObservedState, SurfaceRole};

use super::sim::{EYE_OFFSET, FullWfcRuntime};
use crate::GameState;
use crate::view::components::{GameCam, GameSun, MENU_SUN_ILLUMINANCE};

#[derive(Component)]
pub(super) struct FullWfcCell(CellCoord);

#[derive(Component)]
pub(super) struct CandleLight;

#[derive(Component)]
pub(super) struct ThresholdSignal(ThresholdKey);

type CameraLightFilter = (With<GameCam>, Without<CandleLight>);

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

    for placement in runtime.match_state.facility.placements.values() {
        spawn_cell(
            &mut commands,
            &assets,
            placement,
            runtime.match_state.facility.exit(),
            &runtime.match_state.geometry.pieces,
        );
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
    commands.insert_resource(ClearColor(Color::srgb(0.002, 0.006, 0.015)));

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
    pieces: &[StructurePiece],
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
            for piece in pieces.iter().filter(|piece| piece.cell == placement.coord) {
                let material = match piece.role {
                    StructureRole::Floor if placement.space == ModuleSpace::Room => {
                        assets.room.clone()
                    }
                    StructureRole::Floor => assets.hall.clone(),
                    StructureRole::Ceiling => assets.ceiling.clone(),
                    StructureRole::Wall | StructureRole::Feature => assets.wall.clone(),
                };
                spawn_cube(
                    cell,
                    assets,
                    material,
                    piece.half * 2.0,
                    piece.center - center,
                );
            }
            for face in [
                ModuleFace::East,
                ModuleFace::West,
                ModuleFace::South,
                ModuleFace::North,
            ] {
                if placement.space == ModuleSpace::Room && placement.is_open(face) {
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
                    if placement.is_open(face) {
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
        if let Some(placement) = runtime.match_state.facility.placement(coord) {
            spawn_cell(
                &mut commands,
                &assets,
                placement,
                runtime.match_state.facility.exit(),
                &runtime.match_state.geometry.pieces,
            );
        }
    }
}

pub(super) fn sync_streamed_cells(
    runtime: Res<FullWfcRuntime>,
    mut cells: Query<(&FullWfcCell, &mut Visibility)>,
) {
    let center = runtime.local().cell;
    for (cell, mut visibility) in &mut cells {
        *visibility = if cell_is_streamed(center, cell.0) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn cell_is_streamed(center: CellCoord, candidate: CellCoord) -> bool {
    center.x.abs_diff(candidate.x) <= 3
        && center.z.abs_diff(candidate.z) <= 3
        && center.level.abs_diff(candidate.level) <= 1
}

pub(super) fn sync_threshold_signals(
    runtime: Res<FullWfcRuntime>,
    assets: Res<FullWfcVisualAssets>,
    mut signals: Query<(&ThresholdSignal, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (signal, mut material) in &mut signals {
        let sealed = terminal_face_is_reserved(&runtime, signal.0);
        let anchored = runtime
            .match_state
            .equipment
            .anchors_in(signal.0.room)
            .next()
            .is_some();
        material.0 = if sealed {
            assets.threshold_sealed.clone()
        } else if anchored {
            assets.threshold_observed.clone()
        } else {
            assets.threshold_unobserved.clone()
        };
    }
}

fn terminal_face_is_reserved(runtime: &FullWfcRuntime, threshold: ThresholdKey) -> bool {
    let world = &runtime.match_state.facility;
    if threshold.room == world.exit() {
        return world.reserved_exit_faces.contains(&threshold.face);
    }
    runtime
        .match_state
        .facility
        .config
        .neighbor(threshold.room, threshold.face)
        .is_some_and(|next| {
            next == world.exit()
                && runtime
                    .match_state
                    .facility
                    .reserved_exit_faces
                    .contains(&threshold.face.opposite())
        })
}

pub(super) fn sync_camera_and_candle(
    runtime: Res<FullWfcRuntime>,
    mut camera: Query<(&mut Transform, &mut DistanceFog), CameraLightFilter>,
    mut candle: Query<(&mut Transform, &mut PointLight), With<CandleLight>>,
) {
    let player = runtime.local();
    let rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);
    let pressure = runtime.match_state.guardian_pressure(runtime.local_player);
    if let Ok((mut transform, mut fog)) = camera.single_mut() {
        transform.translation = player.position + Vec3::Y * EYE_OFFSET;
        transform.rotation = rotation;
        fog.falloff = FogFalloff::Linear {
            start: 16.0 - pressure * 8.0,
            end: 42.0 - pressure * 25.0,
        };
    }
    let proximity = runtime.match_state.facility.candle_proximity(player.cell);
    let warning = runtime.match_state.mutation_warning_progress();
    if let Ok((mut transform, mut light)) = candle.single_mut() {
        let forward = rotation * Vec3::NEG_Z;
        let right = rotation * Vec3::X;
        transform.translation =
            player.position + Vec3::Y * EYE_OFFSET + forward * 0.35 + right * 0.28 - Vec3::Y * 0.28;
        let guardian_flicker = if pressure > 0.0 && runtime.match_state.tick % 17 < 3 {
            0.35
        } else {
            1.0
        };
        let breathing = 1.0 + (warning * std::f32::consts::TAU * 2.0).sin().abs() * 0.28;
        let mutation_cut = if runtime.match_state.recent_events.iter().any(|event| {
            event.kind == observed_match::full_wfc::GameplayEventKind::MutationCommitted
        }) {
            0.08
        } else {
            1.0
        };
        light.intensity = (180.0 + proximity.powf(1.6) * 2_300.0)
            * (1.0 - pressure * 0.62)
            * guardian_flicker
            * breathing
            * mutation_cut;
        light.range = (8.0 + proximity * 11.0) * (1.0 - pressure * 0.55);
    }
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
    commands.insert_resource(ClearColor(Color::srgb(0.045, 0.05, 0.065)));
    commands.remove_resource::<FullWfcVisualAssets>();
}
