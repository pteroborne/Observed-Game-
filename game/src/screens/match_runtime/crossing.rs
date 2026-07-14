use bevy::prelude::*;
use observed_core::RoomId;
use observed_match::hybrid::{HybridMatch, LocalAction};
use observed_traversal::rapier_controller::step_character;
use observed_traversal::{FIXED_DT, FpsBody};

use crate::flow::MATCH_SEED;
use crate::items::ItemsState;
use crate::keystones::KeystoneState;
use crate::sim::director::MatchDirector;
use crate::sim::nav::nav_for_place;
use crate::sim::state::{
    MatchIntent, MatchPaused, PlaceSnapshot, PlaceSnapshotId, TeleportState, ThresholdTransform,
    ThresholdTransit,
};
use crate::teleport::{self, GapKind, Place};
use crate::view::components::{CameraJuice, MatchAudioCue};

fn body_xz(tp: &TeleportState) -> Vec2 {
    Vec2::new(tp.body.position.x, tp.body.position.z)
}

/// Mix one stable word into a complete place-snapshot identity. Destinations are captured
/// once on place entry so preview and crossing consume the same immutable transaction.
fn mix_snapshot_word(hash: &mut u64, word: u64) {
    *hash ^= word;
    *hash = hash.wrapping_mul(0x100_0000_01B3);
}

fn mix_snapshot_bytes(hash: &mut u64, bytes: &[u8]) {
    mix_snapshot_word(hash, bytes.len() as u64);
    for byte in bytes {
        mix_snapshot_word(hash, u64::from(*byte));
    }
}

fn snapshot_id(
    place: Place,
    geom: &teleport::PlaceGeom,
    layout: Option<&observed_content::PlaceLayoutSnapshot>,
    arena: &observed_traversal::ArenaSpec,
    simulation_content_hash: [u8; 32],
) -> PlaceSnapshotId {
    let mut hash = 0xCBF2_9CE4_8422_2325_u64;
    for chunk in simulation_content_hash.chunks_exact(8) {
        mix_snapshot_word(&mut hash, u64::from_le_bytes(chunk.try_into().unwrap()));
    }
    match place {
        Place::Room(room) => mix_snapshot_word(&mut hash, u64::from(room.0)),
        Place::Hallway {
            corridor,
            entered_socket,
            variation,
            from,
            to,
        } => {
            for word in [
                u64::from(corridor.0),
                u64::from(entered_socket.0),
                variation as u64,
                u64::from(from.0),
                u64::from(to.0),
            ] {
                mix_snapshot_word(&mut hash, word);
            }
        }
    }
    for value in [geom.half.x, geom.half.y] {
        mix_snapshot_word(&mut hash, u64::from(value.to_bits()));
    }
    for wall in &geom.interior {
        for value in [wall.center.x, wall.center.y, wall.half.x, wall.half.y] {
            mix_snapshot_word(&mut hash, u64::from(value.to_bits()));
        }
    }
    if let Some(poly) = &geom.poly {
        mix_snapshot_word(&mut hash, poly.len() as u64);
        for point in poly {
            for value in [point.x, point.y] {
                mix_snapshot_word(&mut hash, u64::from(value.to_bits()));
            }
        }
    }
    for deck in &geom.decks {
        for value in [
            deck.center.x,
            deck.center.y,
            deck.half.x,
            deck.half.y,
            deck.bottom_y,
            deck.top_y,
        ] {
            mix_snapshot_word(&mut hash, u64::from(value.to_bits()));
        }
    }
    for gap in &geom.gaps {
        for word in [
            u64::from(gap.center.x.to_bits()),
            u64::from(gap.center.y.to_bits()),
            u64::from(gap.normal.x.to_bits()),
            u64::from(gap.normal.y.to_bits()),
            u64::from(gap.width.to_bits()),
            u64::from(gap.floor_y.to_bits()),
            u64::from(gap.threshold.room.room.0),
            u64::from(gap.threshold.room.slot.0),
            u64::from(gap.threshold.hall.corridor.0),
            u64::from(gap.threshold.hall.slot.0),
            gap.kind as u64,
        ] {
            mix_snapshot_word(&mut hash, word);
        }
    }
    for collider in &arena.colliders {
        mix_snapshot_word(&mut hash, u64::from(collider.id.0));
        for value in collider.center.to_array() {
            mix_snapshot_word(&mut hash, u64::from(value.to_bits()));
        }
        for value in collider.rotation {
            mix_snapshot_word(&mut hash, u64::from(value.to_bits()));
        }
        mix_snapshot_word(&mut hash, u64::from(collider.friction.to_bits()));
        match &collider.shape {
            observed_traversal::ColliderShape::Cuboid { half } => {
                mix_snapshot_word(&mut hash, 0);
                for value in half.to_array() {
                    mix_snapshot_word(&mut hash, u64::from(value.to_bits()));
                }
            }
            observed_traversal::ColliderShape::ConvexHull { points } => {
                mix_snapshot_word(&mut hash, 1);
                mix_snapshot_word(&mut hash, points.len() as u64);
                for point in points {
                    for value in point.to_array() {
                        mix_snapshot_word(&mut hash, u64::from(value.to_bits()));
                    }
                }
            }
        }
    }
    for value in [arena.floor_y]
        .into_iter()
        .chain(arena.safety_center.to_array())
        .chain(arena.safety_half.to_array())
    {
        mix_snapshot_word(&mut hash, u64::from(value.to_bits()));
    }
    if let Some(layout) = layout {
        mix_snapshot_word(&mut hash, u64::from(layout.generation));
        for placement in &layout.placements {
            mix_snapshot_bytes(&mut hash, placement.module_id.as_bytes());
            for value in placement.translation.into_iter().chain(placement.scale) {
                mix_snapshot_word(&mut hash, u64::from(value.to_bits()));
            }
            mix_snapshot_word(&mut hash, placement.yaw_degrees as u64);
            mix_snapshot_bytes(&mut hash, placement.entry_port.as_bytes());
            mix_snapshot_bytes(&mut hash, placement.exit_port.as_bytes());
        }
    }
    PlaceSnapshotId(hash)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_place_snapshot(
    seed: u64,
    place: Place,
    arrived_from: Option<RoomId>,
    crossed_threshold: Option<teleport::ThresholdLink>,
    game: &HybridMatch,
    keys: &KeystoneState,
    items: &ItemsState,
    collision_catalog: &crate::content::ContentCollisionCatalog,
    simulation_content_hash: [u8; 32],
) -> PlaceSnapshot {
    let nav = nav_for_place(seed, game, keys, items, place);
    let mut geom = teleport::geom_for(place, &nav);
    teleport::open_entry_threshold(&mut geom, crossed_threshold, arrived_from);
    let layout = collision_catalog.layout_for_place(place, &geom);
    let y_offset = teleport::place_y_offset(place);
    let arena = layout
        .as_ref()
        .and_then(|layout| collision_catalog.arena_for_layout(layout, &geom, y_offset))
        .unwrap_or_else(|| teleport::place_arena_spec(&geom, y_offset, crate::layout::WALL_HEIGHT));
    let entry_gap = crossed_threshold.and_then(|threshold| {
        geom.gaps
            .iter()
            .find(|gap| {
                gap.threshold.room == threshold.room
                    && gap.threshold.hall == threshold.hall
                    && gap.kind.is_passage()
            })
            .copied()
    });
    PlaceSnapshot {
        id: snapshot_id(
            place,
            &geom,
            layout.as_ref(),
            &arena,
            simulation_content_hash,
        ),
        place,
        geom,
        layout,
        arena,
        simulation_content_hash,
        entry_gap,
        arrived_from,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_threshold_transits(
    seed: u64,
    source: &PlaceSnapshot,
    game: &HybridMatch,
    keys: &KeystoneState,
    items: &ItemsState,
    config: &observed_traversal::FpsConfig,
    collision_catalog: &crate::content::ContentCollisionCatalog,
    simulation_content_hash: [u8; 32],
) -> Vec<ThresholdTransit> {
    let nav = nav_for_place(seed, game, keys, items, source.place);
    source
        .geom
        .gaps
        .iter()
        .filter(|gap| gap.kind.is_passage() || gap.kind == GapKind::LockedExit)
        .map(|source_gap| {
            // A room chooses a corridor through the live reciprocal topology. Once that
            // corridor is installed, however, its endpoint identities are immutable even
            // if the round commit refactors or collapse-seals the graph behind the actor.
            // Hallway gaps already carry the stable room socket and target, so resolve
            // them directly and let the exact reciprocal-snapshot check below validate it.
            let destination_place = match source.place {
                Place::Hallway { .. } => Place::Room(source_gap.target),
                Place::Room(_) => {
                    let Some((destination, _)) =
                        teleport::resolve_crossing(source.place, source_gap, &nav)
                    else {
                        return ThresholdTransit {
                            source_gap: *source_gap,
                            destination_gap: None,
                            destination: None,
                            transform: None,
                            fault: Some(
                                "LINK FAULT: source socket has no reciprocal partner".into(),
                            ),
                        };
                    };
                    destination
                }
            };
            let arrived_from = match (source.place, destination_place) {
                (Place::Hallway { from, to, .. }, Place::Room(room)) => {
                    Some(if room == to { from } else { to })
                }
                _ => None,
            };
            let destination = build_place_snapshot(
                seed,
                destination_place,
                arrived_from,
                Some(source_gap.threshold),
                game,
                keys,
                items,
                collision_catalog,
                simulation_content_hash,
            );
            let Some(destination_gap) = destination.entry_gap else {
                return ThresholdTransit {
                    source_gap: *source_gap,
                    destination_gap: None,
                    destination: None,
                    transform: None,
                    fault: Some("LINK FAULT: destination lacks exact reciprocal socket".into()),
                };
            };
            let required_inside = config.radius + teleport::PREVIEW_OUTSET + 0.02;
            let landing_xz = destination_gap.center - destination_gap.normal * required_inside;
            let landing = Vec3::new(
                landing_xz.x,
                teleport::place_y_offset(destination.place)
                    + destination_gap.floor_y
                    + config.half_height,
                landing_xz.y,
            );
            let destination_scene =
                observed_traversal::rapier_controller::RapierTraversalScene::from_arena_spec(
                    &destination.arena,
                );
            if !destination_scene.capsule_is_clear(landing, config.radius, config.half_height) {
                return ThresholdTransit {
                    source_gap: *source_gap,
                    destination_gap: None,
                    destination: None,
                    transform: None,
                    fault: Some(
                        "LINK FAULT: reciprocal socket landing volume is obstructed".into(),
                    ),
                };
            }
            let alignment = match destination.place {
                Place::Hallway { .. } => {
                    teleport::hallway_gap_alignment(source_gap, &destination_gap)
                }
                Place::Room(_) => teleport::room_alignment(source_gap, &destination_gap),
            };
            ThresholdTransit {
                source_gap: *source_gap,
                destination_gap: Some(destination_gap),
                transform: Some(ThresholdTransform {
                    alignment,
                    source_floor_y: teleport::place_y_offset(source.place) + source_gap.floor_y,
                    destination_floor_y: teleport::place_y_offset(destination.place)
                        + destination_gap.floor_y,
                }),
                destination: Some(destination),
                fault: None,
            }
        })
        .collect()
}

/// Build the exact return transaction for a successful crossing. The destination graph
/// may change while the actor is in transit, so resolving the doorway behind them again
/// would be both visually dishonest and unsafe. Reversing the already-validated transform
/// makes the return preview and return crossing point at the precise place just left.
fn reverse_transit(source: &PlaceSnapshot, forward: &ThresholdTransit) -> Option<ThresholdTransit> {
    let destination_gap = forward.destination_gap?;
    let transform = forward.transform?;
    Some(ThresholdTransit {
        source_gap: destination_gap,
        destination_gap: Some(forward.source_gap),
        destination: Some(source.clone()),
        transform: Some(ThresholdTransform {
            alignment: transform.alignment.inverse(),
            source_floor_y: transform.destination_floor_y,
            destination_floor_y: transform.source_floor_y,
        }),
        fault: None,
    })
}

fn install_reverse_transit(
    transits: &mut Vec<ThresholdTransit>,
    reverse: Option<ThresholdTransit>,
) {
    let Some(reverse) = reverse else {
        return;
    };
    transits.retain(|transit| transit.source_gap.threshold != reverse.source_gap.threshold);
    transits.push(reverse);
}

pub(super) fn install_snapshot(tp: &mut TeleportState, snapshot: PlaceSnapshot, body: FpsBody) {
    tp.rapier = observed_traversal::rapier_controller::RapierTraversalScene::from_arena_spec(
        &snapshot.arena,
    );
    tp.place = snapshot.place;
    tp.layout = snapshot.layout.clone();
    tp.geom = snapshot.geom.clone();
    tp.arrived_from = snapshot.arrived_from;
    tp.current_snapshot = snapshot;
    tp.body = body;
    tp.prev_xz = body_xz(tp);
    tp.last_safe_body = body;
    tp.rendered = None;
}

fn sync_dynamic_closures(tp: &mut TeleportState, nav: &teleport::Nav) -> bool {
    let mut changed = false;
    let protected_entry = tp.current_snapshot.entry_gap.map(|gap| gap.threshold);
    for gap in &mut tp.geom.gaps {
        // A hallway is an already-committed traversal transaction, not a projection of
        // either endpoint room's newly-refactored slot table. Likewise, the room socket
        // just used to arrive remains open while its exact reverse transaction is present.
        // Applying bare room-local slot numbers to a hallway used to seal both ends when
        // only the room behind the player had collapsed.
        let may_apply_room_seal = matches!(tp.place, Place::Room(_))
            && protected_entry != Some(gap.threshold)
            && nav.sealed_slots.contains(&gap.threshold.room.slot);
        let next_kind = if may_apply_room_seal {
            GapKind::Collapsed
        } else if gap.target == nav.exit_room
            && matches!(gap.kind, GapKind::Exit | GapKind::LockedExit)
        {
            if nav.exit_locked {
                GapKind::LockedExit
            } else {
                GapKind::Exit
            }
        } else {
            gap.kind
        };
        changed |= next_kind != gap.kind;
        gap.kind = next_kind;
    }
    if !changed {
        return false;
    }
    let y_offset = teleport::place_y_offset(tp.place);
    let arena = tp
        .layout
        .as_ref()
        .and_then(|layout| {
            tp.collision_catalog
                .arena_for_layout(layout, &tp.geom, y_offset)
        })
        .unwrap_or_else(|| {
            teleport::place_arena_spec(&tp.geom, y_offset, crate::layout::WALL_HEIGHT)
        });
    tp.rapier =
        observed_traversal::rapier_controller::RapierTraversalScene::from_arena_spec(&arena);
    let entry_threshold = tp.current_snapshot.entry_gap.map(|gap| gap.threshold);
    tp.current_snapshot.geom = tp.geom.clone();
    tp.current_snapshot.arena = arena;
    tp.current_snapshot.entry_gap = entry_threshold.and_then(|threshold| {
        tp.current_snapshot
            .geom
            .gaps
            .iter()
            .find(|gap| gap.threshold == threshold)
            .copied()
    });
    tp.current_snapshot.id = snapshot_id(
        tp.place,
        &tp.current_snapshot.geom,
        tp.current_snapshot.layout.as_ref(),
        &tp.current_snapshot.arena,
        tp.simulation_content_hash,
    );
    tp.rendered = None;
    true
}

pub(crate) fn sync_threshold_closures(
    runtime: Res<MatchDirector>,
    mut tp: ResMut<TeleportState>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
) {
    let seed = seed.map(|seed| seed.0).unwrap_or(MATCH_SEED);
    let nav = nav_for_place(seed, runtime.live.host_match(), &keys, &items, tp.place);
    if sync_dynamic_closures(&mut tp, &nav) {
        tp.transits = compute_threshold_transits(
            seed,
            &tp.current_snapshot,
            runtime.live.host_match(),
            &keys,
            &items,
            &tp.config,
            &tp.collision_catalog,
            tp.simulation_content_hash,
        );
    }
}

/// Install the exact snapshot already displayed by a threshold. Position, view and
/// velocity are mapped through the same frozen transform; there is no live fallback.
pub(crate) fn cross_transit(tp: &mut TeleportState, transit: &ThresholdTransit) -> bool {
    let (Some(destination), Some(destination_gap), Some(transform)) = (
        transit.destination.clone(),
        transit.destination_gap,
        transit.transform,
    ) else {
        return false;
    };
    assert_eq!(
        destination.simulation_content_hash, tp.simulation_content_hash,
        "threshold snapshot was produced by different simulation content"
    );
    let mut body = tp.body;
    let mut xz = transform
        .alignment
        .inverse_apply(Vec2::new(body.position.x, body.position.z));
    // A swept capsule fires when its leading edge reaches the source plane, while a
    // rigid portal transform maps that still-source-side centre to the exterior side of
    // the destination plane. Move only along the exact reciprocal normal until the full
    // capsule is safely supported inside the installed destination footprint; lateral
    // offset, view, velocity, and floor height remain transform-derived.
    let signed_depth = (xz - destination_gap.center).dot(destination_gap.normal);
    let required_inside = tp.config.radius + teleport::PREVIEW_OUTSET + 0.02;
    if signed_depth > -required_inside {
        xz -= destination_gap.normal * (signed_depth + required_inside);
    }
    body.position.x = xz.x;
    body.position.z = xz.y;
    body.position.y += transform.destination_floor_y - transform.source_floor_y;
    body.yaw = (body.yaw - transform.alignment.yaw).rem_euclid(std::f32::consts::TAU);
    let velocity_origin = transform.alignment.inverse_apply(Vec2::ZERO);
    let velocity_xz = transform
        .alignment
        .inverse_apply(Vec2::new(body.velocity.x, body.velocity.z))
        - velocity_origin;
    body.velocity.x = velocity_xz.x;
    body.velocity.z = velocity_xz.y;
    body.spawn = body.position;
    body.spawn_yaw = body.yaw;
    install_snapshot(tp, destination, body);
    true
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn place_body_at(
    seed: u64,
    tp: &mut TeleportState,
    place: Place,
    pos: Vec2,
    game: &HybridMatch,
    keys: &KeystoneState,
    items: &ItemsState,
) {
    let snapshot = build_place_snapshot(
        seed,
        place,
        None,
        None,
        game,
        keys,
        items,
        &tp.collision_catalog,
        tp.simulation_content_hash,
    );
    let mut body = FpsBody::spawned(
        Vec3::new(
            pos.x,
            teleport::place_y_offset(place) + tp.config.half_height,
            pos.y,
        ),
        tp.body.yaw,
    );
    body.pitch = tp.body.pitch;
    install_snapshot(tp, snapshot, body);
}

fn room_threshold_commit_target(
    place: Place,
    source_gap: &teleport::DoorGap,
    destination: Option<Place>,
) -> Option<(RoomId, RoomId)> {
    match (place, source_gap.kind, destination) {
        (Place::Room(room), GapKind::Forward, Some(Place::Hallway { from, to, .. }))
            if room == from =>
        {
            Some((from, to))
        }
        _ => None,
    }
}

/// Fixed-step teleport controller. Crossing a room's selected forward threshold commits
/// the route, then installs the exact hallway snapshot already shown in that doorway.
/// The hallway remains a frozen traversal transaction; its far threshold installs a
/// post-commit destination-room snapshot with an exact reversible arrival socket.
#[allow(clippy::too_many_arguments)]
pub(crate) fn teleport_sim(
    mut commands: Commands,
    mut runtime: ResMut<MatchDirector>,
    tp: ResMut<TeleportState>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    mut intent: ResMut<MatchIntent>,
    paused: Res<MatchPaused>,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
    assets: Res<crate::view::assets::MatchAssets>,
    settings: Res<crate::settings::Settings>,
    mut juice: ResMut<CameraJuice>,
    mut audio_director: ResMut<crate::screens::audio::AudioDirector>,
) {
    if paused.0 || runtime.done {
        return;
    }
    let seed_val = seed.map(|s| s.0).unwrap_or(MATCH_SEED);
    let tp = tp.into_inner();
    let nav = nav_for_place(seed_val, runtime.live.host_match(), &keys, &items, tp.place);
    if sync_dynamic_closures(tp, &nav) {
        tp.transits = compute_threshold_transits(
            seed_val,
            &tp.current_snapshot,
            runtime.live.host_match(),
            &keys,
            &items,
            &tp.config,
            &tp.collision_catalog,
            tp.simulation_content_hash,
        );
    }
    let prev = body_xz(tp);
    if teleport::inside_footprint(&tp.geom, prev, tp.config.radius) {
        tp.last_safe_body = tp.body;
    }
    let config = tp.config;
    let prev_grounded = tp.prev_grounded;

    {
        let TeleportState { body, rapier, .. } = tp;
        step_character(rapier, body, intent.0, &config, FIXED_DT);
    }
    intent.0.interact_pressed = false;

    tp.prev_grounded = tp.body.grounded;

    // Detect jump/land stings
    if prev_grounded && !tp.body.grounded && tp.body.velocity.y > 0.0 {
        juice.jump_timer = 0.20;
        audio_director.request(
            &mut commands,
            &assets.jump,
            MatchAudioCue::Jump,
            "Jump sting",
            None,
            &settings,
        );
    } else if !prev_grounded && tp.body.grounded {
        juice.land_timer = 0.25;
        audio_director.request(
            &mut commands,
            &assets.land,
            MatchAudioCue::Land,
            "Land sting",
            None,
            &settings,
        );
    }

    let next = body_xz(tp);
    tp.prev_xz = next;

    let place_floor_y = teleport::place_y_offset(tp.place);
    let world_feet_y = tp.body.position.y - config.half_height;
    let place_before = tp.place;
    let crossed_transit = tp
        .transits
        .iter()
        .filter(|transit| transit.is_valid())
        .filter_map(|transit| {
            let live_gap = tp.geom.gaps.iter().find(|gap| {
                gap.threshold == transit.source_gap.threshold && gap.kind.is_passage()
            })?;
            teleport::capsule_crossing_fraction(
                prev,
                next,
                live_gap,
                config.radius,
                world_feet_y,
                config.half_height,
                place_floor_y,
                crate::layout::WALL_HEIGHT,
            )
            .map(|fraction| {
                let mut transit = transit.clone();
                transit.source_gap = *live_gap;
                (fraction, transit)
            })
        })
        .min_by(|(a, _), (b, _)| a.total_cmp(b))
        .map(|(_, transit)| transit);

    let mut crossed_reverse = None;
    if let Some(transit) = crossed_transit {
        // The route choice commits at the room threshold, before the player enters the
        // corridor. The hallway is therefore a committed traversal beat and its frozen
        // destination-room snapshot is built from the post-commit graph. Committing at
        // the far end used to install a pre-commit room whose former side door remained
        // sealed even though the brain's new target had moved there.
        let commit_target = room_threshold_commit_target(
            tp.place,
            &transit.source_gap,
            transit.destination.as_ref().map(|snapshot| snapshot.place),
        );
        let may_cross = if let Some((from, to)) = commit_target {
            let should_commit = {
                let game = runtime.live.host_match();
                game.local_room() == from && game.local_target() == Some(to)
            };
            !should_commit || runtime.live.force_round(LocalAction::Advance)
        } else {
            true
        };
        if may_cross {
            let reverse = reverse_transit(&tp.current_snapshot, &transit);
            if cross_transit(tp, &transit) {
                crossed_reverse = reverse;
            }
        } else if tp.boundary_recoveries < 3 {
            let game = runtime.live.host_match();
            warn!(
                "THRESHOLD_COMMIT_REJECTED place={:?} destination={:?} local_room={:?} local_target={:?} live_finished={}",
                tp.place,
                transit.destination.as_ref().map(|snapshot| snapshot.place),
                game.local_room(),
                game.local_target(),
                runtime.live.finished(),
            );
        }
    }

    if tp.place != place_before {
        let mut transits = compute_threshold_transits(
            seed_val,
            &tp.current_snapshot,
            runtime.live.host_match(),
            &keys,
            &items,
            &tp.config,
            &tp.collision_catalog,
            tp.simulation_content_hash,
        );
        install_reverse_transit(&mut transits, crossed_reverse);
        tp.transits = transits;
    } else if !teleport::inside_footprint(&tp.geom, next, config.radius) {
        tp.body = tp.last_safe_body;
        tp.prev_xz = body_xz(tp);
        tp.boundary_recoveries = tp.boundary_recoveries.saturating_add(1);
        if tp.boundary_recoveries <= 3 || tp.boundary_recoveries.is_multiple_of(120) {
            warn!(
                "THRESHOLD_BOUNDARY_RECOVERY place={:?} count={} prev={:?} next={:?} feet_y={:.3} passage_planes={:?}",
                tp.place,
                tp.boundary_recoveries,
                prev,
                next,
                world_feet_y,
                tp.geom
                    .gaps
                    .iter()
                    .filter(|gap| gap.kind.is_passage())
                    .map(|gap| (
                        gap.kind,
                        gap.target,
                        (prev - gap.center).dot(gap.normal) + config.radius,
                        (next - gap.center).dot(gap.normal) + config.radius,
                        (next - gap.center).dot(Vec2::new(-gap.normal.y, gap.normal.x)),
                        gap.floor_y,
                    ))
                    .collect::<Vec<_>>(),
            );
        }
    }
}

/// Capture/diagnostic helper: drop the player straight into `place` (rebuilding the
/// arena + geometry as if they had teleported in from `from`), without any physical
/// crossing. Used by the maze evidence capture in `crate::capture`.
pub(crate) fn debug_place_into(
    tp: &mut TeleportState,
    runtime: &MatchDirector,
    place: Place,
    from: RoomId,
    keys: &KeystoneState,
    items: &ItemsState,
) {
    let place = match place {
        Place::Hallway {
            from,
            to,
            variation,
            ..
        } => {
            let source_nav = nav_for_place(
                MATCH_SEED,
                runtime.live.host_match(),
                keys,
                items,
                Place::Room(from),
            );
            let source_geom = teleport::geom_for(Place::Room(from), &source_nav);
            source_geom
                .gaps
                .iter()
                .find(|gap| gap.target == to && gap.kind.is_passage())
                .and_then(|gap| {
                    teleport::resolve_crossing(Place::Room(from), gap, &source_nav).and_then(
                        |(resolved, _)| match resolved {
                            Place::Hallway {
                                corridor,
                                entered_socket,
                                ..
                            } => Some(Place::Hallway {
                                corridor,
                                entered_socket,
                                variation,
                                from,
                                to,
                            }),
                            Place::Room(_) => None,
                        },
                    )
                })
                .unwrap_or(place)
        }
        Place::Room(_) => place,
    };
    let snapshot = build_place_snapshot(
        MATCH_SEED,
        place,
        Some(from),
        None,
        runtime.live.host_match(),
        keys,
        items,
        &tp.collision_catalog,
        tp.simulation_content_hash,
    );
    let mut body = FpsBody::spawned(
        Vec3::new(
            0.0,
            teleport::place_y_offset(place) + tp.config.half_height,
            0.0,
        ),
        tp.body.yaw,
    );
    body.pitch = tp.body.pitch;
    install_snapshot(tp, snapshot, body);
    tp.transits = compute_threshold_transits(
        MATCH_SEED,
        &tp.current_snapshot,
        runtime.live.host_match(),
        keys,
        items,
        &tp.config,
        &tp.collision_catalog,
        tp.simulation_content_hash,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_core::{CorridorId, ThresholdSlotId};

    fn gap(kind: GapKind) -> teleport::DoorGap {
        teleport::DoorGap {
            center: Vec2::ZERO,
            normal: Vec2::Y,
            width: teleport::THRESHOLD_WIDTH,
            target: RoomId(2),
            kind,
            threshold: teleport::ThresholdLink {
                room: teleport::RoomThreshold {
                    room: RoomId(1),
                    slot: ThresholdSlotId(3),
                },
                hall: teleport::HallThreshold {
                    corridor: CorridorId(12),
                    slot: ThresholdSlotId(0),
                },
                local_side: teleport::ThresholdLocalSide::Room,
            },
            floor_y: 0.0,
        }
    }

    #[test]
    fn only_a_room_forward_threshold_commits_the_round() {
        let hall = Place::Hallway {
            corridor: CorridorId(12),
            entered_socket: ThresholdSlotId(0),
            variation: 4,
            from: RoomId(1),
            to: RoomId(2),
        };
        assert_eq!(
            room_threshold_commit_target(
                Place::Room(RoomId(1)),
                &gap(GapKind::Forward),
                Some(hall)
            ),
            Some((RoomId(1), RoomId(2)))
        );
        assert_eq!(
            room_threshold_commit_target(Place::Room(RoomId(1)), &gap(GapKind::Entry), Some(hall)),
            None
        );
        assert_eq!(
            room_threshold_commit_target(hall, &gap(GapKind::Exit), Some(Place::Room(RoomId(2)))),
            None
        );
    }
}
