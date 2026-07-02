use bevy::prelude::*;
use observed_core::RoomId;
use observed_match::hybrid::{HybridMatch, LocalAction};
use observed_traversal::{FIXED_DT, FpsBody, step_body};

use super::{
    connections_for_nav, nav_for_place, nav_for_room, room_connection_slots, room_target,
    slot_for_connection,
};
use crate::flow::MATCH_SEED;
use crate::items::ItemsState;
use crate::keystones::KeystoneState;
use crate::screens::{
    FrozenDest, MatchIntent, MatchPaused, MatchRuntime, TeleportState, WALL_HEIGHT,
};
use crate::teleport::{self, GapKind, Place};

fn body_xz(tp: &TeleportState) -> Vec2 {
    Vec2::new(tp.body.position.x, tp.body.position.z)
}

/// Resolve and **freeze** the destination of every passage doorway of `place` *now* — the
/// hallway each room doorway opens into (with its rolled variation locked in the `Place`),
/// and the frozen connection set + spine target of the room each hallway doorway opens
/// into. Captured once at place-entry so the doorway preview and the actual crossing read
/// the identical snapshot ("observed → frozen"); see [`TeleportState::gap_dests`].
pub(crate) fn compute_gap_dests(
    seed: u64,
    place: Place,
    geom: &teleport::PlaceGeom,
    game: &HybridMatch,
    keys: &KeystoneState,
    items: &ItemsState,
) -> Vec<FrozenDest> {
    let nav = nav_for_place(seed, game, keys, items, place);
    geom.gaps
        .iter()
        .filter(|g| g.kind.is_passage())
        .map(|gap| {
            let (dest, _) = teleport::apply_crossing(place, gap, &nav);
            let (conns, connection_slots, hallway_entry_room_slot, hallway_exit_room_slot, target) =
                match dest {
                    Place::Room(r) => {
                        let c = connections_for_nav(game, items, r);
                        let slots = room_connection_slots(game, items, r, &c);
                        let t = room_target(game, r, &c);
                        (c, slots, None, None, t)
                    }
                    Place::Hallway { from, to, .. } => (
                        Vec::new(),
                        Vec::new(),
                        Some(gap.threshold.room.slot),
                        slot_for_connection(game, items, to, from),
                        None,
                    ),
                };
            FrozenDest {
                gap_center: gap.center,
                threshold: gap.threshold,
                place: dest,
                conns,
                connection_slots,
                hallway_entry_room_slot,
                hallway_exit_room_slot,
                target,
            }
        })
        .collect()
}

/// The nav that rebuilds a [`FrozenDest`]'s geometry exactly as it was snapshotted: a
/// hallway uses its frozen `Place` variation + the live exit lock; a room uses its frozen
/// connections + target. (`geom_for` only reads these fields, so version/pins are inert.)
pub(super) fn frozen_nav(seed: u64, dest: &FrozenDest, keys: &KeystoneState) -> teleport::Nav {
    teleport::Nav {
        connections: dest.conns.clone(),
        connection_slots: dest.connection_slots.clone(),
        hallway_entry_room_slot: dest.hallway_entry_room_slot,
        hallway_exit_room_slot: dest.hallway_exit_room_slot,
        target_room: dest.target,
        seed,
        version: 0,
        exit_locked: !keys.gate_open(),
        exit_room: keys.exit_room,
        pins: Vec::new(),
    }
}

/// The frozen destination snapshot for the doorway whose gap is `gap` (matched by
/// threshold identity, with a centre fallback for old/debug snapshots).
pub(super) fn frozen_dest_for<'a>(
    tp: &'a TeleportState,
    gap: &teleport::DoorGap,
) -> Option<&'a FrozenDest> {
    tp.gap_dests
        .iter()
        .find(|d| d.threshold == gap.threshold)
        .or_else(|| {
            tp.gap_dests
                .iter()
                .find(|d| (d.gap_center - gap.center).length() < 0.05)
        })
}

/// Move the body into `place`, having arrived from room `from`. When `crossed` (the
/// doorway just stepped through, in the *old* place's frame) yields an alignment, the
/// body's pre-swap pose is carried continuously into the new place so walking through a
/// door has **no snap and no view reset** — the camera flows on. Otherwise (or for a
/// non-crossing placement, `crossed = None`) the body snaps just inside the arrival
/// doorway facing in, as before.
pub(super) fn place_body(
    tp: &mut TeleportState,
    place: Place,
    from: RoomId,
    crossed: Option<teleport::DoorGap>,
    nav: &teleport::Nav,
) {
    let mut geom = teleport::geom_for(place, nav);
    // Arriving in a room *through* a doorway: keep that doorway an open passage (matching
    // the preview you crossed) so the entry doesn't pop into a wall. The start room and
    // pad/debug placements pass `crossed = None`, so they keep the default sealed doors.
    let arrived_from = match place {
        Place::Room(_) if crossed.is_some() => Some(from),
        _ => None,
    };
    teleport::open_entry(&mut geom, arrived_from);
    let y_offset = teleport::place_y_offset(place);
    let arena = teleport::place_arena(&geom, y_offset, WALL_HEIGHT);
    let (pos, yaw, pitch) = crossed
        .and_then(|gap| teleport::crossing_alignment(&geom, place, &gap, from))
        .map(|align| {
            // Continuous carry: the body's current XZ/heading mapped into the new frame.
            let old = Vec2::new(tp.body.position.x, tp.body.position.z);
            (
                align.inverse_apply(old),
                tp.body.yaw + align.yaw,
                tp.body.pitch,
            )
        })
        .unwrap_or_else(|| {
            // Snap: just inside the arrival doorway, facing in (level pitch).
            let spawn = teleport::entry_spawn(&geom, from);
            let yaw = geom
                .gaps
                .iter()
                .find(|g| g.target == from)
                .map(|g| (-g.normal.x).atan2(g.normal.y))
                .unwrap_or(0.0);
            (spawn, yaw, 0.0)
        });
    tp.arena = arena;
    tp.geom = geom;
    tp.body = FpsBody::spawned(
        Vec3::new(pos.x, y_offset + tp.config.half_height, pos.y),
        yaw,
    );
    tp.body.pitch = pitch;
    tp.place = place;
    tp.prev_xz = pos;
    tp.crossed_exit = false;
    tp.pending_exit = None;
    tp.arrived_from = arrived_from;
}

/// Move the body directly to a point in `place` without committing a match round.
/// Teleport pads use this: they are local traversal tools, not deterministic match
/// actions replicated through the lockstep brain.
pub(crate) fn place_body_at(tp: &mut TeleportState, place: Place, pos: Vec2, nav: &teleport::Nav) {
    let geom = teleport::geom_for(place, nav);
    let yaw = tp.body.yaw;
    let pitch = tp.body.pitch;
    let y_offset = teleport::place_y_offset(place);
    tp.arena = teleport::place_arena(&geom, y_offset, WALL_HEIGHT);
    tp.geom = geom;
    tp.body = FpsBody::spawned(
        Vec3::new(pos.x, y_offset + tp.config.half_height, pos.y),
        yaw,
    );
    tp.body.pitch = pitch;
    tp.place = place;
    tp.prev_xz = pos;
    tp.crossed_exit = false;
    tp.pending_exit = None;
    tp.arrived_from = None;
    tp.rendered = None;
}

/// Cross `gap` into its frozen destination (a hallway from a room, etc.): use the snapshot
/// taken at place-entry so the arrival matches the preview; fall back to a live resolve if
/// the snapshot is missing. `cur` is the place being left, `from` the room you came from.
pub(super) fn cross_into(
    seed: u64,
    tp: &mut TeleportState,
    gap: &teleport::DoorGap,
    cur: Place,
    from: RoomId,
    nav: &teleport::Nav,
    keys: &KeystoneState,
) {
    if let Some(dest) = frozen_dest_for(tp, gap).cloned() {
        place_body(
            tp,
            dest.place,
            from,
            Some(*gap),
            &frozen_nav(seed, &dest, keys),
        );
    } else {
        let (place, _) = teleport::apply_crossing(cur, gap, nav);
        place_body(tp, place, from, Some(*gap), nav);
    }
}

/// Cross `gap` into room `arrived` (from a hallway): prefer the frozen snapshot for that
/// doorway (frozen shape), else rebuild from the live brain. `from` is the room the hallway
/// came from (its arrival doorway stays open).
#[allow(clippy::too_many_arguments)]
pub(super) fn cross_into_room(
    seed: u64,
    tp: &mut TeleportState,
    gap: &teleport::DoorGap,
    arrived: RoomId,
    from: RoomId,
    game: &HybridMatch,
    keys: &KeystoneState,
    items: &ItemsState,
) {
    match frozen_dest_for(tp, gap).cloned() {
        Some(dest) if dest.place == Place::Room(arrived) => {
            place_body(
                tp,
                dest.place,
                from,
                Some(*gap),
                &frozen_nav(seed, &dest, keys),
            );
        }
        _ => {
            let nav = nav_for_room(seed, game, keys, items, arrived);
            place_body(tp, Place::Room(arrived), from, Some(*gap), &nav);
        }
    }
}

/// Fixed-step teleport controller: walk the body inside the current place; crossing
/// the forward doorway teleports into the edge's hallway, and reaching the hallway's
/// exit commits the spine `Advance` to the match brain and teleports into the next
/// room. The brain (rounds / networking / replay) is untouched.
pub(crate) fn teleport_sim(
    mut runtime: ResMut<MatchRuntime>,
    tp: ResMut<TeleportState>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    mut intent: ResMut<MatchIntent>,
    paused: Res<MatchPaused>,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
) {
    if paused.0 || runtime.done {
        return;
    }
    let seed_val = seed.map(|s| s.0).unwrap_or(MATCH_SEED);
    let tp = tp.into_inner();
    let nav = nav_for_place(seed_val, runtime.live.host_match(), &keys, &items, tp.place);
    let prev = body_xz(tp);
    let config = tp.config;
    let arena = tp.arena.clone();
    step_body(&mut tp.body, intent.0, &arena, &config, FIXED_DT);
    intent.0.interact_pressed = false;
    if tp.geom.poly.is_some() {
        let here = body_xz(tp);
        let clamped = teleport::contain(&tp.geom, here, config.radius);
        tp.body.position.x = clamped.x;
        tp.body.position.z = clamped.y;
    }
    let next = body_xz(tp);
    tp.prev_xz = next;

    let place_before = tp.place;
    match tp.place {
        Place::Room(room) => {
            if let Some(gap) = tp
                .geom
                .gaps
                .iter()
                .filter(|g| g.kind.is_passage())
                .find(|g| teleport::crossed(prev, next, g))
                .copied()
            {
                cross_into(seed_val, tp, &gap, Place::Room(room), room, &nav, &keys);
            }
        }
        Place::Hallway { from, to, .. } => {
            if !tp.crossed_exit
                && let Some(exit) = tp
                    .geom
                    .gaps
                    .iter()
                    .filter(|g| g.kind == GapKind::Exit)
                    .find(|g| teleport::crossed(prev, next, g))
                    .copied()
            {
                tp.crossed_exit = true;
                tp.pending_exit = Some(exit);
            }
            if tp.crossed_exit {
                let exit_gap = tp.pending_exit;
                let should_commit = {
                    let game = runtime.live.host_match();
                    game.local_room() == from && game.local_target() == Some(to)
                };
                if should_commit && runtime.live.force_round(LocalAction::Advance) {
                    let arrived = runtime.live.host_match().local_room();
                    if let Some(g) = exit_gap {
                        cross_into_room(
                            seed_val,
                            tp,
                            &g,
                            arrived,
                            from,
                            runtime.live.host_match(),
                            &keys,
                            &items,
                        );
                    }
                } else if !should_commit && let Some(g) = exit_gap {
                    cross_into_room(
                        seed_val,
                        tp,
                        &g,
                        to,
                        from,
                        runtime.live.host_match(),
                        &keys,
                        &items,
                    );
                }
            } else {
                if let Some(entry) = tp
                    .geom
                    .gaps
                    .iter()
                    .filter(|g| g.kind == GapKind::Entry)
                    .find(|g| teleport::crossed(prev, next, g))
                    .copied()
                {
                    cross_into_room(
                        seed_val,
                        tp,
                        &entry,
                        from,
                        to,
                        runtime.live.host_match(),
                        &keys,
                        &items,
                    );
                }
            }
        }
    }

    if tp.place != place_before {
        let dests = compute_gap_dests(
            seed_val,
            tp.place,
            &tp.geom,
            runtime.live.host_match(),
            &keys,
            &items,
        );
        tp.gap_dests = dests;
    }
}

/// Capture/diagnostic helper: drop the player straight into `place` (rebuilding the
/// arena + geometry as if they had teleported in from `from`), without any physical
/// crossing. Used by the maze evidence capture in `crate::capture`.
pub(crate) fn debug_place_into(
    tp: &mut TeleportState,
    runtime: &MatchRuntime,
    place: Place,
    from: RoomId,
    keys: &KeystoneState,
    items: &ItemsState,
) {
    let nav = nav_for_place(MATCH_SEED, runtime.live.host_match(), keys, items, place);
    place_body(tp, place, from, None, &nav);
    tp.gap_dests = compute_gap_dests(
        MATCH_SEED,
        tp.place,
        &tp.geom,
        runtime.live.host_match(),
        keys,
        items,
    );
}

/// Capture/diagnostic helper: complete a threshold crossing once a derived bot has
/// physically routed to the doorway. This deliberately reuses the same frozen-destination
/// crossing helpers as [`teleport_sim`]; it only bypasses the final sub-step crossing
/// detection so evidence bots do not stall at a polygon or maze threshold.
pub(crate) fn debug_cross_gap_for_capture(
    tp: &mut TeleportState,
    runtime: &mut MatchRuntime,
    gap: teleport::DoorGap,
    keys: &KeystoneState,
    items: &ItemsState,
) {
    let place_before = tp.place;
    match tp.place {
        Place::Room(room) => {
            let nav = nav_for_place(MATCH_SEED, runtime.live.host_match(), keys, items, tp.place);
            cross_into(MATCH_SEED, tp, &gap, Place::Room(room), room, &nav, keys);
        }
        Place::Hallway { from, to, .. } if gap.kind == GapKind::Exit => {
            let should_commit = {
                let game = runtime.live.host_match();
                game.local_room() == from && game.local_target() == Some(to)
            };
            if should_commit {
                runtime.live.force_round(LocalAction::Advance);
                let arrived = runtime.live.host_match().local_room();
                cross_into_room(
                    MATCH_SEED,
                    tp,
                    &gap,
                    arrived,
                    from,
                    runtime.live.host_match(),
                    keys,
                    items,
                );
            } else {
                cross_into_room(
                    MATCH_SEED,
                    tp,
                    &gap,
                    to,
                    from,
                    runtime.live.host_match(),
                    keys,
                    items,
                );
            }
        }
        Place::Hallway { from, to, .. } if gap.kind == GapKind::Entry => {
            cross_into_room(
                MATCH_SEED,
                tp,
                &gap,
                from,
                to,
                runtime.live.host_match(),
                keys,
                items,
            );
        }
        _ => {}
    }
    if tp.place != place_before {
        tp.gap_dests = compute_gap_dests(
            MATCH_SEED,
            tp.place,
            &tp.geom,
            runtime.live.host_match(),
            keys,
            items,
        );
        tp.rendered = None;
    }
}
