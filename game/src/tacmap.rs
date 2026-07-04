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
use crate::sim::state::{RivalSightings, SightingKind};
use crate::teleport::Place;

/// A rival's staleness floor (Design ruling: full alpha at 0 commits old, fading to a
/// ~0.25 floor once the witnessing is 6+ reroute commits old — a sighting never fully
/// disappears, it just fades toward "we don't really know anymore").
const STALE_COMMITS_FOR_FLOOR: u32 = 6;
const STALE_ALPHA_FLOOR: f32 = 0.25;

/// A witnessed rival pip on the tac-map: the last-sighted room, how stale that sighting
/// is (in reroute commits), and what kind of evidence it was. Position is frozen at the
/// witnessed room — never the rival's live room — so the map is fog-of-war, not truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RivalPip {
    pub team: usize,
    pub room: RoomId,
    pub kind: SightingKind,
    /// `game.reroute_commits - commits_seen` at snapshot time.
    pub staleness: u32,
}

impl RivalPip {
    /// The alpha the HUD should render this pip at: 1.0 fresh, fading linearly to
    /// [`STALE_ALPHA_FLOOR`] by [`STALE_COMMITS_FOR_FLOOR`] commits old, then flat.
    pub fn alpha(&self) -> f32 {
        if self.staleness == 0 {
            return 1.0;
        }
        let t = (self.staleness as f32 / STALE_COMMITS_FOR_FLOOR as f32).min(1.0);
        1.0 - t * (1.0 - STALE_ALPHA_FLOOR)
    }
}

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
    /// One pip per rival team the local player has ever *witnessed* a trace of — fog of
    /// war over the live truth (Design ruling): a rival still running but never seen or
    /// heard by the local player produces no pip at all. Position is the last-witnessed
    /// room, not the rival's live room.
    pub rivals: Vec<RivalPip>,
    /// Spine rooms the collapse has already swallowed.
    pub collapse: Vec<RoomId>,
    /// Rooms still holding an uncollected keystone.
    pub keystones: Vec<RoomId>,
    pub exit: RoomId,
    pub exit_open: bool,
}

/// Build the map model from the live facility, the keystone inventory, the team-local
/// rival sighting ledger, the current `reroute_commits` (for staleness), and the
/// player's current teleport place. Rival pips come entirely from `sightings` (fog of
/// war): a still-running rival the local player has never witnessed simply produces no
/// pip. The local team's own position stays live (`player`, below), as does all
/// structure (rooms/routes/collapse/exit/keystones).
pub fn build_map(
    facility: &CompetitiveFacility,
    keys: &KeystoneState,
    sightings: &RivalSightings,
    commits: u32,
    place: Place,
) -> MapModel {
    let player = match place {
        Place::Room(r) => PlayerMark::Room(r),
        Place::Hallway { from, to, .. } => PlayerMark::Between(from, to),
    };
    let rivals = (0..TEAM_COUNT)
        .filter(|&i| i as u8 != LOCAL_TEAM.0)
        .filter(|&i| facility.teams.get(i).is_some_and(|t| t.active_runner()))
        .filter_map(|i| {
            let team_id = facility.teams[i].id;
            sightings
                .teams
                .get(&team_id.0)
                .and_then(|rooms| rooms.iter().max_by_key(|(_, s)| s.commits_seen))
                .map(|(&room, sighting)| RivalPip {
                    team: i,
                    room,
                    kind: sighting.kind,
                    staleness: commits.saturating_sub(sighting.commits_seen),
                })
        })
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
        exit: keys.exit_room,
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

    /// A ledger with every non-local, still-active rival recorded `Seen` at `room` at
    /// `commits_seen: 0` — mirrors what `sim::sightings::record_rival_sightings` would
    /// have recorded at match start when every team shares the entrance.
    fn all_rivals_seen_at(facility: &CompetitiveFacility, room: RoomId) -> RivalSightings {
        let mut sightings = RivalSightings::default();
        for i in 0..TEAM_COUNT {
            if i as u8 == LOCAL_TEAM.0 {
                continue;
            }
            sightings.record(facility.teams[i].id, room, SightingKind::Seen, 0);
        }
        sightings
    }

    #[test]
    fn at_match_start_the_map_shows_you_and_the_witnessed_rivals_at_the_entrance_exit_locked() {
        let facility = CompetitiveFacility::authored();
        let keys = KeystoneState::new(crate::flow::MATCH_SEED);
        let entrance = facility.team_room(0);
        let sightings = all_rivals_seen_at(&facility, entrance);
        let model = build_map(&facility, &keys, &sightings, 0, Place::Room(entrance));

        assert_eq!(model.player, PlayerMark::Room(entrance));
        assert_eq!(
            model.rivals.len(),
            3,
            "the three witnessed rivals share the entrance"
        );
        assert!(
            model
                .rivals
                .iter()
                .all(|pip| pip.team != 0 && pip.room == entrance),
            "rivals are non-local and co-located at the start"
        );
        assert!(model.collapse.is_empty(), "the collapse hasn't started");
        assert!(
            !model.exit_open,
            "the exit starts locked (keystones uncollected)"
        );
        assert_eq!(
            model.exit, keys.exit_room,
            "the map's exit room drives the model, not the legacy spine constant"
        );
        assert!(!model.keystones.is_empty(), "keystones are placed to find");
        assert_eq!(model.rooms.len(), 9);
    }

    #[test]
    fn a_rival_never_witnessed_produces_no_pip_even_while_still_running() {
        let facility = CompetitiveFacility::authored();
        let keys = KeystoneState::new(crate::flow::MATCH_SEED);
        let sightings = RivalSightings::default();
        let model = build_map(
            &facility,
            &keys,
            &sightings,
            0,
            Place::Room(facility.team_room(0)),
        );
        assert!(
            model.rivals.is_empty(),
            "an empty sighting ledger must produce zero rival pips regardless of live truth"
        );
    }

    #[test]
    fn a_witnessed_rival_shows_at_its_last_sighted_room_and_fades_with_staleness() {
        let facility = CompetitiveFacility::authored();
        let keys = KeystoneState::new(crate::flow::MATCH_SEED);
        let mut sightings = RivalSightings::default();
        let last_sighted_room = RoomId(4);
        sightings.record(
            facility.teams[1].id,
            last_sighted_room,
            SightingKind::Seen,
            2,
        );

        // Freshly witnessed: staleness 0, full alpha.
        let fresh = build_map(&facility, &keys, &sightings, 2, Place::Room(RoomId(0)));
        let pip = fresh
            .rivals
            .iter()
            .find(|p| p.team == 1)
            .expect("the witnessed rival appears at its last-sighted room");
        assert_eq!(pip.room, last_sighted_room);
        assert_eq!(pip.staleness, 0);
        assert_eq!(pip.alpha(), 1.0);

        // Many reroute commits later without a fresh witnessing: the pip stays at the
        // same last-sighted room (never the rival's live room) but reads stale.
        let stale = build_map(&facility, &keys, &sightings, 8, Place::Room(RoomId(0)));
        let stale_pip = stale.rivals.iter().find(|p| p.team == 1).unwrap();
        assert_eq!(
            stale_pip.room, last_sighted_room,
            "the pip position is frozen at the witnessed room, not live truth"
        );
        assert_eq!(stale_pip.staleness, 6);
        assert!(
            (stale_pip.alpha() - 0.25).abs() < 1e-6,
            "staleness >= 6 commits floors at 0.25 alpha"
        );
        assert!(stale_pip.alpha() < pip.alpha(), "a stale pip fades");
    }

    #[test]
    fn a_hallway_place_reads_as_being_between_two_rooms() {
        let facility = CompetitiveFacility::authored();
        let keys = KeystoneState::new(crate::flow::MATCH_SEED);
        let sightings = RivalSightings::default();
        let model = build_map(
            &facility,
            &keys,
            &sightings,
            0,
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
        let sightings = RivalSightings::default();
        let before = build_map(&facility, &keys, &sightings, 0, Place::Room(RoomId(0)))
            .keystones
            .len();
        let rooms = keys.rooms.clone();
        keys.collect(rooms[0]);
        let after = build_map(&facility, &keys, &sightings, 0, Place::Room(RoomId(0)));
        assert_eq!(
            after.keystones.len(),
            before - 1,
            "a collected keystone leaves the map"
        );
        // Collect the rest → the gate opens.
        for room in rooms {
            keys.collect(room);
        }
        let open = build_map(&facility, &keys, &sightings, 0, Place::Room(RoomId(0)));
        assert!(
            open.exit_open,
            "all keystones held opens the exit on the map"
        );
        assert!(open.keystones.is_empty());
    }
}
