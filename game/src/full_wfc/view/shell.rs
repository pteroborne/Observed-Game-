use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::full_wfc::{
    CellCoord, FullWfcWorld, ModuleFace, ModulePlacement, ModuleSpace, ThresholdKey,
};
use observed_match::full_wfc::{
    CELL_SIZE, LEVEL_HEIGHT, SHAFT_OPENING, StructurePiece, StructureRole, THRESHOLD_CLEAR_HEIGHT,
    THRESHOLD_WIDTH, WALL_HEIGHT, WALL_THICKNESS,
};

use super::assets::FullWfcVisualAssets;
use super::registers::{spawn_register_dressing, thinning_detail};
use super::{CellPractical, FullWfcCell, ThresholdSignal};
use crate::GameState;

#[derive(Component, Clone, Copy)]
pub(in crate::full_wfc) struct ImportedThresholdDressing {
    kind: ImportedThresholdKind,
    register: ArchitectureRegister,
}

#[derive(Clone, Copy)]
enum ImportedThresholdKind {
    Gate,
    Cables,
}

#[derive(Component)]
pub(in crate::full_wfc) struct ImportedMaterialNormalized;

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_cell(
    commands: &mut Commands,
    assets: &mut FullWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    placement: &ModulePlacement,
    world: &FullWfcWorld,
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
                "Full WFC {:?} {:?} {:?} / {}",
                placement.space,
                placement.archetype,
                placement.coord,
                placement.architecture.slug(),
            )),
        ))
        .with_children(|cell| {
            let register = assets.register(placement.architecture).clone();
            for piece in pieces.iter().filter(|piece| piece.cell == placement.coord) {
                let material = match piece.role {
                    StructureRole::Floor if placement.space == ModuleSpace::Room => {
                        register.room_floor.clone()
                    }
                    StructureRole::Floor => register.hall_floor.clone(),
                    StructureRole::Ceiling => register.ceiling.clone(),
                    StructureRole::Wall | StructureRole::Feature => register.wall.clone(),
                };
                cell.spawn((
                    Mesh3d(assets.mesh_for(meshes, piece.half * 2.0)),
                    MeshMaterial3d(material),
                    Transform::from_translation(piece.center - center),
                    Name::new(format!("Canonical {:?} collider shell", piece.role)),
                ));
            }
            for face in horizontal_faces() {
                if placement.space == ModuleSpace::Room && placement.is_open(face) {
                    spawn_threshold_frame(
                        cell,
                        assets,
                        meshes,
                        ThresholdKey {
                            room: placement.coord,
                            face,
                        },
                        placement.architecture,
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
            spawn_register_dressing(cell, assets, meshes, world, placement);
            spawn_practical(cell, assets, meshes, placement, world);
            if placement.coord == exit {
                spawn_exit_beacon(cell, assets, meshes);
            }
        });
}

fn horizontal_faces() -> [ModuleFace; 4] {
    [
        ModuleFace::East,
        ModuleFace::West,
        ModuleFace::South,
        ModuleFace::North,
    ]
}

fn spawn_practical(
    parent: &mut ChildSpawnerCommands,
    assets: &mut FullWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    placement: &ModulePlacement,
    world: &FullWfcWorld,
) {
    let register = assets.register(placement.architecture).clone();
    let vertical = placement.is_open(ModuleFace::Up) || placement.is_open(ModuleFace::Down);
    let position = if vertical {
        Vec3::new(3.8, WALL_HEIGHT - 0.25, 0.0)
    } else {
        Vec3::new(0.0, WALL_HEIGHT - 0.25, 0.0)
    };
    parent.spawn((
        Mesh3d(assets.mesh_for(meshes, Vec3::new(1.25, 0.12, 0.7))),
        MeshMaterial3d(register.fixture.clone()),
        Transform::from_translation(position),
        Name::new("cell practical fixture"),
    ));
    parent.spawn((
        CellPractical {
            cell: placement.coord,
            architecture: placement.architecture,
            phase: (placement.instance.key % 6_283) as f32 * 0.001,
            detail: if placement.architecture == ArchitectureRegister::Thinning {
                thinning_detail(world, placement.coord)
            } else {
                1.0
            },
        },
        PointLight {
            intensity: 0.0,
            range: 10.5,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(position - Vec3::Y * 0.42),
        Name::new("budgeted cell practical light"),
    ));
}

fn spawn_threshold_frame(
    parent: &mut ChildSpawnerCommands,
    assets: &mut FullWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    key: ThresholdKey,
    register: ArchitectureRegister,
) {
    let (translation, rotation) = threshold_transform(key.face);
    if let Some(gate) = assets.threshold_gate.clone() {
        parent.spawn((
            ImportedThresholdDressing {
                kind: ImportedThresholdKind::Gate,
                register,
            },
            SceneRoot(gate.scene),
            Transform::from_translation(translation)
                .with_rotation(rotation)
                .with_scale(kenney_gate_scale(gate.scale)),
            Name::new("Kenney CC0 full-WFC threshold gate"),
        ));
    } else {
        spawn_procedural_frame(parent, assets, meshes, key.face, register);
    }

    let register_materials = assets.register(register).clone();
    let half = THRESHOLD_WIDTH * 0.5;
    if let Some(cables) = assets.cable_bundle.clone() {
        let along = rotation * Vec3::X;
        parent.spawn((
            ImportedThresholdDressing {
                kind: ImportedThresholdKind::Cables,
                register,
            },
            SceneRoot(cables.scene),
            Transform::from_translation(translation + along * (half + 0.55) - Vec3::Y * 0.02)
                .with_rotation(rotation)
                .with_scale(Vec3::splat(cables.scale.min(0.55))),
            Name::new("Kenney CC0 full-WFC threshold cables"),
        ));
    } else {
        let along = rotation * Vec3::X;
        for offset in [-0.10, 0.0, 0.10] {
            parent.spawn((
                Mesh3d(assets.mesh_for(meshes, Vec3::new(0.07, 2.0, 0.07))),
                MeshMaterial3d(register_materials.dark.clone()),
                Transform::from_translation(
                    translation + along * (half + 0.42 + offset) + Vec3::Y * 1.0,
                ),
                Name::new("procedural threshold cable fallback"),
            ));
        }
    }

    // The imported frame carries silhouette; this separate bar is the semantic
    // indicator. Observation never changes it.
    let indicator_size = Vec3::new(1.1, 0.10, 0.10);
    parent.spawn((
        ThresholdSignal(key),
        Mesh3d(assets.mesh_for(meshes, indicator_size)),
        MeshMaterial3d(assets.threshold(observed_style::ThresholdFrameState::Mutable)),
        Transform::from_translation(translation + Vec3::Y * (THRESHOLD_CLEAR_HEIGHT - 0.16))
            .with_rotation(rotation),
        Name::new("mutable threshold indicator"),
    ));
}

fn spawn_procedural_frame(
    parent: &mut ChildSpawnerCommands,
    assets: &mut FullWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    face: ModuleFace,
    register: ArchitectureRegister,
) {
    let (translation, rotation) = threshold_transform(face);
    let material = assets.register(register).wall.clone();
    let post_size = Vec3::new(0.22, THRESHOLD_CLEAR_HEIGHT, 0.30);
    for x in [
        -THRESHOLD_WIDTH * 0.5 - post_size.x * 0.5,
        THRESHOLD_WIDTH * 0.5 + post_size.x * 0.5,
    ] {
        parent.spawn((
            Mesh3d(assets.mesh_for(meshes, post_size)),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(
                translation + rotation * Vec3::new(x, post_size.y * 0.5, 0.0),
            )
            .with_rotation(rotation),
            Name::new("procedural threshold post fallback"),
        ));
    }
    let lintel_size = Vec3::new(THRESHOLD_WIDTH + 0.66, 0.30, 0.30);
    parent.spawn((
        Mesh3d(assets.mesh_for(meshes, lintel_size)),
        MeshMaterial3d(material),
        Transform::from_translation(
            translation + Vec3::Y * (THRESHOLD_CLEAR_HEIGHT + lintel_size.y * 0.5),
        )
        .with_rotation(rotation),
        Name::new("procedural threshold lintel fallback"),
    ));
}

fn threshold_transform(face: ModuleFace) -> (Vec3, Quat) {
    let boundary = CELL_SIZE * 0.5 - WALL_THICKNESS * 0.5;
    match face {
        ModuleFace::North => (Vec3::new(0.0, 0.0, -boundary), Quat::IDENTITY),
        ModuleFace::South => (
            Vec3::new(0.0, 0.0, boundary),
            Quat::from_rotation_y(std::f32::consts::PI),
        ),
        ModuleFace::East => (
            Vec3::new(boundary, 0.0, 0.0),
            Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
        ),
        ModuleFace::West => (
            Vec3::new(-boundary, 0.0, 0.0),
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        ),
        ModuleFace::Up | ModuleFace::Down => unreachable!("horizontal threshold"),
    }
}

fn kenney_gate_scale(authored_scale: f32) -> Vec3 {
    const NATIVE_WIDTH: f32 = 4.2;
    const NATIVE_HEIGHT: f32 = 4.620_71;
    const NATIVE_DEPTH: f32 = 1.4;
    const MAX_DEPTH: f32 = 0.5;
    Vec3::new(
        authored_scale.min(THRESHOLD_WIDTH / NATIVE_WIDTH),
        authored_scale.min(THRESHOLD_CLEAR_HEIGHT / NATIVE_HEIGHT),
        authored_scale.min(MAX_DEPTH / NATIVE_DEPTH),
    )
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
        Mesh3d(assets.vertical_ring.clone()),
        MeshMaterial3d(assets.threshold(observed_style::ThresholdFrameState::Mutable)),
        Transform::from_xyz(0.0, y, 0.0),
        Name::new(format!(
            "vertical threshold ring ({SHAFT_OPENING:.1}m opening)"
        )),
    ));
}

fn spawn_exit_beacon(
    parent: &mut ChildSpawnerCommands,
    assets: &mut FullWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
) {
    parent.spawn((
        Mesh3d(assets.mesh_for(meshes, Vec3::new(1.1, 3.6, 1.1))),
        MeshMaterial3d(assets.exit.clone()),
        Transform::from_xyz(0.0, 1.8, 0.0),
        Name::new("exit beacon"),
    ));
    parent.spawn((
        PointLight {
            color: observed_style::marker(observed_style::MarkerRole::Exit).base_color,
            intensity: 2_200.0,
            range: 18.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 2.5, 0.0),
        Name::new("exit beacon light"),
    ));
}

pub(in crate::full_wfc) fn normalize_imported_materials(
    mut commands: Commands,
    assets: Res<FullWfcVisualAssets>,
    roots: Query<&ImportedThresholdDressing>,
    parents: Query<&ChildOf>,
    imported: Query<Entity, (Added<Mesh3d>, Without<ImportedMaterialNormalized>)>,
) {
    for entity in &imported {
        let mut ancestor = entity;
        let mut dressing = None;
        for _ in 0..16 {
            if let Ok(root) = roots.get(ancestor) {
                dressing = Some(*root);
                break;
            }
            let Ok(parent) = parents.get(ancestor) else {
                break;
            };
            ancestor = parent.parent();
        }
        let Some(dressing) = dressing else {
            continue;
        };
        let register = assets.register(dressing.register);
        let material = match dressing.kind {
            ImportedThresholdKind::Gate => register.wall.clone(),
            ImportedThresholdKind::Cables => register.dark.clone(),
        };
        commands.entity(entity).insert((
            ImportedMaterialNormalized,
            MeshMaterial3d(material),
            Name::new("CC0 full-WFC threshold mesh normalized through observed_style"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kenney_gate_fits_canonical_aperture() {
        let scale = kenney_gate_scale(1.35);
        assert!(4.2 * scale.x <= THRESHOLD_WIDTH + f32::EPSILON);
        assert!(4.620_71 * scale.y <= THRESHOLD_CLEAR_HEIGHT + f32::EPSILON);
        assert!(1.4 * scale.z <= 0.5 + f32::EPSILON);
    }
}
