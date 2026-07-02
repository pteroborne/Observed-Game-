//! Pure **navigation derivation**: project the deterministic brain (the hybrid
//! match's rendered routes), the keystone objectives, and the placed items (anchors,
//! room locks) into the [`teleport::Nav`] table the controller, renderer, and
//! diagnostics all read. No Bevy systems or presentation types — this is the one
//! place the "which thresholds does this room have right now" question is answered.

use observed_core::RoomId;
use observed_match::hybrid::HybridMatch;

use crate::items::ItemsState;
use crate::keystones::KeystoneState;
use crate::teleport::Place;

pub(crate) fn connections_for(game: &HybridMatch, room: RoomId) -> Vec<RoomId> {
    let mut connections: Vec<RoomId> = game
        .rendered
        .iter()
        .filter_map(|route| {
            if route.rooms.0 == room {
                Some(route.rooms.1)
            } else if route.rooms.1 == room {
                Some(route.rooms.0)
            } else {
                None
            }
        })
        .collect();
    connections.sort_unstable_by_key(|r| r.0);
    connections.dedup();
    connections
}

pub(crate) fn connections_for_nav(
    game: &HybridMatch,
    items: &ItemsState,
    room: RoomId,
) -> Vec<RoomId> {
    if let Some(connections) = items.locked_room_connections(room) {
        return connections;
    }
    let mut connections: Vec<RoomId> = connections_for(game, room)
        .into_iter()
        .filter(|&other| items.relation_allowed_by_room_locks(room, other))
        .collect();
    connections.extend(
        items
            .pinned_connections(room)
            .into_iter()
            .filter(|&other| items.relation_allowed_by_room_locks(room, other)),
    );
    connections.sort_by_key(|room| room.0);
    connections.dedup();
    connections
}

fn rendered_slot_for(
    game: &HybridMatch,
    room: RoomId,
    target: RoomId,
) -> Option<crate::teleport::ThresholdSlotId> {
    game.rendered
        .iter()
        .find(|route| {
            (route.rooms.0 == room && route.rooms.1 == target)
                || (route.rooms.0 == target && route.rooms.1 == room)
        })
        .and_then(|route| {
            [route.key.0, route.key.1]
                .into_iter()
                .find(|door| (door.0 as u32 / 4) == room.0)
                .map(|door| crate::teleport::ThresholdSlotId((door.0 % 4) as u8))
        })
}

pub(crate) fn slot_for_connection(
    game: &HybridMatch,
    items: &ItemsState,
    room: RoomId,
    target: RoomId,
) -> Option<crate::teleport::ThresholdSlotId> {
    rendered_slot_for(game, room, target).or_else(|| {
        connections_for_nav(game, items, room)
            .into_iter()
            .position(|candidate| candidate == target)
            .map(|slot| crate::teleport::ThresholdSlotId(slot as u8))
    })
}

pub(crate) fn room_connection_slots(
    game: &HybridMatch,
    items: &ItemsState,
    room: RoomId,
    connections: &[RoomId],
) -> Vec<crate::teleport::RoomConnectionSlot> {
    connections
        .iter()
        .enumerate()
        .map(|(fallback, &target)| crate::teleport::RoomConnectionSlot {
            target,
            slot: slot_for_connection(game, items, room, target)
                .unwrap_or(crate::teleport::ThresholdSlotId(fallback as u8)),
        })
        .collect()
}

pub(crate) fn room_target(
    game: &HybridMatch,
    room: RoomId,
    connections: &[RoomId],
) -> Option<RoomId> {
    if room == game.local_room() {
        return game.local_target();
    }
    if connections.contains(&game.local_room()) {
        Some(game.local_room())
    } else {
        connections.first().copied()
    }
}

pub(crate) fn nav_for_room(
    seed: u64,
    game: &HybridMatch,
    keys: &KeystoneState,
    items: &ItemsState,
    room: RoomId,
) -> crate::teleport::Nav {
    let connections = connections_for_nav(game, items, room);
    let connection_slots = room_connection_slots(game, items, room, &connections);
    let target_room = room_target(game, room, &connections);
    crate::teleport::Nav {
        connections,
        connection_slots,
        hallway_entry_room_slot: None,
        hallway_exit_room_slot: None,
        target_room,
        seed,
        version: game.reroute_commits,
        exit_locked: !keys.gate_open(),
        exit_room: keys.exit_room,
        pins: items.pins(),
    }
}

pub(crate) fn nav_from_brain(
    seed: u64,
    game: &HybridMatch,
    keys: &KeystoneState,
    items: &ItemsState,
) -> crate::teleport::Nav {
    nav_for_room(seed, game, keys, items, game.local_room())
}

pub(crate) fn nav_for_place(
    seed: u64,
    game: &HybridMatch,
    keys: &KeystoneState,
    items: &ItemsState,
    place: Place,
) -> crate::teleport::Nav {
    match place {
        Place::Room(room) => nav_for_room(seed, game, keys, items, room),
        Place::Hallway { from, to, .. } => {
            let mut nav = nav_for_room(seed, game, keys, items, from);
            nav.hallway_entry_room_slot = slot_for_connection(game, items, from, to);
            nav.hallway_exit_room_slot = slot_for_connection(game, items, to, from);
            nav
        }
    }
}
