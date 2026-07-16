//! The Arc L hex-prism WFC facility solver.
//!
//! Sibling of [`crate::full_wfc`] built on the [`observed_hex`] grid
//! vocabulary. Phase 88 scope: a single level (lateral `Door` ports only),
//! deterministic collapse with a forced spawn→exit route, and an optional
//! [`trace::SolveStep`] log so the lab can replay a solve step by step.
//! Verticality (ramp pairs, shaft stacks) and room blueprints arrive in
//! Phase 90; relayout parity in Phase 93.

mod collapse;
mod constraints;
#[cfg(test)]
mod tests;
mod topology;
mod trace;
mod validate;
mod variants;

use std::collections::BTreeMap;

pub use observed_hex::{HexCoord, HexFace, HexGridSize, PortClass, PortSignature};
pub use trace::SolveStep;

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
}

/// One collapsed cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexPlacement {
    pub coord: HexCoord,
    pub space: HexSpace,
    pub archetype: HexArchetype,
    /// Lateral door mask, bit `face.index()` for the six lateral faces.
    pub doors: u8,
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
            min_rooms: 8,
            max_rooms: 18,
            retry_budget: 64,
            min_room_distance: 2,
        }
    }
}

impl HexWfcConfig {
    #[must_use]
    pub const fn grid(&self) -> HexGridSize {
        HexGridSize {
            cols: self.cols,
            rows: self.rows,
            levels: 1,
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
            level: 0,
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
}

/// A solved hex facility.
#[derive(Clone, Debug, PartialEq)]
pub struct HexWfcWorld {
    pub seed: u64,
    pub generation: u32,
    pub config: HexWfcConfig,
    pub placements: BTreeMap<HexCoord, HexPlacement>,
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
        let (placements, attempts) = collapse::collapse(seed, generation, config, trace)?;
        Ok((
            HexWfcWorld {
                seed,
                generation,
                config,
                placements,
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

    #[must_use]
    pub fn room_count(&self) -> usize {
        self.placements
            .values()
            .filter(|placement| placement.space == HexSpace::Room)
            .count()
    }
}
