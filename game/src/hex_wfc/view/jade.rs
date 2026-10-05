//! Render authoritative Monument hulls with jade stone, bronze reveals and fixed coffers.
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
struct JadePanel(HexCoord);
pub(in crate::hex_wfc) fn install(app: &mut App) {
    app.add_systems(PostUpdate, sync_power.run_if(in_state(GameState::HexWfc)));
}
fn sync_power(
    runtime: Res<HexWfcRuntime>,
    assets: Res<HexWfcVisualAssets>,
    mut panels: Query<(&JadePanel, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (panel, mut material) in &mut panels {
        let powered = runtime
            .ascent
            .as_ref()
            .is_none_or(|a| a.rules().economy.is_powered(panel.0.level));
        let wanted = assets.jade_material(if powered { 4 } else { 6 });
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
        let size = max - min;
        let role = if max.y <= 0.51 {
            0
        } else if min.y >= 7.7 {
            5
        } else if max.y > 6.8 && size.x < 3.1 && size.z < 3.1 {
            2
        } else {
            1
        };
        groups[role].push(points.clone());
    }
    // Bronze courses follow the actual hexagonal piers, rather than decorating
    // empty air. These millimetre finishes do not change their solid outline.
    for (x, z) in [(-1.5, 3.75), (3.5, -3.5)] {
        for y in [0.56, 2.92, 6.80] {
            let ring: Vec<_> = [y, y + 0.035]
                .into_iter()
                .flat_map(|h| {
                    (0..6).map(move |i| {
                        let angle = i as f32 * std::f32::consts::TAU / 6.0;
                        turn(
                            Vec3::new(x + 1.128 * angle.cos(), h, z + 1.128 * angle.sin()),
                            heading,
                        )
                    })
                })
                .collect();
            groups[3].push(ring);
        }
    }
    // The nave's overhead lighting is inset into the sealed ceiling. Fine ribs
    // give the stone coffer depth; every visible luminous panel follows power.
    for (x, z, w, d) in [
        (-2.25, -3.0, 2.4, 1.2),
        (4.125, -1.5, 2.4, 1.2),
        (0.0, 4.0, 2.4, 1.2),
    ] {
        groups[5].push(cuboid(
            Vec3::new(x, 7.70, z),
            Vec3::new(w + 0.26, 0.08, d + 0.26),
            Vec3::X,
            heading,
        ));
        groups[4].push(cuboid(
            Vec3::new(x, 7.64, z),
            Vec3::new(w, 0.025, d),
            Vec3::X,
            heading,
        ));
        for dx in [-w * 0.5, 0.0, w * 0.5] {
            groups[3].push(cuboid(
                Vec3::new(x + dx, 7.60, z),
                Vec3::new(0.035, 0.10, d + 0.10),
                Vec3::X,
                heading,
            ));
        }
    }
    // Scarpa-inspired stepped reveal bands give heavy walls a readable scale.
    // The entry and open Span faces remain unobstructed.
    for face in [HexFace::SouthWest, HexFace::West, HexFace::NorthEast] {
        let [a, b] = observed_hex::face_edge(face);
        let a = Vec3::new(a.0 as f32, 0.0, a.1 as f32);
        let b = Vec3::new(b.0 as f32, 0.0, b.1 as f32);
        let axis = (b - a).normalize();
        let inward = -((a + b) * 0.5).normalize();
        for y in [0.72, 3.12, 6.85] {
            groups[3].push(cuboid(
                (a + b) * 0.5 + inward * 0.51 + Vec3::Y * y,
                Vec3::new((b - a).length() - 0.7, 0.055, 0.028),
                axis,
                heading,
            ));
        }
        for t in [0.24, 0.50, 0.76] {
            groups[2].push(cuboid(
                a.lerp(b, t) + inward * 0.505 + Vec3::Y * 3.95,
                Vec3::new(0.75, 5.3, 0.025),
                axis,
                heading,
            ));
            for dx in [-0.44, 0.44] {
                groups[1].push(cuboid(
                    a.lerp(b, t) + inward * 0.54 + axis * dx + Vec3::Y * 3.95,
                    Vec3::new(0.07, 5.45, 0.09),
                    axis,
                    heading,
                ));
            }
        }
    }
    // Flush bronze and deep-jade inlays mark the meeting of the three bays.
    groups[3].push(polygon(
        &[
            (7.0, 4.0),
            (7.0, 2.75),
            (5.8, 2.95),
            (5.6, 3.85),
            (6.0, 4.55),
        ],
        0.508,
        heading,
    ));
    groups[5].push(polygon(
        &[
            (7.0, 4.0),
            (7.0, 2.90),
            (5.95, 3.08),
            (5.78, 3.80),
            (6.1, 4.4),
        ],
        0.517,
        heading,
    ));
    groups[0].push(polygon(
        &[(7.0, 4.0), (7.0, 3.1), (6.1, 3.2), (6.0, 3.8), (6.2, 4.2)],
        0.526,
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
        let key = format!("jade-{role}-{}-{:x}", heading.index(), hash.finish());
        let refs: Vec<_> = hulls.iter().map(Vec::as_slice).collect();
        let Some(mesh) = assets.merged_mesh_for(meshes, Some(&key), MeshGroupKey::Interior, &refs)
        else {
            continue;
        };
        let mut entity = commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(assets.jade_material(role)),
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
            Name::new("Jade Nave shell and finish"),
        ));
        if role == 4 {
            entity.insert(JadePanel(coord));
        }
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests;
