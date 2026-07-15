use observed_facility::full_wfc::{
    FullWfcError, ModuleSpace, ObservationFrame, RelayoutProgress, ThresholdKey,
};

use super::movement::look_face;
use super::{
    CALM_MUTATION_TICKS, FullWfcMatch, GameplayEvent, GameplayEventKind, MUTATION_WARNING_TICKS,
    PRESSURED_MUTATION_TICKS, PendingRelayout,
};
use crate::full_wfc::FullWfcGeometrySnapshot;

impl FullWfcMatch {
    pub(super) fn build_observation(&self) -> ObservationFrame {
        let mut frame = ObservationFrame::default();
        for player in self.players.values().filter(|player| !player.escaped) {
            frame.visible_cells.insert(player.cell);
            frame.occupied_cells.insert(player.id, player.cell);
            if self.facility.placements[&player.cell].space == ModuleSpace::Room {
                frame.visible_thresholds.insert(ThresholdKey {
                    room: player.cell,
                    face: look_face(player.yaw, player.pitch),
                });
            }
        }
        self.equipment
            .apply_relayout_pins(&self.facility, &mut frame);
        frame
    }

    pub(super) fn step_mutation(&mut self) {
        let warning_tick = self
            .next_mutation_tick
            .saturating_sub(MUTATION_WARNING_TICKS);
        if self.tick == warning_tick && self.pending_relayout.is_none() {
            self.pending_relayout = Some(PendingRelayout::Solving(
                self.facility.begin_relayout(&self.observation),
            ));
            self.recent_events.push(GameplayEvent::new(
                self.tick,
                GameplayEventKind::MutationWarning,
                None,
                None,
                None,
                None,
            ));
        }
        if let Some(pending) = self.pending_relayout.take() {
            match pending {
                PendingRelayout::Ready(candidate) => {
                    self.pending_relayout = Some(PendingRelayout::Ready(candidate));
                }
                PendingRelayout::Solving(work) => match self.facility.advance_relayout(work, 1) {
                    Ok(RelayoutProgress::Pending(work)) => {
                        self.pending_relayout = Some(PendingRelayout::Solving(work));
                    }
                    Ok(RelayoutProgress::Ready(candidate)) => {
                        self.pending_relayout = Some(PendingRelayout::Ready(candidate));
                    }
                    Err(FullWfcError::RetryBudgetExhausted { attempts }) => {
                        self.facility.reject_relayout(attempts);
                        self.cancel_mutation();
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
        let kind = if self
            .facility
            .commit_relayout(candidate, self.observation.clone())
            .is_ok()
        {
            self.geometry = FullWfcGeometrySnapshot::project(&self.facility);
            self.physics = self.geometry.rapier_scene();
            GameplayEventKind::MutationCommitted
        } else {
            GameplayEventKind::MutationCancelled
        };
        self.recent_events
            .push(GameplayEvent::new(self.tick, kind, None, None, None, None));
        self.schedule_next_mutation();
    }

    fn cancel_mutation(&mut self) {
        self.pending_relayout = None;
        self.recent_events.push(GameplayEvent::new(
            self.tick,
            GameplayEventKind::MutationCancelled,
            None,
            None,
            None,
            None,
        ));
        self.schedule_next_mutation();
    }

    fn schedule_next_mutation(&mut self) {
        let escaped = self.escape_order.len() as f32;
        let pressure = (escaped / self.config.teams.max(1) as f32).clamp(0.0, 1.0);
        let base = CALM_MUTATION_TICKS as f32
            + (PRESSURED_MUTATION_TICKS as f32 - CALM_MUTATION_TICKS as f32) * pressure;
        let jitter = ((self.seed ^ self.tick).rotate_left(17) % 241) as i64 - 120;
        self.next_mutation_tick = self
            .tick
            .saturating_add((base as i64 + jitter).max(MUTATION_WARNING_TICKS as i64 + 1) as u64);
    }
}
