//! The menu-launched **spectator bot**: it drives the same first-person body and
//! threshold-crossing systems a player uses (the camera simply follows), steering
//! waypoint-to-waypoint through each place, crossing thresholds, dropping the anchor
//! torch when it shares a room with the guardian, and pumping the teamplay brain that
//! feeds the elimination series in spectator mode.

use bevy::prelude::*;
use player_input::PlayerIntent;

use super::body_xz;
use super::crossing::debug_cross_gap_for_capture;
use crate::items::{ItemKind, ItemsState};
use crate::keystones::KeystoneState;
use crate::sim::director::MatchDirector;
use crate::sim::state::{ItemIntent, MatchIntent, MatchPaused, SpectatorBot, TeleportState};
use crate::teleport::Place;

const SPECTATOR_BOT_WAYPOINT_RADIUS: f32 = 0.9;
const SPECTATOR_BOT_CROSS_RADIUS: f32 = 1.2;
pub(crate) const SPECTATOR_TEAMPLAY_STEP_FRAMES: u8 = 30;

#[allow(clippy::too_many_arguments)]
pub(crate) fn drive_spectator_bot(
    paused: Res<MatchPaused>,
    mut spectator: ResMut<SpectatorBot>,
    mut director: ResMut<MatchDirector>,
    mut tp: ResMut<TeleportState>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    guardian: Res<crate::guardian::Guardian>,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
    mut intent: ResMut<MatchIntent>,
    mut item_intent: ResMut<ItemIntent>,
) {
    if paused.0 || director.done {
        intent.0 = PlayerIntent::default();
        return;
    }
    spectator.teamplay_frame_accum = spectator.teamplay_frame_accum.saturating_add(1);
    if spectator.teamplay_frame_accum >= SPECTATOR_TEAMPLAY_STEP_FRAMES {
        spectator.teamplay_frame_accum = 0;
        director.pump_spectator(&mut spectator);
    }
    if spectator.finished {
        intent.0 = PlayerIntent::default();
        return;
    }

    let here = body_xz(&tp);
    let seed_val = seed.map(|seed| seed.0).unwrap_or(crate::flow::MATCH_SEED);
    let local_feet_y =
        crate::bot::local_feet_y(tp.body.position.y - tp.config.half_height, tp.place);
    let in_same_room = matches!(tp.place, Place::Room(room) if room == guardian.room);
    if in_same_room && items.carried(ItemKind::AnchorTorch) > 0 {
        item_intent.torch_action = true;
    }

    let exit_room = director.live.host_match().competitive.exit_room();
    if matches!(tp.place, Place::Room(room) if Some(room) == exit_room) {
        spectator.finished = true;
        spectator.clear_route();
        intent.0 = PlayerIntent::default();
        return;
    }

    if let Some(gap) = crate::bot::target_gap_for_place(tp.place, &tp.geom, here, local_feet_y) {
        let rel = here - gap.center;
        let tangent = Vec2::new(-gap.normal.y, gap.normal.x);
        let at_aperture =
            rel.dot(gap.normal) > -0.45 && rel.dot(tangent).abs() <= gap.width * 0.5 + 0.35;
        if here.distance(gap.center) <= SPECTATOR_BOT_CROSS_RADIUS || at_aperture {
            debug_cross_gap_for_capture(seed_val, &mut tp, &mut director, gap, &keys, &items);
            spectator.clear_route();
            intent.0 = PlayerIntent::default();
            return;
        }
    }

    if spectator.route_place != Some(tp.place)
        || spectator.waypoint >= spectator.route.len()
        || spectator.route.is_empty()
    {
        let Some(gap) = crate::bot::target_gap_for_place(tp.place, &tp.geom, here, local_feet_y)
        else {
            spectator.blocked_ticks += 1;
            spectator.finished =
                director.live.host_match().local_target().is_none() || spectator.blocked_ticks > 90;
            intent.0 = PlayerIntent::default();
            return;
        };
        // On a Gantry hallway's deck, run the platform-centre jump line toward the upper
        // exit instead of the generic 2D navmesh route (which has no notion of a
        // platform-to-platform jump); a ground-level body (fell, or arrived fallen) still
        // takes the ordinary route to the safe-bypass exit `target_gap_for_place` already
        // selected via the feet-height gate.
        let deck_pilot = crate::bot::at_deck_height(local_feet_y)
            .then(|| crate::bot::gantry_deck_route(&tp.geom, here, &gap))
            .flatten();
        if let Some(pilot) = deck_pilot {
            spectator.route_place = Some(tp.place);
            let (waypoints, jumps): (Vec<_>, Vec<_>) = pilot.waypoints.into_iter().unzip();
            spectator.route = waypoints;
            spectator.route_jumps = jumps;
            spectator.waypoint = 0;
            spectator.blocked_ticks = 0;
        } else if let Some(path) =
            crate::bot::route_to_gap(&tp.geom, &tp.arena, &tp.config, here, &gap)
        {
            spectator.route_place = Some(tp.place);
            spectator.route = path.waypoints;
            spectator.route_jumps = vec![false; spectator.route.len()];
            spectator.waypoint = 0;
            spectator.blocked_ticks = 0;
        } else {
            spectator.blocked_ticks += 1;
            if spectator.blocked_ticks > 90 {
                spectator.finished = true;
            }
            intent.0 = PlayerIntent::default();
            return;
        }
    }

    while spectator.waypoint + 1 < spectator.route.len()
        && here.distance(spectator.route[spectator.waypoint]) <= SPECTATOR_BOT_WAYPOINT_RADIUS
    {
        spectator.waypoint += 1;
    }

    let target = spectator.route[spectator.waypoint];
    let leg_needs_jump = spectator
        .route_jumps
        .get(spectator.waypoint)
        .copied()
        .unwrap_or(false);
    let jump_pressed = crate::bot::gantry_jump_pressed_for_leg(
        &tp.geom,
        here,
        local_feet_y,
        tp.body.grounded,
        leg_needs_jump,
    );
    let to = target - here;
    if to.length_squared() < 0.04 {
        intent.0 = PlayerIntent::default();
        return;
    }

    let mut avoidance = Vec2::ZERO;
    let safety_dist = tp.config.radius + 0.05;
    let cy = tp.body.position.y;
    let hy = tp.config.half_height;
    for solid in &tp.arena.solids {
        if cy - hy < solid.max.y && cy + hy > solid.min.y {
            let closest_x = here.x.clamp(solid.min.x, solid.max.x);
            let closest_z = here.y.clamp(solid.min.z, solid.max.z);
            let closest = Vec2::new(closest_x, closest_z);
            let diff = here - closest;
            let dist = diff.length();
            if dist > 0.0 && dist < safety_dist {
                let weight = (safety_dist - dist) / safety_dist;
                avoidance += diff.normalize() * weight * 1.8;
            }
        }
    }

    let mut dir = to.normalize_or_zero();
    if avoidance.length_squared() > 1e-4 {
        dir = (dir + avoidance).normalize_or_zero();
    }
    let forward_dir = Vec2::new(tp.body.forward().x, tp.body.forward().z).normalize_or_zero();
    let is_sharp_turn = forward_dir.dot(dir) < 0.65;
    tp.body.yaw = dir.x.atan2(-dir.y);
    tp.body.pitch = -0.22;

    intent.0 = PlayerIntent {
        movement: Vec2::new(0.0, 1.0),
        sprint_held: !is_sharp_turn,
        jump_pressed,
        ..default()
    };
}
