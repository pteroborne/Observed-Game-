//! Render the actual authored Rain Court hulls, then add bounded non-colliding detail.
use super::{
    archive::cuboid,
    assets::{HexWfcVisualAssets, MeshGroupKey},
    spectate::Cutaway,
};
use bevy::prelude::*;
use observed_hex::{HexCoord, HexFace, hex_origin};
use observed_match::hex_wfc::HexStructurePiece;
use observed_traversal::ColliderShape;
use std::hash::{Hash, Hasher};
mod weather;
pub(in crate::hex_wfc) fn install(app: &mut App) {
    weather::install(app);
}
fn turn(mut p: Vec3, heading: HexFace) -> Vec3 {
    for _ in 0..heading.index() {
        p = Vec3::new(0.5 * p.x - 0.875 * p.z, p.y, 6.0 / 7.0 * p.x + 0.5 * p.z);
    }
    p
}
fn slab(points: &[(f32, f32)], y: f32, heading: HexFace) -> Vec<Vec3> {
    [y, y + 0.012]
        .into_iter()
        .flat_map(|h| {
            points
                .iter()
                .map(move |&(x, z)| turn(Vec3::new(x, h, z), heading))
        })
        .collect()
}
// This polygon is wholly in the canonical cell, bounded by its two span faces.
const GARDEN: [(f32, f32); 6] = [
    (7.0, 4.0),
    (7.0, -1.0),
    (3.0, -1.0),
    (1.0, 2.0),
    (1.0, 4.0),
    (3.0, 44.0 / 7.0),
];
pub(super) fn spawn(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    coord: HexCoord,
    heading: HexFace,
    pieces: &[&HexStructurePiece],
) -> usize {
    let origin = Vec3::from_array(hex_origin(coord));
    let mut groups: [Vec<Vec<Vec3>>; 7] = std::array::from_fn(|_| Vec::new());
    for piece in pieces {
        if !matches!(
            piece.part,
            observed_match::hex_wfc::HexPiecePart::Authored
                | observed_match::hex_wfc::HexPiecePart::Window
        ) || !matches!(
            piece.role,
            observed_match::hex_wfc::HexStructureRole::Hall
                | observed_match::hex_wfc::HexStructureRole::Room
        ) {
            continue;
        }
        let ColliderShape::ConvexHull { points } = &piece.shape else {
            continue;
        };
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for p in points {
            let local = turn(*p, HexFace::LATERAL[(6 - heading.index()) % 6]);
            min = min.min(local);
            max = max.max(local);
        }
        // Geometry, not a second physical model: small low garden hulls are stone;
        // tall wide vertical hulls are paper; floor, eaves and narrow posts are cedar.
        let role = if max.y < 1.5 && min.x > 3.0 {
            2
        } else if max.y - min.y > 2.5 && (max.x - min.x > 1.0 || max.z - min.z > 1.0) {
            1
        } else {
            0
        };
        if role == 2 {
            let top: Vec<_> = points
                .iter()
                .filter(|p| (p.y - max.y).abs() < 0.003)
                .copied()
                .collect();
            if top.len() >= 3 {
                let center: Vec3 = top.iter().copied().sum::<Vec3>() / top.len() as f32;
                groups[4].push(
                    [0.004, 0.012]
                        .into_iter()
                        .flat_map(|rise| {
                            top.iter()
                                .map(move |p| center + (*p - center) * 0.9 + Vec3::Y * rise)
                        })
                        .collect(),
                );
            }
        }
        groups[role].push(points.clone());
    }
    groups[3].push(slab(&GARDEN, 0.504, heading));
    // A moss bed follows the solid paving beneath each real stone cluster.
    groups[4].push(slab(
        &[
            (3.3, 1.2),
            (3.9, 0.8),
            (5.2, 0.9),
            (5.8, 1.7),
            (5.5, 2.8),
            (4.6, 3.0),
            (3.8, 2.4),
        ],
        0.507,
        heading,
    ));
    // Tatami is confined to a small seated bay, away from the timber circulation.
    groups[5].push(cuboid(
        Vec3::new(-4.85, 0.52, 0.0),
        Vec3::new(1.3, 0.015, 2.7),
        Vec3::X,
        heading,
    ));
    // A real opaque paper screen carries thin cedar lattice on both faces.
    for z in [1.892, 2.108] {
        for x in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0] {
            groups[0].push(cuboid(
                Vec3::new(x, 2.05, z),
                Vec3::new(0.045, 3.1, 0.025),
                Vec3::X,
                heading,
            ));
        }
        for y in [0.52, 1.05, 1.58, 2.11, 2.64, 3.17, 3.59] {
            groups[0].push(cuboid(
                Vec3::new(-0.5, y, z),
                Vec3::new(3.0, 0.035, 0.025),
                Vec3::X,
                heading,
            ));
        }
    }
    // External paper wall bays follow the exact inset hex edges; door stays clear.
    for face in [HexFace::SouthWest, HexFace::West, HexFace::NorthEast] {
        let [a, b] = observed_hex::face_edge(face);
        let a = Vec3::new(a.0 as f32, 0.0, a.1 as f32);
        let b = Vec3::new(b.0 as f32, 0.0, b.1 as f32);
        let axis = (b - a).normalize();
        let inward = -(a + b).normalize();
        groups[0].push(cuboid(
            (a + b) * 0.5 + inward * 0.535 + Vec3::Y * 6.1,
            Vec3::new((b - a).length() - 0.75, 3.05, 0.065),
            axis,
            heading,
        ));
        groups[0].push(cuboid(
            (a + b) * 0.5 + inward * 0.57 + Vec3::Y * 4.48,
            Vec3::new((b - a).length() - 0.75, 0.16, 0.12),
            axis,
            heading,
        ));
        for i in 1..16 {
            groups[0].push(cuboid(
                a.lerp(b, i as f32 / 16.0) + inward * 0.53 + Vec3::Y * 2.55,
                Vec3::new(0.055, 4.0, 0.06),
                axis,
                heading,
            ));
        }
        for y in [0.65, 1.3, 2.0, 2.7, 3.4, 4.35] {
            groups[0].push(cuboid(
                (a + b) * 0.5 + inward * 0.53 + Vec3::Y * y,
                Vec3::new((b - a).length() - 0.8, 0.055, 0.06),
                axis,
                heading,
            ));
        }
    }
    // Narrow slats beneath the eaves give dry space a human scale.
    for i in 0..22 {
        let x = -5.5 + i as f32 * 0.35;
        groups[0].push(cuboid(
            Vec3::new(x, 4.66, -1.15),
            Vec3::new(0.09, 0.08, 4.8),
            Vec3::X,
            heading,
        ));
    }
    // Roof aperture and its drain lip; all lie below the sealed storey slab.
    let aperture = [
        (7.0, 4.0),
        (7.0, 0.1),
        (3.5, 0.1),
        (2.2, 2.3),
        (2.2, 4.0),
        (4.0, 40.0 / 7.0),
    ];
    // Recessed roof panel and garden drain have cedar/mineral rims, not sky holes.
    for i in 1..aperture.len() - 1 {
        let (ax, az) = aperture[i];
        let (bx, bz) = aperture[i + 1];
        let a = Vec3::new(ax, 7.60, az);
        let b = Vec3::new(bx, 7.60, bz);
        let axis = (b - a).normalize();
        groups[0].push(cuboid(
            (a + b) * 0.5,
            Vec3::new((b - a).length(), 0.13, 0.10),
            axis,
            heading,
        ));
    }
    for i in 1..GARDEN.len() - 1 {
        let (ax, az) = GARDEN[i];
        let (bx, bz) = GARDEN[i + 1];
        let a = Vec3::new(ax, 0.519, az);
        let b = Vec3::new(bx, 0.519, bz);
        let axis = (b - a).normalize();
        groups[2].push(cuboid(
            (a + b) * 0.5,
            Vec3::new((b - a).length(), 0.01, 0.065),
            axis,
            heading,
        ));
    }
    weather::marker(commands, parent, coord, heading, origin, &aperture, &GARDEN);
    for (x, z) in [(-3.0, -0.8), (0.0, -3.0)] {
        groups[6].push(cuboid(
            Vec3::new(x, 4.39, z),
            Vec3::new(0.8, 0.15, 0.8),
            Vec3::X,
            heading,
        ));
        weather::lantern(
            commands,
            parent,
            coord,
            heading,
            origin,
            Vec3::new(x, 4.30, z),
        );
    }
    let mut count = 0;
    for (role, hulls) in groups.iter().enumerate() {
        let refs: Vec<_> = hulls.iter().map(Vec::as_slice).collect();
        // Derived windows may change a hull without changing its tile or heading.
        // Cache the projected shape itself so later relayouts cannot show stale walls.
        let mut hash = std::hash::DefaultHasher::new();
        for hull in hulls {
            for point in hull {
                point.to_array().map(f32::to_bits).hash(&mut hash);
            }
        }
        let key = format!("rain-court-{role}-{}-{:x}", heading.index(), hash.finish());
        if let Some(mesh) =
            assets.merged_mesh_for(meshes, Some(&key), MeshGroupKey::Interior, &refs)
        {
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(assets.rain_material(role)),
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
                Name::new("Rain Court authored shell and finish"),
            ));
            count += 1;
        }
    }
    count
}
