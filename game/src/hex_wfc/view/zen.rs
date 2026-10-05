//! Cedar lattice and ceiling slats fitted to ordinary Zen's actual supports.
//! These shallow finishes add no colliders and retire with the cell shell.
use super::{
    archive::cuboid,
    assets::{HexWfcVisualAssets, MeshGroupKey},
    spectate::Cutaway,
    support::{inward_wall_frame, points},
};
use bevy::prelude::*;
use observed_facility::hex_wfc::HexWfcWorld;
use observed_hex::{HexCoord, HexFace, hex_origin};
use observed_match::hex_wfc::{HexPiecePart, HexStructurePiece, HexStructureRole};
mod shell;

#[derive(Component)]
pub(super) struct ZenDetail;

struct Detail {
    hulls: Vec<Vec<Vec3>>,
    local: Vec3,
    min_y: f32,
    max_y: f32,
}

fn wall_detail(hull: &[Vec3], axis: Vec3, normal: Vec3) -> Option<Detail> {
    let (axis, normal, plane) = inward_wall_frame(hull, axis, normal, 0.75)?;
    let support: Vec<_> = hull
        .iter()
        .filter(|p| (p.dot(normal) - plane).abs() < 0.002)
        .map(|p| Vec3::new(p.dot(axis), 0.0, p.y))
        .collect();
    let lo = support
        .iter()
        .copied()
        .fold(Vec3::splat(f32::INFINITY), Vec3::min);
    let hi = support
        .iter()
        .copied()
        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
    let (left, right, bottom, top) = (
        lo.x + 0.12,
        hi.x - 0.12,
        lo.z.max(0.5) + 0.12,
        hi.z.min(7.8) - 0.12,
    );
    if right - left < 0.65
        || top - bottom < 0.65
        || [
            Vec2::new(left, bottom),
            Vec2::new(right, bottom),
            Vec2::new(right, top),
            Vec2::new(left, top),
        ]
        .iter()
        .any(|&p| !observed_traversal::point_in_convex_plan_hull(&support, p))
    {
        return None;
    }
    let at = |x, y, depth| axis * x + normal * depth + Vec3::Y * y;
    let mut hulls = Vec::new();
    let mut beam =
        |x, y, size| hulls.push(cuboid(at(x, y, plane + 0.065), size, axis, HexFace::East));
    let width = right - left;
    let middle = (left + right) * 0.5;
    let paper_top = top.min(4.5);
    if paper_top > bottom {
        let bays = (width / 0.65).ceil() as usize;
        for i in 0..=bays {
            beam(
                left + width * i as f32 / bays as f32,
                (bottom + paper_top) * 0.5,
                Vec3::new(
                    if i % 3 == 0 { 0.09 } else { 0.035 },
                    paper_top - bottom,
                    0.08,
                ),
            );
        }
        let rows = ((paper_top - bottom) / 0.8).ceil() as usize;
        for i in 0..=rows {
            beam(
                middle,
                bottom + (paper_top - bottom) * i as f32 / rows as f32,
                Vec3::new(
                    width + 0.035,
                    if i == 0 || i == rows { 0.09 } else { 0.035 },
                    0.06,
                ),
            );
        }
    }
    // Cedar above the paper recalls the court's low eaves without lowering
    // any actual room ceiling or inventing a lintel across an opening.
    let frieze_bottom = bottom.max(4.56);
    if top > frieze_bottom {
        beam(
            middle,
            (frieze_bottom + top) * 0.5,
            Vec3::new(width, top - frieze_bottom, 0.025),
        );
        for x in [left, right] {
            beam(
                x,
                (frieze_bottom + top) * 0.5,
                Vec3::new(0.09, top - frieze_bottom, 0.08),
            );
        }
    }
    beam(middle, top, Vec3::new(width, 0.12, 0.12));
    Some(Detail {
        hulls,
        local: at(middle, 0.0, plane),
        min_y: bottom - 0.06,
        max_y: top + 0.06,
    })
}

fn ceiling_detail(hull: &[Vec3]) -> Option<Detail> {
    let lo = hull
        .iter()
        .copied()
        .fold(Vec3::splat(f32::INFINITY), Vec3::min);
    let hi = hull
        .iter()
        .copied()
        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
    let size = hi - lo;
    // Narrow corridor rafts also supply real flat undersides for slats.
    if lo.y < 4.5 || size.y > 0.75 || size.x.min(size.z) < 0.8 || size.x.max(size.z) < 1.5 {
        return None;
    }
    let y = hull.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let underside: Vec<_> = hull
        .iter()
        .copied()
        .filter(|p| (p.y - y).abs() < 0.002)
        .collect();
    let plan = observed_traversal::plan_convex_hull(&underside);
    if plan.len() < 3 {
        return None;
    }
    // Intersect both edges of each strip with the actual underside polygon.
    // Separate slabs remain separate: no strips over a stairwell or roof gap.
    let interval = |z: f32| -> Option<(f32, f32)> {
        let mut xs = Vec::new();
        for i in 0..plan.len() {
            let a = plan[i];
            let b = plan[(i + 1) % plan.len()];
            if (a.y - b.y).abs() < 0.0001 {
                continue;
            }
            let t = (z - a.y) / (b.y - a.y);
            if (0.0..=1.0).contains(&t) {
                xs.push(a.x + t * (b.x - a.x));
            }
        }
        (xs.len() >= 2).then(|| {
            (
                xs.iter().copied().fold(f32::INFINITY, f32::min),
                xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
            )
        })
    };
    let lo = plan.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let hi = plan.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
    let mut hulls = Vec::new();
    let mut z = (lo / 0.35).ceil() * 0.35 + 0.10;
    while z < hi - 0.15 {
        if let Some(((a, b), (c, d))) = interval(z - 0.04).zip(interval(z + 0.04)) {
            let (left, right) = (a.max(c) + 0.10, b.min(d) - 0.10);
            if right - left > 0.4 {
                hulls.push(cuboid(
                    Vec3::new((left + right) * 0.5, y - 0.035, z),
                    Vec3::new(right - left, 0.05, 0.08),
                    Vec3::X,
                    HexFace::East,
                ));
            }
        }
        z += 0.35;
    }
    if hulls.is_empty() {
        return None;
    }
    let local = hulls.iter().flatten().copied().sum::<Vec3>() / (hulls.len() * 8) as f32;
    Some(Detail {
        hulls,
        local,
        min_y: y - 0.06,
        max_y: y,
    })
}

fn details(world: &HexWfcWorld, coord: HexCoord, pieces: &[&HexStructurePiece]) -> Vec<Detail> {
    let Some(placement) = world.placements.get(&coord) else {
        return Vec::new();
    };
    let origin = Vec3::from_array(hex_origin(coord));
    let mut out = Vec::new();
    for piece in pieces
        .iter()
        .filter(|p| matches!(p.part, HexPiecePart::Authored | HexPiecePart::Window))
    {
        let hull = points(piece, origin);
        if let Some(detail) = ceiling_detail(&hull) {
            out.push(detail);
        }
        let plan = observed_traversal::plan_convex_hull(&hull);
        for i in 0..plan.len() {
            let a = Vec3::new(plan[i].x, 0.0, plan[i].y);
            let next = plan[(i + 1) % plan.len()];
            let b = Vec3::new(next.x, 0.0, next.y);
            let axis = (b - a).normalize_or_zero();
            let mut normal = Vec3::new(-axis.z, 0.0, axis.x);
            if normal.dot(a + b) > 0.0 {
                normal = -normal;
            }
            let plane = a.dot(normal);
            if plane >= -0.75 || hull.iter().any(|p| p.dot(normal) > plane + 0.002) {
                continue;
            }
            // Actual inner screens need not align with a lattice face. A
            // perimeter support still respects its declared door face.
            let face = HexFace::LATERAL
                .into_iter()
                .max_by(|&a, &b| {
                    let inward = |face| {
                        let [a, b] = observed_hex::face_edge(face);
                        -Vec3::new((a.0 + b.0) as f32, 0.0, (a.1 + b.1) as f32).normalize()
                    };
                    normal.dot(inward(a)).total_cmp(&normal.dot(inward(b)))
                })
                .expect("six lateral faces");
            if plane < -5.0 && placement.is_open(face) {
                continue;
            }
            if let Some(detail) = wall_detail(&hull, axis, normal) {
                out.push(detail);
            }
        }
    }
    out
}

fn mesh_key(hulls: &[Vec<Vec3>]) -> String {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for point in hulls.iter().flatten() {
        point.to_array().map(f32::to_bits).hash(&mut hash);
    }
    format!("zen-detail-{:016x}", hash.finish())
}

pub(super) fn spawn(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    coord: HexCoord,
    world: &HexWfcWorld,
    pieces: &[&HexStructurePiece],
) -> usize {
    let origin = Vec3::from_array(hex_origin(coord));
    let climb_wall = pieces.iter().any(|p| p.role == HexStructureRole::Climb);
    let mut count = shell::spawn(commands, assets, meshes, parent, coord, pieces);
    for detail in details(world, coord, pieces) {
        let refs: Vec<_> = detail.hulls.iter().map(Vec::as_slice).collect();
        let Some(mesh) = assets.merged_mesh_for(
            meshes,
            Some(&mesh_key(&detail.hulls)),
            MeshGroupKey::Interior,
            &refs,
        ) else {
            continue;
        };
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(assets.rain_material(0)),
            Transform::from_translation(origin),
            ChildOf(parent),
            ZenDetail,
            Cutaway {
                local: detail.local,
                min_y: detail.min_y,
                max_y: detail.max_y,
                origin_y: origin.y,
                cell_level: coord.level,
                climb_wall,
            },
            Name::new("Zen cedar lattice / supported ceiling slats"),
        ));
        count += 1;
    }
    count
}

/// A cedar rim around the ordinary authored diffuser, at its existing anchor.
pub(super) fn fixture_frame(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    coord: HexCoord,
    at: Vec3,
) -> usize {
    let hulls = [
        cuboid(
            Vec3::new(-1.15, 0.0, 0.0),
            Vec3::new(0.075, 0.18, 0.64),
            Vec3::X,
            HexFace::East,
        ),
        cuboid(
            Vec3::new(1.15, 0.0, 0.0),
            Vec3::new(0.075, 0.18, 0.64),
            Vec3::X,
            HexFace::East,
        ),
        cuboid(
            Vec3::new(0.0, 0.0, -0.30),
            Vec3::new(2.25, 0.18, 0.075),
            Vec3::X,
            HexFace::East,
        ),
        cuboid(
            Vec3::new(0.0, 0.0, 0.30),
            Vec3::new(2.25, 0.18, 0.075),
            Vec3::X,
            HexFace::East,
        ),
    ];
    let refs: Vec<_> = hulls.iter().map(Vec::as_slice).collect();
    let Some(mesh) = assets.merged_mesh_for(
        meshes,
        Some("zen-paper-lantern-frame"),
        MeshGroupKey::Interior,
        &refs,
    ) else {
        return 0;
    };
    let origin = Vec3::from_array(hex_origin(coord));
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(assets.rain_material(0)),
        Transform::from_translation(at),
        ChildOf(parent),
        ZenDetail,
        Cutaway {
            local: at - origin,
            min_y: at.y - origin.y - 0.09,
            max_y: at.y - origin.y + 0.09,
            origin_y: origin.y,
            cell_level: coord.level,
            climb_wall: false,
        },
        Name::new("Zen paper lantern cedar rim"),
    ));
    1
}

#[cfg(test)]
mod tests;
