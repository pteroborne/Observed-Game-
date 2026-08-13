//! End-of-turn facility shift: telegraph a pocket, then try to commit it.
//!
//! The shipped match spreads this over ticks — a warning fires
//! `MUTATION_WARNING_TICKS` before a commit, and observation in between can save
//! the structure. That window is real but nearly invisible at 60 Hz from inside a
//! corridor, which is one of the things Arc T's playtest reported: the facility's
//! changes were imperceptible.
//!
//! A turn is the same window at a readable cadence. The pocket is selected and
//! **drawn** at the start of the turn; the player spends the turn deciding
//! whether to spend units holding it or distance running from it; the commit is
//! attempted against wherever the squad actually ended up. Nothing about the
//! contract changes — this is the same three calls the match makes — only the
//! rate at which a human can read it.

use observed_facility::hex_wfc::{
    HexObservationFrame, HexRelayoutCandidate, HexRelayoutDelta, HexRelayoutProgress, HexWfcError,
    HexWfcWorld,
};
use observed_hex::{HexCoord, HexFace};
use std::collections::BTreeSet;

/// A pocket that has been solved and is waiting for the turn to end.
#[derive(Clone, Debug)]
pub struct Telegraph {
    candidate: HexRelayoutCandidate,
    /// The cells the view draws as about-to-change.
    pub cells: BTreeSet<HexCoord>,
}

impl Telegraph {
    #[must_use]
    pub fn cells(&self) -> &BTreeSet<HexCoord> {
        &self.cells
    }
}

/// What happened when a telegraphed pocket met the end of the turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShiftOutcome {
    /// The pocket re-collapsed. Cells changed.
    Committed,
    /// The squad was standing in it, or holding it, or anchoring it. The
    /// facility tried and was refused.
    Held,
    /// No pocket was available, or every attempt failed to solve.
    NothingToShift,
}

/// Select and solve the pocket that will change at the end of this turn.
///
/// The frontier is the squad's *discovered* territory rather than its current
/// position, matching `HexWfcMatch::step_mutation`: the facility breathes where
/// someone has been, so a match stays legible near the route and stays genuinely
/// unknown ahead of it.
#[must_use]
pub fn telegraph(
    world: &HexWfcWorld,
    observation: &HexObservationFrame,
    frontier: &BTreeSet<HexCoord>,
) -> Option<Telegraph> {
    // The production warning lasts about two real-time seconds, during which a
    // runner cannot cross several complete hexes. A tactics turn can spend up
    // to eight whole-tile moves. Give selection one extra protected ring so an
    // ordinary first step does not accidentally hold nearly every warning;
    // reaching the pocket remains possible, but becomes a deliberate route.
    let mut selection_frame = observation.clone();
    let visible = selection_frame.visible_cells.clone();
    for cell in visible {
        for face in HexFace::ALL {
            if let Some(neighbor) = world.config.grid().neighbor(cell, face) {
                selection_frame.visible_cells.insert(neighbor);
            }
        }
    }
    // The canonical 32-cell pocket is intentionally modest in a production
    // facility, but would occupy 40% of an 80-tile teaching floor. Scale the
    // lab pocket with its configurable map so "held" means the squad actually
    // committed to the warned area, not merely crossed a giant region.
    let target_cells = (world.placements.len() / 16).clamp(8, 32);
    let max_cells = (target_cells * 2).min(64);
    let mut work =
        world.begin_frontier_relayout_sized(&selection_frame, frontier, target_cells, max_cells);
    loop {
        match world.advance_relayout(work) {
            Ok(HexRelayoutProgress::Pending(next)) => work = next,
            Ok(HexRelayoutProgress::Ready(candidate)) => {
                return Some(Telegraph {
                    cells: candidate.region.cells.clone(),
                    candidate,
                });
            }
            Err(_) => return None,
        }
    }
}

/// Try to apply a telegraphed pocket against the observation the squad actually
/// finished the turn with.
///
/// [`HexWfcWorld::commit_relayout_delta`] revalidates rather than trusting the
/// candidate: it re-derives the protected set from `latest`, rejects the patch
/// if the squad moved into the pocket, and restores the previous cells if the
/// resulting layout would break a route. A rejection is therefore a *gameplay
/// result* — the squad held the ground — not an error to report.
pub fn commit(
    world: &mut HexWfcWorld,
    telegraph: Telegraph,
    latest: &HexObservationFrame,
) -> (ShiftOutcome, Option<HexRelayoutDelta>) {
    match world.commit_relayout_delta(telegraph.candidate, latest) {
        Ok(delta) if delta.changed_cells.is_empty() => (ShiftOutcome::NothingToShift, Some(delta)),
        Ok(delta) => (ShiftOutcome::Committed, Some(delta)),
        // Every failure here is the facility being refused, which is what the
        // player was trying to achieve. The variants differ in *why* it was
        // refused, and none of them is worth a different word on screen.
        Err(HexWfcError::UnsafeChange(_) | HexWfcError::StaleCandidate) => {
            (ShiftOutcome::Held, None)
        }
        Err(_) => (ShiftOutcome::NothingToShift, None),
    }
}
