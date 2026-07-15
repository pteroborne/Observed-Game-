//! Bounded fallback for a topological WFC retry-budget exhaustion.
//!
//! An architecture swap gives one unseen module a new physical identity while
//! preserving every opening. It is narrower than a full collapse and runs only
//! after the deterministic topology retry budget is exhausted.

use std::collections::BTreeMap;

use observed_content::ArchitectureRegister;

use super::collapse::collapse_attempts;
use super::config::AttemptRange;
use super::constraints::all_coords;
use super::topology::pinned_cells;
use super::{
    CellCoord, FullWfcConfig, FullWfcError, FullWfcWorld, ModuleInstanceId, ModulePlacement,
    ModuleSpace, ObservationFrame, RelayoutCandidate,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayoutWork {
    generation: u32,
    constraints: ObservationFrame,
    next_attempt: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelayoutProgress {
    Pending(RelayoutWork),
    Ready(RelayoutCandidate),
}

impl FullWfcWorld {
    pub fn begin_relayout(&self, observation: &ObservationFrame) -> RelayoutWork {
        let mut constraints = observation.clone();
        constraints
            .visible_cells
            .extend(self.landmark_cells.iter().copied());
        RelayoutWork {
            generation: self.generation.wrapping_add(1),
            constraints,
            next_attempt: 0,
        }
    }

    pub fn advance_relayout(
        &self,
        mut work: RelayoutWork,
        attempt_budget: u32,
    ) -> Result<RelayoutProgress, FullWfcError> {
        if work.generation != self.generation.wrapping_add(1) {
            return Err(FullWfcError::StaleCandidate);
        }
        let solved = collapse_attempts(
            self.seed,
            work.generation,
            self.config,
            Some(&self.placements),
            &work.constraints,
            &self.reserved_exit_faces,
            AttemptRange {
                start: work.next_attempt,
                budget: attempt_budget.max(1),
            },
        );
        let (placements, attempts) = match solved {
            Ok(result) => result,
            Err(FullWfcError::RetryBudgetExhausted { attempts })
                if attempts < self.config.retry_budget =>
            {
                work.next_attempt = attempts;
                return Ok(RelayoutProgress::Pending(work));
            }
            Err(FullWfcError::RetryBudgetExhausted { attempts }) => {
                let placements = fallback_geometry_relayout(
                    self.seed,
                    work.generation,
                    self.config,
                    &self.placements,
                    &work.constraints,
                )
                .ok_or(FullWfcError::RetryBudgetExhausted { attempts })?;
                (placements, attempts)
            }
            Err(error) => return Err(error),
        };
        let changed_cells = all_coords(self.config)
            .filter(|coord| self.placements.get(coord) != placements.get(coord))
            .collect();
        Ok(RelayoutProgress::Ready(RelayoutCandidate {
            generation: work.generation,
            placements,
            changed_cells,
            attempts,
        }))
    }

    pub fn propose_relayout(
        &self,
        observation: &ObservationFrame,
    ) -> Result<RelayoutCandidate, FullWfcError> {
        match self.advance_relayout(self.begin_relayout(observation), self.config.retry_budget)? {
            RelayoutProgress::Ready(candidate) => Ok(candidate),
            RelayoutProgress::Pending(_) => unreachable!("full retry budget always terminates"),
        }
    }
}

pub(super) fn fallback_geometry_relayout(
    seed: u64,
    generation: u32,
    config: FullWfcConfig,
    previous: &BTreeMap<CellCoord, ModulePlacement>,
    observation: &ObservationFrame,
) -> Option<BTreeMap<CellCoord, ModulePlacement>> {
    let pins = pinned_cells(config, previous, observation);
    let candidates = previous
        .iter()
        .filter_map(|(&coord, placement)| {
            (placement.space != ModuleSpace::Void && !pins.contains(&coord)).then_some(coord)
        })
        .collect::<Vec<_>>();
    let index = (seed.rotate_left(generation % 63) as usize ^ generation as usize)
        % candidates.len().max(1);
    let &selected = candidates.get(index)?;
    let mut placements = previous.clone();
    let placement = placements.get_mut(&selected).expect("candidate exists");
    let register_index = ArchitectureRegister::ALL
        .iter()
        .position(|register| *register == placement.architecture)
        .unwrap_or(0);
    placement.architecture =
        ArchitectureRegister::ALL[(register_index + 1) % ArchitectureRegister::ALL.len()];
    placement.instance = ModuleInstanceId {
        generation,
        key: placement.instance.key
            ^ seed.rotate_left(u32::from(selected.level) + 7)
            ^ u64::from(generation).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ 0xA11C_EF01_DBAC_0FF5,
    };
    Some(placements)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::full_wfc::FullWfcWorld;

    #[test]
    fn fallback_changes_one_unpinned_identity_without_changing_topology() {
        let world = FullWfcWorld::new(81, FullWfcConfig::default()).expect("world");
        let mut observation = ObservationFrame::default();
        observation
            .visible_cells
            .extend(world.landmark_cells.iter());
        let next = fallback_geometry_relayout(
            world.seed,
            world.generation + 1,
            world.config,
            &world.placements,
            &observation,
        )
        .expect("unseen fabric remains");
        let changed = world
            .placements
            .iter()
            .filter(|(coord, placement)| {
                next[coord].geometry_identity() != placement.geometry_identity()
            })
            .count();
        assert_eq!(changed, 1);
        for (&coord, before) in &world.placements {
            assert_eq!(before.openings, next[&coord].openings);
            assert_eq!(before.space, next[&coord].space);
        }
    }

    #[test]
    fn relayout_attempts_can_advance_deterministically_across_ticks() {
        let (world, mut work) = (0..100u64)
            .find_map(|seed| {
                let world = FullWfcWorld::new(seed, FullWfcConfig::default()).ok()?;
                let work = world.begin_relayout(&ObservationFrame::default());
                match world.advance_relayout(work, 1).ok()? {
                    RelayoutProgress::Pending(work) => Some((world, work)),
                    RelayoutProgress::Ready(_) => None,
                }
            })
            .expect("corpus contains a multi-attempt solve");
        loop {
            match world.advance_relayout(work, 1).expect("chunk") {
                RelayoutProgress::Pending(next) => work = next,
                RelayoutProgress::Ready(candidate) => {
                    assert!(candidate.attempts <= world.config.retry_budget);
                    break;
                }
            }
        }
    }
}
