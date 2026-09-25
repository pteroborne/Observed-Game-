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
/// Which cells are held against structural change this turn, and *how*.
///
/// The distinction matters because the two do different jobs. Occupancy holds
/// structure — you cannot rewire the floor somebody is standing on. Being
/// *looked at* is what shields against a guardian. Collapsing them made a pawn
/// permanently invulnerable, since its own cell is always held: guardians could
/// never enter any pawn's cell and simply parked next to the squad doing
/// nothing for a whole match.
#[derive(Clone, Debug)]
pub struct LockSet {
    size: HexGridSize,
    held: Vec<bool>,
    covered: Vec<bool>,
}

impl LockSet {
    #[must_use]
    pub fn empty(size: HexGridSize) -> Self {
        Self {
            size,
            held: vec![false; size.cell_count()],
            covered: vec![false; size.cell_count()],
        }
    }

    /// Held against structural change — by occupancy or by sight.
    pub fn hold(&mut self, coord: HexCoord) {
        if self.size.contains(coord) {
            let index = self.size.index(coord);
            self.held[index] = true;
        }
    }

    /// Held *and* actually looked at, which is what shields it.
    pub fn cover(&mut self, coord: HexCoord) {
        if self.size.contains(coord) {
            let index = self.size.index(coord);
            self.held[index] = true;
            self.covered[index] = true;
        }
    }

    #[must_use]
    pub fn is_held(&self, coord: HexCoord) -> bool {
        self.size.contains(coord) && self.held[self.size.index(coord)]
    }

    /// Whether somebody's cone reaches this cell. A pawn standing in a cell it
    /// is not looking at does not cover it.
    #[must_use]
    pub fn is_covered(&self, coord: HexCoord) -> bool {
        self.size.contains(coord) && self.covered[self.size.index(coord)]
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

    /// Every cell held by every free pawn, separating what is merely occupied
    /// from what is actually being looked at.
    fn locks(&self, state: &MatchState) -> LockSet {
        let mut locks = LockSet::empty(state.board.size());
        for pawn in state.free_pawns() {
            for cell in state.board.cells() {
                if !self.covers(state, pawn.at, pawn.facing, cell) {
                    continue;
                }
                if cell == pawn.at {
                    locks.hold(cell);
                } else {
                    locks.cover(cell);
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

    /// Choose and record what will change, given what is currently held.
    /// Called at the *end* of a turn, for the turn about to be played.
    fn telegraph(&self, state: &mut MatchState, locks: &LockSet);

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
