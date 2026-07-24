//! Bevy-free render data derived from the exact convex hulls used by Rapier.
//!
//! Simulation continues to consume the original hull points. Presentation can
//! turn [`ConvexRenderMesh`] into its engine mesh without repeating
//! triangulation, smoothing, or UV policy in every Bevy adapter.

use std::collections::HashMap;

use glam::Vec3;
use rapier3d::prelude::{SharedShape, Vector};

/// Faces whose normals meet within roughly 45 degrees share a vertex normal.
/// Steeper construction edges remain crisp.
pub const DEFAULT_SMOOTH_CREASE_COS: f32 = 0.70;
/// Box-projected texture repetition in cycles per world metre.
pub const DEFAULT_UV_REPEATS_PER_METER: f32 = 0.25;

/// Engine-independent triangle mesh for one convex render/collision hull.
#[derive(Clone, Debug, PartialEq)]
pub struct ConvexRenderMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

impl ConvexRenderMesh {
    /// Triangulate one convex hull, smooth only shallow creases, and apply
    /// dominant-axis box UVs. Returns `None` for a degenerate hull.
    #[must_use]
    pub fn from_convex_hull(points: &[Vec3]) -> Option<Self> {
        Self::from_convex_hull_with(
            points,
            DEFAULT_SMOOTH_CREASE_COS,
            DEFAULT_UV_REPEATS_PER_METER,
        )
    }

    /// Parameterized form used by focused tests and specialized inspectors.
    #[must_use]
    pub fn from_convex_hull_with(
        points: &[Vec3],
        smooth_crease_cos: f32,
        uv_repeats_per_meter: f32,
    ) -> Option<Self> {
        let points = points
            .iter()
            .map(|point| Vector::new(point.x, point.y, point.z))
            .collect::<Vec<_>>();
        let shape = SharedShape::convex_hull(&points)?;
        let (vertices, mut triangles) = shape.as_convex_polyhedron()?.to_trimesh();

        // Rapier guarantees the convex surface but does not promise the
        // engine-facing winding expected by back-face culling. Orient every
        // triangle away from the hull centroid before deriving normals.
        let centroid = vertices
            .iter()
            .map(|point| Vec3::new(point.x, point.y, point.z))
            .sum::<Vec3>()
            / vertices.len() as f32;
        for triangle in &mut triangles {
            let point = |index: u32| {
                let point = vertices[index as usize];
                Vec3::new(point.x, point.y, point.z)
            };
            let a = point(triangle[0]);
            let b = point(triangle[1]);
            let c = point(triangle[2]);
            let normal = (b - a).cross(c - a);
            if normal.dot((a + b + c) / 3.0 - centroid) < 0.0 {
                triangle.swap(1, 2);
            }
        }

        let triangle_normal = |triangle: &[u32; 3]| {
            let point = |index: u32| {
                let point = vertices[index as usize];
                Vec3::new(point.x, point.y, point.z)
            };
            (point(triangle[1]) - point(triangle[0]))
                .cross(point(triangle[2]) - point(triangle[0]))
                .normalize_or_zero()
        };
        let face_normals = triangles.iter().map(triangle_normal).collect::<Vec<_>>();

        // Rapier's trimesh may reuse or duplicate vertices. Quantized position
        // adjacency handles both while remaining deterministic for authored
        // metre-scale hulls.
        let position_key = |point: &Vector| {
            (
                (point.x * 1024.0).round() as i64,
                (point.y * 1024.0).round() as i64,
                (point.z * 1024.0).round() as i64,
            )
        };
        let mut adjacent = HashMap::<(i64, i64, i64), Vec<usize>>::new();
        for (triangle_index, triangle) in triangles.iter().enumerate() {
            for &vertex in triangle {
                adjacent
                    .entry(position_key(&vertices[vertex as usize]))
                    .or_default()
                    .push(triangle_index);
            }
        }

        // Deliberately duplicate every triangle corner: UV seams and hard
        // creases then need no engine-specific vertex splitting.
        let mut positions = Vec::with_capacity(triangles.len() * 3);
        let mut normals = Vec::with_capacity(triangles.len() * 3);
        let mut uvs = Vec::with_capacity(triangles.len() * 3);
        for (triangle_index, triangle) in triangles.iter().enumerate() {
            let face_normal = face_normals[triangle_index];
            for &vertex in triangle {
                let point = vertices[vertex as usize];
                let normal = adjacent[&position_key(&point)]
                    .iter()
                    .map(|&other| face_normals[other])
                    .filter(|other| face_normal.dot(*other) >= smooth_crease_cos)
                    .fold(Vec3::ZERO, |sum, other| sum + other)
                    .normalize_or(face_normal);
                positions.push([point.x, point.y, point.z]);
                normals.push(normal.to_array());

                let absolute = face_normal.abs();
                let uv = if absolute.y >= absolute.x && absolute.y >= absolute.z {
                    [point.x, point.z]
                } else if absolute.x >= absolute.z {
                    [point.z, point.y]
                } else {
                    [point.x, point.y]
                };
                uvs.push([uv[0] * uv_repeats_per_meter, uv[1] * uv_repeats_per_meter]);
            }
        }
        let indices = (0..u32::try_from(positions.len()).ok()?).collect();
        Some(Self {
            positions,
            normals,
            uvs,
            indices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> Vec<Vec3> {
        [-1.0, 1.0]
            .into_iter()
            .flat_map(|x| {
                [-1.0, 1.0]
                    .into_iter()
                    .flat_map(move |y| [-1.0, 1.0].into_iter().map(move |z| Vec3::new(x, y, z)))
            })
            .collect()
    }

    #[test]
    fn hard_cube_edges_keep_distinct_face_normals() {
        let mesh = ConvexRenderMesh::from_convex_hull(&cube()).expect("cube mesh");
        let corner = [1.0, 1.0, 1.0];
        let normals = mesh
            .positions
            .iter()
            .zip(&mesh.normals)
            .filter_map(|(position, normal)| (*position == corner).then_some(*normal))
            .collect::<Vec<_>>();
        assert!(normals.len() >= 3);
        assert!(normals.iter().any(|normal| normal[0].abs() > 0.99));
        assert!(normals.iter().any(|normal| normal[1].abs() > 0.99));
        assert!(normals.iter().any(|normal| normal[2].abs() > 0.99));
    }

    #[test]
    fn shallow_chamfer_blends_with_the_top_face() {
        let mut points = Vec::new();
        for z in [-1.0, 1.0] {
            for (x, y) in [
                (-1.0, 0.0),
                (1.0, 0.0),
                (1.0, 0.8),
                (0.8, 1.0),
                (-0.8, 1.0),
                (-1.0, 0.8),
            ] {
                points.push(Vec3::new(x, y, z));
            }
        }
        let mesh = ConvexRenderMesh::from_convex_hull(&points).expect("chamfered prism");
        let blended = mesh
            .positions
            .iter()
            .zip(&mesh.normals)
            .filter(|(position, _)| {
                (position[0] - 0.8).abs() < 1e-5 && (position[1] - 1.0).abs() < 1e-5
            })
            .any(|(_, normal)| normal[0] > 0.1 && normal[1] > 0.5);
        assert!(blended, "the 45-degree rim shades into the horizontal top");
    }

    #[test]
    fn box_projection_uses_the_dominant_face_axis() {
        let mesh = ConvexRenderMesh::from_convex_hull(&cube()).expect("cube mesh");
        for ((position, normal), uv) in mesh.positions.iter().zip(&mesh.normals).zip(&mesh.uvs) {
            let normal = Vec3::from_array(*normal).abs();
            let expected = if normal.y >= normal.x && normal.y >= normal.z {
                [position[0] * 0.25, position[2] * 0.25]
            } else if normal.x >= normal.z {
                [position[2] * 0.25, position[1] * 0.25]
            } else {
                [position[0] * 0.25, position[1] * 0.25]
            };
            assert_eq!(*uv, expected);
        }
    }

    #[test]
    fn triangle_winding_faces_away_from_the_hull_centroid() {
        let mesh = ConvexRenderMesh::from_convex_hull(&cube()).expect("cube mesh");
        for triangle in mesh.positions.chunks_exact(3) {
            let [a, b, c] = triangle else {
                unreachable!("chunks_exact returns three positions")
            };
            let (a, b, c) = (
                Vec3::from_array(*a),
                Vec3::from_array(*b),
                Vec3::from_array(*c),
            );
            let normal = (b - a).cross(c - a);
            assert!(normal.dot((a + b + c) / 3.0) > 0.0);
        }
    }
}
