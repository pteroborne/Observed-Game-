//! Sampling the authored hulls: what is underfoot, and what is overhead.
//!
//! Split out of `walk` because it answers a different question. This is pure
//! geometry - "what surface is at this point" - with no notion of a body, a
//! step, or a route. `walk` supplies all of that.

use bevy::prelude::*;
use observed_authoring::TilePrototype;
use observed_traversal::ConvexRenderMesh;

/// The module's surfaces, triangulated once.
pub struct Probe {
    triangles: Vec<[Vec3; 3]>,
}

impl Probe {
    #[must_use]
    pub fn from_prototype(prototype: &TilePrototype) -> Self {
        let mut triangles = Vec::new();
        for hull in &prototype.hulls {
            let Some(mesh) = ConvexRenderMesh::from_convex_hull(hull) else {
                continue;
            };
            for face in mesh.indices.chunks_exact(3) {
                let point = |index: u32| {
                    let p = mesh.positions[index as usize];
                    Vec3::new(p[0], p[1], p[2])
                };
                triangles.push([point(face[0]), point(face[1]), point(face[2])]);
            }
        }
        Self { triangles }
    }

    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// The highest surface at `(x, z)` no higher than `ceiling`.
    ///
    /// Bounded above rather than taking the global maximum, because the global
    /// maximum is the roof. Walking is always "what is under my feet, reachable
    /// from where I am", which is what the bound expresses.
    #[must_use]
    pub fn support(&self, x: f32, z: f32, ceiling: f32) -> Option<f32> {
        let mut best: Option<f32> = None;
        for tri in &self.triangles {
            let Some(y) = height_at(tri, x, z) else {
                continue;
            };
            if y <= ceiling && best.is_none_or(|b| y > b) {
                best = Some(y);
            }
        }
        best
    }

    /// Clear height above `floor` at `(x, z)`; infinite when nothing is over it.
    #[must_use]
    pub fn overhead(&self, x: f32, z: f32, floor: f32) -> f32 {
        let mut lowest = f32::INFINITY;
        for tri in &self.triangles {
            let Some(y) = height_at(tri, x, z) else {
                continue;
            };
            // A surface within a hair of the floor is the floor, not a ceiling.
            if y > floor + 0.05 && y < lowest {
                lowest = y;
            }
        }
        lowest - floor
    }
}

/// Where `(x, z)` meets a triangle, if it is inside its plan projection.
fn height_at(tri: &[Vec3; 3], x: f32, z: f32) -> Option<f32> {
    let (a, b, c) = (tri[0], tri[1], tri[2]);
    // Barycentric in plan. A degenerate triangle - one seen edge-on - has zero
    // plan area and no answer here, which is correct: you cannot stand on it.
    let det = (b.z - c.z) * (a.x - c.x) + (c.x - b.x) * (a.z - c.z);
    if det.abs() < 1e-6 {
        return None;
    }
    let l1 = ((b.z - c.z) * (x - c.x) + (c.x - b.x) * (z - c.z)) / det;
    let l2 = ((c.z - a.z) * (x - c.x) + (a.x - c.x) * (z - c.z)) / det;
    let l3 = 1.0 - l1 - l2;
    const EDGE: f32 = -1e-4;
    (l1 >= EDGE && l2 >= EDGE && l3 >= EDGE).then_some(l1 * a.y + l2 * b.y + l3 * c.y)
}
impl Probe {
    /// A probe over triangles given directly, for synthetic test surfaces.
    #[must_use]
    pub fn from_triangles(triangles: Vec<[Vec3; 3]>) -> Self {
        Self { triangles }
    }
}
