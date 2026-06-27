//! Phase 12 integration: **match replay & spectator**, recorded over the *full
//! competitive match* from Phase 11 ([`competitive_facility`]) rather than a single
//! feasibility lab.
//!
//! Every system in this workspace is deterministic, which is the property a replay
//! needs: the integrated match — observation + the protected spine + competition +
//! the facility director — can be recorded as a compact **tape** (the per-round
//! intents) and **replayed exactly** by re-feeding those intents to a fresh match.
//! A spectator can then seek to any round by replaying up to it; the rendered
//! schematic map is read straight from the replayed simulation, never reconstructed
//! from live entities (per `AGENTS.md`).
//!
//! This is the same tape approach proven in `replay_lab`, lifted from the scalar
//! competition feasibility lab onto the graph-level integrated match.

use bevy::prelude::Resource;
use competition_lab::model::RaceAction;
use competitive_facility::model::CompetitiveFacility;
use director_lab::model::Role;
use observed_core::TeamId;

const MAX_ROUNDS: usize = 400;

/// The intents fed to the match on one round.
pub type Frame = Vec<(TeamId, RaceAction)>;

#[derive(Resource, Clone, Debug)]
pub struct Tape {
    pub frames: Vec<Frame>,
    /// Notable events for the scrubber: (round, label).
    pub markers: Vec<(usize, String)>,
}

impl Tape {
    /// Record a full match by driving the deterministic competitive facility with
    /// a fixed policy, capturing every round's intents and notable events.
    pub fn record() -> Self {
        let mut world = CompetitiveFacility::authored();
        let mut frames: Vec<Frame> = Vec::new();
        let mut markers: Vec<(usize, String)> = Vec::new();

        let mut prev_placed: Vec<Option<u8>> = world.teams.iter().map(|t| t.placement).collect();
        let mut prev_absorbed: Vec<bool> = world.teams.iter().map(is_absorbed).collect();

        while !world.finished && frames.len() < MAX_ROUNDS {
            let round = frames.len();
            let frame: Frame = world
                .teams
                .iter()
                .filter(|t| t.active_runner())
                .map(|t| (t.id, policy(round, t.id)))
                .collect();
            world.advance_round(&frame);
            frames.push(frame);

            // Mark escapes and absorptions, in stable team order.
            for team in &world.teams {
                let i = team.id.0 as usize;
                if let Some(placement) = team.placement
                    && prev_placed[i].is_none()
                {
                    markers.push((
                        frames.len(),
                        format!("{} escaped {}", team.id.label(), ordinal(placement)),
                    ));
                }
                if is_absorbed(team) && !prev_absorbed[i] {
                    markers.push((frames.len(), format!("{} absorbed", team.id.label())));
                }
            }
            prev_placed = world.teams.iter().map(|t| t.placement).collect();
            prev_absorbed = world.teams.iter().map(is_absorbed).collect();
        }

        Self { frames, markers }
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Replay the tape up to `round`, returning the exact match state there. A
    /// fresh match re-fed the recorded intents — never reconstructed.
    pub fn replay_to(&self, round: usize) -> CompetitiveFacility {
        let mut world = CompetitiveFacility::authored();
        let n = round.min(self.frames.len());
        for frame in &self.frames[..n] {
            world.advance_round(frame);
        }
        world
    }
}

fn is_absorbed(team: &competitive_facility::model::TeamRun) -> bool {
    team.role == Role::Director && team.placement.is_none()
}

/// Deterministic recording policy: the slowest team (id 3) seizes the shared
/// control early to grab the sprint, then races — a recorded comeback attempt.
fn policy(round: usize, team: TeamId) -> RaceAction {
    if team == TeamId(3) && round < 4 {
        RaceAction::Seize
    } else {
        RaceAction::Advance
    }
}

fn ordinal(n: u8) -> &'static str {
    match n {
        1 => "1st",
        2 => "2nd",
        3 => "3rd",
        _ => "nth",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use competitive_facility::model::TEAM_COUNT;

    /// A compact, comparable fingerprint of a match state.
    fn snapshot(world: &CompetitiveFacility) -> Vec<(u32, u8, Option<u8>, i32)> {
        (0..world.teams.len())
            .map(|i| {
                let t = &world.teams[i];
                (
                    world.team_room(i).0,
                    match t.role {
                        Role::Runner => 0,
                        Role::Director => 1,
                    },
                    t.placement,
                    (world.purge_line * 1000.0) as i32,
                )
            })
            .collect()
    }

    #[test]
    fn the_recording_captures_a_finished_match() {
        let tape = Tape::record();
        assert!(!tape.is_empty());
        let end = tape.replay_to(tape.len());
        assert!(end.finished);
        // Capacity-limited: exactly two escape, the rest absorbed.
        assert_eq!(end.escaped_count(), 2);
        assert_eq!(end.escaped_count() + end.absorbed_count(), TEAM_COUNT);
        // The fastest team still wins.
        assert_eq!(end.winner, Some(TeamId(0)));
    }

    #[test]
    fn replay_is_exact_and_repeatable() {
        let tape = Tape::record();
        let mid = tape.len() / 2;
        for round in [0, 1, mid, tape.len() - 1, tape.len()] {
            assert_eq!(
                snapshot(&tape.replay_to(round)),
                snapshot(&tape.replay_to(round)),
                "replay must be deterministic at round {round}"
            );
        }
    }

    #[test]
    fn seeking_matches_sequential_playback() {
        // Seeking to round N must equal stepping a world N times with the tape.
        let tape = Tape::record();
        let mut world = CompetitiveFacility::authored();
        for round in 0..tape.len() {
            assert_eq!(
                snapshot(&world),
                snapshot(&tape.replay_to(round)),
                "seek to {round} must match sequential state"
            );
            world.advance_round(&tape.frames[round]);
        }
    }

    #[test]
    fn replay_past_the_end_clamps() {
        let tape = Tape::record();
        assert_eq!(
            snapshot(&tape.replay_to(tape.len())),
            snapshot(&tape.replay_to(tape.len() + 500)),
        );
    }

    #[test]
    fn events_are_marked_for_the_scrubber() {
        let tape = Tape::record();
        // Two escapes + two absorptions = four events, in round order.
        assert_eq!(tape.markers.len(), 4);
        for pair in tape.markers.windows(2) {
            assert!(pair[0].0 <= pair[1].0, "markers must be in round order");
        }
    }
}
