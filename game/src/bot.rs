//! Derived local bot navigation for diagnostics and evidence capture.
//!
//! This is not an authoritative AI system. It reads the current rendered place's
//! footprint, doorway gaps, and collision arena, then produces a local path toward a
//! passage threshold. The normal player controller still performs movement and crossing.

use bevy::prelude::*;
use observed_traversal::gantry;
use observed_traversal::{FpsArena, FpsConfig};
use std::collections::VecDeque;

use crate::teleport::{DeckSeg, DoorGap, GapKind, Place, PlaceGeom, contain};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BotPath {
    pub waypoints: Vec<Vec2>,
}

/// A deck-piloted run through a Gantry hallway's jump platforms: the platform-centre
/// waypoint sequence toward the upper exit (each waypoint tagged with whether the leg
/// arriving there crosses a real jump-map gap, so the driving system can hold
/// `jump_pressed` on those legs).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GantryPilot {
    pub waypoints: Vec<(Vec2, bool)>,
}

/// A body's feet count as "on the gantry deck" once they sit within this tolerance of
/// `UPPER_DECK_Y` — mirrors the gate `feet_at_gap_floor`/`GAP_FLOOR_TOLERANCE` already use
/// for the upper exit, so the pilot only engages once the body has actually reached deck
/// height (not mid-jump arc past a platform edge).
const DECK_HEIGHT_TOLERANCE: f32 = crate::teleport::GAP_FLOOR_TOLERANCE;

/// How close two decks' Z-spans must sit to count as a contiguous landing (no jump needed)
/// rather than a real jump-map gap between platforms.
const CONTIGUOUS_Z_GAP: f32 = 0.35;

/// Is the body's local feet height at deck level (`UPPER_DECK_Y`, within tolerance)?
pub(crate) fn at_deck_height(local_feet_y: f32) -> bool {
    (local_feet_y - gantry::UPPER_DECK_Y).abs() <= DECK_HEIGHT_TOLERANCE
}

/// Whether a deck-piloted Gantry leg should fire the jump button this fixed tick.
///
/// `leg_needs_jump` says the current route segment crosses a real platform gap. The
/// button itself should only fire once the body is grounded on the current upper deck and
/// has committed to the platform's far edge, mirroring the pure gantry runner.
pub(crate) fn gantry_jump_pressed_for_leg(
    geom: &PlaceGeom,
    here: Vec2,
    local_feet_y: f32,
    grounded: bool,
    leg_needs_jump: bool,
) -> bool {
    if !leg_needs_jump || !grounded || !at_deck_height(local_feet_y) {
        return false;
    }
    platform_decks(&geom.decks).into_iter().any(|platform| {
        (here.x - platform.center.x).abs() <= platform.half.x + 0.25
            && (here.y - platform.center.y).abs() <= platform.half.y + 0.25
            && here.y >= platform.center.y + platform.half.y - 0.55
    })
}

/// The platform-only decks of a Gantry hallway's `geom.decks`, ordered by Z (excludes the
/// upper/entry landings, which are wider/shallower than the authored jump platforms).
fn platform_decks(decks: &[DeckSeg]) -> Vec<&DeckSeg> {
    let mut platforms: Vec<&DeckSeg> = decks
        .iter()
        .filter(|d| (d.half.y - gantry::PLATFORM_HALF_LENGTH).abs() < 1e-2)
        .collect();
    platforms.sort_by(|a, b| {
        a.center
            .y
            .partial_cmp(&b.center.y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    platforms
}

/// Is `target_z` reachable from `from_z` by walking across decks alone (no jump), i.e. is
/// there a chain of decks (platforms and/or landings) whose Z-spans overlap/touch all the
/// way from `from_z` to `target_z`? Used to tell whether the final leg into the upper
/// exit needs a jump or is already bridged by the upper landing.
fn contiguous_by_deck(decks: &[DeckSeg], from_z: f32, target_z: f32) -> bool {
    let mut reached = from_z;
    loop {
        if reached >= target_z - 1e-3 {
            return true;
        }
        let next = decks
            .iter()
            .filter(|d| d.center.y - d.half.y <= reached + CONTIGUOUS_Z_GAP)
            .map(|d| d.center.y + d.half.y)
            .fold(reached, f32::max);
        if next <= reached + 1e-3 {
            return false;
        }
        reached = next;
    }
}

/// Deck-piloted waypoints for a Gantry hallway: the platform-centre sequence (ordered by
/// Z) ahead of the body's current position, toward the upper exit, ending just past the
/// exit threshold. `jump_pressed` is set on each leg that crosses a real jump-map gap
/// (mirrors `GantryRunState::should_jump` in `observed_traversal::gantry`, which commits to
/// the jump once the runner is standing at the near platform's far edge moving forward —
/// here expressed as a route property: the leg needs a jump iff the next platform sits
/// behind a real gap rather than a contiguous landing). Deterministic given `geom`/`here`.
/// Returns `None` if this isn't a Gantry hallway or there is no platform ahead of `here`.
pub(crate) fn gantry_deck_route(
    geom: &PlaceGeom,
    here: Vec2,
    upper_exit: &DoorGap,
) -> Option<GantryPilot> {
    let platforms = platform_decks(&geom.decks);
    if platforms.is_empty() {
        return None;
    }
    let mut waypoints = Vec::new();
    let mut prev_max_z = here.y;
    let mut any_ahead = false;
    for platform in &platforms {
        if platform.center.y <= here.y + 0.05 {
            prev_max_z = prev_max_z.max(platform.center.y + platform.half.y);
            continue;
        }
        any_ahead = true;
        let needs_jump =
            !contiguous_by_deck(&geom.decks, prev_max_z, platform.center.y - platform.half.y);
        waypoints.push((platform.center, needs_jump));
        prev_max_z = platform.center.y + platform.half.y;
    }
    if !any_ahead {
        return None;
    }
    // The final leg onto the upper exit: contiguous if the upper landing (or any other
    // deck) bridges the last platform straight through to the exit's Z, jump otherwise.
    let needs_jump = !contiguous_by_deck(&geom.decks, prev_max_z, upper_exit.center.y);
    waypoints.push((upper_exit.center + upper_exit.normal * 0.85, needs_jump));
    Some(GantryPilot { waypoints })
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

    /// A body already at deck height gets the platform-centre jump line instead: waypoints
    /// step through every jump-map platform ahead (ordered by Z), finish just past the
    /// upper exit, and every leg that crosses a real jump-map gap is flagged
    /// `jump_pressed` — the two platform-to-platform legs (contiguous landings need no
    /// jump).
    #[test]
    fn deck_piloted_bot_produces_platform_waypoints_targeting_the_upper_exit() {
        use observed_traversal::gantry;

        let template = hallway::TEMPLATES
            .iter()
            .find(|template| template.flavor == HallwayFlavor::Gantry)
            .unwrap();
        let geom = teleport::hallway_geom(RoomId(0), RoomId(1), template, 0, false);

        // Start right on the entry landing, at deck height, just past the entry threshold.
        let start = Vec2::new(0.0, -gantry::GANTRY_LENGTH * 0.5 + 1.0);
        let local_feet_y = gantry::UPPER_DECK_Y;
        assert!(at_deck_height(local_feet_y), "deck-height gate engages");

        let gap = target_gap_for_place(
            teleport::Place::Hallway {
                from: RoomId(0),
                to: RoomId(1),
                variation: 0,
            },
            &geom,
            start,
            local_feet_y,
        )
        .expect("a deck-level body targets the upper exit");
        assert_eq!(gap.target, RoomId(1));
        assert!(gap.floor_y > 0.0, "the deck body targets the upper exit");

        let pilot =
            gantry_deck_route(&geom, start, &gap).expect("a deck route exists ahead of the entry");
        assert!(
            pilot.waypoints.len() >= gantry::PLATFORM_COUNT,
            "one waypoint per platform ahead, plus the exit: got {}",
            pilot.waypoints.len()
        );

        // Waypoints strictly ascend in Z (ordered platform-to-platform toward the exit).
        for pair in pilot.waypoints.windows(2) {
            assert!(
                pair[1].0.y > pair[0].0.y,
                "waypoints must advance toward the upper exit: {:?} -> {:?}",
                pair[0],
                pair[1]
            );
        }
        // The final waypoint sits beyond the upper exit threshold, along its normal.
        let last = pilot.waypoints.last().unwrap();
        assert!(
            (last.0 - gap.center).dot(gap.normal) > 0.0,
            "the last waypoint crosses outside the upper exit"
        );

        // The body starts on the entry landing, which is contiguous with platform 0 (no
        // jump needed to step onto it), and the last leg lands on the equally-contiguous
        // upper landing/exit. Every platform-to-platform leg in between crosses a real
        // jump-map gap and must hold the jump.
        assert!(
            !pilot.waypoints.first().unwrap().1,
            "the first leg (entry landing -> platform 0) is contiguous: {:?}",
            pilot.waypoints.first()
        );
        assert!(
            !pilot.waypoints.last().unwrap().1,
            "the last leg (platform 5 -> upper landing/exit) is contiguous: {:?}",
            pilot.waypoints.last()
        );
        let platform_to_platform_legs = &pilot.waypoints[1..pilot.waypoints.len() - 1];
        assert!(
            !platform_to_platform_legs.is_empty(),
            "there are jump-map legs between the first and last platform"
        );
        assert!(
            platform_to_platform_legs.iter().all(|(_, jump)| *jump),
            "every platform-to-platform leg crosses a real jump-map gap: {platform_to_platform_legs:?}"
        );
    }

    #[test]
    fn gantry_jump_button_fires_only_at_the_platform_edge() {
        use observed_traversal::gantry;

        let template = hallway::TEMPLATES
            .iter()
            .find(|template| template.flavor == HallwayFlavor::Gantry)
            .unwrap();
        let geom = teleport::hallway_geom(RoomId(0), RoomId(1), template, 0, false);
        let platform = geom
            .decks
            .iter()
            .find(|deck| (deck.half.y - gantry::PLATFORM_HALF_LENGTH).abs() < 1e-2)
            .expect("gantry has a jump platform");

        assert!(
            !gantry_jump_pressed_for_leg(&geom, platform.center, gantry::UPPER_DECK_Y, true, true,),
            "standing at platform center should not jump yet"
        );
        assert!(
            gantry_jump_pressed_for_leg(
                &geom,
                Vec2::new(platform.center.x, platform.center.y + platform.half.y - 0.2),
                gantry::UPPER_DECK_Y,
                true,
                true,
            ),
            "grounded at the far edge of a jump leg should fire jump"
        );
        assert!(
            !gantry_jump_pressed_for_leg(
                &geom,
                Vec2::new(platform.center.x, platform.center.y + platform.half.y - 0.2),
                gantry::UPPER_DECK_Y,
                true,
                false,
            ),
            "contiguous legs should never jump"
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
