//! The solve trace: an ordered event log the lab replays step by step, and the
//! fold that turns a prefix of it back into per-cell state.

use std::collections::BTreeMap;

use observed_hex::{HexCoord, PortClass};

use crate::map_spec::RoomRole;

use super::{HexArchetype, HexPlacement, HexSpace, HexWfcWorld};

/// One event of a traced solve, in the exact order the solver produced it.
/// Determinism contract: the same `(seed, generation)` yields the identical
/// step list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveStep {
    /// A retry attempt began.
    AttemptStart { attempt: u32 },
    /// A blueprint cell was stamped before the WFC collapse.
    BlueprintCell { coord: HexCoord, role: RoomRole },
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
        up: PortClass,
        down: PortClass,
    },
    /// Propagation emptied a domain; the attempt is abandoned.
    Contradiction { coord: HexCoord },
    /// The attempt produced a valid layout.
    Completed { rooms: u16, halls: u16 },
}

/// What a replay knows about one cell partway through a trace.
///
/// Every field answers a question the solver asked and answered in order, so a
/// viewer built on this reports the solver's own record rather than a
/// re-derivation of it. In particular `pruned_to` is *the domain size at the
/// moment propagation last touched this cell*, which is a different and much
/// cheaper quantity than re-opening a cell's domain and running AC-3 over it;
/// anything that shows it to a person should say which one it is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellTrace {
    /// A blueprint stamped this cell before the WFC collapse.
    pub site: bool,
    /// The forced spawn->exit route claimed door bits here.
    pub forced: bool,
    /// How many options propagation last left this cell, or `None` if no
    /// `DomainPruned` has named it yet.
    pub pruned_to: Option<u16>,
    /// The cell as collapsed. Cleared again by a contradiction, because the
    /// solver no longer stands behind it.
    pub resolved: Option<HexPlacement>,
    /// Propagation emptied this cell's domain during the attempt being
    /// replayed. Per-attempt, like every other field here: `AttemptStart`
    /// clears the fold.
    pub contradicted: bool,
    pub role: Option<RoomRole>,
}

/// Totals for the same prefix, for a status readout that counts up as a replay
/// plays rather than only reporting the finished solve.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TraceSummary {
    /// Cells the solver has observed so far.
    pub collapsed: u32,
    /// Cells narrowed but not yet collapsed.
    pub open: u32,
    /// Options remaining across those open cells — the "how much is still
    /// undecided" number, and the one that falls as a solve progresses.
    pub open_domain: u32,
    /// Which retry is being replayed, counting from one.
    pub attempt: u32,
    /// Contradictions seen in this attempt.
    pub contradictions: u32,
    /// Reported by `Completed`, so `None` until the attempt succeeds.
    pub completed: Option<(u16, u16)>,
}

/// Replay a trace prefix into per-cell state.
///
/// `AttemptStart` clears the fold rather than merging attempts: a retry starts
/// from an empty lattice, and carrying the abandoned attempt's cells forward
/// would draw a facility that never existed.
#[must_use]
pub fn fold_trace(steps: &[SolveStep]) -> BTreeMap<HexCoord, CellTrace> {
    let mut cells: BTreeMap<HexCoord, CellTrace> = BTreeMap::new();
    for step in steps {
        match *step {
            SolveStep::AttemptStart { .. } => cells.clear(),
            SolveStep::BlueprintCell { coord, role } => {
                let cell = cells.entry(coord).or_default();
                cell.site = true;
                cell.role = Some(role);
            }
            SolveStep::ForcedCell { coord, .. } => {
                cells.entry(coord).or_default().forced = true;
            }
            SolveStep::DomainPruned { coord, remaining } => {
                cells.entry(coord).or_default().pruned_to = Some(remaining);
            }
            SolveStep::Collapsed {
                coord,
                space,
                archetype,
                doors,
                up,
                down,
            } => {
                cells.entry(coord).or_default().resolved = Some(HexPlacement {
                    coord,
                    space,
                    archetype,
                    doors,
                    up,
                    down,
                });
            }
            SolveStep::Contradiction { coord } => {
                let cell = cells.entry(coord).or_default();
                cell.resolved = None;
                cell.contradicted = true;
            }
            SolveStep::Completed { .. } => {}
        }
    }
    cells
}

/// Summarise the same prefix.
#[must_use]
pub fn summarise_trace(steps: &[SolveStep]) -> TraceSummary {
    let cells = fold_trace(steps);
    let mut summary = TraceSummary {
        attempt: 1,
        ..TraceSummary::default()
    };
    for step in steps {
        match *step {
            // Counted from the log rather than from the fold, because the fold
            // is cleared on retry and these are facts about the replay itself.
            SolveStep::AttemptStart { attempt } => {
                summary.attempt = attempt + 1;
                summary.contradictions = 0;
                summary.completed = None;
            }
            SolveStep::Contradiction { .. } => summary.contradictions += 1,
            SolveStep::Completed { rooms, halls } => summary.completed = Some((rooms, halls)),
            _ => {}
        }
    }
    for cell in cells.values() {
        if cell.resolved.is_some() {
            summary.collapsed += 1;
        } else if let Some(remaining) = cell.pruned_to {
            summary.open += 1;
            summary.open_domain += u32::from(remaining);
        }
    }
    summary
}

/// Per-cell state for a world solved without a trace.
///
/// The production path does not trace, so a viewer that only knew how to fold
/// steps would show an empty facility for a world that is right there. Every
/// cell reads as collapsed, which is exactly what it is.
#[must_use]
pub fn cells_from_world(world: &HexWfcWorld) -> BTreeMap<HexCoord, CellTrace> {
    let mut cells: BTreeMap<HexCoord, CellTrace> = world
        .placements
        .iter()
        .map(|(&coord, &placement)| {
            (
                coord,
                CellTrace {
                    resolved: Some(placement),
                    ..CellTrace::default()
                },
            )
        })
        .collect();
    for blueprint in &world.blueprints {
        for &coord in &blueprint.cells {
            cells.entry(coord).or_default().role = Some(blueprint.role);
        }
    }
    cells
}
