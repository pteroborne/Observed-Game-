//! Pure model for the toggleable **tac-map** overlay — the teleport-model heir of
//! `fps_match_lab`'s Tab tac-map and `match_replay`'s schematic.
//!
//! The map is a top-down schematic of the facility graph, but it is a *survivor's
//! sketch*, not a blueprint (Phase 50 ruling): the player explores an ever-changing
//! maze, so the map draws only what [`MapKnowledge`] records they have personally
//! witnessed. Rooms never entered are simply absent; rooms only seen through an open
//! threshold are hollow "something is there" outlines; connections appear only once a
//! doorway was seen or a hallway walked — and silently drop off when a reroute removes
//! them, because the projection filters the player's notes against the live spec.
//!
//! This is pure projection of the deterministic brain (`team_room` / `collapse_rooms`)
//! plus the local keystone inventory and the two team-local fog-of-war ledgers
//! ([`RivalSightings`], [`MapKnowledge`]); it never writes match state, so
//! determinism/replay/lockstep are untouched. `screens::draw_tac_map` builds the UI
//! from this model.

use bevy::prelude::Vec2;
use observed_core::RoomId;
use observed_facility::map_spec::RoomRole;
use observed_match::facility::{CompetitiveFacility, TEAM_COUNT};
use observed_match::mutable::{START_ROOM, spine_next};

use crate::flow::LOCAL_TEAM;
use crate::keystones::KeystoneState;
use crate::sim::state::{MapKnowledge, RivalSightings, SightingKind};
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

/// Everything the overlay needs to draw, snapshotted from the live match and filtered
/// through the player's [`MapKnowledge`] — the model itself never carries a fact the
/// player hasn't witnessed.
#[derive(Clone, Debug, PartialEq)]
pub struct MapModel {
    /// Rooms the player has physically stood in, with their true roles.
    pub rooms: Vec<(RoomId, Vec2, RoomRole)>,
    /// Rooms only ever seen through an open threshold: their position on the sketch is
    /// known, their role and contents are not.
    pub glimpsed: Vec<(RoomId, Vec2)>,
    /// Connections the player witnessed that still exist in the live facility.
    pub routes: Vec<(RoomId, RoomId)>,
    /// Schematic bounds of the FULL facility, not just the known part, so discovered
    /// rooms keep stable positions as the sketch fills in.
    pub bounds: (Vec2, Vec2),
    /// Full facility room count — stable room-square sizing while exploring.
    pub total_rooms: usize,
    pub player: PlayerMark,
    /// One pip per rival team the local player has ever *witnessed* a trace of — fog of
    /// war over the live truth (Design ruling): a rival still running but never seen or
    /// heard by the local player produces no pip at all. Position is the last-witnessed
    /// room, not the rival's live room.
    pub rivals: Vec<RivalPip>,
    /// Known rooms the collapse has already swallowed.
    pub collapse: Vec<RoomId>,
    /// Known rooms still holding an uncollected keystone.
    pub keystones: Vec<RoomId>,
    pub exit: RoomId,
    /// Whether the player has actually found the exit room; until then the map draws no
    /// exit marker at all.
    pub exit_known: bool,
    pub exit_open: bool,
}

pub fn route_segment_count(model: &MapModel) -> usize {
    model
        .routes
        .iter()
        .map(|&(a, b)| {
            let Some(a_pos) = room_position(model, a) else {
                return 1;
            };
            let Some(b_pos) = room_position(model, b) else {
                return 1;
            };
            if (a_pos.x - b_pos.x).abs() < 0.01 || (a_pos.y - b_pos.y).abs() < 0.01 {
                1
            } else {
                2
            }
        })
        .sum()
}

fn room_position(model: &MapModel, room: RoomId) -> Option<Vec2> {
    model
        .rooms
        .iter()
        .find_map(|(candidate, pos, _)| (*candidate == room).then_some(*pos))
        .or_else(|| {
            model
                .glimpsed
                .iter()
                .find_map(|(candidate, pos)| (*candidate == room).then_some(*pos))
        })
}

/// A room's identity, schematic position, and role — one full-facility layout row.
type SchematicRoom = (RoomId, Vec2, RoomRole);

/// The schematic min/max over the full facility's room positions.
fn schematic_bounds(rooms: &[SchematicRoom]) -> (Vec2, Vec2) {
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for (_, pos, _) in rooms {
        min = min.min(*pos);
        max = max.max(*pos);
    }
    if rooms.is_empty() {
        (Vec2::ZERO, Vec2::ONE)
    } else {
        (min, max)
    }
}

/// Build the map model from the live facility, the keystone inventory, the two
/// team-local fog-of-war ledgers, the current `reroute_commits` (for staleness), and
/// the player's current teleport place. Rival pips come entirely from `sightings`, and
/// *all structure* (rooms/routes/collapse/exit/keystones) is filtered through
/// `knowledge`: a room the player never witnessed simply is not on the map, and a
/// witnessed edge a reroute later removed drops back off. Only the player's own
/// position stays live.
pub fn build_map(
    facility: &CompetitiveFacility,
    keys: &KeystoneState,
    sightings: &RivalSightings,
    knowledge: &MapKnowledge,
    commits: u32,
    place: Place,
) -> MapModel {
    let player = match place {
        Place::Room(r) => PlayerMark::Room(r),
        Place::Hallway { from, to, .. } => PlayerMark::Between(from, to),
    };
    let rivals: Vec<RivalPip> = (0..TEAM_COUNT)
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
        .filter(|pip| knowledge.knows_room(pip.room))
        .collect();
    let keystones = keys
        .rooms
        .iter()
        .copied()
        .filter(|&r| keys.has_uncollected(r) && knowledge.knows_room(r))
        .collect();
    let (all_rooms, all_routes): (Vec<SchematicRoom>, Vec<(RoomId, RoomId)>) =
        if let Some(spec) = &facility.map_spec {
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
                        } else if room == keys.exit_room {
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
    let bounds = schematic_bounds(&all_rooms);
    let total_rooms = all_rooms.len();
    let rooms = all_rooms
        .iter()
        .filter(|(room, _, _)| knowledge.visited.contains(room))
        .cloned()
        .collect();
    let glimpsed = all_rooms
        .iter()
        .filter(|(room, _, _)| knowledge.glimpsed.contains(room))
        .map(|(room, pos, _)| (*room, *pos))
        .collect();
    let routes = all_routes
        .into_iter()
        .filter(|&(a, b)| knowledge.knows_edge(a, b))
        .collect();
    let collapse = facility
        .collapse_rooms()
        .into_iter()
        .filter(|&room| knowledge.knows_room(room))
        .collect();
    MapModel {
        rooms,
        glimpsed,
        routes,
        bounds,
        total_rooms,
        player,
        rivals,
        collapse,
        keystones,
        exit: keys.exit_room,
        exit_known: knowledge.knows_room(keys.exit_room),
        exit_open: keys.gate_open(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spine_runs_entrance_to_exit_and_every_step_is_grid_adjacent() {
        use observed_match::mutable::EXIT_ROOM;

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

    /// A knowledge ledger that has witnessed the whole facility — the pre-Phase-50
    /// "blueprint" view, used where a test exercises live-truth projection rather than
    /// the fog of war itself. Also witnesses the keystone/exit rooms, which
    /// `KeystoneState::new` seeds from the *generated catalog map* while these fixture
    /// tests run the authored nine-room facility.
    fn omniscient(facility: &CompetitiveFacility, keys: &KeystoneState) -> MapKnowledge {
        let mut knowledge = MapKnowledge::default();
        if let Some(spec) = &facility.map_spec {
            for room in &spec.rooms {
                knowledge.visit(room.id);
            }
            for edge in &spec.edges {
                knowledge.connect(edge.a.room, edge.b.room);
            }
        } else {
            for id in 0..9 {
                knowledge.visit(RoomId(id));
            }
            for pair in spine().windows(2) {
                knowledge.connect(pair[0], pair[1]);
            }
        }
        for &room in &keys.rooms {
            knowledge.visit(room);
        }
        knowledge.visit(keys.exit_room);
        knowledge
    }

    #[test]
    fn at_match_start_the_map_shows_you_and_the_witnessed_rivals_at_the_entrance_exit_locked() {
        let facility = CompetitiveFacility::authored();
        let keys = KeystoneState::new(crate::flow::MATCH_SEED);
        let entrance = facility.team_room(0);
        let sightings = all_rivals_seen_at(&facility, entrance);
        let knowledge = omniscient(&facility, &keys);
        let model = build_map(
            &facility,
            &keys,
            &sightings,
            &knowledge,
            0,
            Place::Room(entrance),
        );

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
        let knowledge = omniscient(&facility, &keys);
        let model = build_map(
            &facility,
            &keys,
            &sightings,
            &knowledge,
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
        let knowledge = omniscient(&facility, &keys);
        let mut sightings = RivalSightings::default();
        let last_sighted_room = RoomId(4);
        sightings.record(
            facility.teams[1].id,
            last_sighted_room,
            SightingKind::Seen,
            2,
        );

        // Freshly witnessed: staleness 0, full alpha.
        let fresh = build_map(
            &facility,
            &keys,
            &sightings,
            &knowledge,
            2,
            Place::Room(RoomId(0)),
        );
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
        let stale = build_map(
            &facility,
            &keys,
            &sightings,
            &knowledge,
            8,
            Place::Room(RoomId(0)),
        );
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
        let knowledge = MapKnowledge::default();
        let model = build_map(
            &facility,
            &keys,
            &sightings,
            &knowledge,
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
    fn diagonal_routes_count_as_two_ui_segments() {
        let model = MapModel {
            rooms: vec![
                (RoomId(0), Vec2::new(0.0, 0.0), RoomRole::Start),
                (RoomId(1), Vec2::new(1.0, 1.0), RoomRole::Exit),
            ],
            glimpsed: Vec::new(),
            routes: vec![(RoomId(0), RoomId(1))],
            bounds: (Vec2::ZERO, Vec2::ONE),
            total_rooms: 2,
            player: PlayerMark::Room(RoomId(0)),
            rivals: Vec::new(),
            collapse: Vec::new(),
            keystones: Vec::new(),
            exit: RoomId(1),
            exit_known: true,
            exit_open: false,
        };

        assert_eq!(route_segment_count(&model), 2);
    }

    #[test]
    fn collected_keystones_drop_off_the_map_and_opening_the_gate_flips_exit_open() {
        let facility = CompetitiveFacility::authored();
        let mut keys = KeystoneState::new(crate::flow::MATCH_SEED);
        let sightings = RivalSightings::default();
        let knowledge = omniscient(&facility, &keys);
        let before = build_map(
            &facility,
            &keys,
            &sightings,
            &knowledge,
            0,
            Place::Room(RoomId(0)),
        )
        .keystones
        .len();
        let rooms = keys.rooms.clone();
        keys.collect(rooms[0]);
        let after = build_map(
            &facility,
            &keys,
            &sightings,
            &knowledge,
            0,
            Place::Room(RoomId(0)),
        );
        assert_eq!(
            after.keystones.len(),
            before - 1,
            "a collected keystone leaves the map"
        );
        // Collect the rest → the gate opens.
        for room in rooms {
            keys.collect(room);
        }
        let open = build_map(
            &facility,
            &keys,
            &sightings,
            &knowledge,
            0,
            Place::Room(RoomId(0)),
        );
        assert!(
            open.exit_open,
            "all keystones held opens the exit on the map"
        );
        assert!(open.keystones.is_empty());
    }

    #[test]
    fn an_unwitnessed_facility_yields_an_empty_sketch_with_stable_bounds() {
        let facility = CompetitiveFacility::authored();
        let keys = KeystoneState::new(crate::flow::MATCH_SEED);
        let sightings = RivalSightings::default();
        let knowledge = MapKnowledge::default();
        let model = build_map(
            &facility,
            &keys,
            &sightings,
            &knowledge,
            0,
            Place::Room(RoomId(0)),
        );
        assert!(model.rooms.is_empty(), "no room witnessed, no room drawn");
        assert!(model.glimpsed.is_empty());
        assert!(model.routes.is_empty());
        assert!(model.keystones.is_empty(), "keystones hide in the unknown");
        assert!(!model.exit_known, "the exit must be found, not given");
        assert_eq!(
            model.total_rooms, 9,
            "sizing stays stable at the full facility count"
        );
        assert_eq!(
            model.bounds,
            (Vec2::ZERO, Vec2::new(2.0, 2.0)),
            "bounds span the full 3x3 schematic so discovered rooms never shift"
        );
    }

    #[test]
    fn a_glimpsed_room_is_a_position_only_outline_until_visited() {
        let facility = CompetitiveFacility::authored();
        let keys = KeystoneState::new(crate::flow::MATCH_SEED);
        let sightings = RivalSightings::default();
        let mut knowledge = MapKnowledge::default();
        knowledge.visit(RoomId(0));
        knowledge.glimpse(RoomId(1));
        knowledge.connect(RoomId(0), RoomId(1));

        let model = build_map(
            &facility,
            &keys,
            &sightings,
            &knowledge,
            0,
            Place::Room(RoomId(0)),
        );
        assert_eq!(model.rooms.len(), 1, "only the visited entrance is a room");
        assert_eq!(model.rooms[0].0, RoomId(0));
        assert_eq!(
            model.glimpsed,
            vec![(RoomId(1), grid_pos(RoomId(1)))],
            "the doorway neighbour is a bare position, no role"
        );
        assert_eq!(
            model.routes,
            vec![(RoomId(0), RoomId(1))],
            "the witnessed doorway is the only known connection"
        );

        knowledge.visit(RoomId(1));
        let model = build_map(
            &facility,
            &keys,
            &sightings,
            &knowledge,
            0,
            Place::Room(RoomId(1)),
        );
        assert_eq!(model.rooms.len(), 2, "entering promotes the glimpse");
        assert!(model.glimpsed.is_empty());
    }

    #[test]
    fn a_witnessed_connection_the_facility_no_longer_has_drops_off_the_map() {
        let facility = CompetitiveFacility::authored();
        let keys = KeystoneState::new(crate::flow::MATCH_SEED);
        let sightings = RivalSightings::default();
        let mut knowledge = MapKnowledge::default();
        knowledge.visit(RoomId(0));
        knowledge.visit(RoomId(4));
        // The player once knew a 0–4 connection, but the authored spine has no such
        // edge any more: the sketch silently loses it (the maze owes no corrections).
        knowledge.connect(RoomId(0), RoomId(4));

        let model = build_map(
            &facility,
            &keys,
            &sightings,
            &knowledge,
            0,
            Place::Room(RoomId(0)),
        );
        assert!(
            model.routes.is_empty(),
            "a remembered edge that no longer exists is not drawn"
        );
    }
}
