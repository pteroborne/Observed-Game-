//! The seams. Six traits, and the contract that keeps them honest:
//!
//! > **A trait ships with two implementations or it is not a trait yet.**
//!
//! One implementor is a speculative abstraction, which `agents.md` forbids.
//! Two is a comparison, which is what this lab is for. Every trait below has a
//! second implementation in its own module, and `sim::tests` asserts that each
//! pair actually produces different matches — a seam no test can distinguish is
//! not a seam.

use observed_hex::coords::{HexCoord, HexGridSize};
use observed_hex::faces::HexFace;

use crate::sim::state::{MatchState, Outcome, PawnId};

/// Which cells are held against structural change this turn.
#[derive(Clone, Debug)]
pub struct LockSet {
    size: HexGridSize,
    held: Vec<bool>,
}

impl LockSet {
    #[must_use]
    pub fn empty(size: HexGridSize) -> Self {
        Self {
            size,
            held: vec![false; size.cell_count()],
        }
    }

    pub fn hold(&mut self, coord: HexCoord) {
        if self.size.contains(coord) {
            let index = self.size.index(coord);
            self.held[index] = true;
        }
    }

    #[must_use]
    pub fn is_held(&self, coord: HexCoord) -> bool {
        self.size.contains(coord) && self.held[self.size.index(coord)]
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.held.iter().filter(|held| **held).count()
    }
}

/// How simultaneous intents become positions. The only stage where the *order*
/// of resolution is visible, which is why it is the seam the lab was asked for
/// first.
pub trait Resolution: Send + Sync {
    fn name(&self) -> &'static str;

    /// Given each pawn's desired destination, return the destination each pawn
    /// actually gets. Implementations may return the pawn's current cell to
    /// refuse a move, and must return exactly one entry per input.
    fn settle(&self, state: &MatchState, desired: &[(PawnId, HexCoord)])
    -> Vec<(PawnId, HexCoord)>;
}

/// What a pawn holds by looking at it.
pub trait Vision: Send + Sync {
    fn name(&self) -> &'static str;

    /// Whether a pawn standing at `from` and facing `facing` covers `target`.
    /// Everything else in this trait is built from this one question, which is
    /// also what lets plant legality be asked before the full lock set exists.
    fn covers(&self, state: &MatchState, from: HexCoord, facing: HexFace, target: HexCoord)
    -> bool;

    /// Every cell held by every free pawn.
    fn locks(&self, state: &MatchState) -> LockSet {
        let mut locks = LockSet::empty(state.board.size());
        for pawn in state.free_pawns() {
            for cell in state.board.cells() {
                if self.covers(state, pawn.at, pawn.facing, cell) {
                    locks.hold(cell);
                }
            }
        }
        locks
    }
}

/// What hunts the pawns.
pub trait Threat: Send + Sync {
    fn name(&self) -> &'static str;

    /// Move threats and take whoever they catch, delegating the consequence to
    /// `setback`. Runs after [`Vision::locks`] because guardian behaviour may
    /// consult the lock set.
    fn act(&self, state: &mut MatchState, locks: &LockSet, setback: &dyn Setback);
}

/// What happens to a pawn a threat catches, and how it comes back. Separate
/// from [`Threat`] because what hunts you and what it costs vary independently.
pub trait Setback: Send + Sync {
    fn name(&self) -> &'static str;

    fn take(&self, state: &mut MatchState, pawn: PawnId);

    /// Called once per turn after movement, before threats act.
    fn release(&self, state: &mut MatchState);
}

/// What the facility does when nobody is looking.
pub trait Mutation: Send + Sync {
    fn name(&self) -> &'static str;

    /// Choose and record the cells that will change at the end of this turn.
    /// Called at the top of the turn so the change is telegraphed, which is the
    /// shipped `MUTATION_WARNING_TICKS` contract at a readable cadence.
    fn telegraph(&self, state: &mut MatchState);

    /// Apply whatever the telegraph promised and the locks did not refuse.
    fn apply(&self, state: &mut MatchState, locks: &LockSet);
}

/// What winning is.
pub trait Objective: Send + Sync {
    fn name(&self) -> &'static str;

    /// Whether this pawn may take [`Action::Plant`](crate::sim::state::Action)
    /// where it stands, given what the squad currently observes.
    fn may_claim(&self, state: &MatchState, pawn: PawnId, vision: &dyn Vision) -> bool;

    /// Perform the claim. Only called when `may_claim` agreed.
    fn claim(&self, state: &mut MatchState, pawn: PawnId);

    fn evaluate(&self, state: &MatchState) -> Option<Outcome>;
}
