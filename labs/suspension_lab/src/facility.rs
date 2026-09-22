//! The same sight question, asked of a real generated facility.
//!
//! The model in `lib.rs` says seeing across open air costs about 2x. That was measured on
//! hand-built rectangular grids, so it proves nothing about hex geometry, the real corpus,
//! or the spans the solver actually produces. This asks the solver.
//!
//! No new `HexSpace` variant is introduced to do it. The measurement only needs to know
//! whether sight passes through a cell, and today's `Void` already marks every cell that
//! is not built — so treating `Void` as transparent answers the question without changing
//! a core enum across nineteen crates. If the number justifies the rule, the type split
//! comes after, with a reason.

use observed_facility::hex_wfc::{HexSpace, HexWfcConfig, HexWfcWorld};
use observed_hex::{HexCoord, HexFace};

/// One watcher's cells under each rule, along a single face.
pub struct Sight {
    /// Today's rule: a step onto a connected neighbour, or nothing.
    pub stepped: usize,
    /// Sight that passes through unbuilt space and stops at the first built cell.
    pub across_air: usize,
}

#[must_use]
pub fn sight_along(world: &HexWfcWorld, from: HexCoord, face: HexFace, range: usize) -> Sight {
    let grid = world.config.grid();
    let here = world.placements.get(&from);

    // Today's rule, in miniature: both sides must carry a door.
    let stepped = grid
        .neighbor(from, face)
        .and_then(|next| {
            let other = world.placements.get(&next)?;
            let mine = here?;
            (mine.is_open(face) && other.is_open(face.opposite()) && other.space != HexSpace::Void)
                .then_some(1)
        })
        .unwrap_or(0);

    // Sight: travel until something built stops it.
    let mut across_air = 0usize;
    let mut at = from;
    for _ in 0..range {
        let Some(next) = grid.neighbor(at, face) else {
            break;
        };
        at = next;
        across_air += 1;
        let blocked = world
            .placements
            .get(&at)
            .is_none_or(|placement| placement.space != HexSpace::Void);
        if blocked {
            // You see the wall, and nothing past it.
            break;
        }
    }
    Sight {
        stepped,
        across_air,
    }
}

/// Totals over every built cell of a facility, in every lateral direction.
#[must_use]
pub fn facility_sight(world: &HexWfcWorld, range: usize) -> (usize, usize, usize) {
    let mut stepped = 0usize;
    let mut across = 0usize;
    let mut watchers = 0usize;
    for (&cell, placement) in &world.placements {
        if placement.space == HexSpace::Void {
            continue;
        }
        watchers += 1;
        for face in HexFace::LATERAL {
            let sight = sight_along(world, cell, face, range);
            stepped += sight.stepped;
            across += sight.across_air;
        }
    }
    (watchers, stepped, across)
}

/// The four `architect_lab` scenario shapes.
#[must_use]
pub fn scenario_configs() -> [(&'static str, HexWfcConfig); 4] {
    let shape = |cols, rows, levels| HexWfcConfig {
        cols,
        rows,
        levels,
        ..HexWfcConfig::default()
    };
    [
        ("Pocket", shape(6, 5, 1)),
        ("Quick Climb", shape(8, 6, 2)),
        ("Full Ascent", shape(10, 8, 2)),
        ("Deep Stack", shape(8, 6, 5)),
    ]
}

/// Solve one and hand it back, so a caller can measure rather than guess.
#[must_use]
pub fn solve(config: HexWfcConfig, seed: u64) -> Option<HexWfcWorld> {
    HexWfcWorld::generate(seed, config).ok()
}
