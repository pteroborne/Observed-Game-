//! The fixed turn pipeline.
//!
//! ```text
//! open turn        clear immunity, record prev_at
//!   -> [locks]     when ConeTiming::PreMove
//!   -> resolve     facing, movement conflicts, plants
//!   -> release     a free pawn standing in the prison frees the held
//!   -> [locks]     when ConeTiming::PostMove
//!   -> threats     guardians move and take; cone interaction consults locks
//!   -> mutate      unheld telegraphed boundaries rewire
//!   -> telegraph   mark what the *next* turn will change, so it can be seen
//!   -> evaluate    outcome, or another turn
//! ```
//!
//! The pipeline never varies. Only which strategy fills each stage does, which
//! is what keeps determinism and testing tractable — and why `ConeTiming` is a
//! *relocation* of one call rather than a branch inside `Vision`.

use crate::sim::rules::LockSet;
use crate::sim::state::{Action, Intent, MatchState, Outcome};
use crate::spec::{ConeTiming, Rules};

/// Advance one turn. Returns the outcome if this turn ended the match.
pub fn step(state: &mut MatchState, rules: &Rules, intents: &[Intent]) -> Option<Outcome> {
    if state.outcome.is_some() {
        return state.outcome;
    }
    state.report = Default::default();
    for pawn in &mut state.pawns {
        pawn.immune = false;
        pawn.prev_at = pawn.at;
    }
    let pre_move_locks =
        matches!(rules.cone_timing, ConeTiming::PreMove).then(|| rules.vision.locks(state));

    // Facing is free but declared, so it lands before anything is resolved.
    for intent in intents {
        if !state.pawn(intent.pawn).jailed {
            state.pawn_mut(intent.pawn).facing = intent.facing;
        }
    }

    // Movement, through whichever resolution strategy is loaded.
    let desired: Vec<_> = intents
        .iter()
        .filter(|intent| !state.pawn(intent.pawn).jailed)
        .map(|intent| {
            let from = state.pawn(intent.pawn).at;
            let to = match intent.action {
                Action::Step(face) => state
                    .board
                    .passable(from, face)
                    .then(|| state.board.size().neighbor(from, face))
                    .flatten()
                    .unwrap_or(from),
                Action::Hold | Action::Plant => from,
            };
            (intent.pawn, to)
        })
        .collect();

    for (id, to) in rules.resolution.settle(state, &desired) {
        let wanted = desired
            .iter()
            .find(|&&(candidate, _)| candidate == id)
            .map(|&(_, to)| to);
        if wanted.is_some_and(|wanted| wanted != to) {
            state.report.refused_moves.push(id);
        }
        state.pawn_mut(id).at = to;
    }

    // Plants resolve after movement, so a pawn that was pushed off its flag
    // does not plant one it no longer stands on.
    for intent in intents {
        if intent.action == Action::Plant
            && rules
                .objective
                .may_claim(state, intent.pawn, rules.vision.as_ref())
        {
            rules.objective.claim(state, intent.pawn);
        }
    }

    // Stink base recency: stepping off your own base stamps you fresh. Done
    // after movement so the stamp reflects where the pawn actually ended up,
    // not where it hoped to go.
    for index in 0..state.pawns.len() {
        let pawn = state.pawns[index];
        let base = state.base_of(pawn.team);
        if pawn.prev_at == base && pawn.at != base {
            state.pawns[index].left_base_at = state.turn;
        }
    }

    rules.setback.release(state);

    let locks = pre_move_locks.unwrap_or_else(|| rules.vision.locks(state));

    for threat in &rules.threats {
        threat.act(state, &locks, rules.setback.as_ref());
    }
    rules.mutation.apply(state, &locks);

    state.turn += 1;
    // Telegraph *after* resolving, for the turn about to be played. Generating
    // it at the top of `step` and applying it in the same call meant the marks
    // existed only between two statements, so a player choosing orders never
    // saw them — the whole "you can see it coming, hold it if you can" beat was
    // invisible. Bot play never noticed, because the driver does not read the
    // telegraph. `deal` seeds the first one.
    if state.outcome.is_none() {
        rules.mutation.telegraph(state, &locks);
    }
    state.outcome = rules.objective.evaluate(state);
    state.outcome
}

/// The lock set the current state would produce — what the board overlay draws.
#[must_use]
pub fn current_locks(state: &MatchState, rules: &Rules) -> LockSet {
    rules.vision.locks(state)
}
