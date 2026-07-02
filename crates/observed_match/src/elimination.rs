//! Deterministic elimination-series rules for the next playable slice.
//!
//! A series is made of short maze rounds. Every still-alive team starts a round at
//! the entrance, races the keystone route, and tries to reach the exit. The first
//! escape starts a countdown; when either the countdown expires or all survivor
//! slots are filled, teams that failed to escape are eliminated and join the
//! facility adversary for later rounds. The series ends when one team remains.

use observed_core::TeamId;

use crate::facility::{TEAM_COUNT, TeamRun};
use crate::mutable::{EXIT_ROOM, START_ROOM, spine_next};

pub const DEFAULT_KEYSTONES_REQUIRED: usize = 3;
pub const DEFAULT_COUNTDOWN_ROUNDS: u8 = 4;
pub const MAX_AUTOPLAY_TICKS: u32 = 512;

/// The protected route in visiting order.
pub fn spine_sequence() -> Vec<observed_core::RoomId> {
    let mut seq = vec![observed_core::RoomId(START_ROOM)];
    let mut room = observed_core::RoomId(START_ROOM);
    while let Some((next, _)) = spine_next(room) {
        seq.push(next);
        room = next;
    }
    seq
}

/// Deterministic per-series keystone placement, matching the assembled game's
/// route-gate policy: distinct intermediate spine rooms, always including the
/// last room before the exit.
pub fn keystone_rooms(seed: u64) -> Vec<observed_core::RoomId> {
    let seq = spine_sequence();
    if seq.len() < 3 {
        return Vec::new();
    }
    let inter: Vec<_> = seq[1..seq.len() - 1].to_vec();
    let required = DEFAULT_KEYSTONES_REQUIRED.min(inter.len());
    let mut chosen = vec![*inter.last().expect("intermediate spine rooms")];
    let mut i = (seed % inter.len() as u64) as usize;
    while chosen.len() < required {
        let room = inter[i % inter.len()];
        if !chosen.contains(&room) {
            chosen.push(room);
        }
        i += 1;
    }
    chosen.sort_by_key(|room| room.0);
    chosen
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesAiAction {
    Advance,
    SeizeControl,
    DropAnchor,
    DropTeleportPad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedAnchor {
    pub team: TeamId,
    pub room: observed_core::RoomId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedTeleportPad {
    pub team: TeamId,
    pub room: observed_core::RoomId,
    pub slot: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamToolState {
    pub team: TeamId,
    pub anchor_torches: u8,
    pub teleport_pads: u8,
    pub anchors: Vec<PlacedAnchor>,
    pub pads: Vec<PlacedTeleportPad>,
}

impl TeamToolState {
    pub fn new(team: TeamId) -> Self {
        Self {
            team,
            anchor_torches: 1,
            teleport_pads: 2,
            anchors: Vec::new(),
            pads: Vec::new(),
        }
    }

    pub fn drop_anchor(&mut self, room: observed_core::RoomId) -> bool {
        if self.anchor_torches == 0 {
            return false;
        }
        self.anchor_torches -= 1;
        self.anchors.push(PlacedAnchor {
            team: self.team,
            room,
        });
        true
    }

    pub fn drop_pad(&mut self, room: observed_core::RoomId) -> bool {
        if self.teleport_pads == 0 {
            return false;
        }
        let slot = self.pads.len() as u8;
        self.teleport_pads -= 1;
        self.pads.push(PlacedTeleportPad {
            team: self.team,
            room,
            slot,
        });
        true
    }

    /// Teleport pads are keyed to their owning team; this only links pads from
    /// the same tool state.
    pub fn pad_link_target(&self, room: observed_core::RoomId) -> Option<observed_core::RoomId> {
        if self.pads.len() < 2 || !self.pads.iter().any(|pad| pad.room == room) {
            return None;
        }
        self.pads
            .iter()
            .find(|pad| pad.room != room)
            .map(|pad| pad.room)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TeamObjectiveState {
    pub id: TeamId,
    pub room: observed_core::RoomId,
    pub spine_index: usize,
    pub pace: f32,
    pub collected_keystones: Vec<observed_core::RoomId>,
    pub escaped: bool,
    pub eliminated: bool,
    pub director: bool,
    pub tools: TeamToolState,
}

impl TeamObjectiveState {
    fn new(id: TeamId) -> Self {
        Self {
            id,
            room: observed_core::RoomId(START_ROOM),
            spine_index: 0,
            pace: 0.0,
            collected_keystones: Vec::new(),
            escaped: false,
            eliminated: false,
            director: false,
            tools: TeamToolState::new(id),
        }
    }

    pub fn gate_open(&self, required: usize) -> bool {
        self.collected_keystones.len() >= required
    }

    pub fn status_label(&self, required: usize) -> String {
        if self.escaped {
            "escaped".to_string()
        } else if self.eliminated {
            "joined adversary".to_string()
        } else {
            format!(
                "room {} | key {}/{}",
                self.room.0,
                self.collected_keystones.len(),
                required
            )
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EscapeCountdown {
    pub started_by: TeamId,
    pub duration_rounds: u8,
    pub remaining_rounds: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SeriesRound {
    pub index: u32,
    pub active_teams: Vec<TeamId>,
    pub teams: Vec<TeamObjectiveState>,
    pub keystone_rooms: Vec<observed_core::RoomId>,
    pub countdown: Option<EscapeCountdown>,
    pub escape_order: Vec<TeamId>,
    pub eliminated: Vec<TeamId>,
    pub survivor_slots: usize,
    pub tick: u32,
    pub finished: bool,
    pub last_event: String,
}

impl SeriesRound {
    pub fn new(
        index: u32,
        active_teams: Vec<TeamId>,
        keystone_rooms: Vec<observed_core::RoomId>,
    ) -> Self {
        let survivor_slots = active_teams.len().saturating_sub(1);
        let teams = active_teams
            .iter()
            .copied()
            .map(TeamObjectiveState::new)
            .collect();
        Self {
            index,
            active_teams,
            teams,
            keystone_rooms,
            countdown: None,
            escape_order: Vec::new(),
            eliminated: Vec::new(),
            survivor_slots,
            tick: 0,
            finished: false,
            last_event: "Teams entered a fresh keystone route.".to_string(),
        }
    }

    pub fn team(&self, id: TeamId) -> Option<&TeamObjectiveState> {
        self.teams.iter().find(|team| team.id == id)
    }

    pub fn team_mut(&mut self, id: TeamId) -> Option<&mut TeamObjectiveState> {
        self.teams.iter_mut().find(|team| team.id == id)
    }

    pub fn required_keystones(&self) -> usize {
        self.keystone_rooms.len()
    }

    pub fn active_runners(&self) -> usize {
        self.teams
            .iter()
            .filter(|team| !team.escaped && !team.eliminated)
            .count()
    }

    pub fn exit_open_for(&self, id: TeamId) -> bool {
        self.team(id)
            .is_some_and(|team| team.gate_open(self.required_keystones()))
    }

    pub fn remaining_countdown(&self) -> Option<u8> {
        self.countdown.map(|countdown| countdown.remaining_rounds)
    }

    pub fn team_action(&self, team: &TeamObjectiveState) -> SeriesAiAction {
        if team.room == observed_core::RoomId(START_ROOM) && team.tools.anchor_torches > 0 {
            SeriesAiAction::DropAnchor
        } else if self.keystone_rooms.contains(&team.room) && team.tools.teleport_pads > 0 {
            SeriesAiAction::DropTeleportPad
        } else {
            SeriesAiAction::Advance
        }
    }

    pub fn advance_autoplay_tick(&mut self, adversary_strength: usize) {
        if self.finished {
            return;
        }
        self.tick += 1;
        let mut just_started_countdown = false;
        let spine = spine_sequence();
        let required = self.required_keystones();
        let keystone_rooms = self.keystone_rooms.clone();
        let survivor_slots = self.survivor_slots;
        let mut escaped_this_tick = Vec::new();

        for team in &mut self.teams {
            if team.escaped || team.eliminated {
                continue;
            }

            match Self::team_action_for(&keystone_rooms, team) {
                SeriesAiAction::DropAnchor => {
                    team.tools.drop_anchor(team.room);
                }
                SeriesAiAction::DropTeleportPad => {
                    team.tools.drop_pad(team.room);
                }
                SeriesAiAction::SeizeControl | SeriesAiAction::Advance => {}
            }

            if keystone_rooms.contains(&team.room) && !team.collected_keystones.contains(&team.room)
            {
                team.collected_keystones.push(team.room);
                team.collected_keystones.sort_by_key(|room| room.0);
            }

            if team.room.0 == EXIT_ROOM && team.gate_open(required) {
                escaped_this_tick.push(team.id);
                continue;
            }

            let speed = team_speed(team.id, adversary_strength);
            team.pace += speed;
            while team.pace >= 1.0 && team.spine_index + 1 < spine.len() {
                team.pace -= 1.0;
                team.spine_index += 1;
                team.room = spine[team.spine_index];
                if keystone_rooms.contains(&team.room)
                    && !team.collected_keystones.contains(&team.room)
                {
                    team.collected_keystones.push(team.room);
                    team.collected_keystones.sort_by_key(|room| room.0);
                }
            }

            if team.room.0 == EXIT_ROOM && team.gate_open(required) {
                escaped_this_tick.push(team.id);
            }
        }

        escaped_this_tick.sort_by_key(|team| team.0);
        escaped_this_tick.dedup();
        for id in escaped_this_tick {
            if self.escape_order.len() >= survivor_slots {
                break;
            }
            if let Some(team) = self.team_mut(id)
                && !team.escaped
            {
                team.escaped = true;
                self.escape_order.push(id);
                if self.countdown.is_none() {
                    self.countdown = Some(EscapeCountdown {
                        started_by: id,
                        duration_rounds: DEFAULT_COUNTDOWN_ROUNDS,
                        remaining_rounds: DEFAULT_COUNTDOWN_ROUNDS,
                    });
                    just_started_countdown = true;
                    self.last_event = format!("{} escaped first; countdown started.", id.label());
                } else {
                    self.last_event =
                        format!("{} escaped before the countdown closed.", id.label());
                }
            }
        }

        if self.escape_order.len() >= survivor_slots {
            self.finish_round("survivor slots filled");
            return;
        }

        if let Some(countdown) = &mut self.countdown {
            if !just_started_countdown {
                countdown.remaining_rounds = countdown.remaining_rounds.saturating_sub(1);
            }
            if countdown.remaining_rounds == 0 {
                self.finish_round("countdown expired");
            }
        }
    }

    fn team_action_for(
        keystone_rooms: &[observed_core::RoomId],
        team: &TeamObjectiveState,
    ) -> SeriesAiAction {
        if team.room == observed_core::RoomId(START_ROOM) && team.tools.anchor_torches > 0 {
            SeriesAiAction::DropAnchor
        } else if keystone_rooms.contains(&team.room) && team.tools.teleport_pads > 0 {
            SeriesAiAction::DropTeleportPad
        } else {
            SeriesAiAction::Advance
        }
    }

    fn finish_round(&mut self, reason: &str) {
        let mut eliminated: Vec<TeamId> = self
            .teams
            .iter()
            .filter(|team| !team.escaped && !team.eliminated)
            .map(|team| team.id)
            .collect();
        eliminated.sort_by_key(|team| team.0);
        for id in &eliminated {
            if let Some(team) = self.team_mut(*id) {
                team.eliminated = true;
                team.director = true;
            }
        }
        self.eliminated = eliminated;
        self.finished = true;
        self.last_event = format!(
            "Round {} ended: {}; {} escaped, {} joined the adversary.",
            self.index,
            reason,
            self.escape_order.len(),
            self.eliminated.len()
        );
    }
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug, PartialEq)]
pub struct EliminationSeries {
    pub seed: u64,
    pub keystone_rooms: Vec<observed_core::RoomId>,
    pub alive_teams: Vec<TeamId>,
    pub adversary_teams: Vec<TeamId>,
    pub elimination_order: Vec<TeamId>,
    pub completed_rounds: Vec<SeriesRound>,
    pub current: SeriesRound,
    pub winner: Option<TeamId>,
    pub tick: u32,
    pub last_event: String,
}

impl EliminationSeries {
    pub fn new(seed: u64) -> Self {
        let alive_teams = (0..TEAM_COUNT).map(|i| TeamId(i as u8)).collect::<Vec<_>>();
        let keystone_rooms = keystone_rooms(seed);
        let current = SeriesRound::new(1, alive_teams.clone(), keystone_rooms.clone());
        Self {
            seed,
            keystone_rooms,
            alive_teams,
            adversary_teams: Vec::new(),
            elimination_order: Vec::new(),
            completed_rounds: Vec::new(),
            current,
            winner: None,
            tick: 0,
            last_event: "Elimination series ready.".to_string(),
        }
    }

    pub fn finished(&self) -> bool {
        self.winner.is_some()
    }

    pub fn active_team_count(&self) -> usize {
        self.alive_teams.len()
    }

    pub fn adversary_strength(&self) -> usize {
        self.adversary_teams.len()
    }

    pub fn advance_autoplay_tick(&mut self) {
        if self.finished() {
            return;
        }
        if self.alive_teams.len() <= 1 {
            self.winner = self.alive_teams.first().copied();
            self.last_event = self
                .winner
                .map(|winner| format!("{} is the last team standing.", winner.label()))
                .unwrap_or_else(|| "No teams remain.".to_string());
            return;
        }
        self.tick += 1;
        self.current
            .advance_autoplay_tick(self.adversary_strength());
        self.last_event = self.current.last_event.clone();
        if !self.current.finished {
            return;
        }

        let eliminated = self.current.eliminated.clone();
        self.alive_teams
            .retain(|team| !eliminated.iter().any(|dead| dead == team));
        self.adversary_teams.extend(eliminated.iter().copied());
        self.elimination_order.extend(eliminated);
        self.completed_rounds.push(self.current.clone());

        if self.alive_teams.len() <= 1 {
            self.winner = self.alive_teams.first().copied();
            self.last_event = self
                .winner
                .map(|winner| format!("{} wins the elimination series.", winner.label()))
                .unwrap_or_else(|| "The facility consumed every team.".to_string());
            return;
        }

        let next_index = self.completed_rounds.len() as u32 + 1;
        self.current = SeriesRound::new(
            next_index,
            self.alive_teams.clone(),
            self.keystone_rooms.clone(),
        );
        self.last_event = format!(
            "Round {next_index} begins: {} active team{}, {} adversary team{}.",
            self.alive_teams.len(),
            if self.alive_teams.len() == 1 { "" } else { "s" },
            self.adversary_teams.len(),
            if self.adversary_teams.len() == 1 {
                ""
            } else {
                "s"
            },
        );
    }

    /// Resolve the current series round from the bot-spectated two-member co-op
    /// simulation. This keeps the elimination-series standings authoritative while
    /// letting the richer teamplay model own how each team escaped or fell behind.
    pub fn apply_teamplay_round(&mut self, outcome: crate::teamplay::TeamplayRoundOutcome) {
        if self.finished() || self.current.finished {
            return;
        }

        let active = self.current.active_teams.clone();
        let escape_order: Vec<TeamId> = outcome
            .escape_order
            .into_iter()
            .filter(|team| active.contains(team))
            .collect();
        let mut eliminated: Vec<TeamId> = outcome
            .eliminated
            .into_iter()
            .filter(|team| active.contains(team) && !escape_order.contains(team))
            .collect();
        eliminated.sort_by_key(|team| team.0);
        eliminated.dedup();

        self.current.tick = outcome.tick;
        self.current.escape_order = escape_order.clone();
        self.current.eliminated = eliminated.clone();
        self.current.countdown = escape_order
            .first()
            .copied()
            .map(|started_by| EscapeCountdown {
                started_by,
                duration_rounds: DEFAULT_COUNTDOWN_ROUNDS,
                remaining_rounds: 0,
            });
        for id in &escape_order {
            if let Some(team) = self.current.team_mut(*id) {
                team.escaped = true;
            }
        }
        for id in &eliminated {
            if let Some(team) = self.current.team_mut(*id) {
                team.eliminated = true;
                team.director = true;
            }
        }
        self.current.finished = true;
        self.current.last_event = format!(
            "Round {} ended from co-op bot play: {} escaped, {} joined the adversary.",
            self.current.index,
            escape_order.len(),
            eliminated.len()
        );
        self.last_event = self.current.last_event.clone();

        self.alive_teams
            .retain(|team| !eliminated.iter().any(|dead| dead == team));
        self.adversary_teams.extend(eliminated.iter().copied());
        self.elimination_order.extend(eliminated);
        self.completed_rounds.push(self.current.clone());

        if self.alive_teams.len() <= 1 {
            self.winner = self.alive_teams.first().copied();
            self.last_event = self
                .winner
                .map(|winner| format!("{} wins the co-op elimination series.", winner.label()))
                .unwrap_or_else(|| "The facility consumed every team.".to_string());
            return;
        }

        let next_index = self.completed_rounds.len() as u32 + 1;
        self.current = SeriesRound::new(
            next_index,
            self.alive_teams.clone(),
            self.keystone_rooms.clone(),
        );
        self.last_event = format!(
            "Round {next_index} begins from co-op bot play: {} active team{}, {} adversary team{}.",
            self.alive_teams.len(),
            if self.alive_teams.len() == 1 { "" } else { "s" },
            self.adversary_teams.len(),
            if self.adversary_teams.len() == 1 {
                ""
            } else {
                "s"
            },
        );
    }

    pub fn run_to_winner(&mut self, max_ticks: u32) {
        for _ in 0..max_ticks {
            if self.finished() {
                return;
            }
            self.advance_autoplay_tick();
        }
    }

    pub fn standings(&self) -> Vec<TeamId> {
        let mut out = Vec::new();
        if let Some(winner) = self.winner {
            out.push(winner);
        }
        for team in self.elimination_order.iter().rev() {
            if !out.contains(team) {
                out.push(*team);
            }
        }
        for team in 0..TEAM_COUNT {
            let id = TeamId(team as u8);
            if !out.contains(&id) {
                out.push(id);
            }
        }
        out
    }

    pub fn placement_for(&self, team: TeamId) -> Option<u8> {
        self.standings()
            .iter()
            .position(|candidate| *candidate == team)
            .map(|index| index as u8 + 1)
    }

    pub fn team_objective(&self, team: TeamId) -> Option<&TeamObjectiveState> {
        self.current.team(team).or_else(|| {
            self.completed_rounds
                .iter()
                .rev()
                .find_map(|round| round.team(team))
        })
    }

    pub fn escaped_total(&self) -> usize {
        self.winner.is_some() as usize
    }

    pub fn absorbed_total(&self) -> usize {
        self.adversary_teams.len()
    }

    pub fn as_team_runs(&self) -> Vec<TeamRun> {
        self.standings()
            .into_iter()
            .enumerate()
            .map(|(index, id)| TeamRun {
                id,
                member_base: id.0 as usize * crate::facility::MEMBERS_PER_TEAM,
                members: crate::facility::MEMBERS_PER_TEAM as u8,
                speed: team_speed(id, self.adversary_strength()),
                pace: 0.0,
                role: if self.adversary_teams.contains(&id) {
                    crate::director::Role::Director
                } else {
                    crate::director::Role::Runner
                },
                placement: (self.winner == Some(id)).then_some(index as u8 + 1),
                objective_index: 0,
            })
            .collect()
    }
}

fn team_speed(team: TeamId, adversary_strength: usize) -> f32 {
    const BASE: [f32; TEAM_COUNT] = [1.0, 0.82, 0.64, 0.5];
    let pressure_penalty = (adversary_strength as f32 * 0.03).min(0.12);
    (BASE[team.0 as usize] - pressure_penalty).max(0.35)
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_observation::ROOM_COUNT;

    #[test]
    fn keystone_placement_is_distinct_and_route_bound() {
        let spine = spine_sequence();
        for seed in 0..64 {
            let rooms = keystone_rooms(seed);
            assert_eq!(rooms.len(), DEFAULT_KEYSTONES_REQUIRED);
            let mut unique = rooms.clone();
            unique.dedup();
            assert_eq!(unique, rooms);
            assert!(!rooms.contains(&observed_core::RoomId(START_ROOM)));
            assert!(!rooms.contains(&observed_core::RoomId(EXIT_ROOM)));
            assert!(rooms.iter().all(|room| {
                spine
                    .iter()
                    .position(|candidate| candidate == room)
                    .is_some()
            }));
        }
    }

    #[test]
    fn team_keystones_are_independent() {
        let mut round = SeriesRound::new(
            1,
            vec![TeamId(0), TeamId(1)],
            vec![observed_core::RoomId(1), observed_core::RoomId(2)],
        );
        for _ in 0..3 {
            round.advance_autoplay_tick(0);
        }
        let a = round.team(TeamId(0)).unwrap();
        let b = round.team(TeamId(1)).unwrap();
        assert_eq!(a.collected_keystones, b.collected_keystones);
        assert!(
            a.gate_open(round.required_keystones()),
            "each team owns its own keystone progress"
        );
    }

    #[test]
    fn first_escape_starts_a_countdown_and_late_teams_join_the_adversary() {
        let mut round = SeriesRound::new(
            1,
            (0..TEAM_COUNT).map(|i| TeamId(i as u8)).collect(),
            keystone_rooms(1),
        );
        for _ in 0..128 {
            if round.finished {
                break;
            }
            round.advance_autoplay_tick(0);
        }
        assert!(round.finished);
        assert!(round.countdown.is_some());
        assert!(!round.escape_order.is_empty());
        assert!(!round.eliminated.is_empty());
        assert!(
            round
                .eliminated
                .iter()
                .all(|id| round.team(*id).unwrap().director)
        );
    }

    #[test]
    fn elimination_series_runs_until_one_team_stands() {
        let mut series = EliminationSeries::new(7);
        series.run_to_winner(MAX_AUTOPLAY_TICKS);
        assert_eq!(series.winner, Some(TeamId(0)));
        assert_eq!(series.alive_teams, vec![TeamId(0)]);
        assert_eq!(series.absorbed_total(), TEAM_COUNT - 1);
        assert_eq!(series.placement_for(TeamId(0)), Some(1));
        assert_eq!(series.standings().len(), TEAM_COUNT);
    }

    #[test]
    fn same_seed_produces_identical_series() {
        let mut a = EliminationSeries::new(23);
        let mut b = EliminationSeries::new(23);
        a.run_to_winner(MAX_AUTOPLAY_TICKS);
        b.run_to_winner(MAX_AUTOPLAY_TICKS);
        assert_eq!(a, b);
    }

    #[test]
    fn teleport_pads_are_keyed_to_their_team() {
        let mut team_a = TeamToolState::new(TeamId(0));
        let mut team_b = TeamToolState::new(TeamId(1));
        assert!(team_a.drop_pad(observed_core::RoomId(1)));
        assert!(team_a.drop_pad(observed_core::RoomId(5)));
        assert!(team_b.drop_pad(observed_core::RoomId(1)));

        assert_eq!(
            team_a.pad_link_target(observed_core::RoomId(1)),
            Some(observed_core::RoomId(5))
        );
        assert_eq!(
            team_b.pad_link_target(observed_core::RoomId(1)),
            None,
            "a rival team's pad never completes this team's link"
        );
    }

    #[test]
    fn room_count_still_matches_the_nine_room_route() {
        assert_eq!(spine_sequence().len(), ROOM_COUNT);
    }

    #[test]
    fn teamplay_round_outcome_advances_the_elimination_series() {
        let mut teamplay = crate::teamplay::TeamplayMatch::new(31);
        teamplay.run_to_outcome(crate::teamplay::MAX_TEAMPLAY_TICKS);
        let outcome = teamplay.round_outcome().expect("co-op round outcome");

        let mut series = EliminationSeries::new(31);
        series.apply_teamplay_round(outcome);

        assert_eq!(series.completed_rounds.len(), 1);
        assert!(!series.adversary_teams.is_empty());
        assert!(
            series.alive_teams.len() < TEAM_COUNT,
            "at least one team should be eliminated by the co-op outcome"
        );
    }
}
