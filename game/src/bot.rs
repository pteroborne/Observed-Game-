//! Derived local bot navigation for diagnostics and evidence capture.
//!
//! This is not an authoritative AI system. It reads the current rendered place's
//! footprint, doorway gaps, and collision arena, then produces a local path toward a
//! passage threshold. The normal player controller still performs movement and crossing.

use bevy::prelude::*;
use observed_traversal::{FpsArena, FpsConfig};

use crate::teleport::{DoorGap, PlaceGeom};

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

    let path = navmesh.path(clamped_start, clamped_goal);
    if path.is_none() {
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
        return None;
    }
    let mut waypoints = path.unwrap().path;
    if waypoints.is_empty() {
        waypoints.push(clamped_goal);
    }
    Some(waypoints)
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
