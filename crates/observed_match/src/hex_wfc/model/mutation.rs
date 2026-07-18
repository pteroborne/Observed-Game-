//! Mid-match observation-safe relayout for the hex facility.
//!
//! This mirrors the square lattice's `full_wfc/model/mutation.rs` state machine:
//! at a deterministic warning tick the match begins a relayout keyed on the
//! current [`HexObservationFrame`](observed_facility::hex_wfc::HexObservationFrame)
//! (built by [`HexWfcMatch::build_observation`]); it advances one seeded collapse
//! attempt per tick; and at the scheduled commit tick it either commits — bumping
//! `facility.generation`, re-projecting geometry and physics — or cancels. Every
//! decision is a pure function of `(seed, tick, observation)`, so the same seed and
//! scripted inputs reproduce the same generation timeline byte-for-byte.

use observed_facility::hex_wfc::{HexRelayoutCandidate, HexRelayoutProgress};

use super::{
    CALM_MUTATION_TICKS, HexMatchEvent, HexMatchEventKind, HexWfcGeometrySnapshot, HexWfcMatch,
    MUTATION_WARNING_TICKS, PRESSURED_MUTATION_TICKS, PendingRelayout,
};

impl HexWfcMatch {
    /// Advance the relayout cycle exactly one tick. Called once per `step` after
    /// the observation frame for this tick has been rebuilt.
    pub(super) fn step_mutation(&mut self) {
        let warning_tick = self
            .next_mutation_tick
            .saturating_sub(MUTATION_WARNING_TICKS);
        if self.tick == warning_tick && self.pending_relayout.is_none() {
            self.pending_relayout = Some(PendingRelayout::Solving(
                self.facility.begin_relayout(&self.observation),
            ));
            self.push_mutation_event(HexMatchEventKind::MutationWarning);
        }

        if let Some(pending) = self.pending_relayout.take() {
            match pending {
                PendingRelayout::Ready(candidate) => {
                    self.pending_relayout = Some(PendingRelayout::Ready(candidate));
                }
                PendingRelayout::Solving(work) => match self.facility.advance_relayout(work) {
                    Ok(HexRelayoutProgress::Pending(work)) => {
                        self.pending_relayout = Some(PendingRelayout::Solving(work));
                    }
                    Ok(HexRelayoutProgress::Ready(candidate)) => {
                        self.pending_relayout = Some(PendingRelayout::Ready(candidate));
                    }
                    Err(_) => self.cancel_mutation(),
                },
            }
        }

        if self.tick != self.next_mutation_tick {
            return;
        }
        let candidate = match self.pending_relayout.take() {
            Some(PendingRelayout::Ready(candidate)) => candidate,
            Some(PendingRelayout::Solving(_)) | None => {
                self.cancel_mutation();
                return;
            }
        };
        let kind = self.commit_mutation(candidate);
        self.push_mutation_event(kind);
        self.schedule_next_mutation();
    }

    /// Commit a solved candidate against the latest observation, or reject it.
    ///
    /// The commit is staged on a clone of the facility so that a candidate that
    /// re-validates but no longer projects (a register/signature the corpus does
    /// not cover) leaves the live facility and geometry untouched — never a
    /// facility/geometry generation mismatch.
    fn commit_mutation(&mut self, candidate: HexRelayoutCandidate) -> HexMatchEventKind {
        let mut probe = self.facility.clone();
        if probe.commit_relayout(candidate, &self.observation).is_err() {
            return HexMatchEventKind::MutationCancelled;
        }
        match HexWfcGeometrySnapshot::project(&probe, &self.prototypes) {
            Ok(geometry) => {
                self.physics = geometry.rapier_scene();
                self.geometry = geometry;
                self.facility = probe;
                HexMatchEventKind::MutationCommitted
            }
            Err(_) => HexMatchEventKind::MutationCancelled,
        }
    }

    fn cancel_mutation(&mut self) {
        self.pending_relayout = None;
        self.push_mutation_event(HexMatchEventKind::MutationCancelled);
        self.schedule_next_mutation();
    }

    /// Schedule the next commit tick. Spacing tightens with escape pressure, plus
    /// a seed/tick-derived jitter so cycles are aperiodic yet fully deterministic.
    fn schedule_next_mutation(&mut self) {
        let escaped = self.escape_order.len() as f32;
        let pressure = (escaped / self.players.len().max(1) as f32).clamp(0.0, 1.0);
        let base = CALM_MUTATION_TICKS as f32
            + (PRESSURED_MUTATION_TICKS as f32 - CALM_MUTATION_TICKS as f32) * pressure;
        let jitter = ((self.seed ^ self.tick).rotate_left(17) % 241) as i64 - 120;
        self.next_mutation_tick = self
            .tick
            .saturating_add((base as i64 + jitter).max(MUTATION_WARNING_TICKS as i64 + 1) as u64);
    }

    fn push_mutation_event(&mut self, kind: HexMatchEventKind) {
        self.recent_events.push(HexMatchEvent {
            tick: self.tick,
            kind,
            player: None,
            cell: None,
        });
    }
}
