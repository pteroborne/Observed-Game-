//! Two answers to "what does the facility do when nobody is looking".

use observed_hex::ports::PortClass;

use crate::sim::board::Edge;
use crate::sim::rules::{LockSet, Mutation};
use crate::sim::state::{MatchState, PendingChange};

/// Doorways open and close a turn after they are marked, unless held.
///
/// It **rewires** rather than decays: each turn closes some doorways and opens
/// others, so the wall moves around the board rather than closing in on it.
/// The first draft only ever sealed, ate thirty of thirty-seven cells by turn
/// eight, and left every seam test comparing two identical dead boards. It was
/// also wrong about the modelled system, where architecture changes its
/// connections rather than losing its floor.
///
/// A change is refused when either cell it joins is held, so standing in a
/// doorway keeps it — which is the anchor-and-threshold mechanic the shipped
/// game has, at a cadence a person can read.
#[derive(Clone, Copy, Debug)]
pub struct TelegraphedRewire {
    pub base: u16,
    pub cap: u16,
}

impl Default for TelegraphedRewire {
    fn default() -> Self {
        Self { base: 2, cap: 6 }
    }
}

impl Mutation for TelegraphedRewire {
    fn name(&self) -> &'static str {
        "TelegraphedRewire"
    }

    fn telegraph(&self, state: &mut MatchState) {
        let wanted = (self.base + state.turn).min(self.cap) as usize;
        let size = state.board.size();

        // Never touch a boundary of a flag or a prison cell. Walling one off is
        // a match made unwinnable by accident, which is a bug wearing a
        // difficulty curve's clothes.
        let protected: Vec<_> = state
            .flags
            .iter()
            .map(|flag| flag.at)
            .chain(state.prisons.iter().copied())
            .collect();

        let mut candidates: Vec<Edge> = state
            .board
            .interior_edges()
            .into_iter()
            .filter(|edge| {
                let other = size.neighbor(edge.cell, edge.face);
                !protected.contains(&edge.cell)
                    && other.is_some_and(|other| !protected.contains(&other))
            })
            .collect();
        state.rng.shuffle(&mut candidates);
        candidates.truncate(wanted);
        candidates.sort_unstable();

        state.telegraph = candidates
            .into_iter()
            .map(|edge| PendingChange {
                edge,
                to: match state.board.port(edge) {
                    PortClass::Door => PortClass::Sealed,
                    _ => PortClass::Door,
                },
            })
            .collect();
    }

    fn apply(&self, state: &mut MatchState, locks: &LockSet) {
        let size = state.board.size();
        let pending = std::mem::take(&mut state.telegraph);
        for change in pending {
            let other = size.neighbor(change.edge.cell, change.edge.face);
            let held =
                locks.is_held(change.edge.cell) || other.is_some_and(|other| locks.is_held(other));
            if held {
                state.report.refused_rewires.push(change);
            } else {
                state.board.set_port(change.edge, change.to);
                state.report.rewired.push(change);
            }
        }
    }
}

/// The facility holds still — the control for asking whether the rewiring is
/// carrying a mode or merely decorating it.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoMutation;

impl Mutation for NoMutation {
    fn name(&self) -> &'static str {
        "None"
    }

    fn telegraph(&self, state: &mut MatchState) {
        state.telegraph.clear();
    }

    fn apply(&self, _state: &mut MatchState, _locks: &LockSet) {}
}
