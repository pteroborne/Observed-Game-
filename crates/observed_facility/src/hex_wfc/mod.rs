//! The Arc L hex-prism WFC facility solver.
//!
//! Sibling of [`crate::full_wfc`] built on the [`observed_hex`] grid
//! vocabulary. Phase 90 scope: full 3D (lateral `Door` ports plus vertical
//! `ShaftOpen`/`RampOpen` ports), multi-hex room blueprints stamped before
//! collapse, deterministic collapse with a forced spawn→exit route that may
//! climb, an optional [`trace::SolveStep`] log so the lab can replay a solve
//! step by step, and incremental observation-safe relayout.

pub mod blueprint;
mod collapse;
mod constraints;
mod context;
pub mod neighborhood;
#[cfg(test)]
mod neighborhood_tests;
pub mod pins;
pub mod profile;
mod relayout;
#[cfg(test)]
mod relayout_tests;
pub mod score;
#[cfg(test)]
mod tests;
mod topology;
mod trace;
#[cfg(test)]
mod trace_tests;
mod validate;
mod variants;

use std::collections::{BTreeMap, BTreeSet};

use observed_content::ArchitectureRegister;
use observed_core::{CorridorId, RoomId};

pub use blueprint::{
    RoomBlueprint, StampedBlueprint, blueprint_cell_archetype, blueprint_for_role,
};
pub use context::{HexInfluenceField, PROFILE_MAX, PROFILE_MIN};
pub use neighborhood::{
    FaceDomain, NeighborCandidate, Neighborhood, NeighborhoodError, neighborhood,
};
pub use observed_hex::{HexCoord, HexFace, HexGridSize, PortClass, PortSignature};
pub use pins::{PinDiagnostic, PinFailure, diagnose_pins, resolved_pins};
pub use profile::{
    ArchetypeBias, COMPOSITION_PROFILE_VERSION, CompositionTendencies, DistrictBias,
    HexCompositionProfile, HexPin, PinIntent, PinPortClass, PinSet, ProfileDefect, ScoreWeights,
    SearchPolicy,
};
pub use relayout::{
    DEFAULT_MUTATION_MAX_CELLS, DEFAULT_MUTATION_TARGET_CELLS, DistrictSite, HexMutationRegion,
    HexObservationFrame, HexRelayoutCandidate, HexRelayoutDelta, HexRelayoutProgress,
    HexRelayoutWork, HexThresholdKey, district_sites,
};
pub use score::{LayoutScore, score_layout, score_layout_with};
pub use topology::{HexRoute, MAX_CONNECTION_COST};
pub use trace::{
    CellTrace, SolveStep, TraceSummary, cells_from_world, fold_trace, summarise_trace,
};
pub use variants::{
    HexGeometryDemand, demandable_signatures, geometry_demands, placement_tile_archetype,
};

/// What a collapsed cell is, coarsely.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum HexSpace {
    Void,
    Room,
    Hall,
}

/// Traversal grammar of a collapsed cell (Phase 88 lateral subset).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum HexArchetype {
    Void,
    Room,
    Straight,
    Corner,
    Junction,
    RampUp,
    RampHead,
    Shaft,
    /// Open floor with no perimeter walls of its own. Adjacent `Expanse` cells
    /// leave their shared faces open, so a run of them reads as one continuous
    /// volume rather than as a row of tiles — the vocabulary the solver was
    /// missing for a vast space.
    Expanse,
}

/// One collapsed cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexPlacement {
    pub coord: HexCoord,
    pub space: HexSpace,
    pub archetype: HexArchetype,
    /// Lateral door mask, bit `face.index()` for the six lateral faces.
    pub doors: u8,
    pub up: PortClass,
    pub down: PortClass,
}

impl HexPlacement {
    #[must_use]
    pub const fn is_open(&self, face: HexFace) -> bool {
        face.is_lateral() && self.doors & lateral_bit(face) != 0
    }

    /// The typed port view of this cell (Phase 88: `Sealed`/`Door` only).
    #[must_use]
    pub fn ports(&self) -> PortSignature {
        let mut ports = [PortClass::Sealed; 8];
        for face in HexFace::LATERAL {
            if self.is_open(face) {
                ports[face.index()] = PortClass::Door;
            }
        }
        ports[HexFace::Up.index()] = self.up;
        ports[HexFace::Down.index()] = self.down;
        PortSignature::try_from_ports(ports).expect("lateral doors are always a valid signature")
    }
}

/// Bit for a lateral face in a door mask.
#[must_use]
pub const fn lateral_bit(face: HexFace) -> u8 {
    debug_assert!(face.is_lateral());
    1 << (face as u8)
}

/// Solver configuration. Defaults give a mid-size single-level rhombus that
/// the corpus tests prove solvable across seeds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexWfcConfig {
    pub cols: u16,
    pub rows: u16,
    pub levels: u8,
    pub min_rooms: usize,
    pub max_rooms: usize,
    pub retry_budget: u32,
    /// Minimum lateral hex distance between any two rooms.
    pub min_room_distance: u32,
}

/// What one candidate layout produced in a multi-candidate search.
///
/// The whole ladder is returned, losers included, because an author choosing a
/// candidate count needs to see whether the extra solves are buying anything.
/// A search that reports only its winner cannot answer "was four candidates
/// worth four times the solve time" - the spread between best and worst is the
/// answer, and it is only visible if the losers are kept.
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateOutcome {
    /// Position in the search, where 0 is `seed` itself.
    pub index: u32,
    /// The seed this candidate was actually solved at.
    pub seed: u64,
    /// `None` when this candidate failed to solve. A failed candidate is
    /// skipped rather than fatal, so it is recorded rather than dropped: a
    /// ladder with a hole in it is a solvability signal.
    pub score: Option<LayoutScore>,
    /// Whether this candidate won and is the layout being returned.
    pub winner: bool,
}

/// Minimum authored-room distribution for a production match.
///
/// The compact lab fixtures keep using [`HexWfcConfig::min_rooms`] and
/// [`HexWfcConfig::max_rooms`]. The canonical match supplies this explicit
/// quota so multiple rooms of the same role are intentional and the amount of
/// contested keystone supply scales with the number of teams.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexRoomQuotas {
    pub decision: usize,
    pub decoherence_fork: usize,
    pub dual_station: usize,
    pub monitor: usize,
    pub anchor_checkpoint: usize,
    pub recovery: usize,
    pub guardian_control: usize,
    pub keystone: usize,
}

impl HexRoomQuotas {
    #[must_use]
    pub const fn for_team_count(team_count: u8) -> Self {
        let scaled_keystones = (team_count as usize * 5).div_ceil(2);
        Self {
            decision: 6,
            decoherence_fork: 4,
            dual_station: 3,
            monitor: 3,
            anchor_checkpoint: 3,
            recovery: 3,
            guardian_control: 1,
            keystone: if scaled_keystones < 4 {
                4
            } else {
                scaled_keystones
            },
        }
    }

    #[must_use]
    pub const fn total_with_start_and_exit(self) -> usize {
        2 + self.decision
            + self.decoherence_fork
            + self.dual_station
            + self.monitor
            + self.anchor_checkpoint
            + self.recovery
            + self.guardian_control
            + self.keystone
    }
}

impl Default for HexWfcConfig {
    fn default() -> Self {
        Self {
            cols: 12,
            rows: 9,
            levels: 1,
            min_rooms: 4,
            max_rooms: 8,
            retry_budget: 100,
            min_room_distance: 2,
        }
    }
}

impl HexWfcConfig {
    /// Arc L production-scale facility dimensions. [`Default`] remains the
    /// compact solver/corpus fixture; benchmarks and yield audits use this
    /// explicit 5,600-cell configuration.
    #[must_use]
    pub const fn arc_default() -> Self {
        Self {
            cols: 28,
            rows: 20,
            levels: 10,
            // Start + exit + every current authored gameplay role. Production
            // must never silently omit the lantern caches or Guardian origin.
            min_rooms: 9,
            max_rooms: 10,
            retry_budget: 100,
            min_room_distance: 2,
        }
    }

    #[must_use]
    pub const fn grid(&self) -> HexGridSize {
        HexGridSize {
            cols: self.cols,
            rows: self.rows,
            levels: self.levels,
        }
    }

    #[must_use]
    pub const fn spawn(&self) -> HexCoord {
        HexCoord {
            q: 0,
            r: 0,
            level: 0,
        }
    }

    #[must_use]
    pub const fn exit(&self) -> HexCoord {
        HexCoord {
            q: self.cols - 1,
            r: self.rows - 1,
            level: self.levels.saturating_sub(1),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HexWfcError {
    RetryBudgetExhausted {
        attempts: u32,
        /// Why the final attempt was rejected — retry-budget tuning signal.
        last_failure: Option<&'static str>,
    },
    InvalidConfig,
    StaleCandidate,
    UnsafeChange(HexCoord),
    MissingPlayerRoute(observed_core::PlayerId),
    MissingObjectiveRoute(HexCoord),
    /// A production relayout would destroy the guaranteed open/decision cadence.
    OpenVolumeContract,
    NoMutationRegion,
    /// An authored pin can never be satisfied on this lattice.
    ///
    /// Raised before the first attempt, so it names the mistake instead of
    /// surfacing as [`Self::RetryBudgetExhausted`] once the budget runs out.
    PinContradiction {
        coord: HexCoord,
        reason: pins::PinFailure,
    },
}

/// Stable room identity projected from a blueprint's generation key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexRoomInstance {
    pub id: RoomId,
    pub generation_key: u64,
    pub anchor: HexCoord,
}

/// Stable corridor identity projected from its exact connected cell set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexCorridorInstance {
    pub id: CorridorId,
    pub generation_key: u64,
    pub cells: std::collections::BTreeSet<HexCoord>,
}

/// A stable named blueprint port re-derived against the current corridor map.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexThresholdAttachment {
    pub room: RoomId,
    pub port_name: &'static str,
    pub room_cell: HexCoord,
    pub face: HexFace,
    pub corridor: CorridorId,
}

/// Facility-owned semantic geometry selection identity. Presentation may map
/// this to an authored prefab, but must not add relayout generation to the key:
/// a pinned cell's exact hull selection is part of its frozen geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexGeometryIdentity {
    pub placement: HexPlacement,
    pub architecture: ArchitectureRegister,
    pub variation_key: u64,
}

/// A solved hex facility.
#[derive(Clone, Debug, PartialEq)]
pub struct HexWfcWorld {
    pub seed: u64,
    pub generation: u32,
    pub config: HexWfcConfig,
    pub placements: BTreeMap<HexCoord, HexPlacement>,
    /// The room blueprints stamped into this solve, in stamp order.
    pub blueprints: Vec<StampedBlueprint>,
    /// Presentation-independent architectural treatment. The fallback relayout
    /// changes this while preserving the cell's exact traversal ports.
    pub architecture: BTreeMap<HexCoord, ArchitectureRegister>,
    /// Per-cell topology/presentation revision. A local relayout increments
    /// only cells whose placement or architectural treatment changed.
    pub cell_revisions: BTreeMap<HexCoord, u32>,
    /// Attempts consumed by the accepted solve (1-based).
    pub last_attempts: u32,
    /// Cells an author pinned in the profile this world was solved under.
    ///
    /// Carried on the world because relayout has to know: it rewires cells at
    /// runtime, and without this it would happily reroll a cell somebody
    /// deliberately placed, silently erasing authored intent between one tick
    /// and the next.
    pub authored_pins: BTreeSet<HexCoord>,
}

impl HexWfcWorld {
    /// Solve a fresh world. Deterministic in `(seed, generation)`.
    ///
    /// Uses [`HexCompositionProfile::baseline`]; see
    /// [`Self::generate_with_profile`] for the authored-composition entry point.
    pub fn generate(seed: u64, config: HexWfcConfig) -> Result<HexWfcWorld, HexWfcError> {
        Self::generate_inner(seed, config, None, &HexCompositionProfile::baseline(), None)
            .map(|(world, _)| world)
    }

    /// Solve a production facility with an explicit repeated-role room quota
    /// and the open-volume/cadence gate enabled.
    pub fn generate_with_room_quotas(
        seed: u64,
        config: HexWfcConfig,
        quotas: HexRoomQuotas,
    ) -> Result<HexWfcWorld, HexWfcError> {
        Self::generate_inner(
            seed,
            config,
            Some(quotas),
            &HexCompositionProfile::baseline(),
            None,
        )
        .map(|(world, _)| world)
    }

    /// Solve under an authored composition profile.
    ///
    /// Deterministic in `(seed, generation, profile)`. The profile bends the
    /// weighted lottery only — it can neither zero a legal variant nor change
    /// how many random draws the collapse makes — so a baseline profile is
    /// byte-identical to [`Self::generate`].
    pub fn generate_with_profile(
        seed: u64,
        config: HexWfcConfig,
        quotas: Option<HexRoomQuotas>,
        profile: &HexCompositionProfile,
    ) -> Result<HexWfcWorld, HexWfcError> {
        Self::generate_searched_with_profile(seed, config, quotas, profile).map(|(world, _)| world)
    }

    /// Solve `profile.search.candidates` layouts and keep the highest-scoring
    /// one, returning the whole ladder so an authoring tool can show the losers
    /// as well as the winner.
    ///
    /// **Candidate 0 is `seed` itself**, and the rest are derived. That makes
    /// raising `candidates` strictly additive: the layout you already had stays
    /// in the running, and the seed only maps to a different facility when a
    /// candidate genuinely scored higher. Deriving candidate 0 too - which is
    /// what this function replaced - meant that merely turning the search on
    /// discarded the facility you had been tuning, with no way to say "keep it
    /// unless something beats it".
    ///
    /// Ties keep the lower index: candidates are scanned in ascending order and
    /// the incumbent is only replaced on a **strictly** higher total, so the
    /// result is stable across repeated calls and a tie prefers `seed` itself.
    ///
    /// Candidates that fail to solve are recorded and skipped. Only if every
    /// candidate fails is the last error returned.
    ///
    /// Scoring uses the profile's own [`ScoreWeights`], so what the search
    /// optimises for is authored content, not a compile-time constant.
    pub fn generate_searched_with_profile(
        seed: u64,
        config: HexWfcConfig,
        quotas: Option<HexRoomQuotas>,
        profile: &HexCompositionProfile,
    ) -> Result<(HexWfcWorld, Vec<CandidateOutcome>), HexWfcError> {
        let candidates = profile.search.candidates.max(1);
        let mut outcomes: Vec<CandidateOutcome> = Vec::with_capacity(candidates as usize);
        let mut best: Option<(HexWfcWorld, f64, usize)> = None;
        let mut last_err: Option<HexWfcError> = None;

        for index in 0..candidates {
            let candidate_seed = if index == 0 {
                seed
            } else {
                score::candidate_seed(seed, index)
            };
            match Self::generate_inner(candidate_seed, config, quotas, profile, None) {
                Ok((world, _)) => {
                    let score = score::score_layout_with(&world, profile.score);
                    let total = score.total;
                    outcomes.push(CandidateOutcome {
                        index,
                        seed: candidate_seed,
                        score: Some(score),
                        winner: false,
                    });
                    let better = match &best {
                        Some((_, best_total, _)) => total > *best_total,
                        None => true,
                    };
                    if better {
                        best = Some((world, total, outcomes.len() - 1));
                    }
                }
                Err(err) => {
                    outcomes.push(CandidateOutcome {
                        index,
                        seed: candidate_seed,
                        score: None,
                        winner: false,
                    });
                    last_err = Some(err);
                }
            }
        }

        match best {
            Some((world, _, at)) => {
                outcomes[at].winner = true;
                Ok((world, outcomes))
            }
            None => Err(last_err.unwrap_or(HexWfcError::InvalidConfig)),
        }
    }

    /// Solve while recording every step for the lab's animated replay.
    pub fn generate_traced(
        seed: u64,
        config: HexWfcConfig,
    ) -> Result<(HexWfcWorld, Vec<SolveStep>), HexWfcError> {
        Self::generate_traced_with_profile(seed, config, &HexCompositionProfile::baseline())
    }

    /// [`Self::generate_traced`] under an authored composition profile.
    ///
    /// The step log is what pins the "same RNG draw sequence" invariant: a
    /// baseline profile must produce an identical `Vec<SolveStep>`.
    pub fn generate_traced_with_profile(
        seed: u64,
        config: HexWfcConfig,
        profile: &HexCompositionProfile,
    ) -> Result<(HexWfcWorld, Vec<SolveStep>), HexWfcError> {
        let mut steps = Vec::new();
        Self::generate_inner(seed, config, None, profile, Some(&mut steps))
            .map(|(world, _)| world)
            .map(|world| (world, steps))
    }

    /// Solve `candidates` layouts from `seed` at the **baseline** profile and
    /// keep the highest-scoring one.
    ///
    /// A thin wrapper over [`Self::generate_searched_with_profile`], which is
    /// where the selection rule and its guarantees are documented. Prefer that
    /// function when a profile is in hand: this one scores with baseline
    /// weights, so it optimises for something the author may not have asked
    /// for.
    pub fn generate_best(
        seed: u64,
        config: HexWfcConfig,
        candidates: u32,
    ) -> Result<HexWfcWorld, HexWfcError> {
        let mut profile = HexCompositionProfile::baseline();
        profile.search.candidates = candidates.max(1);
        Self::generate_searched_with_profile(seed, config, None, &profile).map(|(world, _)| world)
    }

    fn generate_inner(
        seed: u64,
        config: HexWfcConfig,
        room_quotas: Option<HexRoomQuotas>,
        profile: &HexCompositionProfile,
        trace: Option<&mut Vec<SolveStep>>,
    ) -> Result<(HexWfcWorld, u32), HexWfcError> {
        if config.cols < 3 || config.rows < 3 || config.min_rooms < 2 {
            return Err(HexWfcError::InvalidConfig);
        }
        let generation = 0;
        let (placements, blueprints, attempts) =
            collapse::collapse(seed, generation, config, room_quotas, profile, trace)?;
        let architecture = relayout::initial_architecture(seed, config, &placements, &blueprints);
        let cell_revisions = placements.keys().copied().map(|coord| (coord, 0)).collect();
        Ok((
            HexWfcWorld {
                seed,
                generation,
                config,
                placements,
                blueprints,
                architecture,
                cell_revisions,
                last_attempts: attempts,
                authored_pins: pins::resolved_pins(config, profile).0.into_keys().collect(),
            },
            attempts,
        ))
    }

    /// Breadth-first route between two cells over open doors.
    #[must_use]
    pub fn route_between(&self, from: HexCoord, to: HexCoord) -> Option<Vec<HexCoord>> {
        topology::route_between(self.config, &self.placements, from, to)
    }

    /// Weighted route between two live cells with per-port-class costs
    /// (`Door` lateral = hall tier, `RampOpen` = climb tier, `ShaftOpen` =
    /// shaft tier). The shared traversal oracle for match systems such as
    /// bots; callers must not cache it across a relayout generation.
    #[must_use]
    pub fn route_between_cells(&self, from: HexCoord, to: HexCoord) -> Option<HexRoute> {
        topology::costed_route_between(self.config, &self.placements, from, to)
    }

    /// [`Self::route_between_cells`], abandoned once no route within `max_cost` remains.
    ///
    /// For callers whose answer saturates past a known cost, an unbounded search is pure
    /// waste: it expands the entire reachable component just to report `None` for a pair
    /// that is far apart or disconnected. Inside the bound this returns exactly what the
    /// unbounded search returns.
    #[must_use]
    pub fn route_within_cost(
        &self,
        from: HexCoord,
        to: HexCoord,
        max_cost: u32,
    ) -> Option<HexRoute> {
        topology::costed_route_within(self.config, &self.placements, from, to, max_cost)
    }

    #[must_use]
    pub fn room_count(&self) -> usize {
        self.placements
            .values()
            .filter(|placement| placement.space == HexSpace::Room)
            .count()
    }

    /// Logical rooms with IDs derived from blueprint generation keys. A room
    /// therefore retains its ID whenever role and anchor survive a relayout.
    #[must_use]
    pub fn rooms(&self) -> Vec<HexRoomInstance> {
        self.blueprints
            .iter()
            .map(|blueprint| {
                let generation_key = blueprint.generation_key();
                HexRoomInstance {
                    id: RoomId(relayout::fold_generation_key(generation_key)),
                    generation_key,
                    anchor: blueprint.anchor,
                }
            })
            .collect()
    }

    #[must_use]
    pub fn room_id_at(&self, coord: HexCoord) -> Option<RoomId> {
        self.blueprints.iter().find_map(|blueprint| {
            blueprint
                .cells
                .contains(&coord)
                .then(|| RoomId(relayout::fold_generation_key(blueprint.generation_key())))
        })
    }

    /// Logical corridors: connected hall components, identified by the named
    /// room thresholds they join rather than by the cells they occupy.
    #[must_use]
    pub fn corridors(&self) -> Vec<HexCorridorInstance> {
        topology::corridor_instances(self.config, &self.placements, &self.blueprints)
    }

    #[must_use]
    pub fn corridor_id_at(&self, coord: HexCoord) -> Option<CorridorId> {
        self.corridors()
            .into_iter()
            .find_map(|corridor| corridor.cells.contains(&coord).then_some(corridor.id))
    }

    /// Rebuild stable named room-to-corridor attachments from current geometry.
    #[must_use]
    pub fn threshold_attachments(&self) -> Vec<HexThresholdAttachment> {
        topology::threshold_attachments(self)
    }

    /// Stable projector key for selecting among authored variations at a cell.
    /// It is intentionally independent of relayout generation.
    #[must_use]
    pub fn tile_variation_key(&self, coord: HexCoord) -> u64 {
        let coord_key =
            u64::from(coord.q) | (u64::from(coord.r) << 16) | (u64::from(coord.level) << 32);
        let mut rng =
            observed_core::SplitMix::new(self.seed ^ coord_key.wrapping_mul(0xD6E8_FEB8_6659_FD93));
        rng.next_u64()
    }

    #[must_use]
    pub fn geometry_identity(&self, coord: HexCoord) -> Option<HexGeometryIdentity> {
        Some(HexGeometryIdentity {
            placement: *self.placements.get(&coord)?,
            architecture: *self.architecture.get(&coord)?,
            variation_key: self.tile_variation_key(coord),
        })
    }

    #[must_use]
    pub fn cell_revision(&self, coord: HexCoord) -> Option<u32> {
        self.cell_revisions.get(&coord).copied()
    }
}
