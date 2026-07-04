//! Derived local bot navigation for diagnostics and evidence capture.
//!
//! This is not an authoritative AI system. It reads the current rendered place's
//! footprint, doorway gaps, and collision arena, then produces a local path toward a
//! passage threshold. The normal player controller still performs movement and crossing.

use bevy::prelude::*;
use observed_traversal::{FpsArena, FpsConfig};
use std::collections::VecDeque;

use crate::teleport::{DoorGap, GapKind, Place, PlaceGeom, contain};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BotPath {
    pub waypoints: Vec<Vec2>,
}

/// Route from `start` to just inside `gap`, then append an outside crossing point so the
/// normal doorway-crossing code takes over. Returns `None` if the current place has no
/// valid local walk to that threshold.
pub(crate) fn route_to_gap(
    geom: &PlaceGeom,
    arena: &FpsArena,
    config: &FpsConfig,
    start: Vec2,
    gap: &DoorGap,
) -> Option<BotPath> {
    if !gap.kind.is_passage() {
        return None;
    }
    let inside = gap.center - gap.normal * (config.radius + 0.45);
    let outside = gap.center + gap.normal * (config.radius + 0.85);
    let mut waypoints = route_between(geom, arena, config, start, inside)?;
    waypoints.push(outside);
    Some(BotPath { waypoints })
}

pub(crate) fn target_gap_for_place(
    place: Place,
    geom: &PlaceGeom,
    here: Vec2,
    local_feet_y: f32,
) -> Option<DoorGap> {
    let at_current_floor =
        |gap: &&DoorGap| (local_feet_y - gap.floor_y).abs() <= crate::teleport::GAP_FLOOR_TOLERANCE;
    match place {
        Place::Room(_) => geom.forward_gap().copied(),
        Place::Hallway { to, .. } => geom
            .gaps
            .iter()
            .filter(|gap| gap.kind == GapKind::Exit)
            .filter(at_current_floor)
            .min_by(|a, b| {
                let a_forward = a.target == to;
                let b_forward = b.target == to;
                b_forward.cmp(&a_forward).then_with(|| {
                    here.distance_squared(a.center)
                        .partial_cmp(&here.distance_squared(b.center))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            })
            .or_else(|| {
                geom.gaps
                    .iter()
                    .filter(|gap| gap.kind == GapKind::Exit)
                    .min_by(|a, b| {
                        here.distance_squared(a.center)
                            .partial_cmp(&here.distance_squared(b.center))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            })
            .copied(),
    }
}

pub(crate) fn local_feet_y(world_feet_y: f32, place: Place) -> f32 {
    world_feet_y - crate::teleport::place_y_offset(place)
}

fn clamp_to_place(p: Vec2, geom: &PlaceGeom) -> Vec2 {
    let mut clamped = p;
    let margin = 0.05;
    clamped.x = clamped.x.clamp(-geom.half.x + margin, geom.half.x - margin);
    clamped.y = clamped.y.clamp(-geom.half.y + margin, geom.half.y - margin);

    for gap in &geom.gaps {
        let rel = clamped - gap.center;
        let depth = rel.dot(gap.normal);
        let tangent = Vec2::new(-gap.normal.y, gap.normal.x);
        let lateral = rel.dot(tangent).abs();

        if lateral <= gap.width * 0.5 + 0.4 && depth > -0.1 {
            clamped = gap.center - gap.normal * 0.1 + tangent * rel.dot(tangent);
        }
    }
    clamped
}

fn route_between(
    geom: &PlaceGeom,
    arena: &FpsArena,
    config: &FpsConfig,
    start: Vec2,
    goal: Vec2,
) -> Option<Vec<Vec2>> {
    let navmesh = crate::navmesh::build_navmesh(geom, arena, config);

    let clamped_start = clamp_to_place(start, geom);
    let clamped_goal = clamp_to_place(goal, geom);

    if let Some(path) = navmesh.path(clamped_start, clamped_goal) {
        let mut waypoints = path.path;
        if waypoints.is_empty() {
            waypoints.push(clamped_goal);
        }
        return Some(waypoints);
    }

    if let Some(path) = grid_route_between(geom, arena, config, clamped_start, clamped_goal) {
        return Some(path);
    }

    info!(
        "BOT_NAV: route_between failed. start={:?} (clamped: {:?}), goal={:?} (clamped: {:?}), start_in_mesh={}, goal_in_mesh={}, geom.half={:?}, geom.gaps={:?}",
        start,
        clamped_start,
        goal,
        clamped_goal,
        navmesh.is_in_mesh(clamped_start),
        navmesh.is_in_mesh(clamped_goal),
        geom.half,
        geom.gaps
            .iter()
            .map(|g| (g.center, g.normal, g.kind))
            .collect::<Vec<_>>()
    );
    None
}

fn grid_route_between(
    geom: &PlaceGeom,
    arena: &FpsArena,
    config: &FpsConfig,
    start: Vec2,
    goal: Vec2,
) -> Option<Vec<Vec2>> {
    const STEP: f32 = 0.65;
    let margin = config.radius + 0.08;
    let min = Vec2::new(-geom.half.x + margin, -geom.half.y + margin);
    let max = Vec2::new(geom.half.x - margin, geom.half.y - margin);
    if min.x >= max.x || min.y >= max.y {
        return None;
    }
    let cols = (((max.x - min.x) / STEP).ceil() as usize + 1).max(2);
    let rows = (((max.y - min.y) / STEP).ceil() as usize + 1).max(2);
    let index = |x: usize, y: usize| y * cols + x;
    let pos = |x: usize, y: usize| {
        Vec2::new(
            (min.x + x as f32 * STEP).min(max.x),
            (min.y + y as f32 * STEP).min(max.y),
        )
    };
    let key = |p: Vec2| {
        let x = ((p.x.clamp(min.x, max.x) - min.x) / STEP).round() as usize;
        let y = ((p.y.clamp(min.y, max.y) - min.y) / STEP).round() as usize;
        (x.min(cols - 1), y.min(rows - 1))
    };
    let blocked = |p: Vec2| {
        if geom.poly.is_some() && (contain(geom, p, config.radius) - p).length() > 0.08 {
            return true;
        }
        let cy = arena.floor_y + config.half_height;
        arena.solids.iter().any(|solid| {
            p.x - config.radius < solid.max.x
                && p.x + config.radius > solid.min.x
                && cy - config.half_height < solid.max.y
                && cy + config.half_height > solid.min.y
                && p.y - config.radius < solid.max.z
                && p.y + config.radius > solid.min.z
        })
    };
    let nearest_open = |want: (usize, usize)| {
        let max_radius = cols.max(rows);
        for radius in 0..=max_radius {
            let rx = radius as isize;
            for dy in -rx..=rx {
                for dx in -rx..=rx {
                    if dx.abs().max(dy.abs()) != rx {
                        continue;
                    }
                    let x = want.0 as isize + dx;
                    let y = want.1 as isize + dy;
                    if x < 0 || y < 0 || x >= cols as isize || y >= rows as isize {
                        continue;
                    }
                    let key = (x as usize, y as usize);
                    if !blocked(pos(key.0, key.1)) {
                        return Some(key);
                    }
                }
            }
        }
        None
    };

    let start_key = nearest_open(key(start))?;
    let goal_key = nearest_open(key(goal))?;
    let mut parent = vec![None::<(usize, usize)>; cols * rows];
    let mut seen = vec![false; cols * rows];
    let mut queue = VecDeque::new();
    seen[index(start_key.0, start_key.1)] = true;
    queue.push_back(start_key);
    while let Some((x, y)) = queue.pop_front() {
        if (x, y) == goal_key {
            let mut out = vec![goal];
            let mut current = goal_key;
            while current != start_key {
                out.push(pos(current.0, current.1));
                current = parent[index(current.0, current.1)]?;
            }
            out.push(start);
            out.reverse();
            return Some(out);
        }
        for (dx, dy) in [
            (-1isize, 0isize),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (-1, 1),
            (1, -1),
            (1, 1),
        ] {
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx < 0 || ny < 0 || nx >= cols as isize || ny >= rows as isize {
                continue;
            }
            let next = (nx as usize, ny as usize);
            let next_index = index(next.0, next.1);
            if seen[next_index] || blocked(pos(next.0, next.1)) {
                continue;
            }
            seen[next_index] = true;
            parent[next_index] = Some((x, y));
            queue.push_back(next);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use observed_core::RoomId;

    use super::*;
    use crate::hallway::{self, HallwayFlavor};
    use crate::teleport;

    fn config() -> FpsConfig {
        FpsConfig::default()
    }

    #[test]
    fn routes_through_every_hallway_template_from_entry_to_exit() {
        let config = config();
        for (index, template) in hallway::TEMPLATES.iter().enumerate() {
            for seed in 0..8_u64 {
                let geom = teleport::hallway_geom(RoomId(0), RoomId(1), template, seed, false);
                let arena = teleport::place_arena(&geom, 0.0, 4.6);
                let start = teleport::entry_spawn(&geom, RoomId(0));
                let exit = target_gap_for_place(
                    teleport::Place::Hallway {
                        from: RoomId(0),
                        to: RoomId(1),
                        variation: index,
                    },
                    &geom,
                    start,
                    0.0,
                )
                .expect("hallway has an exit");

                let path =
                    route_to_gap(&geom, &arena, &config, start, &exit).unwrap_or_else(|| {
                        panic!(
                            "template {index} ({:?}) seed {seed} must route entry -> exit",
                            template.flavor
                        )
                    });

                assert!(path.waypoints.len() >= 2);
                assert!(
                    path.waypoints
                        .last()
                        .is_some_and(|p| (*p - exit.center).dot(exit.normal) > 0.0),
                    "last waypoint crosses outside the exit threshold"
                );
            }
        }
    }

    #[test]
    fn ground_level_gantry_bot_targets_the_safe_onward_exit() {
        let template = hallway::TEMPLATES
            .iter()
            .find(|template| template.flavor == HallwayFlavor::Gantry)
            .unwrap();
        let geom = teleport::hallway_geom(RoomId(0), RoomId(1), template, 0, false);
        let start = teleport::entry_spawn(&geom, RoomId(0));
        let gap = target_gap_for_place(
            teleport::Place::Hallway {
                from: RoomId(0),
                to: RoomId(1),
                variation: 0,
            },
            &geom,
            start,
            0.0,
        )
        .expect("ground route has an onward exit");

        assert_eq!(gap.target, RoomId(1), "bot should prefer onward exits");
        assert_eq!(
            gap.floor_y, 0.0,
            "ground bot should not target the upper exit"
        );
    }

    #[test]
    fn blocked_locked_exit_is_not_routed() {
        let config = config();
        let template = hallway::TEMPLATES
            .iter()
            .find(|template| template.flavor != HallwayFlavor::Maze)
            .unwrap();
        let geom = teleport::hallway_geom(
            RoomId(7),
            RoomId(observed_match::mutable::EXIT_ROOM),
            template,
            0,
            true,
        );
        let arena = teleport::place_arena(&geom, 0.0, 4.6);
        let start = teleport::entry_spawn(&geom, RoomId(7));
        let locked = geom
            .gaps
            .iter()
            .find(|gap| gap.kind == teleport::GapKind::LockedExit)
            .expect("exit is locked");

        assert!(route_to_gap(&geom, &arena, &config, start, locked).is_none());
    }
}
