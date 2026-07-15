//! Dynamic navmesh generation for the production first-person game.
//!
//! Converts the current Place geometry and collision solids into a 2D polyanya
//! navmesh for pathfinding.

use bevy::math::Vec2;
use observed_traversal::{FpsConfig, rapier_controller::StructuralCollider};
use vleue_navigator::NavMesh;

use crate::teleport::PlaceGeom;

/// Builds a 2D navmesh of the walkable area inside a room or hallway.
pub fn build_navmesh(
    geom: &PlaceGeom,
    _primitives: &[StructuralCollider],
    config: &FpsConfig,
) -> NavMesh {
    let half = geom.half;
    // Use a tiny epsilon margin to ensure start/goal points near outer walls
    // lie safely inside the walkable area of the navmesh.
    let margin = 0.01;

    let edges = geom.poly.clone().unwrap_or_else(|| {
        // Hallway: shrunken outer boundary rectangle.
        vec![
            Vec2::new(-half.x + margin, -half.y + margin),
            Vec2::new(half.x - margin, -half.y + margin),
            Vec2::new(half.x - margin, half.y - margin),
            Vec2::new(-half.x + margin, half.y - margin),
        ]
    });

    // Project and inflate only the interior obstacles from the geometry.
    let obs_margin = config.radius + 0.05;
    let mut obstacles = Vec::new();
    for seg in &geom.interior {
        let min_x =
            (seg.center.x - seg.half.x - obs_margin).clamp(-half.x + margin, half.x - margin);
        let max_x =
            (seg.center.x + seg.half.x + obs_margin).clamp(-half.x + margin, half.x - margin);
        let min_z =
            (seg.center.y - seg.half.y - obs_margin).clamp(-half.y + margin, half.y - margin);
        let max_z =
            (seg.center.y + seg.half.y + obs_margin).clamp(-half.y + margin, half.y - margin);

        if min_x < max_x && min_z < max_z {
            // CCW rectangle corners
            let poly = vec![
                Vec2::new(min_x, min_z),
                Vec2::new(max_x, min_z),
                Vec2::new(max_x, max_z),
                Vec2::new(min_x, max_z),
            ];
            obstacles.push(poly);
        }
    }

    // New architecture may use yawed walls and platforms. Only solids intersecting
    // the ground-layer capsule belong in this 2D mesh; raised gantry routes have their
    // own traversal pilot and must not erase the understory route below them.
    let body_top = config.half_height * 2.0 + config.step_height;
    for solid in &geom.oriented_solids {
        if solid.bottom_y > body_top || solid.top_y <= config.step_height {
            continue;
        }
        let half = solid.half + Vec2::splat(obs_margin);
        let (sin_yaw, cos_yaw) = solid.yaw.sin_cos();
        let rotate = |local: Vec2| {
            solid.center
                + Vec2::new(
                    cos_yaw * local.x + sin_yaw * local.y,
                    -sin_yaw * local.x + cos_yaw * local.y,
                )
        };
        obstacles.push(vec![
            rotate(Vec2::new(-half.x, -half.y)),
            rotate(Vec2::new(half.x, -half.y)),
            rotate(Vec2::new(half.x, half.y)),
            rotate(Vec2::new(-half.x, half.y)),
        ]);
    }
    for solid in &geom.convex_solids {
        if solid.footprint.len() < 3
            || solid.bottom_y > body_top
            || solid.top_y <= config.step_height
        {
            continue;
        }
        let centre = solid.footprint.iter().copied().sum::<Vec2>() / solid.footprint.len() as f32;
        obstacles.push(
            solid
                .footprint
                .iter()
                .map(|point| {
                    let radial = (*point - centre).normalize_or_zero();
                    *point + radial * obs_margin * 1.5
                })
                .collect(),
        );
    }

    NavMesh::from_edge_and_obstacles(edges, obstacles)
}
