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
mod relayout;
#[cfg(test)]
mod relayout_tests;
#[cfg(test)]
mod tests;
mod topology;
mod trace;
mod validate;
mod variants;

use std::collections::BTreeMap;

use observed_content::ArchitectureRegister;
use observed_core::{CorridorId, RoomId};

pub use blueprint::{
    RoomBlueprint, StampedBlueprint, blueprint_cell_archetype, blueprint_for_role,
};
pub use observed_hex::{HexCoord, HexFace, HexGridSize, PortClass, PortSignature};
pub use relayout::{
    HexObservationFrame, HexRelayoutCandidate, HexRelayoutProgress, HexRelayoutWork,
    HexThresholdKey,
};
pub use topology::HexRoute;
pub use trace::SolveStep;
pub use variants::{
    HexGeometryDemand, demandable_signatures, geometry_demands, placement_tile_archetype,
};

/// What a collapsed cell is, coarsely.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HexSpace {
    Void,
    Room,
    Hall,
}

/// Traversal grammar of a collapsed cell (Phase 88 lateral subset).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HexArchetype {
    Void,
    Room,
    Straight,
    Corner,
    Junction,
    RampUp,
    RampHead,
    Shaft,
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
            min_rooms: 4,
            max_rooms: 8,
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
    /// Attempts consumed by the accepted solve (1-based).
    pub last_attempts: u32,
}

impl HexWfcWorld {
    /// Solve a fresh world. Deterministic in `(seed, generation)`.
    pub fn generate(seed: u64, config: HexWfcConfig) -> Result<HexWfcWorld, HexWfcError> {
        Self::generate_inner(seed, config, None).map(|(world, _)| world)
    }

    /// Solve while recording every step for the lab's animated replay.
    pub fn generate_traced(
        seed: u64,
        config: HexWfcConfig,
    ) -> Result<(HexWfcWorld, Vec<SolveStep>), HexWfcError> {
        let mut steps = Vec::new();
        Self::generate_inner(seed, config, Some(&mut steps))
            .map(|(world, _)| world)
            .map(|world| (world, steps))
    }

    fn generate_inner(
        seed: u64,
        config: HexWfcConfig,
        trace: Option<&mut Vec<SolveStep>>,
    ) -> Result<(HexWfcWorld, u32), HexWfcError> {
        if config.cols < 3 || config.rows < 3 || config.min_rooms < 2 {
            return Err(HexWfcError::InvalidConfig);
        }
        let generation = 0;
        let (placements, blueprints, attempts) =
            collapse::collapse(seed, generation, config, trace)?;
        let architecture = relayout::initial_architecture(seed, &placements);
        Ok((
            HexWfcWorld {
                seed,
                generation,
                config,
                placements,
                blueprints,
                architecture,
                last_attempts: attempts,
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

    /// Logical corridors projected from connected hall components.
    #[must_use]
    pub fn corridors(&self) -> Vec<HexCorridorInstance> {
        topology::corridor_instances(self.config, &self.placements)
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
}
