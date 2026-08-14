//! Replaying the solve that produced the facility on screen.
//!
//! The studio has always thrown its trace away. `run_solve` asks for a traced
//! solve, keeps the step log on [`crate::SolveResult`], and then draws only the
//! finished world — so the one question the tool exists to answer, *what does
//! this profile make the solver do*, could only ever be answered from the
//! result. This module reads the log back.
//!
//! The cursor sits at the end of the trace by default, which is the finished
//! world and exactly what the studio drew before. Nothing here changes what a
//! solve produces, what a capture writes, or what the content hash folds; it is
//! a view onto a log that was already being computed.

use std::collections::BTreeMap;

use bevy::prelude::*;
use observed_facility::hex_wfc::{CellTrace, TraceSummary, fold_trace, summarise_trace};
use observed_hex::HexCoord;

use crate::StudioState;

/// Decisions consumed per frame at the default speed.
///
/// The compact fixture's trace is around 400 steps, so this replays it in about
/// a second and a half — long enough to watch, short enough that it is not a
/// wait. `+`/`-` scale it for the ten-level facility, whose trace is roughly
/// twenty times longer.
pub const DEFAULT_STEPS_PER_TICK: usize = 6;
const MAX_STEPS_PER_TICK: usize = 128;
/// How far `Shift` + a step key jumps, for finding a region of a long trace.
pub const COARSE_STEP: isize = 20;

/// Where the replay is, and whether it is running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeline {
    /// How many steps of the trace have been replayed.
    pub cursor: usize,
    pub playing: bool,
    pub steps_per_tick: usize,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            cursor: 0,
            playing: false,
            steps_per_tick: DEFAULT_STEPS_PER_TICK,
        }
    }
}

impl Timeline {
    /// Whether the cursor is at the end — which is to say, showing the solve
    /// the studio would have drawn anyway.
    #[must_use]
    pub const fn settled(&self, steps: usize) -> bool {
        self.cursor >= steps
    }

    /// A new solve arrived. Land on its finished world rather than rewinding:
    /// tuning a value is a request to see the *result*, and a studio that
    /// replayed four hundred steps after every slider nudge would be unusable.
    pub fn settle(&mut self, steps: usize) {
        self.cursor = steps;
        self.playing = false;
    }

    /// Play, pause, or — from the end — start again from an empty lattice.
    ///
    /// Restarting is what the key means at the end of a replay. The alternative
    /// is a play button that does nothing on the state the tool is almost
    /// always in.
    pub fn toggle(&mut self, steps: usize) {
        if self.settled(steps) {
            self.cursor = 0;
            self.playing = steps > 0;
        } else {
            self.playing = !self.playing;
        }
    }

    /// Move by hand. Stepping always pauses: a cursor that kept running out
    /// from under the key would make single-stepping impossible.
    pub fn step(&mut self, steps: usize, delta: isize) {
        self.playing = false;
        self.cursor = self.cursor.saturating_add_signed(delta).min(steps);
    }

    pub fn to_end(&mut self, steps: usize) {
        self.cursor = steps;
        self.playing = false;
    }

    pub fn change_speed(&mut self, faster: bool) {
        self.steps_per_tick = if faster {
            (self.steps_per_tick * 2).min(MAX_STEPS_PER_TICK)
        } else {
            (self.steps_per_tick / 2).max(1)
        };
    }
}

/// How many steps the current solve has, or zero when there is no solve.
#[must_use]
pub fn trace_len(state: &StudioState) -> usize {
    state.solved.as_ref().map_or(0, |solved| solved.steps.len())
}

/// Whether a replay is part-way through, rather than showing a finished solve.
#[must_use]
pub fn replaying(state: &StudioState) -> bool {
    state
        .solved
        .as_ref()
        .is_some_and(|solved| !state.timeline.settled(solved.steps.len()))
}

/// Per-cell state at the cursor, or `None` when the replay is settled.
///
/// `None` means "draw the world as normal". Keeping the settled case out of the
/// fold entirely is deliberate: it is the case the studio is in essentially all
/// the time, and it should cost exactly what it cost before this module existed.
#[must_use]
pub fn replay_cells(state: &StudioState) -> Option<BTreeMap<HexCoord, CellTrace>> {
    let solved = state.solved.as_ref()?;
    if state.timeline.settled(solved.steps.len()) {
        return None;
    }
    Some(fold_trace(&solved.steps[..state.timeline.cursor]))
}

/// Totals at the cursor. Always available, so the readout reports the finished
/// solve when settled and counts up while replaying.
#[must_use]
pub fn summary(state: &StudioState) -> Option<TraceSummary> {
    let solved = state.solved.as_ref()?;
    let cursor = state.timeline.cursor.min(solved.steps.len());
    Some(summarise_trace(&solved.steps[..cursor]))
}

/// Advance a running replay.
pub fn advance_timeline(mut state: ResMut<StudioState>) {
    if !state.timeline.playing {
        return;
    }
    let steps = trace_len(&state);
    if state.timeline.settled(steps) {
        state.timeline.playing = false;
        return;
    }
    let per_tick = state.timeline.steps_per_tick;
    state.timeline.cursor = (state.timeline.cursor + per_tick).min(steps);
    if state.timeline.settled(steps) {
        state.timeline.playing = false;
    }
    state.touch_view();
}

/// The solver's own state, for the SOLVE tab.
///
/// Every number here is read back out of the step log rather than recomputed,
/// so this reports what the solver did and not a second opinion about it.
#[must_use]
pub fn format_solver_state(state: &StudioState) -> String {
    let Some(solved) = state.solved.as_ref() else {
        return String::from("SOLVER STATE:\n\nNo solve result available.\n");
    };
    let steps = solved.steps.len();
    let Some(summary) = summary(state) else {
        return String::from("SOLVER STATE:\n\nNo solve result available.\n");
    };

    let mut s = String::from("SOLVER STATE:\n\n");
    s.push_str(&format!("  seed          {:#018x}\n", state.seed()));
    s.push_str(&format!(
        "  collapsed     {} of {} cell(s)\n",
        summary.collapsed,
        solved.world.placements.len()
    ));
    s.push_str(&format!(
        "  open          {} cell(s), {} option(s) still standing\n",
        summary.open, summary.open_domain
    ));
    s.push_str(&format!(
        "  attempt       {} ({} contradiction(s) so far)\n",
        summary.attempt, summary.contradictions
    ));
    match summary.completed {
        Some((rooms, halls)) => {
            s.push_str(&format!(
                "  completed     {rooms} room(s), {halls} hall(s)\n"
            ));
        }
        None => s.push_str("  completed     not yet\n"),
    }
    s.push_str(&format!("  solved in     {}ms\n", solved.elapsed_ms));

    s.push_str("\nREPLAY:\n\n");
    if steps == 0 {
        // Multi-candidate search re-solves the winner to recover its trace, so
        // an empty log here means something went wrong rather than "no trace
        // was asked for". Say so rather than drawing an empty scrubber.
        s.push_str("  this solve produced no step log\n");
        return s;
    }
    s.push_str(&format!("  {}\n", scrub_bar(state.timeline.cursor, steps)));
    s.push_str(&format!(
        "  step {} / {}{}\n",
        state.timeline.cursor,
        steps,
        if state.timeline.playing {
            " - playing"
        } else if state.timeline.settled(steps) {
            " - settled"
        } else {
            " - paused"
        }
    ));
    s.push_str(&format!(
        "  T play/restart | Left/Right step (Shift x{COARSE_STEP}) | End settle | \
         +/- speed ({}/frame)\n",
        state.timeline.steps_per_tick
    ));
    if !state.timeline.settled(steps) {
        s.push_str(
            "\n  Observed cells build in authored geometry. The lattice returns\n  \
             for cells not yet observed, and a red ring is a contradiction.\n",
        );
    }
    s
}

/// A fixed-width progress bar. ASCII only: this tool ships no font asset and
/// Bevy's default subset is all it can draw.
fn scrub_bar(cursor: usize, steps: usize) -> String {
    const WIDTH: usize = 44;
    // A traceless solve reads as complete rather than as a bar stuck at zero.
    let filled = (cursor * WIDTH).checked_div(steps).unwrap_or(WIDTH);
    let mut bar = String::with_capacity(WIDTH + 2);
    bar.push('[');
    for slot in 0..WIDTH {
        bar.push(if slot < filled { '=' } else { '.' });
    }
    bar.push(']');
    bar
}

/// What the solver knew about one cell, at the cursor.
///
/// `pruned_to` is *the domain size at the moment propagation last touched this
/// cell*. That is a different and far cheaper quantity than the neighbourhood
/// explorer's number, which re-opens a cell's domain and runs the solver's own
/// AC-3 over the ring — so the label says which one this is. An author who read
/// this as "what could stand here" would be tuning against the wrong figure,
/// and `N` is where that question is answered honestly.
#[must_use]
pub fn describe_cell(state: &StudioState, coord: HexCoord) -> Option<String> {
    let solved = state.solved.as_ref()?;
    let cursor = state.timeline.cursor.min(solved.steps.len());
    let cells = fold_trace(&solved.steps[..cursor]);
    let cell = cells.get(&coord).copied()?;

    let domain = match cell.pruned_to {
        Some(remaining) => format!("domain at collapse {remaining}"),
        None => String::from("domain never narrowed"),
    };
    Some(match cell.resolved {
        Some(placement) => format!("{domain} -> {:?}", placement.archetype),
        None if cell.contradicted => format!("{domain} -> CONTRADICTION"),
        None if cell.site => format!("{domain} -> room site, not yet observed"),
        None => format!("{domain} -> not yet observed"),
    })
}
