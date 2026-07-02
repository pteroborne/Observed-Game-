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
    mut intent: ResMut<MatchIntent>,
    mut item_intent: ResMut<ItemIntent>,
) {
    spectator.teamplay_frame_accum = spectator.teamplay_frame_accum.saturating_add(1);
    if spectator.teamplay_frame_accum >= SPECTATOR_TEAMPLAY_STEP_FRAMES {
        spectator.teamplay_frame_accum = 0;
        director.pump_spectator(&mut spectator);
    }
    if paused.0 || director.done {
        intent.0 = PlayerIntent::default();
        return;
    }
    if spectator.finished {
        intent.0 = PlayerIntent::default();
        return;
    }

    let here = body_xz(&tp);
    let in_same_room = matches!(tp.place, Place::Room(room) if room == guardian.room);
    if in_same_room && items.carried(ItemKind::AnchorTorch) > 0 {
        item_intent.torch_action = true;
    }

    if matches!(tp.place, Place::Room(room) if room.0 == observed_match::mutable::EXIT_ROOM) {
        spectator.finished = true;
        spectator.clear_route();
        intent.0 = PlayerIntent::default();
        return;
    }

    if let Some(gap) = crate::bot::target_gap_for_place(tp.place, &tp.geom, here) {
        let rel = here - gap.center;
        let tangent = Vec2::new(-gap.normal.y, gap.normal.x);
        let at_aperture =
            rel.dot(gap.normal) > -0.45 && rel.dot(tangent).abs() <= gap.width * 0.5 + 0.35;
        if here.distance(gap.center) <= SPECTATOR_BOT_CROSS_RADIUS || at_aperture {
            debug_cross_gap_for_capture(&mut tp, &mut director, gap, &keys, &items);
            spectator.clear_route();
            intent.0 = PlayerIntent::default();
            return;
        }
    }

    if spectator.route_place != Some(tp.place)
        || spectator.waypoint >= spectator.route.len()
        || spectator.route.is_empty()
    {
        let Some(gap) = crate::bot::target_gap_for_place(tp.place, &tp.geom, here) else {
            spectator.blocked_ticks += 1;
            spectator.finished =
                director.live.host_match().local_target().is_none() || spectator.blocked_ticks > 90;
            intent.0 = PlayerIntent::default();
            return;
        };
        if let Some(path) = crate::bot::route_to_gap(&tp.geom, &tp.arena, &tp.config, here, &gap) {
            spectator.route_place = Some(tp.place);
            spectator.route = path.waypoints;
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
        ..default()
    };
}
