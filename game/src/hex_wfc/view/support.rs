//! Actual authored convex supports shared by wall detail renderers.
use bevy::prelude::*;
use observed_match::hex_wfc::HexStructurePiece;
use observed_traversal::ColliderShape;

pub(super) fn points(piece: &HexStructurePiece, origin: Vec3) -> Vec<Vec3> {
    let ColliderShape::ConvexHull { points } = &piece.shape else {
        return Vec::new();
    };
    let rotation = Quat::from_array(piece.rotation);
    points
        .iter()
        .map(|&p| piece.center - origin + rotation * p)
        .collect()
}

pub(super) fn wall_frame(hull: &[Vec3], axis: Vec3, normal: Vec3) -> Option<(Vec3, Vec3, f32)> {
    inward_wall_frame(hull, axis, normal, 3.0)
}

/// `inset` allows narrow corridors to dress their actual interior screens.
pub(super) fn inward_wall_frame(
    hull: &[Vec3],
    axis: Vec3,
    normal: Vec3,
    inset: f32,
) -> Option<(Vec3, Vec3, f32)> {
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
                && plane < -inset
                && hull.iter().all(|p| p.dot(actual_normal) <= plane + 0.002))
            .then_some((edge.length_squared(), actual_axis, actual_normal, plane))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, axis, normal, plane)| (axis, normal, plane))
}
