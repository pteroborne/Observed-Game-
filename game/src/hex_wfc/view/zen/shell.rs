//! Ordinary Zen's real hulls: broad screens are paper, posts and joinery cedar.
use super::super::{
    assets::{HexWfcVisualAssets, MeshGroupKey},
    spectate::Cutaway,
    support::points,
};
use bevy::prelude::*;
use observed_hex::{HexCoord, hex_origin};
use observed_match::hex_wfc::{HexStructurePiece, HexStructureRole};
use std::collections::BTreeMap;

pub(super) fn finish(piece: &HexStructurePiece, hull: &[Vec3]) -> usize {
    let lo = hull
        .iter()
        .copied()
        .fold(Vec3::splat(f32::INFINITY), Vec3::min);
    let hi = hull
        .iter()
        .copied()
        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
    let plan = observed_traversal::plan_convex_hull(hull);
    let width = (0..plan.len())
        .map(|i| plan[i].distance(plan[(i + 1) % plan.len()]))
        .fold(0.0, f32::max);
    usize::from(
        matches!(piece.role, HexStructureRole::Hall | HexStructureRole::Room)
            && hi.y - lo.y > 1.6
            && lo.y < 4.5
            && width > 1.6,
    )
}

pub(super) fn spawn(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    coord: HexCoord,
    pieces: &[&HexStructurePiece],
) -> usize {
    let origin = Vec3::from_array(hex_origin(coord));
    let mut groups: BTreeMap<(MeshGroupKey, usize), Vec<Vec<Vec3>>> = BTreeMap::new();
    for piece in pieces {
        let key = MeshGroupKey::for_piece(piece);
        if !matches!(key, MeshGroupKey::Interior | MeshGroupKey::Perimeter(_)) {
            continue;
        }
        let hull = points(piece, origin);
        if hull.is_empty() {
            continue;
        }
        groups
            .entry((key, finish(piece, &hull)))
            .or_default()
            .push(hull);
    }
    let mut count = 0;
    for ((group, role), hulls) in groups {
        let refs: Vec<_> = hulls.iter().map(Vec::as_slice).collect();
        let Some(mesh) = assets.merged_mesh_for(
            meshes,
            Some(&format!("zen-shell-{}", super::mesh_key(&hulls))),
            group,
            &refs,
        ) else {
            continue;
        };
        let min_y = hulls
            .iter()
            .flatten()
            .map(|p| p.y)
            .fold(f32::INFINITY, f32::min);
        let max_y = hulls
            .iter()
            .flatten()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max);
        let local = hulls.iter().flatten().copied().sum::<Vec3>()
            / hulls.iter().map(Vec::len).sum::<usize>() as f32;
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(assets.rain_material(role)),
            Transform::from_translation(origin),
            ChildOf(parent),
            Cutaway {
                local,
                min_y,
                max_y,
                origin_y: origin.y,
                cell_level: coord.level,
                climb_wall: false,
            },
            Name::new(format!("Zen {:?} finish {role}", group)),
        ));
        count += 1;
    }
    count
}
