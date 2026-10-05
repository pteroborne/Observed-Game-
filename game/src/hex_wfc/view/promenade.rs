//! Render authoritative Sky hulls with mineral slabs, thin titanium edges and fixed guide lights.
use super::{
    archive::cuboid,
    assets::{HexWfcVisualAssets, MeshGroupKey},
    spectate::Cutaway,
};
use crate::{GameState, hex_wfc::sim::HexWfcRuntime};
use bevy::prelude::*;
use observed_hex::{HexCoord, HexFace, hex_origin};
use observed_match::hex_wfc::{HexPiecePart, HexStructurePiece, HexStructureRole};
use observed_traversal::ColliderShape;
use std::hash::{Hash, Hasher};
#[derive(Component)]
struct PromenadePanel(HexCoord);
pub(in crate::hex_wfc) fn install(app: &mut App) {
    app.add_systems(PostUpdate, sync_power.run_if(in_state(GameState::HexWfc)));
}
fn sync_power(
    runtime: Res<HexWfcRuntime>,
    assets: Res<HexWfcVisualAssets>,
    mut panels: Query<(&PromenadePanel, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (panel, mut material) in &mut panels {
        let powered = runtime
            .ascent
            .as_ref()
            .is_none_or(|a| a.rules().economy.is_powered(panel.0.level));
        let wanted = assets.promenade_material(if powered { 4 } else { 6 });
        if material.0 != wanted {
            material.0 = wanted;
        }
    }
}
fn turn(mut p: Vec3, heading: HexFace) -> Vec3 {
    for _ in 0..heading.index() {
        p = Vec3::new(0.5 * p.x - 0.875 * p.z, p.y, 6.0 / 7.0 * p.x + 0.5 * p.z);
    }
    p
}
pub(super) fn spawn(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    coord: HexCoord,
    heading: HexFace,
    pieces: &[&HexStructurePiece],
) -> usize {
    let mut groups: [Vec<Vec<Vec3>>; 6] = std::array::from_fn(|_| Vec::new());
    for piece in pieces {
        if !matches!(piece.part, HexPiecePart::Authored | HexPiecePart::Window)
            || !matches!(piece.role, HexStructureRole::Hall | HexStructureRole::Room)
        {
            continue;
        }
        let ColliderShape::ConvexHull { points } = &piece.shape else {
            continue;
        };
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for point in points {
            let p = turn(*point, HexFace::LATERAL[(6 - heading.index()) % 6]);
            min = min.min(p);
            max = max.max(p);
        }
        let size = max - min;
        let role = if max.y <= 2.51 {
            0
        } else if min.y >= 5.4 {
            5
        } else if max.y > 4.5 && (size.x < 0.7 || size.z < 0.7) {
            2
        } else {
            1
        };
        groups[role].push(points.clone());
    }
    // Flush edge courses follow the actual narrow floor. These finishes never
    // cover a gap or add an invisible collision barrier to the exposed lip.
    for h in [0, 1] {
        let bearing = HexFace::LATERAL[(heading.index() + h) % 6];
        for z in [-1.29, 1.29] {
            groups[3].push(cuboid(
                Vec3::new(4.8, 2.50, z),
                Vec3::new(4.4, 0.16, 0.05),
                Vec3::X,
                bearing,
            ));
            groups[4].push(cuboid(
                Vec3::new(4.8, 2.512, z),
                Vec3::new(4.4, 0.008, 0.035),
                Vec3::X,
                bearing,
            ));
        }
    }
    // Supported portal header and canopy carry the light panels. The open sky
    // and open floor remain actual absences in the authoritative geometry.
    groups[4].push(cuboid(
        Vec3::new(5.375, 5.495, 0.0),
        Vec3::new(0.31, 0.009, 3.55),
        Vec3::X,
        heading,
    ));
    let entry = HexFace::LATERAL[(heading.index() + 4) % 6];
    for x in [3.8, 5.8] {
        groups[4].push(cuboid(
            Vec3::new(x, 5.735, 0.0),
            Vec3::new(0.25, 0.009, 3.85),
            Vec3::X,
            entry,
        ));
        groups[5].push(cuboid(
            Vec3::new(x, 5.755, 0.0),
            Vec3::new(0.31, 0.022, 3.90),
            Vec3::X,
            entry,
        ));
    }
    // Shallow metal bases and caps stay on the two physical portal uprights.
    for z in [-1.75, 1.75] {
        for y in [0.56, 5.45] {
            groups[2].push(cuboid(
                Vec3::new(5.375, y, z),
                Vec3::new(0.26, 0.10, 0.26),
                Vec3::X,
                heading,
            ));
        }
    }
    // The central gathering landing has a flush mineral disk, not a beacon or
    // an exit socket. Its radial joints make the waiting place easy to read.
    for i in 0..6 {
        let bearing = HexFace::LATERAL[(heading.index() + i) % 6];
        groups[1].push(cuboid(
            Vec3::new(1.60, 2.509, 0.0),
            Vec3::new(1.60, 0.009, 0.018),
            Vec3::X,
            bearing,
        ));
    }
    let origin = Vec3::from_array(hex_origin(coord));
    let mut count = 0;
    for (role, hulls) in groups.iter().enumerate() {
        let mut hash = std::hash::DefaultHasher::new();
        for hull in hulls {
            for p in hull {
                p.to_array().map(f32::to_bits).hash(&mut hash);
            }
        }
        let key = format!("promenade-{role}-{}-{:x}", heading.index(), hash.finish());
        let refs: Vec<_> = hulls.iter().map(Vec::as_slice).collect();
        let Some(mesh) = assets.merged_mesh_for(meshes, Some(&key), MeshGroupKey::Interior, &refs)
        else {
            continue;
        };
        let mut entity = commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(assets.promenade_material(role)),
            Transform::from_translation(origin),
            ChildOf(parent),
            Cutaway {
                local: Vec3::ZERO,
                min_y: 0.5,
                max_y: 7.75,
                origin_y: origin.y,
                cell_level: coord.level,
                climb_wall: false,
            },
            Name::new("Last Promenade bridges and finish"),
        ));
        if role == 4 {
            entity.insert(PromenadePanel(coord));
        }
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests;
