//! Pure model for the toggleable **tac-map** overlay — the teleport-model heir of
//! `fps_match_lab`'s Tab tac-map and `match_replay`'s schematic.
//!
//! The map is a top-down schematic of the nine-room facility graph laid out on a 3×3
//! grid by `RoomId`. The protected spine `[0,1,2,5,4,3,6,7,8]` snakes through that grid
//! (a boustrophedon), so every spine step is to a *grid-adjacent* room — which lets the
//! overlay draw the route as plain horizontal/vertical bars (no rotated lines) and renders
//! cleanly as UI nodes, no second camera needed.
//!
//! This is pure projection of the deterministic brain (`team_room` / `collapse_rooms`) plus
//! the local keystone inventory; it never writes match state, so determinism/replay/lockstep
//! are untouched. `screens::draw_tac_map` builds the UI from this model.

use bevy::prelude::Vec2;
use observed_core::RoomId;
use observed_facility::map_spec::RoomRole;
use observed_match::facility::{CompetitiveFacility, TEAM_COUNT};
use observed_match::mutable::{EXIT_ROOM, START_ROOM, spine_next};

use crate::flow::LOCAL_TEAM;
use crate::keystones::KeystoneState;
use crate::teleport::Place;

/// The facility's `(col, row)` on the 3×3 schematic, by `RoomId`.
pub fn grid_pos(room: RoomId) -> Vec2 {
    Vec2::new((room.0 % 3) as f32, (room.0 / 3) as f32)
}

/// The protected spine in visiting order (entrance → exit), via the proven `spine_next`.
pub fn spine() -> Vec<RoomId> {
    let mut seq = vec![RoomId(START_ROOM)];
    let mut room = RoomId(START_ROOM);
    while let Some((next, _)) = spine_next(room) {
        seq.push(next);
        room = next;
    }
    seq
}

/// Where the player reads on the map: standing in a room, or walking a hallway between two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerMark {
    Room(RoomId),
    Between(RoomId, RoomId),
}

/// Everything the overlay needs to draw, snapshotted from the live match.
#[derive(Clone, Debug, PartialEq)]
pub struct MapModel {
    pub rooms: Vec<(RoomId, Vec2, RoomRole)>,
    pub routes: Vec<(RoomId, RoomId)>,
    pub player: PlayerMark,
    /// `(rival team index, its room)` for each still-running rival.
    pub rivals: Vec<(usize, RoomId)>,
    /// Spine rooms the collapse has already swallowed.
    pub collapse: Vec<RoomId>,
    /// Rooms still holding an uncollected keystone.
    pub keystones: Vec<RoomId>,
    pub exit: RoomId,
    pub exit_open: bool,
}

/// Build the map model from the live facility, the keystone inventory, and the player's
/// current teleport place.
pub fn build_map(facility: &CompetitiveFacility, keys: &KeystoneState, place: Place) -> MapModel {
    let player = match place {
        Place::Room(r) => PlayerMark::Room(r),
        Place::Hallway { from, to, .. } => PlayerMark::Between(from, to),
    };
    let rivals = (0..TEAM_COUNT)
        .filter(|&i| i as u8 != LOCAL_TEAM.0)
        .filter(|&i| facility.teams.get(i).is_some_and(|t| t.active_runner()))
        .map(|i| (i, facility.team_room(i)))
        .collect();
    let keystones = keys
        .rooms
        .iter()
        .copied()
        .filter(|&r| keys.has_uncollected(r))
        .collect();
    let (rooms, routes) = if let Some(spec) = &facility.map_spec {
        (
            spec.rooms
                .iter()
                .map(|room| (room.id, room.schematic, room.role))
                .collect(),
            spec.edges
                .iter()
                .map(|edge| (edge.a.room, edge.b.room))
                .collect(),
        )
    } else {
        (
            (0..9)
                .map(|id| {
                    let room = RoomId(id);
                    let role = if id == START_ROOM {
                        RoomRole::Start
                    } else if id == EXIT_ROOM {
                        RoomRole::Exit
                    } else {
                        RoomRole::Decision
                    };
                    (room, grid_pos(room), role)
                })
                .collect(),
            spine().windows(2).map(|pair| (pair[0], pair[1])).collect(),
        )
    };
    MapModel {
        rooms,
        routes,
        player,
        rivals,
        collapse: facility.collapse_rooms(),
        keystones,
        exit: RoomId(EXIT_ROOM),
        exit_open: keys.gate_open(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spine_runs_entrance_to_exit_and_every_step_is_grid_adjacent() {
        let spine = spine();
        assert_eq!(spine.len(), 9, "all nine rooms are on the spine");
        assert_eq!(spine.first(), Some(&RoomId(START_ROOM)));
        assert_eq!(spine.last(), Some(&RoomId(EXIT_ROOM)));
        // The no-diagonal-lines assumption the UI renderer relies on: consecutive spine
        // rooms differ by exactly one grid step (Manhattan distance 1).
        for pair in spine.windows(2) {
            let d = grid_pos(pair[0]) - grid_pos(pair[1]);
            assert!(
                (d.x.abs() + d.y.abs() - 1.0).abs() < 1e-6,
                "{:?}→{:?} must be grid-adjacent",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn at_match_start_the_map_shows_you_and_the_rivals_at_the_entrance_exit_locked() {
        let facility = CompetitiveFacility::authored();
        let keys = KeystoneState::new(crate::flow::MATCH_SEED);
        let entrance = facility.team_room(0);
        let model = build_map(&facility, &keys, Place::Room(entrance));

        assert_eq!(model.player, PlayerMark::Room(entrance));
        assert_eq!(model.rivals.len(), 3, "the three rivals share the entrance");
        assert!(
            model.rivals.iter().all(|(i, r)| *i != 0 && *r == entrance),
            "rivals are non-local and co-located at the start"
        );
        assert!(model.collapse.is_empty(), "the collapse hasn't started");
        assert!(
            !model.exit_open,
            "the exit starts locked (keystones uncollected)"
        );
        assert_eq!(model.exit, RoomId(EXIT_ROOM));
        assert!(!model.keystones.is_empty(), "keystones are placed to find");
        assert_eq!(model.rooms.len(), 9);
    }

    #[test]
    fn a_hallway_place_reads_as_being_between_two_rooms() {
        let facility = CompetitiveFacility::authored();
        let keys = KeystoneState::new(crate::flow::MATCH_SEED);
        let model = build_map(
            &facility,
            &keys,
            Place::Hallway {
                from: RoomId(0),
                to: RoomId(1),
                variation: 0,
            },
        );
        assert_eq!(model.player, PlayerMark::Between(RoomId(0), RoomId(1)));
    }

    #[test]
    fn collected_keystones_drop_off_the_map_and_opening_the_gate_flips_exit_open() {
        let facility = CompetitiveFacility::authored();
        let mut keys = KeystoneState::new(crate::flow::MATCH_SEED);
        let before = build_map(&facility, &keys, Place::Room(RoomId(0)))
            .keystones
            .len();
        let rooms = keys.rooms.clone();
        keys.collect(rooms[0]);
        let after = build_map(&facility, &keys, Place::Room(RoomId(0)));
        assert_eq!(
            after.keystones.len(),
            before - 1,
            "a collected keystone leaves the map"
        );
        // Collect the rest → the gate opens.
        for room in rooms {
            keys.collect(room);
        }
        let open = build_map(&facility, &keys, Place::Room(RoomId(0)));
        assert!(
            open.exit_open,
            "all keystones held opens the exit on the map"
        );
        assert!(open.keystones.is_empty());
    }
}
