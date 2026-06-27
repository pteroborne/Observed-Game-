//! Phase 24: the full competitive match, playable and replayable in first person.
//!
//! `CompetitiveFacility` remains the authoritative match brain: observation,
//! protected spine, teams, contested control, capacity-limited exits, collapse,
//! absorption, director escalation, and deterministic placement. `FacilityStage`
//! remains the authoritative first-person projection and controller.
//!
//! The integration boundary is one local round action:
//!
//! - `Advance` is emitted only after the local player physically crosses the next
//!   protected-spine Passage in the 3D facility.
//! - `Seize` is emitted at the authored control-room console.
//! - bots and director policy are deterministic;
//! - after the round resolves, the 3D facility synchronizes from the exact
//!   competitive graph;
//! - the tape stores local round actions and reconstructs both match and
//!   first-person state from a fresh session.

use bevy::prelude::*;
use competition_lab::model::RaceAction;
use competitive_facility::model::{CompetitiveFacility, START_ROOM, TEAM_COUNT};
use director_lab::model::Role;
use fps_facility_lab::model::{FacilityStage, module_center};
use mutable_facility::model::spine_next;
use observation_lab::model::Side;
use observed_core::{RoomId, TeamId};
use player_input::PlayerIntent;

pub const LOCAL_TEAM: TeamId = TeamId(0);
pub const CONTROL_ROOM: RoomId = RoomId(3);
const MAX_ROUNDS: usize = 64;
const CONTROL_RADIUS: f32 = 2.4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalAction {
    Advance,
    Seize,
    Wait,
}

impl LocalAction {
    fn race_action(self) -> RaceAction {
        match self {
            Self::Advance => RaceAction::Advance,
            Self::Seize => RaceAction::Seize,
            Self::Wait => RaceAction::Advance,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatchFrame {
    pub local: LocalAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchMarker {
    pub round: usize,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchSnapshot {
    pub round: u32,
    pub player_room: RoomId,
    pub body_position: Vec3,
    pub body_yaw: f32,
    pub team_rooms: Vec<RoomId>,
    pub roles: Vec<u8>,
    pub placements: Vec<Option<u8>>,
    pub graph_links: Vec<observation_lab::model::DoorId>,
    pub purge_line: f32,
    pub control_holder: Option<TeamId>,
    pub winner: Option<TeamId>,
    pub finished: bool,
}

#[derive(Clone, Debug)]
pub struct FirstPersonMatch {
    pub competitive: CompetitiveFacility,
    pub facility: FacilityStage,
    pub local_team: TeamId,
    pub last_event: String,
}

impl Default for FirstPersonMatch {
    fn default() -> Self {
        let competitive = CompetitiveFacility::authored();
        let mut facility = FacilityStage::default();
        facility.sync_graph(competitive.structure.graph.clone());
        facility.relocate_player(RoomId(START_ROOM), Side::West);
        Self {
            competitive,
            facility,
            local_team: LOCAL_TEAM,
            last_event: "Follow the highlighted spine Passage; reach the exit before the collapse."
                .to_string(),
        }
    }
}

impl FirstPersonMatch {
    pub fn local_index(&self) -> usize {
        self.competitive
            .teams
            .iter()
            .position(|team| team.id == self.local_team)
            .expect("local team exists")
    }

    pub fn local_room(&self) -> RoomId {
        self.competitive.team_room(self.local_index())
    }

    pub fn local_progress(&self) -> f32 {
        self.competitive.progress_of(self.local_team)
    }

    pub fn target(&self) -> Option<(RoomId, Side)> {
        spine_next(self.local_room())
    }

    pub fn can_seize(&self) -> bool {
        if self.competitive.finished || self.facility.player_room != CONTROL_ROOM {
            return false;
        }
        let centre = module_center(CONTROL_ROOM);
        Vec2::new(
            self.facility.body.position.x - centre.x,
            self.facility.body.position.z - centre.z,
        )
        .length()
            <= CONTROL_RADIUS
    }

    /// Step the deterministic first-person controller. Crossing the highlighted
    /// spine threshold emits an Advance frame; interact at the console emits Seize.
    pub fn step_player(&mut self, intent: PlayerIntent) -> Option<LocalAction> {
        if self.competitive.finished {
            return None;
        }
        if intent.interact_pressed && self.can_seize() {
            self.apply_action(LocalAction::Seize);
            return Some(LocalAction::Seize);
        }

        let expected = self.target();
        let traversal = self.facility.step(intent);
        if let (Some((expected_room, expected_side)), Some(record)) = (expected, traversal) {
            if record.from_room == self.local_room()
                && record.from_side == expected_side
                && record.to_room == expected_room
            {
                self.resolve_round(LocalAction::Advance, Some(record.to_side));
                return Some(LocalAction::Advance);
            }
            self.last_event = format!(
                "Scouted room {} via {}; the race route still points to room {}.",
                record.to_room.0,
                record.from_side.label(),
                expected_room.0
            );
        }
        None
    }

    /// Apply a round action directly. Used by the deterministic tape recorder and
    /// replay; Advance still traverses the facility's current graph port first.
    pub fn apply_action(&mut self, action: LocalAction) -> bool {
        if self.competitive.finished {
            return false;
        }
        match action {
            LocalAction::Advance => {
                let Some((expected_room, side)) = self.target() else {
                    return false;
                };
                if self.facility.player_room != self.local_room() {
                    self.facility.relocate_player(self.local_room(), Side::West);
                }
                let Some(record) = self.facility.traverse(side) else {
                    return false;
                };
                assert_eq!(
                    record.to_room, expected_room,
                    "the protected spine must remain the graph navigation route"
                );
                self.resolve_round(action, Some(record.to_side));
            }
            LocalAction::Seize => self.resolve_round(action, None),
            LocalAction::Wait => self.resolve_round(action, None),
        }
        true
    }

    fn resolve_round(&mut self, local: LocalAction, arrival_side: Option<Side>) {
        let before_placed: Vec<Option<u8>> = self
            .competitive
            .teams
            .iter()
            .map(|team| team.placement)
            .collect();
        let before_roles: Vec<Role> = self
            .competitive
            .teams
            .iter()
            .map(|team| team.role)
            .collect();

        // Absorbed teams become deterministic facility agents and periodically
        // shove the collapse before the next race round.
        if self.competitive.round > 0 && self.competitive.round.is_multiple_of(3) {
            let director = self
                .competitive
                .teams
                .iter()
                .find(|team| team.role == Role::Director && team.placement.is_none())
                .map(|team| team.id);
            if let Some(team) = director {
                self.competitive.scramble(team);
            }
        }

        let round = self.competitive.round as usize;
        let intents = self
            .competitive
            .teams
            .iter()
            .filter(|team| team.active_runner())
            .map(|team| {
                let action = if team.id == self.local_team {
                    local.race_action()
                } else {
                    bot_policy(round, team.id)
                };
                (team.id, action)
            })
            .collect::<Vec<_>>();
        self.competitive.advance_round(&intents);

        let graph = self.competitive.structure.graph.clone();
        self.facility.sync_graph(graph);
        let authoritative_room = self.local_room();
        if self.facility.player_room != authoritative_room {
            self.facility
                .relocate_player(authoritative_room, arrival_side.unwrap_or(Side::West));
        }
        self.last_event = round_event(&self.competitive, &before_placed, &before_roles, local);
    }

    pub fn snapshot(&self) -> MatchSnapshot {
        MatchSnapshot {
            round: self.competitive.round,
            player_room: self.facility.player_room,
            body_position: self.facility.body.position,
            body_yaw: self.facility.body.yaw,
            team_rooms: (0..TEAM_COUNT)
                .map(|index| self.competitive.team_room(index))
                .collect(),
            roles: self
                .competitive
                .teams
                .iter()
                .map(|team| match team.role {
                    Role::Runner => 0,
                    Role::Director => 1,
                })
                .collect(),
            placements: self
                .competitive
                .teams
                .iter()
                .map(|team| team.placement)
                .collect(),
            graph_links: self.competitive.structure.graph.links.clone(),
            purge_line: self.competitive.purge_line,
            control_holder: self.competitive.control_holder,
            winner: self.competitive.winner,
            finished: self.competitive.finished,
        }
    }
}

#[derive(Resource, Clone, Debug, Default)]
pub struct MatchTape {
    pub frames: Vec<MatchFrame>,
    pub markers: Vec<MatchMarker>,
    pub snapshots: Vec<MatchSnapshot>,
}

impl MatchTape {
    pub fn record_demo() -> Self {
        let mut session = FirstPersonMatch::default();
        let mut tape = Self {
            snapshots: vec![session.snapshot()],
            ..Default::default()
        };
        let mut seized = false;
        while !session.competitive.finished && tape.frames.len() < MAX_ROUNDS {
            let local = session
                .competitive
                .team(session.local_team)
                .expect("local team exists");
            let action = if !local.active_runner() {
                LocalAction::Wait
            } else if !seized && session.local_room() == CONTROL_ROOM {
                seized = true;
                LocalAction::Seize
            } else {
                LocalAction::Advance
            };
            let before = session.snapshot();
            assert!(session.apply_action(action));
            tape.frames.push(MatchFrame { local: action });
            tape.capture_markers(&before, &session);
            tape.snapshots.push(session.snapshot());
        }
        assert!(session.competitive.finished, "demo match must terminate");
        tape
    }

    pub fn push_live(
        &mut self,
        action: LocalAction,
        before: &MatchSnapshot,
        after: &FirstPersonMatch,
    ) {
        if self.snapshots.is_empty() {
            self.snapshots.push(before.clone());
        }
        self.frames.push(MatchFrame { local: action });
        self.capture_markers(before, after);
        self.snapshots.push(after.snapshot());
    }

    fn capture_markers(&mut self, before: &MatchSnapshot, after: &FirstPersonMatch) {
        for (index, team) in after.competitive.teams.iter().enumerate() {
            if let Some(placement) = team.placement
                && before.placements[index].is_none()
            {
                self.markers.push(MatchMarker {
                    round: self.frames.len(),
                    label: format!("{} escaped {}", team.id.label(), ordinal(placement)),
                });
            }
            if team.role == Role::Director && before.roles[index] == 0 {
                self.markers.push(MatchMarker {
                    round: self.frames.len(),
                    label: format!("{} absorbed", team.id.label()),
                });
            }
        }
    }

    pub fn replay_to(&self, round: usize) -> FirstPersonMatch {
        let mut session = FirstPersonMatch::default();
        for frame in self.frames.iter().take(round.min(self.frames.len())) {
            session.apply_action(frame.local);
        }
        session
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn exact_at(&self, round: usize) -> bool {
        let index = round.min(self.frames.len());
        self.snapshots
            .get(index)
            .is_some_and(|expected| self.replay_to(index).snapshot() == *expected)
    }
}

fn bot_policy(round: usize, team: TeamId) -> RaceAction {
    if team == TeamId(3) && round < 4 {
        RaceAction::Seize
    } else {
        RaceAction::Advance
    }
}

fn round_event(
    match_state: &CompetitiveFacility,
    before_placed: &[Option<u8>],
    before_roles: &[Role],
    local: LocalAction,
) -> String {
    let mut events = Vec::new();
    events.push(match local {
        LocalAction::Advance => "Local team crossed the protected Passage.".to_string(),
        LocalAction::Seize => "Local team seized the shared control.".to_string(),
        LocalAction::Wait => "Local team resolved; bots and director advanced.".to_string(),
    });
    for (index, team) in match_state.teams.iter().enumerate() {
        if team.placement.is_some() && before_placed[index].is_none() {
            events.push(format!("{} escaped.", team.id.label()));
        }
        if team.role == Role::Director && before_roles[index] == Role::Runner {
            events.push(format!("{} joined the facility director.", team.id.label()));
        }
    }
    if match_state.finished {
        events.push(format!(
            "Match over: {} escaped, {} absorbed.",
            match_state.escaped_count(),
            match_state.absorbed_count()
        ));
    }
    events.join(" ")
}

fn ordinal(place: u8) -> &'static str {
    match place {
        1 => "1st",
        2 => "2nd",
        3 => "3rd",
        _ => "nth",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use observation_lab::model::Side;

    #[test]
    fn demo_match_is_first_person_integrated_and_resolves() {
        let tape = MatchTape::record_demo();
        assert!(!tape.is_empty());
        let end = tape.replay_to(tape.len());
        assert!(end.competitive.finished);
        assert_eq!(end.competitive.escaped_count(), 2);
        assert_eq!(end.competitive.winner, Some(LOCAL_TEAM));
        assert_eq!(
            end.competitive.escaped_count() + end.competitive.absorbed_count(),
            TEAM_COUNT
        );
        assert!(end.facility.projection_exact());
    }

    #[test]
    fn replay_reproduces_the_full_match_and_first_person_state_exactly() {
        let tape = MatchTape::record_demo();
        for round in [
            0,
            1,
            tape.len() / 2,
            tape.len().saturating_sub(1),
            tape.len(),
        ] {
            assert!(tape.exact_at(round));
            assert_eq!(tape.replay_to(round).snapshot(), tape.snapshots[round]);
        }
    }

    #[test]
    fn sequential_playback_matches_seek_at_every_round() {
        let tape = MatchTape::record_demo();
        let mut session = FirstPersonMatch::default();
        for round in 0..=tape.len() {
            assert_eq!(session.snapshot(), tape.replay_to(round).snapshot());
            if let Some(frame) = tape.frames.get(round) {
                session.apply_action(frame.local);
            }
        }
    }

    #[test]
    fn advance_uses_the_same_protected_graph_passage_as_the_3d_facility() {
        let mut session = FirstPersonMatch::default();
        let before = session.local_room();
        let (expected, side) = session.target().unwrap();
        let source = session.competitive.structure.graph.door_id(before, side);
        let partner = session.competitive.structure.graph.partner(source);
        assert_eq!(
            session.competitive.structure.graph.door(partner).room,
            expected
        );
        assert!(session.apply_action(LocalAction::Advance));
        assert_eq!(session.local_room(), expected);
        assert_eq!(session.facility.player_room, expected);
    }

    #[test]
    fn a_real_controller_threshold_crossing_emits_a_live_round() {
        let mut session = FirstPersonMatch::default();
        let room = session.local_room();
        let (_, side) = session.target().unwrap();
        session.facility.relocate_player(room, Side::West);
        let centre = module_center(room);
        let direction = fps_facility_lab::model::side_direction(side);
        session.facility.body.position =
            centre + direction * (fps_facility_lab::model::MODULE_HALF - 1.0);
        session.facility.body.yaw = fps_facility_lab::model::yaw_from_direction(direction);
        session.facility.body.grounded = true;
        let before_round = session.competitive.round;
        let mut emitted = None;
        for _ in 0..90 {
            emitted = session.step_player(PlayerIntent {
                movement: Vec2::Y,
                ..default()
            });
            if emitted.is_some() {
                break;
            }
        }
        assert_eq!(emitted, Some(LocalAction::Advance));
        assert_eq!(session.competitive.round, before_round + 1);
    }

    #[test]
    fn seize_is_spatially_gated_to_the_control_room_console() {
        let mut session = FirstPersonMatch::default();
        assert!(!session.can_seize());
        session.facility.relocate_player(CONTROL_ROOM, Side::West);
        session.facility.body.position = module_center(CONTROL_ROOM);
        assert!(session.can_seize());
        let before = session.competitive.round;
        assert_eq!(
            session.step_player(PlayerIntent {
                interact_pressed: true,
                ..default()
            }),
            Some(LocalAction::Seize)
        );
        assert_eq!(session.competitive.round, before + 1);
        assert_eq!(session.competitive.control_holder, Some(LOCAL_TEAM));
    }

    #[test]
    fn replay_tape_contains_escape_and_absorption_markers() {
        let tape = MatchTape::record_demo();
        assert!(
            tape.markers
                .iter()
                .any(|marker| marker.label.contains("escaped"))
        );
        assert!(
            tape.markers
                .iter()
                .any(|marker| marker.label.contains("absorbed"))
        );
        assert!(
            tape.markers
                .windows(2)
                .all(|pair| pair[0].round <= pair[1].round)
        );
    }

    #[test]
    fn deterministic_demo_recordings_are_identical() {
        let a = MatchTape::record_demo();
        let b = MatchTape::record_demo();
        assert_eq!(a.frames, b.frames);
        assert_eq!(a.markers, b.markers);
        assert_eq!(
            a.replay_to(a.len()).snapshot(),
            b.replay_to(b.len()).snapshot()
        );
    }
}
