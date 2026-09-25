//! What a team has seen.
//!
//! The asymmetry between an architect and an operator is **information, not
//! camera**. An operator is interesting because they cannot see the system they
//! are playing inside; first person is one way to arrange that and a fogged
//! board is another, far cheaper one. If the request-and-serve loop between the
//! two seats fails here it would fail in first person too, and if it works here
//! then first person becomes a presentation question rather than a design one.
//!
//! Knowledge is deliberately **stale, not live**: a cell records what it looked
//! like when last seen, so an operator's picture of the facility can be wrong.
//! That is the whole point — the architect rewires ground nobody is watching,
//! and an operator who trusted their memory would walk into a wall that was a
//! doorway when they last looked. It is also why this is per team rather than
//! per pawn: a squad shares what it has learned.

use observed_hex::coords::{HexCoord, HexGridSize};
use observed_hex::faces::HexFace;
use observed_hex::ports::PortClass;

use crate::sim::rules::Vision;
use crate::sim::state::{MatchState, TeamId};

/// One cell as a team last saw it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Remembered {
    /// The turn it was last observed.
    pub seen_on: u16,
    /// Its six lateral ports as they stood then.
    pub ports: [PortClass; 6],
}

/// A team's picture of the facility.
#[derive(Clone, Debug)]
pub struct Knowledge {
    size: HexGridSize,
    cells: Vec<Option<Remembered>>,
}

impl Knowledge {
    #[must_use]
    pub fn blank(size: HexGridSize) -> Self {
        Self {
            size,
            cells: vec![None; size.cell_count()],
        }
    }

    /// Whether this cell has ever been seen.
    #[must_use]
    pub fn known(&self, coord: HexCoord) -> bool {
        self.remembered(coord).is_some()
    }

    #[must_use]
    pub fn remembered(&self, coord: HexCoord) -> Option<Remembered> {
        self.size
            .contains(coord)
            .then(|| self.cells[self.size.index(coord)])
            .flatten()
    }

    /// Whether the memory of this cell is older than the current turn — the
    /// operator's cue that what they are looking at may no longer be true.
    #[must_use]
    pub fn stale(&self, coord: HexCoord, turn: u16) -> bool {
        self.remembered(coord)
            .is_some_and(|cell| cell.seen_on < turn)
    }

    #[must_use]
    pub fn known_count(&self) -> usize {
        self.cells.iter().filter(|cell| cell.is_some()).count()
    }

    /// Record everything this team can currently see.
    ///
    /// A pawn always knows the cell it stands in, whichever way it faces —
    /// standing somewhere is a kind of looking. Beyond that the cone decides,
    /// so what a squad learns is a consequence of where it points.
    pub fn observe(&mut self, state: &MatchState, vision: &dyn Vision, team: TeamId) {
        for pawn in state.free_pawns().filter(|pawn| pawn.team == team) {
            for cell in state.board.cells() {
                if cell != pawn.at && !vision.covers(state, pawn.at, pawn.facing, cell) {
                    continue;
                }
                let mut ports = [PortClass::Sealed; 6];
                for face in HexFace::LATERAL {
                    ports[face.index()] = state.board.port(crate::sim::board::Edge { cell, face });
                }
                let index = self.size.index(cell);
                self.cells[index] = Some(Remembered {
                    seen_on: state.turn,
                    ports,
                });
            }
        }
    }
}
