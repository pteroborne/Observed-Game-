//! Union coplanar render faces from overlapping authored solid brushes.
//! Collision hulls remain unchanged. No depth bias or temporal suppression.
use super::ConvexRenderMesh;
use glam::{Vec2, Vec3};
use std::collections::BTreeMap;

const EPS: f32 = 0.00001;

#[derive(Clone, Copy)]
struct Triangle {
    points: [Vec3; 3],
    normals: [Vec3; 3],
    uvs: [Vec2; 3],
    normal: Vec3,
}

fn projected(p: Vec3, axis: usize) -> Vec2 {
    match axis {
        0 => Vec2::new(p.y, p.z),
        1 => Vec2::new(p.x, p.z),
        _ => Vec2::new(p.x, p.y),
    }
}
fn axis(n: Vec3) -> usize {
    if n.x.abs() > n.y.abs() && n.x.abs() > n.z.abs() {
        0
    } else if n.y.abs() > n.z.abs() {
        1
    } else {
        2
    }
}
fn bounds(p: &[Vec3], axis: usize) -> (Vec2, Vec2) {
    p.iter().map(|&p| projected(p, axis)).fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(lo, hi), p| (lo.min(p), hi.max(p)),
    )
}
fn clip(poly: &[Vec3], a: Vec2, b: Vec2, sign: f32, axis: usize, inside: bool) -> Vec<Vec3> {
    let distance = |p: Vec3| sign * (b - a).perp_dot(projected(p, axis) - a);
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let p = poly[i];
        let q = poly[(i + 1) % poly.len()];
        let dp = distance(p);
        let dq = distance(q);
        let accepts = |d: f32| if inside { d >= -EPS } else { d <= EPS };
        if accepts(dp) {
            out.push(p);
        }
        if (dp > EPS && dq < -EPS) || (dp < -EPS && dq > EPS) {
            out.push(p.lerp(q, dp / (dp - dq)));
        }
    }
    out.dedup_by(|a, b| a.distance_squared(*b) < EPS * EPS);
    out
}
fn subtract(poly: &[Vec3], cutter: &Triangle) -> Vec<Vec<Vec3>> {
    let axis = axis(cutter.normal);
    let (lo, hi) = bounds(poly, axis);
    let (clo, chi) = bounds(&cutter.points, axis);
    if (hi.min(chi) - lo.max(clo)).min_element() <= EPS {
        return vec![poly.to_vec()];
    }
    let [a, b, c] = cutter.points.map(|p| projected(p, axis));
    let sign = (b - a).perp_dot(c - a).signum();
    let mut remainder = poly.to_vec();
    let mut pieces = Vec::new();
    for (a, b) in [(a, b), (b, c), (c, a)] {
        let outside = clip(&remainder, a, b, sign, axis, false);
        if outside.len() >= 3 {
            pieces.push(outside);
        }
        remainder = clip(&remainder, a, b, sign, axis, true);
        if remainder.len() < 3 {
            break;
        }
    }
    pieces
}
fn attributes(t: Triangle, p: Vec3) -> ([f32; 3], [f32; 2]) {
    let axis = axis(t.normal);
    let [a, b, c] = t.points.map(|p| projected(p, axis));
    let p = projected(p, axis);
    let total = (b - a).perp_dot(c - a);
    let v = (p - a).perp_dot(c - a) / total;
    let w = (b - a).perp_dot(p - a) / total;
    let u = 1.0 - v - w;
    (
        (t.normals[0] * u + t.normals[1] * v + t.normals[2] * w)
            .normalize_or_zero()
            .to_array(),
        (t.uvs[0] * u + t.uvs[1] * v + t.uvs[2] * w).to_array(),
    )
}

/// Merge a deterministic ordered set of meshes with each outward surface drawn once.
/// UVs/normals are interpolated onto cut vertices, preserving material scale.
#[must_use]
pub fn merge_coplanar_surfaces(meshes: &[ConvexRenderMesh]) -> ConvexRenderMesh {
    let mut out = ConvexRenderMesh {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        indices: Vec::new(),
    };
    let mut planes = BTreeMap::<[i64; 4], Vec<Triangle>>::new();
    for mesh in meshes {
        for ids in mesh.indices.chunks_exact(3) {
            let indices = [ids[0] as usize, ids[1] as usize, ids[2] as usize];
            let points = indices.map(|i| Vec3::from_array(mesh.positions[i]));
            let normal = (points[1] - points[0])
                .cross(points[2] - points[0])
                .normalize_or_zero();
            if normal.length_squared() < 0.5 {
                continue;
            }
            let triangle = Triangle {
                points,
                normal,
                normals: indices.map(|i| Vec3::from_array(mesh.normals[i])),
                uvs: indices.map(|i| Vec2::from_array(mesh.uvs[i])),
            };
            let key = [normal.x, normal.y, normal.z, normal.dot(points[0])]
                .map(|v| (v * 10000.0).round() as i64);
            let previous = planes.entry(key).or_default();
            let mut pieces = vec![points.to_vec()];
            for old in previous.iter() {
                pieces = pieces.iter().flat_map(|poly| subtract(poly, old)).collect();
                if pieces.is_empty() {
                    break;
                }
            }
            for polygon in pieces {
                for i in 1..polygon.len() - 1 {
                    let [a, b, c] = [polygon[0], polygon[i], polygon[i + 1]];
                    if (b - a).cross(c - a).length_squared() < EPS * EPS {
                        continue;
                    }
                    for p in [a, b, c] {
                        let (normal, uv) = attributes(triangle, p);
                        out.indices.push(
                            u32::try_from(out.positions.len()).expect("render mesh fits u32"),
                        );
                        out.positions.push(p.to_array());
                        out.normals.push(normal);
                        out.uvs.push(uv);
                    }
                }
            }
            previous.push(triangle);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cube(offset: Vec3) -> ConvexRenderMesh {
        let points: Vec<_> = [-1.0, 1.0]
            .into_iter()
            .flat_map(|x| {
                [-1.0, 1.0].into_iter().flat_map(move |y| {
                    [-1.0, 1.0]
                        .into_iter()
                        .map(move |z| offset + Vec3::new(x, y, z))
                })
            })
            .collect();
        ConvexRenderMesh::from_convex_hull(&points).unwrap()
    }
    fn top_area(m: &ConvexRenderMesh) -> f32 {
        m.indices
            .chunks_exact(3)
            .filter_map(|t| {
                let p = t
                    .iter()
                    .map(|i| Vec3::from_array(m.positions[*i as usize]))
                    .collect::<Vec<_>>();
                let cross = (p[1] - p[0]).cross(p[2] - p[0]);
                (cross.y > 0.0).then_some(cross.length() * 0.5)
            })
            .sum()
    }
    #[test]
    fn duplicate_and_partially_overlapping_caps_draw_each_area_once() {
        let original = cube(Vec3::ZERO);
        assert!(
            (top_area(&merge_coplanar_surfaces(&[
                original.clone(),
                original.clone()
            ])) - 4.0)
                .abs()
                < 0.001
        );
        let joined = merge_coplanar_surfaces(&[original, cube(Vec3::X)]);
        assert!(
            (top_area(&joined) - 6.0).abs() < 0.001,
            "area={}",
            top_area(&joined)
        );
        for n in joined.normals {
            assert!(Vec3::from_array(n).is_finite());
        }
    }
    #[test]
    fn adjacent_faces_and_separate_heights_are_preserved() {
        assert!(
            (top_area(&merge_coplanar_surfaces(&[
                cube(Vec3::ZERO),
                cube(Vec3::X * 2.0)
            ])) - 8.0)
                .abs()
                < 0.001
        );
        assert!(
            (top_area(&merge_coplanar_surfaces(&[
                cube(Vec3::ZERO),
                cube(Vec3::Y * 3.0)
            ])) - 8.0)
                .abs()
                < 0.001
        );
    }
}
