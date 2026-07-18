//! Incremental observation-safe relayout for the hex facility.
//!
//! Each call to [`HexWfcWorld::advance_relayout`] executes exactly one seeded
//! collapse attempt. Dropping [`HexRelayoutWork`] cancels it without mutating
//! the world; committing re-checks the latest pins and routes.

use std::collections::{BTreeMap, BTreeSet};

use observed_content::ArchitectureRegister;
use observed_core::{PlayerId, SplitMix};
use observed_hex::HexCoord;

use super::blueprint::StampedBlueprint;
use super::collapse::collapse_relayout_attempt;
use super::topology::{change_is_safe, pinned_cells};
use super::{HexPlacement, HexSpace, HexWfcConfig, HexWfcError, HexWfcWorld};

/// Stable identity of one named room threshold.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HexThresholdKey {
    /// Stable blueprint identity, independent of vector order and relayout
    /// generation when the same room survives.
    pub room_generation_key: u64,
    pub port: &'static str,
}

/// One simulation-frame observation projection. It contains domain IDs and
/// coordinates only; visibility remains presentation-owned.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HexObservationFrame {
    pub visible_cells: BTreeSet<HexCoord>,
    pub visible_thresholds: BTreeSet<HexThresholdKey>,
    pub occupied_cells: BTreeMap<PlayerId, HexCoord>,
    pub landmark_cells: BTreeSet<HexCoord>,
    /// Remaining objectives whose routes to the exit must survive a commit.
    pub objective_cells: BTreeSet<HexCoord>,
}

/// Cancellable incremental relayout state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexRelayoutWork {
    base_seed: u64,
    base_config: HexWfcConfig,
    generation: u32,
    constraints: HexObservationFrame,
    pins: BTreeSet<HexCoord>,
    locked_blueprints: Vec<StampedBlueprint>,
    next_attempt: u32,
}

impl HexRelayoutWork {
    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }

    #[must_use]
    pub const fn base_seed(&self) -> u64 {
        self.base_seed
    }

    #[must_use]
    pub const fn next_attempt(&self) -> u32 {
        self.next_attempt
    }

    #[must_use]
    pub fn pinned_cells(&self) -> &BTreeSet<HexCoord> {
        &self.pins
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexRelayoutCandidate {
    pub base_seed: u64,
    pub base_config: HexWfcConfig,
    pub generation: u32,
    pub placements: BTreeMap<HexCoord, HexPlacement>,
    pub blueprints: Vec<StampedBlueprint>,
    pub architecture: BTreeMap<HexCoord, ArchitectureRegister>,
    pub changed_cells: BTreeSet<HexCoord>,
    /// Number of topological attempts consumed. The fallback reports the full
    /// retry budget even though it is a port-preserving architecture change.
    pub attempts: u32,
    pub used_fallback: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HexRelayoutProgress {
    Pending(HexRelayoutWork),
    Ready(HexRelayoutCandidate),
}

impl HexWfcWorld {
    #[must_use]
    pub fn begin_relayout(&self, observation: &HexObservationFrame) -> HexRelayoutWork {
        let pins = pinned_cells(self.config, &self.placements, &self.blueprints, observation);
        let locked_blueprints = self
            .blueprints
            .iter()
            .filter(|blueprint| blueprint.cells.iter().any(|cell| pins.contains(cell)))
            .cloned()
            .collect();
        HexRelayoutWork {
            base_seed: self.seed,
            base_config: self.config,
            generation: self.generation.wrapping_add(1),
            constraints: observation.clone(),
            pins,
            locked_blueprints,
            next_attempt: 0,
        }
    }

    /// Execute exactly one deterministic topological attempt.
    pub fn advance_relayout(
        &self,
        mut work: HexRelayoutWork,
    ) -> Result<HexRelayoutProgress, HexWfcError> {
        if work.base_seed != self.seed
            || work.base_config != self.config
            || work.generation != self.generation.wrapping_add(1)
        {
            return Err(HexWfcError::StaleCandidate);
        }
        let attempt = work.next_attempt;
        match collapse_relayout_attempt(
            self.seed,
            work.generation,
            attempt,
            self.config,
            &self.placements,
            &work.pins,
            &work.locked_blueprints,
        ) {
            Ok(solved) => {
                let architecture = architecture_for_generation(
                    self.seed,
                    work.generation,
                    &solved.placements,
                    &self.architecture,
                    &work.pins,
                );
                Ok(HexRelayoutProgress::Ready(make_candidate(
                    self,
                    work.generation,
                    solved.placements,
                    solved.blueprints,
                    architecture,
                    attempt + 1,
                    false,
                )))
            }
            Err(_) if attempt + 1 < self.config.retry_budget => {
                work.next_attempt += 1;
                Ok(HexRelayoutProgress::Pending(work))
            }
            Err(last_failure) => fallback_geometry_relayout(
                self,
                work.generation,
                &work.pins,
                self.config.retry_budget,
            )
            .map(HexRelayoutProgress::Ready)
            .ok_or(HexWfcError::RetryBudgetExhausted {
                attempts: self.config.retry_budget,
                last_failure: Some(last_failure),
            }),
        }
    }

    pub fn propose_relayout(
        &self,
        observation: &HexObservationFrame,
    ) -> Result<HexRelayoutCandidate, HexWfcError> {
        let mut work = self.begin_relayout(observation);
        loop {
            match self.advance_relayout(work)? {
                HexRelayoutProgress::Pending(next) => work = next,
                HexRelayoutProgress::Ready(candidate) => return Ok(candidate),
            }
        }
    }

    /// Revalidate against the latest observation and atomically accept a
    /// candidate. No logical or presentation state changes on rejection.
    pub fn commit_relayout(
        &mut self,
        candidate: HexRelayoutCandidate,
        latest: &HexObservationFrame,
    ) -> Result<(), HexWfcError> {
        if candidate.base_seed != self.seed
            || candidate.base_config != self.config
            || candidate.generation != self.generation.wrapping_add(1)
        {
            return Err(HexWfcError::StaleCandidate);
        }
        let pins = pinned_cells(self.config, &self.placements, &self.blueprints, latest);
        for &coord in &pins {
            if !change_is_safe(
                coord,
                &self.placements[&coord],
                &candidate.placements[&coord],
                &pins,
            ) || self.architecture[&coord] != candidate.architecture[&coord]
            {
                return Err(HexWfcError::UnsafeChange(coord));
            }
        }
        for blueprint in self
            .blueprints
            .iter()
            .filter(|blueprint| blueprint.cells.iter().any(|cell| pins.contains(cell)))
        {
            if !candidate.blueprints.contains(blueprint) {
                return Err(HexWfcError::UnsafeChange(blueprint.anchor));
            }
        }
        for (&player, &coord) in &latest.occupied_cells {
            if super::topology::route_between(
                self.config,
                &candidate.placements,
                coord,
                self.config.exit(),
            )
            .is_none()
            {
                return Err(HexWfcError::MissingPlayerRoute(player));
            }
        }
        for &coord in &latest.objective_cells {
            if super::topology::route_between(
                self.config,
                &candidate.placements,
                coord,
                self.config.exit(),
            )
            .is_none()
            {
                return Err(HexWfcError::MissingObjectiveRoute(coord));
            }
        }
        self.generation = candidate.generation;
        self.placements = candidate.placements;
        self.blueprints = candidate.blueprints;
        self.architecture = candidate.architecture;
        self.last_attempts = candidate.attempts;
        Ok(())
    }

    /// `(pinned, free)` lattice-cell counts for decoherence-yield measurement.
    #[must_use]
    pub fn decoherence_yield(&self, observation: &HexObservationFrame) -> (usize, usize) {
        let pins = pinned_cells(self.config, &self.placements, &self.blueprints, observation);
        (pins.len(), self.placements.len().saturating_sub(pins.len()))
    }
}

fn make_candidate(
    world: &HexWfcWorld,
    generation: u32,
    placements: BTreeMap<HexCoord, HexPlacement>,
    blueprints: Vec<StampedBlueprint>,
    architecture: BTreeMap<HexCoord, ArchitectureRegister>,
    attempts: u32,
    used_fallback: bool,
) -> HexRelayoutCandidate {
    let changed_cells = placements
        .iter()
        .filter_map(|(&coord, placement)| {
            (world.placements.get(&coord) != Some(placement)
                || world.architecture.get(&coord) != architecture.get(&coord))
            .then_some(coord)
        })
        .collect();
    HexRelayoutCandidate {
        base_seed: world.seed,
        base_config: world.config,
        generation,
        placements,
        blueprints,
        architecture,
        changed_cells,
        attempts,
        used_fallback,
    }
}

pub(super) fn initial_architecture(
    seed: u64,
    placements: &BTreeMap<HexCoord, HexPlacement>,
) -> BTreeMap<HexCoord, ArchitectureRegister> {
    placements
        .keys()
        .copied()
        .map(|coord| (coord, register_for(seed, 0, coord)))
        .collect()
}

fn architecture_for_generation(
    seed: u64,
    generation: u32,
    placements: &BTreeMap<HexCoord, HexPlacement>,
    previous: &BTreeMap<HexCoord, ArchitectureRegister>,
    pins: &BTreeSet<HexCoord>,
) -> BTreeMap<HexCoord, ArchitectureRegister> {
    placements
        .keys()
        .copied()
        .map(|coord| {
            let register = if pins.contains(&coord) {
                previous[&coord]
            } else {
                register_for(seed, generation, coord)
            };
            (coord, register)
        })
        .collect()
}

fn register_for(seed: u64, generation: u32, coord: HexCoord) -> ArchitectureRegister {
    let coord_key =
        u64::from(coord.q) | (u64::from(coord.r) << 16) | (u64::from(coord.level) << 32);
    let mut rng = SplitMix::new(
        seed ^ coord_key.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ u64::from(generation).wrapping_mul(0xBF58_476D_1CE4_E5B9),
    );
    ArchitectureRegister::ALL[(rng.next_u64() % ArchitectureRegister::ALL.len() as u64) as usize]
}

pub(super) fn fallback_geometry_relayout(
    world: &HexWfcWorld,
    generation: u32,
    pins: &BTreeSet<HexCoord>,
    attempts: u32,
) -> Option<HexRelayoutCandidate> {
    let candidates = world
        .placements
        .iter()
        .filter_map(|(&coord, placement)| {
            (placement.space != HexSpace::Void && !pins.contains(&coord)).then_some(coord)
        })
        .collect::<Vec<_>>();
    let index = (world.seed.rotate_left(generation % 63) as usize ^ generation as usize)
        % candidates.len().max(1);
    let &selected = candidates.get(index)?;
    let mut architecture = world.architecture.clone();
    let register = architecture
        .get_mut(&selected)
        .expect("fallback candidate has a register");
    let register_index = ArchitectureRegister::ALL
        .iter()
        .position(|candidate| candidate == register)
        .unwrap_or(0);
    *register = ArchitectureRegister::ALL[(register_index + 1) % ArchitectureRegister::ALL.len()];
    Some(make_candidate(
        world,
        generation,
        world.placements.clone(),
        world.blueprints.clone(),
        architecture,
        attempts,
        true,
    ))
}

pub(super) const fn fold_generation_key(key: u64) -> u32 {
    (key ^ (key >> 32)) as u32
}
