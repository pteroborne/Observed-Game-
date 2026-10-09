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
    let pieces = geometry.pieces_in_cell(at).collect::<Vec<_>>();
    let key = crate::hex_wfc::view::shell::cell_mesh_key(&pieces, at);
    let shell = commands
        .spawn((
            ExteriorShell(at),
            Transform::from_translation(Vec3::from_array(hex_origin(at))),
            Visibility::default(),
            DespawnOnExit(GameState::HexWfc),
            Name::new(format!("Projected hall exterior {at:?}")),
        ))
        .id();
    for (group, hulls) in groups {
        let refs = hulls.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let Some(mesh) = assets.merged_mesh_for(meshes, key.as_deref(), group, &refs) else {
            continue;
        };
        let surface = if matches!(group, MeshGroupKey::Floor | MeshGroupKey::Ceiling) {
            MeshGroupKey::Roof
        } else {
            MeshGroupKey::Facade
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
