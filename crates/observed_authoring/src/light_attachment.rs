//! Geometry-backed fixture attachments. No nominal storey or ceiling heights.
use glam::Vec3;
use observed_traversal::ConvexRenderMesh;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct LightAttachment {
    pub position: [f32; 3],
    /// Outward normal points from the mounting surface into the lit space.
    pub normal: [f32; 3],
}

impl LightAttachment {
    #[must_use]
    pub fn transformed(self, rotation: glam::Quat, translation: Vec3) -> Self {
        Self {
            position: (translation + rotation * Vec3::from_array(self.position)).to_array(),
            normal: (rotation * Vec3::from_array(self.normal)).to_array(),
        }
    }
}

/// Find a real ceiling, nearby wall, or support below a freestanding lamp.
/// The source stays where authored; only visible hardware attaches to the support.
#[must_use]
pub fn light_attachment(source: Vec3, hulls: &[Vec<Vec3>]) -> Option<LightAttachment> {
    let triangles: Vec<_> = hulls
        .iter()
        .filter_map(|h| ConvexRenderMesh::from_convex_hull(h))
        .flat_map(|mesh| {
            mesh.indices
                .chunks_exact(3)
                .map(|t| {
                    [
                        Vec3::from_array(mesh.positions[t[0] as usize]),
                        Vec3::from_array(mesh.positions[t[1] as usize]),
                        Vec3::from_array(mesh.positions[t[2] as usize]),
                    ]
                })
                .collect::<Vec<_>>()
        })
        .collect();
    for direction in [Vec3::Y, Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z, -Vec3::Y] {
        let reach = if direction == Vec3::Y { 16.0 } else { 3.0 };
        let nearest = triangles
            .iter()
            .filter_map(|&[a, b, c]| {
                let normal = (b - a).cross(c - a).normalize_or_zero();
                let denominator = direction.dot(normal);
                if denominator >= if direction == Vec3::Y { -0.9 } else { -0.4 } {
                    return None;
                }
                let distance = (a - source).dot(normal) / denominator;
                if !(0.0..=reach).contains(&distance) {
                    return None;
                }
                let point = source + direction * distance;
                let inside = [(a, b), (b, c), (c, a)]
                    .into_iter()
                    .all(|(a, b)| (b - a).cross(point - a).dot(normal) >= -0.002);
                inside.then_some((
                    distance,
                    LightAttachment {
                        position: point.to_array(),
                        normal: normal.to_array(),
                    },
                ))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, attachment)) = nearest {
            return Some(attachment);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn slab(y: f32) -> Vec<Vec3> {
        [-2.0, 2.0]
            .into_iter()
            .flat_map(|x| {
                [y, y + 0.2]
                    .into_iter()
                    .flat_map(move |y| [-2.0, 2.0].into_iter().map(move |z| Vec3::new(x, y, z)))
            })
            .collect()
    }
    #[test]
    fn fixtures_attach_to_actual_low_and_high_ceilings() {
        for y in [3.5, 7.5, 15.5] {
            let attachment = light_attachment(Vec3::Y * 2.5, &[slab(y)]).unwrap();
            assert!((attachment.position[1] - y).abs() < 0.001);
            assert_eq!(attachment.normal, Vec3::NEG_Y.to_array());
        }
        assert!(light_attachment(Vec3::Y * 2.5, &[]).is_none());
    }
    #[test]
    fn attachment_follows_rotation_and_room_translation() {
        let mount = light_attachment(Vec3::Y * 2.5, &[slab(7.5)]).unwrap();
        let moved = mount.transformed(glam::Quat::from_rotation_y(1.0), Vec3::new(20.0, 8.0, 4.0));
        assert!((moved.position[1] - 15.5).abs() < 0.001);
    }
}
