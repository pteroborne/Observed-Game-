//! Hybrid match tape, record, and play functionality.

use super::{HybridFrame, HybridMatch, HybridSnapshot, LOCAL_TEAM, LocalAction, MAX_ROUNDS};

#[derive(Clone, Debug, Default)]
pub struct HybridTape {
    pub frames: Vec<HybridFrame>,
    pub snapshots: Vec<HybridSnapshot>,
    pub seed: u64,
}

impl HybridTape {
    pub fn record_demo(seed: u64) -> Self {
        let mut session = HybridMatch::authored(seed);
        let mut tape = Self {
            seed,
            snapshots: vec![session.snapshot()],
            ..Default::default()
        };
        while !session.competitive.finished && tape.frames.len() < MAX_ROUNDS {
            let local_active = session
                .competitive
                .team(LOCAL_TEAM)
                .is_some_and(|team| team.active_runner());
            let local = if local_active {
                LocalAction::Advance
            } else {
                LocalAction::Wait
            };
            assert!(session.apply_action(local));
            tape.frames.push(HybridFrame { local });
            tape.snapshots.push(session.snapshot());
        }
        assert!(
            session.competitive.finished,
            "the hybrid match must resolve"
        );
        tape
    }

    pub fn replay_to(&self, round: usize) -> HybridMatch {
        let mut session = HybridMatch::authored(self.seed);
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
