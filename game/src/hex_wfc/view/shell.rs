//! Instances the authoritative hex geometry snapshot: one render mesh per collider
//! piece, district-tinted by role and source-cell architecture. Tile pieces are grouped
//! under a per-cell parent so visibility streaming can hide distant cells wholesale; the
//! outer boundary shell is always visible.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::HexCoord;
use observed_match::hex_wfc::{HexStructurePiece, HexStructureRole};

use super::assets::HexWfcVisualAssets;
use super::{HexWfcCell, HexWfcGeometry};
use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;

pub(super) fn spawn_geometry(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    runtime: &HexWfcRuntime,
) {
    let world = &runtime.match_state.facility;
    let fallback_arch = *world
        .architecture
        .get(&world.config.spawn())
        .unwrap_or(&ArchitectureRegister::ALL[0]);

    let mut by_cell: BTreeMap<HexCoord, Vec<&HexStructurePiece>> = BTreeMap::new();
    for piece in &runtime.match_state.geometry.pieces {
        if piece.role == HexStructureRole::Boundary {
            spawn_piece(commands, assets, meshes, piece, fallback_arch, None, 0);
        } else {
            by_cell.entry(piece.source_cell).or_default().push(piece);
        }
    }

    for (coord, pieces) in by_cell {
        spawn_cell(
            commands,
            assets,
            meshes,
            coord,
            pieces,
            world,
            fallback_arch,
        );
    }
}

pub(super) fn spawn_cells(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    runtime: &HexWfcRuntime,
    changed: &BTreeSet<HexCoord>,
) {
    let world = &runtime.match_state.facility;
    let fallback_arch = *world
        .architecture
        .get(&world.config.spawn())
        .unwrap_or(&ArchitectureRegister::ALL[0]);
    let mut by_cell: BTreeMap<HexCoord, Vec<&HexStructurePiece>> = BTreeMap::new();
    for piece in runtime.match_state.geometry.pieces.iter().filter(|piece| {
        piece.role != HexStructureRole::Boundary && changed.contains(&piece.source_cell)
    }) {
        by_cell.entry(piece.source_cell).or_default().push(piece);
    }
    for (coord, pieces) in by_cell {
        spawn_cell(
            commands,
            assets,
            meshes,
            coord,
            pieces,
            world,
            fallback_arch,
        );
    }
}

fn spawn_cell(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    coord: HexCoord,
    pieces: Vec<&HexStructurePiece>,
    world: &observed_facility::hex_wfc::HexWfcWorld,
    fallback_arch: ArchitectureRegister,
) {
    let architecture = *world.architecture.get(&coord).unwrap_or(&fallback_arch);
    let cell = commands
        .spawn((
            HexWfcCell(coord),
            HexWfcGeometry,
            DespawnOnExit(GameState::HexWfc),
            Transform::IDENTITY,
            Visibility::default(),
            Name::new(format!(
                "Hex cell q{} r{} L{}",
                coord.q, coord.r, coord.level
            )),
        ))
        .id();
    for (index, piece) in pieces.into_iter().enumerate() {
        spawn_piece(
            commands,
            assets,
            meshes,
            piece,
            architecture,
            Some(cell),
            index,
        );
    }
}

fn spawn_piece(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    piece: &HexStructurePiece,
    architecture: ArchitectureRegister,
    parent: Option<Entity>,
    hull_index: usize,
) {
    let Some(mesh) = assets.mesh_for(meshes, piece, hull_index) else {
        return;
    };
    let material = assets.register(architecture).for_role(piece.role);
    let mut entity = commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(piece.center).with_rotation(Quat::from_array(piece.rotation)),
        Name::new(format!("Hex {:?} collider {}", piece.role, piece.id.0)),
    ));
    if let Some(parent) = parent {
        entity.insert(ChildOf(parent));
    } else {
        entity.insert((HexWfcGeometry, DespawnOnExit(GameState::HexWfc)));
    }
}
