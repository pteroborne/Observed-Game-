//! The solve step: run the current profile, score it, and keep the baseline
//! solve of the same seed beside it for comparison.
//!
//! Debouncing lives in the caller (`update_studio_solve`), not here, so this
//! function stays a pure "solve once, now" that tests can call directly.

use std::time::Instant;

use observed_facility::hex_wfc::profile::ScoreWeights;
use observed_facility::hex_wfc::score::score_layout_with;
use observed_facility::hex_wfc::{
    CandidateOutcome, HexCompositionProfile, HexWfcError, HexWfcWorld, SolveStep,
};
use observed_match::hex_wfc::HexWfcGeometrySnapshot;

use crate::coverage::CoverageReport;
use crate::{SolveResult, StudioState, corpus};

/// Solve, and produce the step trace the timeline needs, honouring the
/// profile's candidate count.
///
/// One candidate is the common case and stays a single traced solve. A real
/// search cannot be traced as it runs - the winner is not known until every
/// candidate has been scored, and tracing all of them to throw away all but one
/// would hold N step logs to use a single one. So the winner is re-solved at its
/// own seed to recover its trace. That is one extra solve on top of N, and it is
/// exact rather than approximate: same seed, same config, same profile, so it
/// reproduces the winning world step for step.
fn solve_and_trace(
    seed: u64,
    state: &StudioState,
) -> Result<(HexWfcWorld, Vec<SolveStep>, Vec<CandidateOutcome>), HexWfcError> {
    if state.profile.search.candidates <= 1 {
        let (world, steps) =
            HexWfcWorld::generate_traced_with_profile(seed, state.config, &state.profile)?;
        let score = score_layout_with(&world, state.profile.score);
        return Ok((
            world,
            steps,
            vec![CandidateOutcome {
                index: 0,
                seed,
                score: Some(score),
                winner: true,
            }],
        ));
    }

    let (world, candidates) =
        HexWfcWorld::generate_searched_with_profile(seed, state.config, None, &state.profile)?;
    let (_, steps) =
        HexWfcWorld::generate_traced_with_profile(world.seed, state.config, &state.profile)?;
    Ok((world, steps, candidates))
}

/// Solve the current seed under the current profile.
///
/// A failed solve deliberately **keeps the previous layout on screen** — a
/// blank viewport reads as a crash, and the interesting case is exactly when an
/// edit makes the facility unsolvable, which is when you most want to still see
/// what you had.
pub fn run_solve(state: &mut StudioState) {
    let seed = state.seed();

    // The baseline solve of this same seed backs both the score delta and the
    // compare overlay. Cached until the seed or config moves.
    if state.baseline_world.is_none() {
        let baseline = HexCompositionProfile::baseline();
        match HexWfcWorld::generate_with_profile(seed, state.config, None, &baseline) {
            Ok(world) => {
                state.baseline_score = Some(score_layout_with(&world, ScoreWeights::baseline()));
                state.baseline_world = Some(world);
            }
            Err(error) => {
                // Not fatal: the authored profile may still solve. Say so rather
                // than leaving an empty delta column unexplained.
                state.status = format!("baseline solve failed for {seed:#x}: {error:?}");
            }
        }
    }

    let started = Instant::now();
    match solve_and_trace(seed, state) {
        Ok((world, steps, candidates)) => {
            #[allow(clippy::cast_possible_truncation)]
            let elapsed_ms = started.elapsed().as_millis() as u32;
            let score = score_layout_with(&world, state.profile.score);
            let attempts = world.last_attempts;

            let (geometry, coverage, projection_note) = match corpus() {
                Ok((cells, rooms)) => {
                    let projected =
                        HexWfcGeometrySnapshot::project_with_rooms(&world, cells, rooms);
                    let (snapshot, error) = match projected {
                        Ok(snapshot) => (Some(snapshot), None),
                        Err(error) => (None, Some(error)),
                    };
                    // Coverage is built from the same world and corpus the
                    // projector just saw, so its prediction and the projector's
                    // verdict are always about the same thing.
                    let coverage = CoverageReport::build(
                        &world,
                        cells,
                        rooms,
                        snapshot.as_ref(),
                        error.as_ref(),
                    );
                    let note = match &error {
                        Some(error) => format!("  |  projection: {error:?}"),
                        None if !coverage.unmet.is_empty() => {
                            format!("  |  {} uncovered demand(s)", coverage.unmet.len())
                        }
                        None => String::new(),
                    };
                    (snapshot, coverage, note)
                }
                Err(detail) => (None, CoverageReport::default(), format!("  |  {detail}")),
            };

            let search_note = if candidates.len() > 1 {
                format!("  |  best of {}", candidates.len())
            } else {
                String::new()
            };
            state.status = format!(
                "seed {seed:#x} solved in {elapsed_ms}ms, \
                 {attempts} attempt(s){search_note}{projection_note}"
            );
            state.solved = Some(SolveResult {
                world,
                steps,
                score,
                attempts,
                elapsed_ms,
                geometry,
                coverage,
                candidates,
            });
            state.solve_dirty = false;
        }
        Err(error) => {
            #[allow(clippy::cast_possible_truncation)]
            let elapsed_ms = started.elapsed().as_millis() as u32;
            state.solve_dirty = false;

            // When pins are authored, a bare `RetryBudgetExhausted` names the
            // budget rather than the mistake. Attribution costs a run of solves,
            // which is exactly affordable on a failure an author is staring at,
            // and it answers the question they actually have: *which pin?*
            let attributed = if state.profile.pin_sets.is_empty() {
                None
            } else {
                state.refresh_pin_diagnostics();
                state
                    .pin_diagnostics
                    .iter()
                    .find(|diagnostic| {
                        !matches!(
                            diagnostic,
                            observed_facility::hex_wfc::PinDiagnostic::BlueprintCollision { .. }
                                | observed_facility::hex_wfc::PinDiagnostic::Shadowed { .. }
                        )
                    })
                    .map(ToString::to_string)
            };

            state.status = match attributed {
                Some(detail) => format!(
                    "seed {seed:#x} FAILED after {elapsed_ms}ms - PIN: {detail} \
                     (showing previous layout)"
                ),
                None => format!(
                    "seed {seed:#x} FAILED after {elapsed_ms}ms: {error:?} (showing previous layout)"
                ),
            };
        }
    }
}
