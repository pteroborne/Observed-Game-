//! Meshes for the eye's shapes: smooth, because an eye is the one round thing in a
//! facility of hexagons, and its roundness is the silhouette.
use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::math::primitives::{Sphere, Torus};
use bevy::mesh::{Indices, Mesh, MeshBuilder, Meshable, PrimitiveTopology};

use crate::form::Shape;

/// Segments round the globe: enough that its outline never reads as a polygon at the
/// distance an eye is noticed.
const AROUND: u32 = 40;
const DOWN: u32 = 20;

/// The mesh for a shape.
#[must_use]
pub fn mesh(shape: Shape) -> Mesh {
    match shape {
        Shape::Sphere { radius } => Sphere::new(radius).mesh().uv(AROUND, DOWN),
        Shape::Torus { ring, tube } => Torus {
            minor_radius: tube,
            major_radius: ring,
        }
        .mesh()
        .build(),
        Shape::Lid { radius } => lid(radius),
    }
}

/// A hemispherical shell, dome up, its rim on the XZ plane: faces both ways, since an
/// open lid is seen from inside.
fn lid(radius: f32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for side in [1.0_f32, -1.0] {
        let base = u32::try_from(positions.len()).expect("small mesh");
        for ring in 0..=DOWN / 2 {
            #[allow(clippy::cast_precision_loss)]
            let polar = FRAC_PI_2 * ring as f32 / (DOWN / 2) as f32;
            for step in 0..=AROUND {
                #[allow(clippy::cast_precision_loss)]
                let around = TAU * step as f32 / AROUND as f32;
                let normal = [
                    polar.sin() * around.cos(),
                    polar.cos(),
                    polar.sin() * around.sin(),
                ];
                positions.push(normal.map(|n| n * radius));
                normals.push(normal.map(|n| n * side));
                #[allow(clippy::cast_precision_loss)]
                uvs.push([step as f32 / AROUND as f32, ring as f32 / (DOWN / 2) as f32]);
            }
        }
        let row = AROUND + 1;
        for ring in 0..DOWN / 2 {
            for step in 0..AROUND {
                let a = base + ring * row + step;
                let (b, c, d) = (a + 1, a + row, a + row + 1);
                if side > 0.0 {
                    indices.extend([a, b, c, b, d, c]);
                } else {
                    indices.extend([a, c, b, b, c, d]);
                }
            }
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec3;
    use bevy::mesh::VertexAttributeValues;

    use super::*;

    #[test]
    fn a_lid_faces_out_and_in() {
        let mesh = lid(1.0);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("normals");
        };
        let (mut out, mut inward) = (0, 0);
        for (p, n) in positions.iter().zip(normals) {
            let (p, n) = (Vec3::from(*p), Vec3::from(*n));
            assert!(p.y >= -1e-6, "a lid is a dome up: {p}");
            if n.dot(p) > 0.0 {
                out += 1;
            } else {
                inward += 1;
            }
        }
        assert_eq!(out, inward);
    }

    #[test]
    fn a_lid_is_wound_the_way_it_faces() {
        let mesh = lid(1.0);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("normals");
        };
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("indices");
        };
        for triangle in indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(positions[triangle[k] as usize]));
            let face = (b - a).cross(c - a);
            if face.length() < 1e-7 {
                continue; // the degenerate slivers at the crown
            }
            let normal = Vec3::from(normals[triangle[0] as usize]);
            assert!(
                face.dot(normal) > 0.0,
                "a triangle faces against its normal"
            );
        }
    }
}
