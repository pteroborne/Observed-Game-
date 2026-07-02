//! Dynamic navmesh generation for the production first-person game.
//!
//! Converts the current Place geometry and collision solids into a 2D polyanya
//! navmesh for pathfinding.

use bevy::math::Vec2;
use observed_traversal::{FpsArena, FpsConfig};
use vleue_navigator::NavMesh;

use crate::teleport::PlaceGeom;

/// Builds a 2D navmesh of the walkable area inside a room or hallway.
pub fn build_navmesh(geom: &PlaceGeom, _arena: &FpsArena, config: &FpsConfig) -> NavMesh {
    let half = geom.half;
    // Use a tiny epsilon margin to ensure start/goal points near outer walls
    // lie safely inside the walkable area of the navmesh.
    let margin = 0.01;

    if let Some(edges) = &geom.poly {
        // Rooms are empty convex polygons; no interior obstacles.
        return NavMesh::from_edge_and_obstacles(edges.clone(), vec![]);
    }

    // Hallway: shrunken outer boundary rectangle.
    let edges = vec![
        Vec2::new(-half.x + margin, -half.y + margin),
        Vec2::new(half.x - margin, -half.y + margin),
        Vec2::new(half.x - margin, half.y - margin),
        Vec2::new(-half.x + margin, half.y - margin),
    ];

    // Project and inflate only the interior obstacles from the geometry.
    let obs_margin = config.radius + 0.05;
    let mut obstacles = Vec::new();
    for seg in &geom.interior {
        let min_x = seg.center.x - seg.half.x - obs_margin;
        let max_x = seg.center.x + seg.half.x + obs_margin;
        let min_z = seg.center.y - seg.half.y - obs_margin;
        let max_z = seg.center.y + seg.half.y + obs_margin;

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

    NavMesh::from_edge_and_obstacles(edges, obstacles)
}
