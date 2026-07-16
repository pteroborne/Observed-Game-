//! Dynamic navmesh generation for the production first-person game.
//!
//! Converts the current Place geometry and collision solids into a 2D polyanya
//! navmesh for pathfinding.

use bevy::math::Vec2;
use observed_traversal::{FpsConfig, rapier_controller::StructuralCollider};
use vleue_navigator::NavMesh;

use crate::teleport::PlaceGeom;

fn merge_wall_segs(interior: &[crate::teleport::WallSeg]) -> Vec<crate::teleport::WallSeg> {
    if interior.is_empty() {
        return Vec::new();
    }
    let mut horizontal = Vec::new();
    let mut vertical = Vec::new();
    for seg in interior {
        if seg.half.x > seg.half.y {
            horizontal.push(*seg);
        } else {
            vertical.push(*seg);
        }
    }

    let mut merged = Vec::new();

    // Merge horizontal walls
    let mut h_groups: Vec<(f32, Vec<crate::teleport::WallSeg>)> = Vec::new();
    for seg in horizontal {
        if let Some(group) = h_groups
            .iter_mut()
            .find(|g| (g.0 - seg.center.y).abs() < 0.01)
        {
            group.1.push(seg);
        } else {
            h_groups.push((seg.center.y, vec![seg]));
        }
    }
    for (y, mut segs) in h_groups {
        segs.sort_by(|a, b| (a.center.x - a.half.x).total_cmp(&(b.center.x - b.half.x)));
        let mut current_left = segs[0].center.x - segs[0].half.x;
        let mut current_right = segs[0].center.x + segs[0].half.x;
        let half_y = segs[0].half.y;
        for seg in segs.into_iter().skip(1) {
            let left = seg.center.x - seg.half.x;
            let right = seg.center.x + seg.half.x;
            if left <= current_right + 0.05 {
                current_right = current_right.max(right);
            } else {
                let half_x = (current_right - current_left) * 0.5;
                merged.push(crate::teleport::WallSeg {
                    center: Vec2::new(current_left + half_x, y),
                    half: Vec2::new(half_x, half_y),
                });
                current_left = left;
                current_right = right;
            }
        }
        let half_x = (current_right - current_left) * 0.5;
        merged.push(crate::teleport::WallSeg {
            center: Vec2::new(current_left + half_x, y),
            half: Vec2::new(half_x, half_y),
        });
    }

    // Merge vertical walls
    let mut v_groups: Vec<(f32, Vec<crate::teleport::WallSeg>)> = Vec::new();
    for seg in vertical {
        if let Some(group) = v_groups
            .iter_mut()
            .find(|g| (g.0 - seg.center.x).abs() < 0.01)
        {
            group.1.push(seg);
        } else {
            v_groups.push((seg.center.x, vec![seg]));
        }
    }
    for (x, mut segs) in v_groups {
        segs.sort_by(|a, b| (a.center.y - a.half.y).total_cmp(&(b.center.y - b.half.y)));
        let mut current_bottom = segs[0].center.y - segs[0].half.y;
        let mut current_top = segs[0].center.y + segs[0].half.y;
        let half_x = segs[0].half.x;
        for seg in segs.into_iter().skip(1) {
            let bottom = seg.center.y - seg.half.y;
            let top = seg.center.y + seg.half.y;
            if bottom <= current_top + 0.05 {
                current_top = current_top.max(top);
            } else {
                let half_y = (current_top - current_bottom) * 0.5;
                merged.push(crate::teleport::WallSeg {
                    center: Vec2::new(x, current_bottom + half_y),
                    half: Vec2::new(half_x, half_y),
                });
                current_bottom = bottom;
                current_top = top;
            }
        }
        let half_y = (current_top - current_bottom) * 0.5;
        merged.push(crate::teleport::WallSeg {
            center: Vec2::new(x, current_bottom + half_y),
            half: Vec2::new(half_x, half_y),
        });
    }

    merged
}

/// Builds a 2D navmesh of the walkable area inside a room or hallway.
pub fn build_navmesh(
    geom: &PlaceGeom,
    _primitives: &[StructuralCollider],
    config: &FpsConfig,
) -> NavMesh {
    let half = geom.half;
    // Use a tiny epsilon margin to ensure start/goal points near outer walls
    // lie safely inside the walkable area of the navmesh.
    let boundary_margin = 0.005;
    let obstacle_clamp_margin = 0.01;

    let edges = geom.poly.clone().unwrap_or_else(|| {
        // Hallway: shrunken outer boundary rectangle.
        vec![
            Vec2::new(-half.x + boundary_margin, -half.y + boundary_margin),
            Vec2::new(half.x - boundary_margin, -half.y + boundary_margin),
            Vec2::new(half.x - boundary_margin, half.y - boundary_margin),
            Vec2::new(-half.x + boundary_margin, half.y - boundary_margin),
        ]
    });

    // Project and inflate only the interior obstacles from the geometry.
    let obs_margin = config.radius + 0.05;
    let mut obstacles = Vec::new();
    let merged_interior = merge_wall_segs(&geom.interior);
    for seg in &merged_interior {
        let min_x = (seg.center.x - seg.half.x - obs_margin).clamp(
            -half.x + obstacle_clamp_margin,
            half.x - obstacle_clamp_margin,
        );
        let max_x = (seg.center.x + seg.half.x + obs_margin).clamp(
            -half.x + obstacle_clamp_margin,
            half.x - obstacle_clamp_margin,
        );
        let min_z = (seg.center.y - seg.half.y - obs_margin).clamp(
            -half.y + obstacle_clamp_margin,
            half.y - obstacle_clamp_margin,
        );
        let max_z = (seg.center.y + seg.half.y + obs_margin).clamp(
            -half.y + obstacle_clamp_margin,
            half.y - obstacle_clamp_margin,
        );

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

    fn perturb_poly(poly: Vec<Vec2>, obs_index: usize) -> Vec<Vec2> {
        poly.into_iter()
            .enumerate()
            .map(|(v_index, v)| {
                let seed = (obs_index * 13 + v_index * 37) as f32;
                let hash_x = (seed.sin() * 43758.547).fract();
                let hash_y = ((seed + 1.234).sin() * 43758.547).fract();
                Vec2::new(v.x + hash_x * 0.0001, v.y + hash_y * 0.0001)
            })
            .collect()
    }

    let perturbed_obstacles: Vec<Vec<Vec2>> = obstacles
        .into_iter()
        .enumerate()
        .map(|(obs_index, poly)| perturb_poly(poly, obs_index))
        .collect();

    let edges_clone = edges.clone();
    let obstacles_clone = perturbed_obstacles.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        NavMesh::from_edge_and_obstacles(edges, perturbed_obstacles)
    }));
    match result {
        Ok(mesh) => mesh,
        Err(err) => {
            println!("NavMesh building panicked!");
            println!("edges = {:?}", edges_clone);
            println!("obstacles = [");
            for obs in &obstacles_clone {
                println!("    {:?},", obs);
            }
            println!("]");
            std::panic::resume_unwind(err);
        }
    }
}
