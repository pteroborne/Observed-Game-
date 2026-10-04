//! Batched shelf bindings, reading-perch metalwork and ceiling panels.
//! Details sit within authored shelves; collision and observation use their shell.
use super::{
    assets::{HexWfcVisualAssets, MeshGroupKey},
    spectate::Cutaway,
};
use bevy::prelude::*;
use observed_hex::{HexCoord, HexFace, hex_origin};

fn turn(mut p: Vec3, heading: HexFace) -> Vec3 {
    for _ in 0..heading.index() {
        p = Vec3::new(0.5 * p.x - 0.875 * p.z, p.y, 6.0 / 7.0 * p.x + 0.5 * p.z);
    }
    p
}
pub(super) fn cuboid(center: Vec3, size: Vec3, axis: Vec3, heading: HexFace) -> Vec<Vec3> {
    let across = Vec3::new(-axis.z, 0.0, axis.x);
    let mut points = Vec::with_capacity(8);
    for x in [-0.5, 0.5] {
        for y in [-0.5, 0.5] {
            for z in [-0.5, 0.5] {
                points.push(turn(
                    center + axis * x * size.x + Vec3::Y * y * size.y + across * z * size.z,
                    heading,
                ));
            }
        }
    }
    points
}

pub(super) fn spawn(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    coord: HexCoord,
    heading: HexFace,
) -> usize {
    let mut groups: [Vec<Vec<Vec3>>; 8] = std::array::from_fn(|_| Vec::new());
    for face in [HexFace::SouthWest, HexFace::West, HexFace::NorthEast] {
        let [a, b] = observed_hex::face_edge(face);
        let a = Vec3::new(a.0 as f32, 0.0, a.1 as f32);
        let b = Vec3::new(b.0 as f32, 0.0, b.1 as f32);
        let axis = (b - a).normalize();
        let inward = -((a + b) * 0.5).normalize();
        // Repeat packed bindings within each shelf's non-traversable depth.
        for (row, base) in [1.1875, 2.4375, 4.1875, 5.5625, 6.9375]
            .into_iter()
            .enumerate()
        {
            let mut distance = 0.7;
            let mut index = 0usize;
            while distance < (b - a).length() - 0.7 {
                let hash = (index * 37 + row * 17 + face.index() * 13) as f32;
                let width = 0.10 + (hash % 5.0) * 0.018;
                let height = 0.52 + (hash % 7.0) * 0.037;
                let at = a
                    + axis * (distance + width * 0.5)
                    + inward * 0.96
                    + Vec3::Y * (base + height * 0.5);
                let binding = (index + row * 3 + face.index()) % 6;
                groups[binding].push(cuboid(at, Vec3::new(width, height, 0.56), axis, heading));
                // Two small embossed spine bands, inset and non-emissive.
                for fraction in [0.18, 0.82] {
                    let band = at + inward * 0.286 + Vec3::Y * (height * (fraction - 0.5));
                    groups[6].push(cuboid(
                        band,
                        Vec3::new(width * 0.82, 0.022, 0.009),
                        axis,
                        heading,
                    ));
                }
                distance += width + 0.016;
                index += 1;
            }
        }
        // Tall repeated bookcase uprights make shelves read as bays.
        for t in [0.16, 0.5, 0.84] {
            groups[6].push(cuboid(
                a.lerp(b, t) + inward * 1.05 + Vec3::Y * 3.8,
                Vec3::new(0.09, 6.6, 0.08),
                axis,
                heading,
            ));
        }
    }
    // Fine bronze detail belongs to the solid authored perch rail.
    for x in [-5.8, -5.2, -4.6, -4.0, -3.4] {
        groups[6].push(cuboid(
            Vec3::new(x, 3.51, 2.94),
            Vec3::new(0.055, 0.80, 0.04),
            Vec3::X,
            heading,
        ));
    }
    // A luminous ceiling panel frames each authored downlight; it is architectural light.
    for (x, z) in [(-2.25, -3.0), (4.125, -1.5), (0.0, 4.0)] {
        groups[7].push(cuboid(
            Vec3::new(x, 7.69, z),
            Vec3::new(3.0, 0.025, 2.0),
            Vec3::X,
            heading,
        ));
        for dx in [-1.5, -0.5, 0.5, 1.5] {
            groups[6].push(cuboid(
                Vec3::new(x + dx, 7.66, z),
                Vec3::new(0.045, 0.08, 2.05),
                Vec3::X,
                heading,
            ));
        }
    }
    let origin = Vec3::from_array(hex_origin(coord));
    let mut count = 0;
    for (index, hulls) in groups.iter().enumerate() {
        let refs: Vec<_> = hulls.iter().map(Vec::as_slice).collect();
        let key = format!("archive-details-{index}-{}", heading.index());
        let Some(mesh) = assets.merged_mesh_for(meshes, Some(&key), MeshGroupKey::Interior, &refs)
        else {
            continue;
        };
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(assets.archive_detail(index)),
            Transform::from_translation(origin),
            ChildOf(parent),
            Cutaway {
                local: Vec3::ZERO,
                min_y: 0.5,
                max_y: 7.72,
                origin_y: origin.y,
                cell_level: coord.level,
                climb_wall: false,
            },
            Name::new("Archive shelf bindings and gallery detail"),
        ));
        count += 1;
    }
    count
}
