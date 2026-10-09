//! Initial kits and ordinary flat halls share authoritative exterior hulls.
//! Keep their silhouette, door openings and roof gaps without far-field dressing,
//! fixtures or shadow casters. Complex modules retain the procedural skin.
use super::{ExteriorShell, MeshGroupKey};
use bevy::{light::NotShadowCaster, prelude::*};
use observed_content::ArchitectureRegister;
use observed_hex::{HexCoord, hex_origin};
use observed_match::hex_wfc::HexWfcGeometrySnapshot;
use std::collections::BTreeMap;

use crate::{
    GameState,
    hex_wfc::view::{HexWfcVisualAssets, NeverShadowCaster, support::points},
};

type HullGroups = BTreeMap<MeshGroupKey, Vec<Vec<Vec3>>>;

fn hulls(geometry: &HexWfcGeometrySnapshot, at: HexCoord) -> HullGroups {
    let origin = Vec3::from_array(hex_origin(at));
    let mut groups = HullGroups::new();
    for piece in geometry
        .pieces_in_cell(at)
        .filter(|piece| piece.source_cell == at)
    {
        let group = MeshGroupKey::for_piece(piece);
        if group == MeshGroupKey::Hidden {
            continue;
        }
        let hull = points(piece, origin);
        if !hull.is_empty() {
            groups.entry(group).or_default().push(hull);
        }
    }
    groups
}

pub(super) fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    assets: &mut HexWfcVisualAssets,
    geometry: &HexWfcGeometrySnapshot,
    at: HexCoord,
    register: ArchitectureRegister,
) -> Option<Entity> {
    let groups = hulls(geometry, at);
    if groups.is_empty() {
        return None;
    }
    let covered = groups
        .iter()
        .filter(|(group, _)| matches!(group, MeshGroupKey::Floor | MeshGroupKey::Ceiling))
        .flat_map(|(_, hulls)| hulls.iter().cloned())
        .collect::<Vec<_>>();
    let covered_refs = covered.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let pieces = geometry.pieces_in_cell(at).collect::<Vec<_>>();
    let key = crate::hex_wfc::view::shell::cell_mesh_key(&pieces);
    let shell = commands
        .spawn((
            ExteriorShell(at),
            Transform::from_translation(Vec3::from_array(hex_origin(at))),
            Visibility::default(),
            DespawnOnExit(GameState::HexWfc),
            Name::new(format!("Projected hall exterior {at:?}")),
        ))
        .id();
    // Far presentation has only two finishes. Joining their geometry keeps
    // doors/roof gaps exact while avoiding a child draw for every near material.
    let mut batches = HullGroups::new();
    for (group, hulls) in groups {
        let surface = if matches!(group, MeshGroupKey::Floor | MeshGroupKey::Ceiling) {
            MeshGroupKey::Roof
        } else {
            MeshGroupKey::Facade
        };
        batches.entry(surface).or_default().extend(hulls);
    }
    let recipe = format!("proxy/{key:?}");
    for (surface, hulls) in batches {
        let refs = hulls.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let occluders = if surface == MeshGroupKey::Facade {
            covered_refs.as_slice()
        } else {
            &[]
        };
        let Some(mesh) =
            assets.merged_mesh_for_owned(meshes, Some(&recipe), surface, &refs, occluders)
        else {
            continue;
        };
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(assets.material_for_group(register, surface)),
            Transform::IDENTITY,
            ChildOf(shell),
            NotShadowCaster,
            NeverShadowCaster,
        ));
    }
    Some(shell)
}

#[cfg(test)]
mod tests;
