//! Mid-match observation-safe local relayout state machine.

use std::collections::BTreeSet;

use observed_facility::hex_wfc::{HexRelayoutCandidate, HexRelayoutProgress};

use super::{
    HexMatchEvent, HexMatchEventKind, HexWfcMatch, MAX_MUTATION_TICKS, MIN_MUTATION_TICKS,
    MUTATION_WARNING_TICKS, PendingRelayout,
};

impl HexWfcMatch {
    /// Advance at most one seeded collapse attempt this tick, then atomically
    /// apply the bounded logical/geometry/physics delta at the scheduled tick.
    pub(super) fn step_mutation(&mut self) {
        let warning_tick = self
            .next_mutation_tick
            .saturating_sub(MUTATION_WARNING_TICKS);
        if self.tick == warning_tick && self.pending_relayout.is_none() {
            let frontier = self
                .map_knowledge
                .values()
                .flat_map(|knowledge| knowledge.cells.keys().copied())
                .collect::<BTreeSet<_>>();
            self.pending_relayout = Some(PendingRelayout::Solving(
                self.facility
                    .begin_frontier_relayout(&self.observation, &frontier),
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

    fn commit_mutation(&mut self, candidate: HexRelayoutCandidate) -> HexMatchEventKind {
        let logical = match self
            .facility
            .commit_relayout_delta(candidate, &self.observation)
        {
            Ok(delta) => delta,
            Err(_) => return HexMatchEventKind::MutationCancelled,
        };
        let geometry = match self.geometry.project_delta_with_rooms(
            &self.facility,
            &logical,
            &self.prototypes,
            &self.room_prototypes,
        ) {
            Ok(delta) => delta,
            Err(_) => {
                self.facility
                    .revert_relayout_delta(logical)
                    .expect("the just-accepted logical delta is revertible");
                return HexMatchEventKind::MutationCancelled;
            }
        };
        if self
            .physics
            .apply_collider_delta(&geometry.colliders)
            .is_err()
        {
            self.facility
                .revert_relayout_delta(logical)
                .expect("the just-accepted logical delta is revertible");
            return HexMatchEventKind::MutationCancelled;
        }
        self.geometry
            .apply_delta(&geometry)
            .expect("geometry delta was projected from this snapshot");
        self.last_relayout_delta = Some(logical);
        HexMatchEventKind::MutationCommitted
    }

    fn cancel_mutation(&mut self) {
        self.pending_relayout = None;
        self.push_mutation_event(HexMatchEventKind::MutationCancelled);
        self.schedule_next_mutation();
    }

    fn schedule_next_mutation(&mut self) {
        self.next_mutation_tick = scheduled_mutation_tick(self.seed, self.tick);
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

pub(super) fn scheduled_mutation_tick(seed: u64, from_tick: u64) -> u64 {
    let span = MAX_MUTATION_TICKS - MIN_MUTATION_TICKS + 1;
    let mixed = (seed ^ from_tick.wrapping_mul(0x9E37_79B9_7F4A_7C15)).rotate_left(17);
    from_tick.saturating_add(MIN_MUTATION_TICKS + mixed % span)
}
