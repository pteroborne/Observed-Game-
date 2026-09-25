//! Meshes for the forms' shapes: flat-shaded hexagons and panels, and fading light.
//!
//! Hexagon corners sit 30 degrees either side of +Z, so a face looks straight down +Z,
//! where the forms put their eyes.
use std::f32::consts::{FRAC_PI_3, FRAC_PI_6, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::form::Shape;

fn corner(i: usize, radius: f32) -> Vec2 {
    #[allow(clippy::cast_precision_loss)]
    let angle = FRAC_PI_6 + FRAC_PI_3 * (i % 6) as f32;
    Vec2::new(angle.sin(), angle.cos()) * radius
}

fn at(plan: Vec2, y: f32) -> Vec3 {
    Vec3::new(plan.x, y, plan.y)
}

#[derive(Default)]
struct Builder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Builder {
    /// A flat convex polygon, wound to face `normal`, its vertex colour from `shade`.
    fn face(&mut self, points: &[Vec3], normal: Vec3, shade: impl Fn(Vec3) -> f32) {
        let base = u32::try_from(self.positions.len()).expect("small mesh");
        let forward = (points[1] - points[0])
            .cross(points[2] - points[0])
            .dot(normal)
            >= 0.0;
        for &p in points {
            let s = shade(p);
            self.positions.push(p.to_array());
            self.normals.push(normal.to_array());
            self.colors.push([s, s, s, 1.0]);
        }
        let n = u32::try_from(points.len()).expect("small polygon");
        for k in 1..n - 1 {
            if forward {
                self.indices.extend([base, base + k, base + k + 1]);
            } else {
                self.indices.extend([base, base + k + 1, base + k]);
            }
        }
    }

    /// A side quad between two edges, facing away from `centre` (or toward it).
    fn side(&mut self, quad: [Vec3; 4], centre: Vec3, inward: bool) {
        let normal = (quad[1] - quad[0]).cross(quad[3] - quad[0]).normalize();
        let mid = (quad[0] + quad[1] + quad[2] + quad[3]) * 0.25;
        let away = if normal.dot(mid - centre) >= 0.0 {
            normal
        } else {
            -normal
        };
        self.face(&quad, if inward { -away } else { away }, |_| 1.0);
    }

    fn mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

fn hex_frustum(bottom: f32, top: f32, height: f32) -> Mesh {
    let mut b = Builder::default();
    let low: Vec<Vec3> = (0..6).map(|i| at(corner(i, bottom), 0.0)).collect();
    let high: Vec<Vec3> = (0..6).map(|i| at(corner(i, top), height)).collect();
    b.face(&high, Vec3::Y, |_| 1.0);
    b.face(&low, -Vec3::Y, |_| 1.0);
    let centre = Vec3::Y * height * 0.5;
    for i in 0..6 {
        let j = (i + 1) % 6;
        b.side([low[i], low[j], high[j], high[i]], centre, false);
    }
    b.mesh()
}

fn hex_ring(outer: f32, inner: f32, height: f32) -> Mesh {
    let mut b = Builder::default();
    for i in 0..6 {
        let j = (i + 1) % 6;
        let (oi, oj, ii, ij) = (
            corner(i, outer),
            corner(j, outer),
            corner(i, inner),
            corner(j, inner),
        );
        for (y, normal) in [(height, Vec3::Y), (0.0, -Vec3::Y)] {
            b.face(
                &[at(ii, y), at(oi, y), at(oj, y), at(ij, y)],
                normal,
                |_| 1.0,
            );
        }
        let centre = Vec3::Y * height * 0.5;
        b.side(
            [at(oi, 0.0), at(oj, 0.0), at(oj, height), at(oi, height)],
            centre,
            false,
        );
        b.side(
            [at(ii, 0.0), at(ij, 0.0), at(ij, height), at(ii, height)],
            centre,
            true,
        );
    }
    b.mesh()
}

fn panel(corners: [Vec3; 3], inward: Vec3, thickness: f32) -> Mesh {
    let mut b = Builder::default();
    let outer = corners;
    let inner = corners.map(|p| p + inward * thickness);
    let normal = (outer[1] - outer[0]).cross(outer[2] - outer[0]).normalize();
    let out = if normal.dot(inward) > 0.0 {
        -normal
    } else {
        normal
    };
    b.face(&outer, out, |_| 1.0);
    b.face(&inner, -out, |_| 1.0);
    let centre = (outer[0] + outer[1] + outer[2] + inner[0] + inner[1] + inner[2]) / 6.0;
    for i in 0..3 {
        let j = (i + 1) % 3;
        b.side([outer[i], outer[j], inner[j], inner[i]], centre, false);
    }
    b.mesh()
}

/// A cone of light from the apex along +Z, brightest at the eye and gone by its end.
/// Both sides of its walls face out, so it reads from inside too.
fn beam(length: f32, radius: f32) -> Mesh {
    let mut b = Builder::default();
    let sides = 16;
    let fade = |p: Vec3| {
        let u = 1.0 - (p.z / length).clamp(0.0, 1.0);
        u * u
    };
    for i in 0..sides {
        #[allow(clippy::cast_precision_loss)]
        let (a0, a1) = (
            TAU * i as f32 / sides as f32,
            TAU * (i + 1) as f32 / sides as f32,
        );
        let rim = |a: f32| Vec3::new(a.cos() * radius, a.sin() * radius, length);
        let tri = [Vec3::ZERO, rim(a0), rim(a1)];
        let normal = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize();
        b.face(&tri, normal, fade);
        b.face(&tri, -normal, fade);
    }
    b.mesh()
}

/// A flat hexagon of light, brightest at its rim, where the catch has just spread to.
fn stamp(radius: f32) -> Mesh {
    let mut b = Builder::default();
    for i in 0..6 {
        let j = (i + 1) % 6;
        b.face(
            &[
                Vec3::ZERO,
                at(corner(i, radius), 0.0),
                at(corner(j, radius), 0.0),
            ],
            Vec3::Y,
            |p| 0.25 + 0.75 * (p.length() / radius),
        );
    }
    b.mesh()
}

#[must_use]
pub fn mesh(shape: Shape) -> Mesh {
    match shape {
        Shape::HexFrustum {
            bottom,
            top,
            height,
        } => hex_frustum(bottom, top, height),
        Shape::HexRing {
            outer,
            inner,
            height,
        } => hex_ring(outer, inner, height),
        Shape::Sphere { radius } => Sphere::new(radius).mesh().uv(32, 18),
        Shape::Torus { major, minor } => Torus::new(major - minor, major + minor)
            .mesh()
            .major_resolution(64)
            .minor_resolution(12)
            .build(),
        Shape::Panel {
            corners,
            inward,
            thickness,
        } => panel(corners, inward, thickness),
        Shape::Rod { radius, length } => Cylinder::new(radius, length)
            .mesh()
            .resolution(24)
            .build()
            .translated_by(Vec3::Y * length * 0.5),
        Shape::Beam { length, radius } => beam(length, radius),
        Shape::Stamp { radius } => stamp(radius),
    }
}

#[cfg(test)]
mod tests {
    use bevy::mesh::{Indices, VertexAttributeValues};
    use bevy::prelude::*;

    use super::mesh;
    use crate::form::{Form, parts};

    /// Every flat-shaded face is wound toward its normal, so nothing is culled from the
    /// side it should be seen from.
    #[test]
    fn every_face_of_every_form_is_wound_toward_its_normal() {
        for form in [Form::Tumbler { tiers: 4 }, Form::Plumb, Form::Roller] {
            for part in parts(form) {
                let mesh = mesh(part.shape);
                let (
                    Some(VertexAttributeValues::Float32x3(positions)),
                    Some(VertexAttributeValues::Float32x3(normals)),
                    Some(Indices::U32(indices)),
                ) = (
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION),
                    mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
                    mesh.indices(),
                )
                else {
                    continue;
                };
                // Built-in primitives are smooth; only our flat faces are checked.
                if mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_none() {
                    continue;
                }
                for t in indices.chunks(3) {
                    let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(positions[i as usize]));
                    let area = (b - a).cross(c - a);
                    if area.length() < 1e-9 {
                        continue;
                    }
                    let n = Vec3::from(normals[t[0] as usize]);
                    assert!(area.normalize().dot(n) > 0.99, "{:?}", part.shape);
                }
            }
        }
    }
}
