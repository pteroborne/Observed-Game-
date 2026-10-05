//! Render authoritative concourse hulls and bounded ceramic, rib and platform detail.
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
struct ConcoursePanel(HexCoord);
pub(in crate::hex_wfc) fn install(app: &mut App) {
    app.add_systems(PostUpdate, sync_power.run_if(in_state(GameState::HexWfc)));
}
fn sync_power(
    runtime: Res<HexWfcRuntime>,
    assets: Res<HexWfcVisualAssets>,
    mut panels: Query<(&ConcoursePanel, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (panel, mut material) in &mut panels {
        let powered = runtime
            .ascent
            .as_ref()
            .is_none_or(|a| a.rules().economy.is_powered(panel.0.level));
        let wanted = assets.concourse_material(if powered { 4 } else { 6 });
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
fn polygon(points: &[(f32, f32)], y: f32, heading: HexFace) -> Vec<Vec3> {
    [y, y + 0.009]
        .into_iter()
        .flat_map(|h| {
            points
                .iter()
                .map(move |&(x, z)| turn(Vec3::new(x, h, z), heading))
        })
        .collect()
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
        let role = if max.y <= 0.51 {
            0
        } else if max.y < 2.0 {
            5
        } else {
            1
        };
        groups[role].push(points.clone());
    }
    // Dark track-like insets are finish on the solid floor, never imaginary pits.
    groups[2].push(polygon(
        &[(-4.35, 0.7), (-3.35, 0.7), (-1.7, 5.75), (-2.5, 5.3)],
        0.507,
        heading,
    ));
    for offset in [0.0, 0.34, 0.68] {
        groups[5].push(polygon(
            &[
                (-4.25 + offset, 0.75),
                (-4.20 + offset, 0.75),
                (-2.0 + offset * 0.2, 5.50),
                (-2.05 + offset * 0.2, 5.50),
            ],
            0.520,
            heading,
        ));
    }
    // A narrow ochre platform edge reads at eye level without competing with cues.
    groups[3].push(polygon(
        &[(-3.23, 0.68), (-3.11, 0.68), (-1.42, 5.7), (-1.54, 5.7)],
        0.516,
        heading,
    ));
    // Broad ceiling coffers and fine white cross-bars sit behind the real ribs.
    for (x, z, w, d) in [
        (-2.8, -1.5, 3.2, 2.2),
        (2.8, -0.6, 3.2, 2.6),
        (1.0, 5.3, 3.0, 1.3),
    ] {
        groups[4].push(cuboid(
            Vec3::new(x, 7.67, z),
            Vec3::new(w, 0.025, d),
            Vec3::X,
            heading,
        ));
        for dx in [-w * 0.5, -w * 0.25, 0.0, w * 0.25, w * 0.5] {
            groups[1].push(cuboid(
                Vec3::new(x + dx, 7.61, z),
                Vec3::new(0.055, 0.14, d + 0.12),
                Vec3::X,
                heading,
            ));
        }
        for dz in [-d * 0.5, 0.0, d * 0.5] {
            groups[1].push(cuboid(
                Vec3::new(x, 7.61, z + dz),
                Vec3::new(w + 0.1, 0.14, 0.055),
                Vec3::X,
                heading,
            ));
        }
    }
    // Outer wall relief and skirting follow the actual inner hex shell; the
    // doorway face is deliberately clear. Insets remain inside solid walls.
    for face in [HexFace::SouthWest, HexFace::West, HexFace::NorthEast] {
        let [a, b] = observed_hex::face_edge(face);
        let a = Vec3::new(a.0 as f32, 0.0, a.1 as f32);
        let b = Vec3::new(b.0 as f32, 0.0, b.1 as f32);
        let axis = (b - a).normalize();
        let inward = -((a + b) * 0.5).normalize();
        for t in [0.12, 0.28, 0.44, 0.60, 0.76, 0.92] {
            groups[1].push(cuboid(
                a.lerp(b, t) + inward * 0.54 + Vec3::Y * 3.55,
                Vec3::new(0.085, 5.8, 0.065),
                axis,
                heading,
            ));
        }
        groups[5].push(cuboid(
            (a + b) * 0.5 + inward * 0.53 + Vec3::Y * 0.72,
            Vec3::new((b - a).length() - 0.8, 0.12, 0.055),
            axis,
            heading,
        ));
    }
    // Three arcs meet as a broad circulation medallion at the shared join.
    groups[5].push(polygon(
        &[(7.0, 4.0), (7.0, 2.4), (5.6, 2.5), (5.2, 3.9), (5.85, 4.65)],
        0.513,
        heading,
    ));
    groups[0].push(polygon(
        &[
            (7.0, 4.0),
            (7.0, 2.72),
            (5.85, 2.86),
            (5.56, 3.82),
            (6.04, 4.44),
        ],
        0.525,
        heading,
    ));
    let origin = Vec3::from_array(hex_origin(coord));
    let mut count = 0;
    for (role, hulls) in groups.iter().enumerate() {
        let mut hash = std::hash::DefaultHasher::new();
        for hull in hulls {
            for p in hull {
                p.to_array().map(f32::to_bits).hash(&mut hash);
            }
        }
        let key = format!("concourse-{role}-{}-{:x}", heading.index(), hash.finish());
        let refs: Vec<_> = hulls.iter().map(Vec::as_slice).collect();
        let Some(mesh) = assets.merged_mesh_for(meshes, Some(&key), MeshGroupKey::Interior, &refs)
        else {
            continue;
        };
        let mut entity = commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(assets.concourse_material(role)),
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
            Name::new("Switching Concourse shell and finish"),
        ));
        if role == 4 {
            entity.insert(ConcoursePanel(coord));
        }
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests;
