//! Derived local bot navigation for diagnostics and evidence capture.
//!
//! This is not an authoritative AI system. It reads the current rendered place's
//! footprint, doorway gaps, and collision arena, then produces a local path toward a
//! passage threshold. The normal player controller still performs movement and crossing.

use std::collections::{HashMap, VecDeque};

use bevy::prelude::*;
use observed_traversal::{FpsArena, FpsConfig};

use crate::teleport::{self, DoorGap, PlaceGeom};

const GRID_STEP: f32 = 0.55;
const NEAREST_SCAN: i32 = 10;

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

fn route_between(
    geom: &PlaceGeom,
    arena: &FpsArena,
    config: &FpsConfig,
    start: Vec2,
    goal: Vec2,
) -> Option<Vec<Vec2>> {
    if line_walkable(geom, arena, config, start, goal) {
        return Some(vec![goal]);
    }

    let start_key = nearest_walkable_key(geom, arena, config, key(start))?;
    let goal_key = nearest_walkable_key(geom, arena, config, key(goal))?;
    let mut queue = VecDeque::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    came_from.insert(start_key, start_key);
    queue.push_back(start_key);

    while let Some(cur) = queue.pop_front() {
        if cur == goal_key {
            break;
        }
        for next in neighbours(cur) {
            if came_from.contains_key(&next) {
                continue;
            }
            let p = point(next);
            if !point_walkable(geom, arena, config, p) {
                continue;
            }
            came_from.insert(next, cur);
            queue.push_back(next);
        }
    }

    if !came_from.contains_key(&goal_key) {
        return None;
    }

    let mut cells = vec![goal_key];
    let mut cur = goal_key;
    while cur != start_key {
        cur = came_from[&cur];
        cells.push(cur);
    }
    cells.reverse();

    let mut waypoints: Vec<Vec2> = cells.into_iter().skip(1).map(point).collect();
    waypoints.push(goal);
    Some(prune_collinear(waypoints))
}

fn key(p: Vec2) -> (i32, i32) {
    (
        (p.x / GRID_STEP).round() as i32,
        (p.y / GRID_STEP).round() as i32,
    )
}

fn point((x, z): (i32, i32)) -> Vec2 {
    Vec2::new(x as f32 * GRID_STEP, z as f32 * GRID_STEP)
}

fn neighbours((x, z): (i32, i32)) -> [(i32, i32); 8] {
    [
        (x + 1, z),
        (x - 1, z),
        (x, z + 1),
        (x, z - 1),
        (x + 1, z + 1),
        (x + 1, z - 1),
        (x - 1, z + 1),
        (x - 1, z - 1),
    ]
}

fn nearest_walkable_key(
    geom: &PlaceGeom,
    arena: &FpsArena,
    config: &FpsConfig,
    origin: (i32, i32),
) -> Option<(i32, i32)> {
    (0..=NEAREST_SCAN)
        .flat_map(|r| {
            (-r..=r).flat_map(move |dx| {
                (-r..=r).filter_map(move |dz| {
                    (dx.abs().max(dz.abs()) == r).then_some((origin.0 + dx, origin.1 + dz))
                })
            })
        })
        .filter(|candidate| point_walkable(geom, arena, config, point(*candidate)))
        .min_by(|a, b| {
            let da = (point(*a) - point(origin)).length_squared();
            let db = (point(*b) - point(origin)).length_squared();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn line_walkable(geom: &PlaceGeom, arena: &FpsArena, config: &FpsConfig, a: Vec2, b: Vec2) -> bool {
    let dist = a.distance(b);
    let steps = (dist / (GRID_STEP * 0.5)).ceil().max(1.0) as usize;
    (0..=steps).all(|i| {
        let t = i as f32 / steps as f32;
        point_walkable(geom, arena, config, a.lerp(b, t))
    })
}

fn point_walkable(geom: &PlaceGeom, arena: &FpsArena, config: &FpsConfig, p: Vec2) -> bool {
    let margin = config.radius + 0.03;
    if p.x.abs() > geom.half.x - margin || p.y.abs() > geom.half.y - margin {
        return false;
    }

    if geom.poly.is_some() {
        return (teleport::contain(geom, p, config.radius) - p).length() < 0.05;
    }

    let r = config.radius;
    let cy = config.half_height;
    let hy = config.half_height;
    !arena.solids.iter().any(|solid| {
        p.x - r < solid.max.x
            && p.x + r > solid.min.x
            && cy - hy < solid.max.y
            && cy + hy > solid.min.y
            && p.y - r < solid.max.z
            && p.y + r > solid.min.z
    })
}

fn prune_collinear(points: Vec<Vec2>) -> Vec<Vec2> {
    if points.len() < 3 {
        return points;
    }
    let mut out = Vec::with_capacity(points.len());
    out.push(points[0]);
    for window in points.windows(3) {
        let a = window[0];
        let b = window[1];
        let c = window[2];
        let ab = (b - a).normalize_or_zero();
        let bc = (c - b).normalize_or_zero();
        if ab.dot(bc) < 0.999 {
            out.push(b);
        }
    }
    out.push(*points.last().expect("points is non-empty"));
    out
}

#[cfg(test)]
mod tests {
    use observed_core::RoomId;

    use super::*;
    use crate::hallway::{self, HallwayFlavor};

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
                let exit = geom
                    .gaps
                    .iter()
                    .find(|gap| gap.kind == teleport::GapKind::Exit)
                    .expect("hallway has an exit");

                let path = route_to_gap(&geom, &arena, &config, start, exit).unwrap_or_else(|| {
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
