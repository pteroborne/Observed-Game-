//! Library bookcase facades fitted to existing convex wall faces.
//! Source walls, windows and openings remain authoritative. Shallow bindings
//! add no collision; meshes stream, rewrite and cut away with their cell.
use super::{
    archive::cuboid,
    assets::{HexWfcVisualAssets, MeshGroupKey},
    spectate::Cutaway,
};
use bevy::prelude::*;
use observed_facility::hex_wfc::HexWfcWorld;
use observed_hex::{HexCoord, HexFace, hex_origin};
use observed_match::hex_wfc::{HexPiecePart, HexStructurePiece};
use observed_traversal::ColliderShape;

#[derive(Component)]
pub(super) struct LibraryBookcase;

#[derive(Clone, Debug)]
struct Row {
    height: f32,
    front: f32,
    underside: f32,
}

#[derive(Clone, Debug)]
struct Bookcase {
    face: HexFace,
    axis: Vec3,
    normal: Vec3,
    plane: f32,
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    rows: Vec<Row>,
}

impl Bookcase {
    fn at(&self, x: f32, y: f32, depth: f32) -> Vec3 {
        self.axis * x + self.normal * depth + Vec3::Y * y
    }
    fn key(&self) -> String {
        // Fit dimensions, rather than a cell ID, share identical walls between
        // variants. Removed walls and split window frames produce different fits.
        let mut values = vec![
            self.axis.x,
            self.axis.z,
            self.normal.x,
            self.normal.z,
            self.plane,
            self.left,
            self.right,
            self.bottom,
            self.top,
        ];
        for row in &self.rows {
            values.extend([row.height, row.front, row.underside]);
        }
        let fit: Vec<_> = values.iter().map(|v| (v * 4096.0).round() as i32).collect();
        format!("babel-bookcase-{}-{fit:?}", self.face.index())
    }
}

fn points(piece: &HexStructurePiece, origin: Vec3) -> Vec<Vec3> {
    let ColliderShape::ConvexHull { points } = &piece.shape else {
        return Vec::new();
    };
    let rotation = Quat::from_array(piece.rotation);
    points
        .iter()
        .map(|&p| piece.center - origin + rotation * p)
        .collect()
}

fn wall_frame(hull: &[Vec3], axis: Vec3, normal: Vec3) -> Option<(Vec3, Vec3, f32)> {
    // Authored variants can use slightly different rotations from the lattice.
    // Fit their actual inward support plane, never a nearly parallel grid plane.
    let plan = observed_traversal::plan_convex_hull(hull);
    (0..plan.len())
        .filter_map(|i| {
            let edge = plan[(i + 1) % plan.len()] - plan[i];
            let mut actual_axis = Vec3::new(edge.x, 0.0, edge.y).normalize_or_zero();
            if actual_axis.dot(axis) < 0.0 {
                actual_axis = -actual_axis;
            }
            let mut actual_normal = Vec3::new(-actual_axis.z, 0.0, actual_axis.x);
            if actual_normal.dot(normal) < 0.0 {
                actual_normal = -actual_normal;
            }
            let plane = Vec3::new(plan[i].x, 0.0, plan[i].y).dot(actual_normal);
            (actual_normal.dot(normal) > 0.98
                && plane < -3.0
                && hull.iter().all(|p| p.dot(actual_normal) <= plane + 0.002))
            .then_some((edge.length_squared(), actual_axis, actual_normal, plane))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, axis, normal, plane)| (axis, normal, plane))
}

fn bookcases(world: &HexWfcWorld, coord: HexCoord, pieces: &[&HexStructurePiece]) -> Vec<Bookcase> {
    let Some(placement) = world.placements.get(&coord) else {
        return Vec::new();
    };
    let origin = Vec3::from_array(hex_origin(coord));
    let hulls: Vec<_> = pieces
        .iter()
        .filter(|p| matches!(p.part, HexPiecePart::Authored | HexPiecePart::Window))
        .map(|p| points(p, origin))
        .filter(|p| !p.is_empty())
        .collect();
    let mut cases = Vec::new();
    for face in HexFace::LATERAL {
        if placement.is_open(face) {
            continue;
        }
        let [a, b] = observed_hex::face_edge(face);
        let a = Vec3::new(a.0 as f32, 0.0, a.1 as f32);
        let b = Vec3::new(b.0 as f32, 0.0, b.1 as f32);
        let axis = (b - a).normalize();
        let mut normal = Vec3::new(-axis.z, 0.0, axis.x);
        if normal.dot(a + b) > 0.0 {
            normal = -normal;
        }
        for hull in &hulls {
            // An exact convex support plane supplies the wall face. A vertex,
            // narrow jamb or low rail cannot masquerade as a bookcase wall.
            let Some((axis, normal, plane)) = wall_frame(hull, axis, normal) else {
                continue;
            };
            let support: Vec<_> = hull
                .iter()
                .filter(|p| (p.dot(normal) - plane).abs() < 0.002)
                .map(|p| Vec3::new(p.dot(axis), 0.0, p.y))
                .collect();
            if support.len() < 4 {
                continue;
            }
            let bounds = |project: fn(&Vec3) -> f32| {
                support
                    .iter()
                    .map(project)
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
                        (lo.min(v), hi.max(v))
                    })
            };
            let (left, right) = bounds(|p| p.x);
            let (bottom, top) = bounds(|p| p.z);
            let (left, right, bottom, top) = (
                left + 0.32,
                right - 0.32,
                (bottom + 0.18).max(1.10),
                (top - 0.28).min(7.45),
            );
            if right - left < 1.6 || top - bottom < 2.1 {
                continue;
            }
            // Each full corner must lie on this convex support face. This rejects
            // sloping silhouettes and never joins separate window-frame pieces.
            if [
                Vec2::new(left, bottom),
                Vec2::new(right, bottom),
                Vec2::new(right, top),
                Vec2::new(left, top),
            ]
            .iter()
            .any(|&p| !observed_traversal::point_in_convex_plan_hull(&support, p))
            {
                continue;
            }
            let mut rows: Vec<Row> = hulls
                .iter()
                .filter_map(|shelf| {
                    let min_y = shelf.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
                    let max_y = shelf.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
                    let front = shelf
                        .iter()
                        .map(|p| p.dot(normal))
                        .fold(f32::NEG_INFINITY, f32::max);
                    let lo = shelf
                        .iter()
                        .map(|p| p.dot(axis))
                        .fold(f32::INFINITY, f32::min);
                    let hi = shelf
                        .iter()
                        .map(|p| p.dot(axis))
                        .fold(f32::NEG_INFINITY, f32::max);
                    (max_y - min_y < 0.65
                        && max_y >= bottom
                        && max_y < top - 0.3
                        && front > plane + 0.10
                        && front < plane + 1.6
                        && lo <= left
                        && hi >= right)
                        .then_some(Row {
                            height: max_y + 0.012,
                            front: front - 0.045,
                            underside: min_y,
                        })
                })
                .collect();
            rows.sort_by(|a, b| a.height.total_cmp(&b.height));
            rows.dedup_by(|a, b| (a.height - b.height).abs() < 0.01);
            // Low-relief inlays fill blank backing between widely separated
            // source shelves, or the whole wall when it only has a plinth.
            let authored_rows = rows.clone();
            let mut height = bottom + 0.12;
            while height < top - 0.35 {
                if authored_rows
                    .iter()
                    .all(|r| (r.height - height).abs() > 0.85)
                {
                    rows.push(Row {
                        height,
                        front: plane + 0.20,
                        underside: height - 0.05,
                    });
                }
                height += 1.02;
            }
            rows.sort_by(|a, b| a.height.total_cmp(&b.height));
            cases.push(Bookcase {
                face,
                axis,
                normal,
                plane,
                left,
                right,
                bottom,
                top,
                rows,
            });
        }
    }
    cases.sort_by_key(Bookcase::key);
    cases.dedup_by_key(|c| c.key());
    cases
}

fn geometry(case: &Bookcase) -> [Vec<Vec<Vec3>>; 7] {
    let mut groups: [Vec<Vec<Vec3>>; 7] = std::array::from_fn(|_| Vec::new());
    let width = case.right - case.left;
    let height = case.top - case.bottom;
    let middle = (case.left + case.right) * 0.5;
    let box_at = |x, y, depth, size| cuboid(case.at(x, y, depth), size, case.axis, HexFace::East);
    groups[1].push(box_at(
        middle,
        (case.bottom + case.top) * 0.5,
        case.plane + 0.018,
        Vec3::new(width, height, 0.025),
    ));
    let bays = (width / 2.2).ceil() as usize;
    let bay_width = width / bays as f32;
    for bay in 0..=bays {
        groups[6].push(box_at(
            case.left + bay as f32 * bay_width,
            (case.bottom + case.top) * 0.5,
            case.plane + 0.19,
            Vec3::new(0.075, height, 0.045),
        ));
    }
    for y in [case.bottom, case.top] {
        groups[6].push(box_at(
            middle,
            y,
            case.plane + 0.19,
            Vec3::new(width, 0.07, 0.045),
        ));
    }
    for (row_index, row) in case.rows.iter().enumerate() {
        groups[6].push(box_at(
            middle,
            row.height - 0.025,
            row.front - 0.05,
            Vec3::new(width, 0.04, 0.12),
        ));
        let next = case
            .rows
            .get(row_index + 1)
            .map_or(case.top, |r| r.underside);
        let max_height = (next - row.height - 0.10).min(0.76);
        if max_height < 0.23 {
            continue;
        }
        let depth = (row.front - case.plane - 0.045).clamp(0.10, 0.52);
        for bay in 0..bays {
            let mut x = case.left + bay as f32 * bay_width + 0.12;
            let end = case.left + (bay + 1) as f32 * bay_width - 0.12;
            let mut index = 0usize;
            while x < end - 0.20 {
                let hash = index * 37 + row_index * 17 + case.face.index() * 13 + bay * 29;
                if hash % 19 == 7 {
                    x += 0.24;
                    index += 1;
                    continue;
                }
                let binding = (index + row_index * 3 + bay + case.face.index()) % 6;
                if hash % 23 == 8 && x + 0.34 < end {
                    for stack in 0..3 {
                        groups[binding].push(box_at(
                            x + 0.16,
                            row.height + 0.036 + stack as f32 * 0.075,
                            row.front - depth * 0.5,
                            Vec3::new(0.32, 0.068, depth),
                        ));
                    }
                    x += 0.36;
                } else {
                    let book_width = 0.105 + (hash % 5) as f32 * 0.018;
                    let book_height = (0.43 + (hash % 7) as f32 * 0.038).min(max_height);
                    let at = case.at(
                        x + book_width * 0.5,
                        row.height + book_height * 0.5 + 0.01,
                        row.front - depth * 0.5,
                    );
                    let tilt = if hash % 13 == 4 { 0.09 } else { 0.0 };
                    let rotation = Quat::from_axis_angle(case.normal, tilt);
                    let transform = |points: Vec<Vec3>| {
                        points
                            .into_iter()
                            .map(|p| at + rotation * (p - at))
                            .collect()
                    };
                    groups[binding].push(transform(cuboid(
                        at,
                        Vec3::new(book_width, book_height, depth),
                        case.axis,
                        HexFace::East,
                    )));
                    for fraction in [0.18, 0.82] {
                        let band = at
                            + case.normal * (depth * 0.5 + 0.004)
                            + Vec3::Y * (book_height * (fraction - 0.5));
                        groups[6].push(transform(cuboid(
                            band,
                            Vec3::new(book_width * 0.82, 0.022, 0.009),
                            case.axis,
                            HexFace::East,
                        )));
                    }
                    x += book_width + 0.018 + tilt * book_height;
                }
                index += 1;
            }
        }
    }
    groups
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
    let mut count = 0;
    let origin = Vec3::from_array(hex_origin(coord));
    let climb_wall = pieces
        .iter()
        .any(|p| p.role == observed_match::hex_wfc::HexStructureRole::Climb);
    for case in bookcases(world, coord, pieces) {
        let key = case.key();
        for (index, hulls) in geometry(&case).iter().enumerate() {
            let refs: Vec<_> = hulls.iter().map(Vec::as_slice).collect();
            let Some(mesh) = assets.merged_mesh_for(
                meshes,
                Some(&format!("{key}-{index}")),
                MeshGroupKey::Interior,
                &refs,
            ) else {
                continue;
            };
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(assets.archive_detail(index)),
                Transform::from_translation(origin),
                ChildOf(parent),
                LibraryBookcase,
                Cutaway {
                    local: case.at((case.left + case.right) * 0.5, 0.0, case.plane),
                    min_y: case.bottom,
                    max_y: case.top,
                    origin_y: origin.y,
                    cell_level: coord.level,
                    climb_wall,
                },
                Name::new(format!("Babel bookcase {:?}, material {index}", case.face)),
            ));
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests;
