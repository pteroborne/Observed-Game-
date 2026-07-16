//! The solve trace: an ordered event log the lab replays step by step.

use observed_hex::HexCoord;

use super::{HexArchetype, HexSpace};

/// One event of a traced solve, in the exact order the solver produced it.
/// Determinism contract: the same `(seed, generation)` yields the identical
/// step list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveStep {
    /// A retry attempt began.
    AttemptStart { attempt: u32 },
    /// A cell was seeded as a candidate room site for this attempt.
    RoomSite { coord: HexCoord },
    /// The forced spawn→exit route claimed door bits on this cell.
    ForcedCell { coord: HexCoord, required: u8 },
    /// Propagation narrowed a cell's domain (only emitted on change).
    DomainPruned { coord: HexCoord, remaining: u16 },
    /// The solver observed (collapsed) one cell.
    Collapsed {
        coord: HexCoord,
        space: HexSpace,
        archetype: HexArchetype,
        doors: u8,
    },
    /// Propagation emptied a domain; the attempt is abandoned.
    Contradiction { coord: HexCoord },
    /// The attempt produced a valid layout.
    Completed { rooms: u16, halls: u16 },
}
