//! Stink base: rival pawns take each other by recency, not by force.
//!
//! The rule is Prisoner's Base's: whoever left their own base most recently has
//! power over whoever left earlier. Fresher takes staler; staler cannot touch
//! fresher; equal freshness is a standoff and nobody moves. A pawn standing in
//! its own base is maximally fresh and untouchable, which is what makes the
//! trip home cost something.
//!
//! Two things recommend it here beyond that it plays well. It is a capture rule
//! with **no combat** — purely positional, purely stateful — which is what the
//! north star's "players cannot directly harm opponents" asks for. And it puts
//! a second tempo cost against the same limited turns as holding ground does:
//! going home to refresh is distance you do not travel toward the objective.

use crate::sim::rules::{LockSet, Setback, Threat};
use crate::sim::state::{MatchState, PawnId};

/// Cross-team contact, adjudicated by recency.
///
/// Contact is co-location, not adjacency. On a 37-cell board an adjacency rule
/// would let three rival pawns threaten roughly eighteen cells at once, and a
/// board that is mostly lethal has nothing left to decide.
#[derive(Clone, Copy, Debug, Default)]
pub struct RivalPawns;

impl Threat for RivalPawns {
    fn name(&self) -> &'static str {
        "RivalPawns"
    }

    fn act(&self, state: &mut MatchState, _locks: &LockSet, setback: &dyn Setback) {
        let mut taken: Vec<PawnId> = Vec::new();

        for i in 0..state.pawns.len() {
            for j in 0..state.pawns.len() {
                if i == j {
                    continue;
                }
                let (a, b) = (&state.pawns[i], &state.pawns[j]);
                if a.team == b.team || a.jailed || b.jailed || a.immune {
                    continue;
                }
                // Sharing a hex, or having run straight through each other.
                let shared = a.at == b.at;
                let swapped = a.at == b.prev_at && b.at == a.prev_at && a.at != b.at;
                if !shared && !swapped {
                    continue;
                }
                // Equal freshness is a standoff: neither may take the other.
                if state.freshness(a.id) < state.freshness(b.id) {
                    taken.push(a.id);
                }
            }
        }

        taken.sort_unstable();
        taken.dedup();
        for id in taken {
            setback.take(state, id);
        }
    }
}
