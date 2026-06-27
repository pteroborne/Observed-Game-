//! Pure-logic feasibility model for **replay & spectator presentation**.
//!
//! Every simulation in this workspace is deterministic, which is the property a
//! replay needs: a match can be recorded as a compact **tape** (a fixed timestep
//! plus the per-tick inputs) and **replayed exactly** by re-feeding those inputs
//! to a fresh world. A spectator can then seek to any tick by replaying up to it —
//! the rendered state is read straight from the simulation, never reconstructed
//! from entities (per `AGENTS.md`).
//!
//! The recorded match reuses the deterministic `competition_lab` so this proves
//! replay over a real system, not a toy.

use bevy::prelude::*;
use competition_lab::model::{CompetitionWorld, RaceAction};
use observed_core::TeamId;

pub const REPLAY_DT: f32 = 1.0 / 30.0;
const MAX_FRAMES: usize = 4000;

/// The inputs fed to the simulation on one tick.
pub type Frame = Vec<(TeamId, RaceAction)>;

#[derive(Resource, Clone, Debug)]
pub struct Tape {
    pub dt: f32,
    pub frames: Vec<Frame>,
    /// Notable events for the scrubber: (tick, label).
    pub markers: Vec<(usize, String)>,
}

impl Tape {
    /// Record a full match by driving the deterministic competition with a fixed
    /// policy, capturing every tick's inputs.
    pub fn record() -> Self {
        let dt = REPLAY_DT;
        let mut world = CompetitionWorld::authored();
        let mut frames: Vec<Frame> = Vec::new();
        let mut markers: Vec<(usize, String)> = Vec::new();
        let mut placed = 0usize;

        while !world.finished && frames.len() < MAX_FRAMES {
            let tick = frames.len();
            let frame: Frame = world
                .teams
                .iter()
                .filter(|team| team.racing())
                .map(|team| (team.id, policy(tick, team.id)))
                .collect();
            world.tick(&frame, dt);
            frames.push(frame);

            let now_placed = world.teams.iter().filter(|t| t.placement.is_some()).count();
            while placed < now_placed {
                let rank = (placed + 1) as u8;
                if let Some(team) = world.teams.iter().find(|t| t.placement == Some(rank)) {
                    markers.push((
                        frames.len(),
                        format!("{} escaped {}", team.id.label(), ordinal(rank)),
                    ));
                }
                placed += 1;
            }
        }

        Self {
            dt,
            frames,
            markers,
        }
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Replay the tape up to `tick`, returning the exact world state there.
    pub fn replay_to(&self, tick: usize) -> CompetitionWorld {
        let mut world = CompetitionWorld::authored();
        let n = tick.min(self.frames.len());
        for frame in &self.frames[..n] {
            world.tick(frame, self.dt);
        }
        world
    }
}

/// Deterministic recording policy: team 3 (id 2) seizes the shared control early,
/// then races — so the recording is a match it comes back to win.
fn policy(tick: usize, team: TeamId) -> RaceAction {
    if team == TeamId(2) && tick < 8 {
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

    fn snapshot(world: &CompetitionWorld) -> (Vec<u32>, Vec<Option<u8>>, Option<u8>, bool, u32) {
        (
            world
                .teams
                .iter()
                .map(|t| (t.progress * 1_000_000.0) as u32)
                .collect(),
            world.teams.iter().map(|t| t.placement).collect(),
            world.winner.map(|w| w.0),
            world.finished,
            world.tick_count,
        )
    }

    #[test]
    fn the_recording_captures_a_finished_match() {
        let tape = Tape::record();
        assert!(!tape.is_empty());
        let end = tape.replay_to(tape.len());
        assert!(end.finished);
        // The policy makes team 3 (id 2) win.
        assert_eq!(end.winner, Some(TeamId(2)));
    }

    #[test]
    fn replay_is_exact_and_repeatable() {
        let tape = Tape::record();
        let mid = tape.len() / 2;
        for tick in [0, 1, mid, tape.len() - 1, tape.len()] {
            assert_eq!(
                snapshot(&tape.replay_to(tick)),
                snapshot(&tape.replay_to(tick)),
                "replay must be deterministic at tick {tick}"
            );
        }
    }

    #[test]
    fn seeking_matches_sequential_playback() {
        // Replaying to tick N must equal stepping the world N times with the tape.
        let tape = Tape::record();
        let mut world = CompetitionWorld::authored();
        for tick in 0..tape.len() {
            assert_eq!(
                snapshot(&world),
                snapshot(&tape.replay_to(tick)),
                "seek to {tick} must match sequential state"
            );
            world.tick(&tape.frames[tick], tape.dt);
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
    fn escape_events_are_marked_for_the_scrubber() {
        let tape = Tape::record();
        // Two teams escape (capacity 2), so there are two markers, in tick order.
        assert_eq!(tape.markers.len(), 2);
        assert!(tape.markers[0].0 <= tape.markers[1].0);
    }
}
