//! Two ways of holding ground by looking at it.
//!
//! [`Cone`] is the mode-1 rule: a pawn holds an arc, so facing is a coverage
//! choice rather than a tempo cost. [`Radius`] is `tactics_lab`'s proven
//! `Sight` — omnidirectional, facing irrelevant — and exists so the lab can ask
//! whether directionality earns its complexity.

use observed_hex::coords::{HexCoord, lateral_distance};
use observed_hex::faces::HexFace;

use crate::sim::rules::Vision;
use crate::sim::state::MatchState;

/// An arc of `width` faces centred on the pawn's facing, reaching `range`
/// steps. The cone is built by walking arc faces rather than by angle maths, so
/// a wedge is exactly "where I could walk while still looking that way".
///
/// Sight travels through doorways only, so a wall occludes. That is what makes
/// facing worth a control of its own: where a pawn can see is a consequence of
/// where it stands, not merely of which way it points.
#[derive(Clone, Copy, Debug)]
pub struct Cone {
    /// 1, 3 or 5 faces. Even widths have no centre face and are rejected at
    /// construction.
    pub width: u8,
    pub range: u16,
    /// Whether a pawn holds the cell it stands in. Default true: without it,
    /// occupancy does not hold ground, which contradicts the observe-to-freeze
    /// premise the whole project is built on.
    pub lock_own_hex: bool,
}

impl Default for Cone {
    fn default() -> Self {
        Self {
            width: 1,
            range: 1,
            lock_own_hex: true,
        }
    }
}

impl Cone {
    /// The face indices the arc spans, centred on `facing`.
    fn arc(&self, facing: HexFace) -> Vec<HexFace> {
        let half = i32::from(self.width.saturating_sub(1) / 2);
        let centre = facing.index() as i32;
        (-half..=half)
            .map(|offset| {
                let index = (centre + offset).rem_euclid(6) as usize;
                HexFace::LATERAL[index]
            })
            .collect()
    }
}

impl Vision for Cone {
    fn name(&self) -> &'static str {
        "Cone"
    }

    fn covers(
        &self,
        state: &MatchState,
        from: HexCoord,
        facing: HexFace,
        target: HexCoord,
    ) -> bool {
        if target == from {
            return self.lock_own_hex;
        }
        if !state.board.on_board(target) {
            return false;
        }
        let arc = self.arc(facing);
        let size = state.board.size();
        let mut seen = vec![false; size.cell_count()];
        seen[size.index(from)] = true;
        let mut frontier = vec![(from, 0_u16)];
        while let Some((cell, depth)) = frontier.pop() {
            if depth == self.range {
                continue;
            }
            for &face in &arc {
                if !state.board.passable(cell, face) {
                    continue;
                }
                let Some(next) = size.neighbor(cell, face) else {
                    continue;
                };
                if seen[size.index(next)] {
                    continue;
                }
                seen[size.index(next)] = true;
                if next == target {
                    return true;
                }
                frontier.push((next, depth + 1));
            }
        }
        false
    }
}

/// Omnidirectional: every cell within `range`, facing ignored.
#[derive(Clone, Copy, Debug)]
pub struct Radius {
    pub range: u16,
}

impl Default for Radius {
    fn default() -> Self {
        Self { range: 1 }
    }
}

impl Vision for Radius {
    fn name(&self) -> &'static str {
        "Radius"
    }

    fn covers(
        &self,
        state: &MatchState,
        from: HexCoord,
        _facing: HexFace,
        target: HexCoord,
    ) -> bool {
        state.board.on_board(target) && lateral_distance(from, target) <= u32::from(self.range)
    }
}
