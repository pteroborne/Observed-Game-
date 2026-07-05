//! Pure **navigation derivation**: project the deterministic brain (the hybrid
//! match's rendered routes), the keystone objectives, and the placed items (anchors,
//! room locks) into the [`teleport::Nav`] table the controller, renderer, and
//! diagnostics all read. No Bevy systems or presentation types — this is the one
//! place the "which thresholds does this room have right now" question is answered.

use observed_core::{RoomId, TeamId};
use observed_match::hybrid::HybridMatch;

use crate::items::ItemsState;
use crate::keystones::KeystoneState;
use crate::teleport::Place;

/// One neighbour's rival attribution: whether a rival team's clump currently occupies
/// it, and whether a rival team has anchored it. Both can be `Some` (a team may anchor
/// a room and stand in it), and they can name different teams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RivalSignal {
    pub(crate) neighbor: RoomId,
    pub(crate) presence: Option<TeamId>,
    pub(crate) anchor: Option<TeamId>,
}

/// Every rival-relevant neighbour of `room` — the room's current rendered connections
/// plus its four graph-side base neighbours (so a collapsed/unrendered side still gets
/// an attribution if a rival is somehow attributed to it) — with a deterministic
/// presence/anchor projection for each. This is the single reconciliation point between
/// the room-occupancy heuristic (`crate::rivals::rivals_in_room`, kept for the rival
/// avatar walk) and `CompetitiveFacility::pin_sources`' anchor bookkeeping: the frame
/// style factory must consume this projection, not re-derive its own.
pub(crate) fn rival_signals(
    game: &HybridMatch,
    local_team: usize,
    room: RoomId,
) -> Vec<RivalSignal> {
    let facility = &game.competitive;
    let mut neighbors = connections_for(game, room);
    for side in observed_observation::Side::ALL {
        let door = facility.structure.graph.door_id(room, side);
        let partner = facility.structure.graph.partner(door);
        let other = facility.structure.graph.door(partner).room;
        // A door can partner back to its own room (e.g. a sealed/self-linked side); that
        // is not a neighbour a threshold frame can point rivals at.
        if other != room {
            neighbors.push(other);
        }
    }
    neighbors.sort_unstable_by_key(|r| r.0);
    neighbors.dedup();

    neighbors
        .into_iter()
        .map(|neighbor| {
            let presence = (0..facility.teams.len())
                .filter(|&i| i != local_team)
                .find(|&i| facility.teams[i].active_runner() && facility.team_room(i) == neighbor)
                .map(|i| facility.teams[i].id);

            let anchor = facility
                .structure
                .anchors
                .iter()
                .filter(|anchor| anchor.room == neighbor && anchor.team.0 as usize != local_team)
                .map(|anchor| anchor.team)
                .min_by_key(|team| team.0);

            RivalSignal {
                neighbor,
                presence,
                anchor,
            }
        })
        .collect()
}

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
    if game.competitive.structure.is_sealed_room(room) {
        return Vec::new();
    }
    let relation_allowed = |other: RoomId| {
        !game.competitive.structure.is_sealed_room(other)
            && items.relation_allowed_by_room_locks(room, other)
    };
    if let Some(connections) = items.locked_room_connections(room) {
        return connections
            .into_iter()
            .filter(|&other| relation_allowed(other))
            .collect();
    }
    let mut connections: Vec<RoomId> = connections_for(game, room)
        .into_iter()
        .filter(|&other| relation_allowed(other))
        .collect();
    connections.extend(
        items
            .pinned_connections(room)
            .into_iter()
            .filter(|&other| relation_allowed(other)),
    );
    connections.sort_by_key(|room| room.0);
    connections.dedup();
    connections
}

pub(crate) fn sealed_slots_for_room(
    game: &HybridMatch,
    room: RoomId,
) -> Vec<crate::teleport::ThresholdSlotId> {
    observed_observation::Side::ALL
        .iter()
        .enumerate()
        .filter_map(|(slot, side)| {
            let door = game.competitive.structure.graph.door_id(room, *side);
            game.competitive
                .structure
                .graph
                .is_sealed(door)
                .then_some(crate::teleport::ThresholdSlotId(slot as u8))
        })
        .collect()
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
    let sealed_slots = sealed_slots_for_room(game, room);
    let target_room = room_target(game, room, &connections);
    let room_role = game
        .competitive
        .map_spec
        .as_ref()
        .and_then(|spec| spec.room(room).map(|room| room.role));
    let corridor_roles = game
        .competitive
        .map_spec
        .as_ref()
        .map(|spec| {
            connections
                .iter()
                .filter_map(|&target| {
                    spec.corridor_role_between(room, target)
                        .map(|role| (target, role))
                })
                .collect()
        })
        .unwrap_or_default();
    crate::teleport::Nav {
        connections,
        connection_slots,
        sealed_slots,
        hallway_entry_room_slot: None,
        hallway_exit_room_slot: None,
        target_room,
        room_role,
        corridor_roles,
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
            // `nav_for_room`'s `corridor_roles` is keyed off `from`'s *live* rendered
            // connections, which should include `to` — but a hallway can be rendered
            // for a `to` that just decohered off that list, so make sure this specific
            // edge's role is present regardless of whether `to` still counts as a live
            // connection of `from`.
            if nav.corridor_role_for(to).is_none()
                && let Some(role) = game
                    .competitive
                    .map_spec
                    .as_ref()
                    .and_then(|spec| spec.corridor_role_between(from, to))
            {
                nav.corridor_roles.push((to, role));
            }
            nav
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_match::hybrid::HybridMatch;

    const SEED: u64 = 1;

    #[test]
    fn no_rivals_means_every_signal_is_none() {
        let game = HybridMatch::authored(SEED);
        let room = game.local_room();
        let signals = rival_signals(&game, 0, room);
        assert!(!signals.is_empty(), "a room has at least one neighbour");
        for s in &signals {
            assert_ne!(s.neighbor, room, "a room is never its own neighbour");
            assert_eq!(s.presence, None, "no rival has moved yet");
            assert_eq!(s.anchor, None, "no anchor has been placed yet");
        }
    }

    #[test]
    fn a_rival_clump_in_a_neighbour_is_reported_as_presence_only() {
        let mut game = HybridMatch::authored(SEED);
        let room = game.local_room();
        let neighbor = connections_for(&game, room)
            .first()
            .copied()
            .expect("the start room has a neighbour");
        let base = game.competitive.teams[1].member_base;
        game.competitive.structure.graph.players[base] = neighbor;

        let signals = rival_signals(&game, 0, room);
        let signal = signals
            .iter()
            .find(|s| s.neighbor == neighbor)
            .expect("the neighbour is in the projection");
        assert_eq!(signal.presence, Some(observed_core::TeamId(1)));
        assert_eq!(signal.anchor, None, "presence without an anchor");
    }

    #[test]
    fn a_rival_anchor_in_a_neighbour_is_reported_as_anchor_only() {
        let mut game = HybridMatch::authored(SEED);
        let room = game.local_room();
        let neighbor = connections_for(&game, room)
            .first()
            .copied()
            .expect("the start room has a neighbour");
        game.competitive
            .place_team_anchor(observed_core::TeamId(2), neighbor);

        let signals = rival_signals(&game, 0, room);
        let signal = signals
            .iter()
            .find(|s| s.neighbor == neighbor)
            .expect("the neighbour is in the projection");
        assert_eq!(signal.presence, None, "no team is standing there");
        assert_eq!(signal.anchor, Some(observed_core::TeamId(2)));
    }

    #[test]
    fn a_neighbour_can_carry_both_presence_and_a_different_anchor() {
        let mut game = HybridMatch::authored(SEED);
        let room = game.local_room();
        let neighbor = connections_for(&game, room)
            .first()
            .copied()
            .expect("the start room has a neighbour");
        let base = game.competitive.teams[1].member_base;
        game.competitive.structure.graph.players[base] = neighbor;
        game.competitive
            .place_team_anchor(observed_core::TeamId(2), neighbor);

        let signals = rival_signals(&game, 0, room);
        let signal = signals
            .iter()
            .find(|s| s.neighbor == neighbor)
            .expect("the neighbour is in the projection");
        assert_eq!(signal.presence, Some(observed_core::TeamId(1)));
        assert_eq!(signal.anchor, Some(observed_core::TeamId(2)));
    }

    #[test]
    fn the_local_team_is_never_reported_as_a_rival_signal() {
        let mut game = HybridMatch::authored(SEED);
        let room = game.local_room();
        let neighbor = connections_for(&game, room)
            .first()
            .copied()
            .expect("the start room has a neighbour");
        // Move the local team's own clump and anchor into the neighbour: neither
        // should ever surface as a rival signal for itself.
        let base = game.competitive.teams[0].member_base;
        game.competitive.structure.graph.players[base] = neighbor;
        game.competitive
            .place_team_anchor(observed_core::TeamId(0), neighbor);

        let signals = rival_signals(&game, 0, room);
        let signal = signals
            .iter()
            .find(|s| s.neighbor == neighbor)
            .expect("the neighbour is in the projection");
        assert_eq!(signal.presence, None, "the local team is never a rival");
        assert_eq!(
            signal.anchor, None,
            "the local team's own anchor is not a rival"
        );
    }

    #[test]
    fn signals_are_deterministic_across_calls() {
        let game = HybridMatch::authored(SEED);
        let room = game.local_room();
        assert_eq!(rival_signals(&game, 0, room), rival_signals(&game, 0, room));
    }
}
