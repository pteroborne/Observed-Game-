//! Incremental, observation-safe local relayout for the hex facility.
//!
//! A relayout selects a bounded pocket, freezes every cell outside it, and
//! executes at most one seeded WFC attempt per call to
//! [`HexWfcWorld::advance_relayout`]. The solver still uses the proven global
//! constraint tables, but its complete result is reduced immediately to a
//! pocket patch. Commit applies and, on validation failure, rolls back only
//! that bounded patch.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use observed_content::ArchitectureRegister;
use observed_core::{PlayerId, SplitMix};
use observed_hex::{HexCoord, HexFace, travel_distance};

use super::blueprint::StampedBlueprint;
use super::collapse::collapse_pocket_attempt;
use super::topology::pinned_cells;
use super::{HexArchetype, HexPlacement, HexSpace, HexWfcConfig, HexWfcError, HexWfcWorld};

pub const DEFAULT_MUTATION_TARGET_CELLS: usize = 32;
pub const DEFAULT_MUTATION_MAX_CELLS: usize = 64;

/// Stable identity of one named room threshold.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HexThresholdKey {
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

/// The exact bounded unit selected for one mutation. `protected_cells`
/// includes observation pins and their one-cell safety halo; `boundary_cells`
/// are frozen neighbors whose live port signatures constrain the pocket.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexMutationRegion {
    pub cells: BTreeSet<HexCoord>,
    pub boundary_cells: BTreeSet<HexCoord>,
    pub protected_cells: BTreeSet<HexCoord>,
}

/// Cancellable incremental relayout state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexRelayoutWork {
    base_seed: u64,
    base_config: HexWfcConfig,
    generation: u32,
    region: HexMutationRegion,
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

    /// Observation pins plus the safety halo. Kept as the compatibility view
    /// of "pins" rather than exposing the much larger outside-region solver
    /// freeze set.
    #[must_use]
    pub fn pinned_cells(&self) -> &BTreeSet<HexCoord> {
        &self.region.protected_cells
    }

    #[must_use]
    pub const fn region(&self) -> &HexMutationRegion {
        &self.region
    }
}

/// A bounded candidate. Placement and architecture maps contain the region
/// only; blueprints contain only complete instances wholly inside it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexRelayoutCandidate {
    pub base_seed: u64,
    pub base_config: HexWfcConfig,
    pub generation: u32,
    pub region: HexMutationRegion,
    pub placements: BTreeMap<HexCoord, HexPlacement>,
    pub blueprints: Vec<StampedBlueprint>,
    pub architecture: BTreeMap<HexCoord, ArchitectureRegister>,
    pub changed_cells: BTreeSet<HexCoord>,
    pub attempts: u32,
    pub used_fallback: bool,
}

/// Accepted logical delta. It is sufficient for geometry, map-knowledge, and
/// replay consumers without carrying a second facility snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexRelayoutDelta {
    pub previous_generation: u32,
    pub generation: u32,
    pub previous_attempts: u32,
    pub region: HexMutationRegion,
    pub changed_cells: BTreeSet<HexCoord>,
    pub placements: BTreeMap<HexCoord, HexPlacement>,
    pub architecture: BTreeMap<HexCoord, ArchitectureRegister>,
    pub cell_revisions: BTreeMap<HexCoord, u32>,
    pub previous_placements: BTreeMap<HexCoord, HexPlacement>,
    pub previous_architecture: BTreeMap<HexCoord, ArchitectureRegister>,
    pub previous_cell_revisions: BTreeMap<HexCoord, u32>,
    /// Blueprint lists are tiny room-instance catalogues (not cell geometry);
    /// retaining their exact order makes rollback byte-identical.
    pub previous_blueprints: Vec<StampedBlueprint>,
    pub removed_blueprints: Vec<StampedBlueprint>,
    pub upserted_blueprints: Vec<StampedBlueprint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HexRelayoutProgress {
    Pending(HexRelayoutWork),
    Ready(HexRelayoutCandidate),
}

impl HexWfcWorld {
    /// Select a deterministic pocket using the current observed cells as the
    /// frontier hint. Call [`Self::begin_frontier_relayout`] when a simulation
    /// owns richer discovered-territory knowledge.
    #[must_use]
    pub fn begin_relayout(&self, observation: &HexObservationFrame) -> HexRelayoutWork {
        let mut frontier = observation.visible_cells.clone();
        frontier.extend(observation.occupied_cells.values().copied());
        frontier.extend(observation.landmark_cells.iter().copied());
        self.begin_frontier_relayout(observation, &frontier)
    }

    /// Select the target-32/hard-max-64 pocket nearest the supplied exploration
    /// frontier. Complete blueprints, ramp pairs, and connected shaft columns
    /// are indivisible.
    #[must_use]
    pub fn begin_frontier_relayout(
        &self,
        observation: &HexObservationFrame,
        frontier: &BTreeSet<HexCoord>,
    ) -> HexRelayoutWork {
        let protected = protected_with_halo(self, observation);
        let region = select_region(
            self,
            frontier,
            protected,
            DEFAULT_MUTATION_TARGET_CELLS,
            DEFAULT_MUTATION_MAX_CELLS,
        );
        HexRelayoutWork {
            base_seed: self.seed,
            base_config: self.config,
            generation: self.generation.wrapping_add(1),
            region,
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
        if work.region.cells.is_empty() {
            return Err(HexWfcError::NoMutationRegion);
        }
        let attempt = work.next_attempt;
        match collapse_pocket_attempt(
            self.seed,
            work.generation,
            attempt,
            self.config,
            &self.placements,
            &self.blueprints,
            &work.region,
        ) {
            Ok(solved) => Ok(HexRelayoutProgress::Ready(make_candidate(
                self,
                work.generation,
                work.region,
                &solved.placements,
                &solved.blueprints,
                attempt + 1,
                false,
            ))),
            Err(_) if attempt + 1 < self.config.retry_budget => {
                work.next_attempt += 1;
                Ok(HexRelayoutProgress::Pending(work))
            }
            Err(last_failure) => fallback_geometry_relayout(
                self,
                work.generation,
                &work.region,
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

    /// Compatibility commit that discards the accepted delta.
    pub fn commit_relayout(
        &mut self,
        candidate: HexRelayoutCandidate,
        latest: &HexObservationFrame,
    ) -> Result<(), HexWfcError> {
        self.commit_relayout_delta(candidate, latest).map(|_| ())
    }

    /// Revalidate and atomically accept a bounded patch. A failed route or
    /// boundary check restores the previous pocket before returning.
    pub fn commit_relayout_delta(
        &mut self,
        candidate: HexRelayoutCandidate,
        latest: &HexObservationFrame,
    ) -> Result<HexRelayoutDelta, HexWfcError> {
        if candidate.base_seed != self.seed
            || candidate.base_config != self.config
            || candidate.generation != self.generation.wrapping_add(1)
        {
            return Err(HexWfcError::StaleCandidate);
        }
        if candidate.region.cells.is_empty()
            || candidate.region.cells.len() > DEFAULT_MUTATION_MAX_CELLS
            || candidate
                .placements
                .keys()
                .copied()
                .collect::<BTreeSet<_>>()
                != candidate.region.cells
            || candidate
                .architecture
                .keys()
                .copied()
                .collect::<BTreeSet<_>>()
                != candidate.region.cells
        {
            return Err(HexWfcError::NoMutationRegion);
        }

        let latest_protected = protected_with_halo(self, latest);
        if let Some(&unsafe_cell) = candidate
            .region
            .cells
            .iter()
            .find(|coord| latest_protected.contains(coord))
        {
            return Err(HexWfcError::UnsafeChange(unsafe_cell));
        }

        let old_placements = candidate
            .region
            .cells
            .iter()
            .map(|&coord| (coord, self.placements[&coord]))
            .collect::<BTreeMap<_, _>>();
        let old_architecture = candidate
            .region
            .cells
            .iter()
            .map(|&coord| (coord, self.architecture[&coord]))
            .collect::<BTreeMap<_, _>>();
        let old_blueprints = self.blueprints.clone();

        for (&coord, &placement) in &candidate.placements {
            self.placements.insert(coord, placement);
        }
        for (&coord, &register) in &candidate.architecture {
            self.architecture.insert(coord, register);
        }
        self.blueprints.retain(|blueprint| {
            !blueprint
                .cells
                .iter()
                .any(|cell| candidate.region.cells.contains(cell))
        });
        self.blueprints.extend(candidate.blueprints.iter().cloned());

        let validation = self.validate_patch_routes_and_boundary(&candidate, latest);
        if let Err(error) = validation {
            for (coord, placement) in old_placements {
                self.placements.insert(coord, placement);
            }
            for (coord, register) in old_architecture {
                self.architecture.insert(coord, register);
            }
            self.blueprints = old_blueprints;
            return Err(error);
        }

        let previous_generation = self.generation;
        let previous_attempts = self.last_attempts;
        self.generation = candidate.generation;
        self.last_attempts = candidate.attempts;
        let cell_revisions = candidate
            .changed_cells
            .iter()
            .map(|&coord| {
                let revision = self.cell_revisions.entry(coord).or_default();
                *revision = revision.wrapping_add(1);
                (coord, *revision)
            })
            .collect();
        let removed_blueprints = old_blueprints
            .iter()
            .filter(|blueprint| {
                blueprint
                    .cells
                    .iter()
                    .any(|cell| candidate.region.cells.contains(cell))
            })
            .cloned()
            .collect();
        let previous_cell_revisions = candidate
            .changed_cells
            .iter()
            .map(|&coord| (coord, self.cell_revisions[&coord].wrapping_sub(1)))
            .collect();
        Ok(HexRelayoutDelta {
            previous_generation,
            generation: self.generation,
            previous_attempts,
            region: candidate.region,
            changed_cells: candidate.changed_cells.clone(),
            placements: candidate
                .placements
                .into_iter()
                .filter(|(coord, _)| candidate.changed_cells.contains(coord))
                .collect(),
            architecture: candidate
                .architecture
                .into_iter()
                .filter(|(coord, _)| candidate.changed_cells.contains(coord))
                .collect(),
            cell_revisions,
            previous_placements: old_placements,
            previous_architecture: old_architecture,
            previous_cell_revisions,
            previous_blueprints: old_blueprints,
            removed_blueprints,
            upserted_blueprints: candidate.blueprints,
        })
    }

    /// Roll back the most recently accepted delta. This supports atomic
    /// facility/geometry commits when authored projection rejects a patch.
    pub fn revert_relayout_delta(&mut self, delta: HexRelayoutDelta) -> Result<(), HexWfcError> {
        if self.generation != delta.generation
            || delta.previous_generation.wrapping_add(1) != delta.generation
        {
            return Err(HexWfcError::StaleCandidate);
        }
        for (coord, placement) in delta.previous_placements {
            self.placements.insert(coord, placement);
        }
        for (coord, register) in delta.previous_architecture {
            self.architecture.insert(coord, register);
        }
        for (coord, revision) in delta.previous_cell_revisions {
            self.cell_revisions.insert(coord, revision);
        }
        self.blueprints = delta.previous_blueprints;
        self.generation = delta.previous_generation;
        self.last_attempts = delta.previous_attempts;
        Ok(())
    }

    fn validate_patch_routes_and_boundary(
        &self,
        candidate: &HexRelayoutCandidate,
        latest: &HexObservationFrame,
    ) -> Result<(), HexWfcError> {
        let grid = self.config.grid();
        for &coord in &candidate.region.cells {
            let placement = &self.placements[&coord];
            for face in HexFace::ALL {
                let Some(neighbor) = grid.neighbor(coord, face) else {
                    continue;
                };
                if candidate.region.cells.contains(&neighbor) {
                    continue;
                }
                if !ports_match(placement, face, &self.placements[&neighbor]) {
                    return Err(HexWfcError::UnsafeChange(coord));
                }
            }
        }
        // A single reverse connectivity flood replaces one full A* allocation
        // per player/objective at the commit boundary. Commit needs existence,
        // not a weighted path reconstruction.
        let exit_component =
            super::topology::active_component(self.config, &self.placements, self.config.exit());
        for (&player, &coord) in &latest.occupied_cells {
            if !exit_component.contains(&coord) {
                return Err(HexWfcError::MissingPlayerRoute(player));
            }
        }
        for &coord in &latest.objective_cells {
            if !exit_component.contains(&coord) {
                return Err(HexWfcError::MissingObjectiveRoute(coord));
            }
        }
        Ok(())
    }

    /// `(protected, mutable-pocket-eligible)` counts for diagnostics.
    #[must_use]
    pub fn decoherence_yield(&self, observation: &HexObservationFrame) -> (usize, usize) {
        let protected = protected_with_halo(self, observation);
        (
            protected.len(),
            self.placements.len().saturating_sub(protected.len()),
        )
    }
}

fn make_candidate(
    world: &HexWfcWorld,
    generation: u32,
    region: HexMutationRegion,
    solved_placements: &BTreeMap<HexCoord, HexPlacement>,
    solved_blueprints: &[StampedBlueprint],
    attempts: u32,
    used_fallback: bool,
) -> HexRelayoutCandidate {
    let placements = region
        .cells
        .iter()
        .map(|&coord| (coord, solved_placements[&coord]))
        .collect::<BTreeMap<_, _>>();
    let architecture = region
        .cells
        .iter()
        .map(|&coord| (coord, register_for(world.seed, generation, coord)))
        .collect::<BTreeMap<_, _>>();
    let changed_cells = region
        .cells
        .iter()
        .copied()
        .filter(|coord| {
            world.placements.get(coord) != placements.get(coord)
                || world.architecture.get(coord) != architecture.get(coord)
                || blueprint_cell_identity(&world.blueprints, *coord)
                    != blueprint_cell_identity(solved_blueprints, *coord)
        })
        .collect();
    let blueprints = solved_blueprints
        .iter()
        .filter(|blueprint| {
            blueprint
                .cells
                .iter()
                .all(|cell| region.cells.contains(cell))
        })
        .cloned()
        .collect();
    HexRelayoutCandidate {
        base_seed: world.seed,
        base_config: world.config,
        generation,
        region,
        placements,
        blueprints,
        architecture,
        changed_cells,
        attempts,
        used_fallback,
    }
}

fn blueprint_cell_identity(
    blueprints: &[StampedBlueprint],
    coord: HexCoord,
) -> Option<(crate::map_spec::RoomRole, HexCoord, usize)> {
    blueprints.iter().find_map(|blueprint| {
        blueprint
            .cells
            .iter()
            .position(|cell| *cell == coord)
            .map(|index| (blueprint.role, blueprint.anchor, index))
    })
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

fn register_for(seed: u64, generation: u32, coord: HexCoord) -> ArchitectureRegister {
    let coord_key = coord_key(coord);
    let mut rng = SplitMix::new(
        seed ^ coord_key.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ u64::from(generation).wrapping_mul(0xBF58_476D_1CE4_E5B9),
    );
    ArchitectureRegister::ALL[(rng.next_u64() % ArchitectureRegister::ALL.len() as u64) as usize]
}

pub(super) fn fallback_geometry_relayout(
    world: &HexWfcWorld,
    generation: u32,
    region: &HexMutationRegion,
    attempts: u32,
) -> Option<HexRelayoutCandidate> {
    let candidates = region
        .cells
        .iter()
        .copied()
        .filter(|coord| world.placements[coord].space != HexSpace::Void)
        .collect::<Vec<_>>();
    let candidates = if candidates.is_empty() {
        region.cells.iter().copied().collect::<Vec<_>>()
    } else {
        candidates
    };
    let index = (world.seed.rotate_left(generation % 63) as usize ^ generation as usize)
        % candidates.len().max(1);
    let selected = *candidates.get(index)?;
    let placements = region
        .cells
        .iter()
        .map(|&coord| (coord, world.placements[&coord]))
        .collect::<BTreeMap<_, _>>();
    let mut architecture = region
        .cells
        .iter()
        .map(|&coord| (coord, world.architecture[&coord]))
        .collect::<BTreeMap<_, _>>();
    let register = architecture.get_mut(&selected)?;
    let register_index = ArchitectureRegister::ALL
        .iter()
        .position(|candidate| candidate == register)
        .unwrap_or(0);
    *register = ArchitectureRegister::ALL[(register_index + 1) % ArchitectureRegister::ALL.len()];
    let blueprints = world
        .blueprints
        .iter()
        .filter(|blueprint| {
            blueprint
                .cells
                .iter()
                .all(|cell| region.cells.contains(cell))
        })
        .cloned()
        .collect();
    Some(HexRelayoutCandidate {
        base_seed: world.seed,
        base_config: world.config,
        generation,
        region: region.clone(),
        placements,
        blueprints,
        architecture,
        changed_cells: BTreeSet::from([selected]),
        attempts,
        used_fallback: true,
    })
}

fn protected_with_halo(
    world: &HexWfcWorld,
    observation: &HexObservationFrame,
) -> BTreeSet<HexCoord> {
    let mut protected = pinned_cells(
        world.config,
        &world.placements,
        &world.blueprints,
        observation,
    );
    let seeds = protected.clone();
    for coord in seeds {
        for face in HexFace::ALL {
            if let Some(neighbor) = world.config.grid().neighbor(coord, face) {
                protected.insert(neighbor);
            }
        }
    }
    protected
}

fn select_region(
    world: &HexWfcWorld,
    frontier: &BTreeSet<HexCoord>,
    protected_cells: BTreeSet<HexCoord>,
    target: usize,
    max: usize,
) -> HexMutationRegion {
    let occupants = frontier
        .iter()
        .copied()
        .chain(
            protected_cells
                .iter()
                .copied()
                .filter(|coord| world.placements[coord].space != HexSpace::Void),
        )
        .collect::<BTreeSet<_>>();
    let mut seeds = world
        .placements
        .keys()
        .copied()
        .filter(|coord| !protected_cells.contains(coord))
        .filter(|coord| {
            frontier.is_empty()
                || frontier
                    .iter()
                    .map(|known| travel_distance(*coord, *known))
                    .min()
                    .is_some_and(|distance| (3..=8).contains(&distance))
        })
        .collect::<Vec<_>>();
    if seeds.is_empty() {
        seeds = world
            .placements
            .keys()
            .copied()
            .filter(|coord| !protected_cells.contains(coord))
            .collect();
    }
    seeds.sort_by_key(|coord| {
        let frontier_distance = occupants
            .iter()
            .map(|known| travel_distance(*coord, *known))
            .min()
            .unwrap_or(u32::MAX);
        let mixed = pocket_hash(world.seed, world.generation.wrapping_add(1), *coord);
        (frontier_distance, mixed, *coord)
    });

    let mut best = BTreeSet::new();
    for seed in seeds {
        let mut cells = BTreeSet::new();
        let mut queue = VecDeque::from([seed]);
        let mut queued = BTreeSet::from([seed]);
        while let Some(coord) = queue.pop_front() {
            if protected_cells.contains(&coord) || cells.contains(&coord) {
                continue;
            }
            let mut tentative = cells.clone();
            tentative.insert(coord);
            if close_units(world, &mut tentative, &protected_cells, max) {
                cells = tentative;
            }
            if cells.len() >= target {
                break;
            }
            let mut neighbors = HexFace::ALL
                .iter()
                .filter_map(|&face| world.config.grid().neighbor(coord, face))
                .filter(|next| !protected_cells.contains(next) && queued.insert(*next))
                .collect::<Vec<_>>();
            neighbors.sort_by_key(|next| {
                (
                    pocket_hash(world.seed, world.generation.wrapping_add(1), *next),
                    *next,
                )
            });
            queue.extend(neighbors);
        }
        if cells.len() > best.len() {
            best = cells;
        }
        if best.len() >= target {
            break;
        }
    }
    let cells = best;
    let boundary_cells = cells
        .iter()
        .flat_map(|&coord| {
            HexFace::ALL
                .iter()
                .filter_map(move |&face| world.config.grid().neighbor(coord, face))
        })
        .filter(|coord| !cells.contains(coord))
        .collect();
    HexMutationRegion {
        cells,
        boundary_cells,
        protected_cells,
    }
}

fn close_units(
    world: &HexWfcWorld,
    cells: &mut BTreeSet<HexCoord>,
    protected: &BTreeSet<HexCoord>,
    max: usize,
) -> bool {
    loop {
        let before = cells.len();
        for blueprint in &world.blueprints {
            if blueprint.cells.iter().any(|cell| cells.contains(cell)) {
                cells.extend(blueprint.cells.iter().copied());
            }
        }
        let snapshot = cells.clone();
        for coord in snapshot {
            let placement = &world.placements[&coord];
            match placement.archetype {
                HexArchetype::RampUp => {
                    if let Some(mate) = world.config.grid().neighbor(coord, HexFace::Up) {
                        cells.insert(mate);
                    }
                }
                HexArchetype::RampHead => {
                    if let Some(mate) = world.config.grid().neighbor(coord, HexFace::Down) {
                        cells.insert(mate);
                    }
                }
                HexArchetype::Shaft => {
                    for face in [HexFace::Up, HexFace::Down] {
                        if let Some(mate) = world.config.grid().neighbor(coord, face)
                            && world.placements[&mate].archetype == HexArchetype::Shaft
                        {
                            cells.insert(mate);
                        }
                    }
                }
                _ => {}
            }
        }
        if cells.len() > max || cells.iter().any(|cell| protected.contains(cell)) {
            return false;
        }
        if cells.len() == before {
            return true;
        }
    }
}

fn ports_match(a: &HexPlacement, face: HexFace, b: &HexPlacement) -> bool {
    a.ports().port(face) == b.ports().port(face.opposite())
}

fn coord_key(coord: HexCoord) -> u64 {
    u64::from(coord.q) | (u64::from(coord.r) << 16) | (u64::from(coord.level) << 32)
}

fn pocket_hash(seed: u64, generation: u32, coord: HexCoord) -> u64 {
    let mut rng = SplitMix::new(
        seed ^ coord_key(coord).wrapping_mul(0xD6E8_FEB8_6659_FD93)
            ^ u64::from(generation).wrapping_mul(0xA076_1D64_78BD_642F),
    );
    rng.next_u64()
}

pub(super) const fn fold_generation_key(key: u64) -> u32 {
    (key ^ (key >> 32)) as u32
}
