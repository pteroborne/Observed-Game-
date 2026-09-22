//! Two rules the facility does not have yet, modelled cheaply enough to be wrong about.
//!
//! See `README.md` for why this is a model rather than the real machinery, and what that
//! costs.

pub mod facility;
pub mod measure;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// What occupies a cell.
///
/// The facility's `HexSpace` is `Void | Room | Hall`, and `Void` currently means two
/// different things: unbuilt rock, and outside the building. Those want different
/// physics, and conflating them is why a sightline cannot leave a corridor.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Space {
    /// Unbuilt mass. Opaque, impassable, cannot be built into.
    #[default]
    Rock,
    /// Open air. You cannot walk it and you cannot stand in it, but you can **see**
    /// through it, **fall** through it, and **build** into it given something to carry
    /// the load.
    Air,
    /// Walkable surface.
    Floor,
    /// Walkable, and carries load upward. The thing a suspended platform needs under it.
    Pylon,
}

impl Space {
    /// Blocks a line of sight.
    #[must_use]
    pub const fn opaque(self) -> bool {
        matches!(self, Self::Rock | Self::Floor | Self::Pylon)
    }

    /// Can be stood on and walked across.
    #[must_use]
    pub const fn solid(self) -> bool {
        matches!(self, Self::Floor | Self::Pylon)
    }
}

/// How far a floor may reach sideways from something that carries load.
///
/// Zero would mean every cell needs a pylon directly beneath it, which is a stack of
/// columns rather than a building. Large values make support meaningless. Three is a
/// cantilever you can see: a platform reaches a little way past what holds it, and a
/// suspended deck far out over the air needs its own column.
pub const CANTILEVER_REACH: i32 = 3;

/// A rectangular block of cells. Coordinates are `(x, y, level)`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Site {
    pub width: i32,
    pub depth: i32,
    pub levels: i32,
    cells: BTreeMap<(i32, i32, i32), Space>,
}

impl Site {
    #[must_use]
    pub fn new(width: i32, depth: i32, levels: i32) -> Self {
        Self {
            width,
            depth,
            levels,
            cells: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn contains(&self, at: (i32, i32, i32)) -> bool {
        (0..self.width).contains(&at.0)
            && (0..self.depth).contains(&at.1)
            && (0..self.levels).contains(&at.2)
    }

    #[must_use]
    pub fn get(&self, at: (i32, i32, i32)) -> Space {
        if !self.contains(at) {
            return Space::Rock;
        }
        self.cells.get(&at).copied().unwrap_or(Space::Rock)
    }

    pub fn set(&mut self, at: (i32, i32, i32), space: Space) {
        if self.contains(at) {
            self.cells.insert(at, space);
        }
    }

    /// Every solid cell, in a deterministic order.
    #[must_use]
    pub fn solids(&self) -> Vec<(i32, i32, i32)> {
        self.cells
            .iter()
            .filter(|(_, space)| space.solid())
            .map(|(at, _)| *at)
            .collect()
    }

    /// Cells a watcher at `from` can see along `direction`, stopping at the first opaque
    /// cell and at the edge of the site.
    ///
    /// This is the whole point of separating air from rock: sight crosses air, and it is
    /// **not** `step_through`. You can see where you could never walk.
    #[must_use]
    pub fn sightline(
        &self,
        from: (i32, i32, i32),
        direction: (i32, i32),
        range: i32,
    ) -> Vec<(i32, i32, i32)> {
        let mut seen = Vec::new();
        let mut at = from;
        for _ in 0..range {
            at = (at.0 + direction.0, at.1 + direction.1, at.2);
            if !self.contains(at) {
                break;
            }
            seen.push(at);
            if self.get(at).opaque() {
                // You see the wall, and nothing past it.
                break;
            }
        }
        seen
    }

    /// The old rule, kept for comparison: one step, and only onto something walkable.
    ///
    /// This is `architect_lab`'s observation model in miniature — vision borrowing a
    /// movement function, which is exactly what stops an Observer seeing out of a window.
    #[must_use]
    pub fn step_sightline(
        &self,
        from: (i32, i32, i32),
        direction: (i32, i32),
    ) -> Vec<(i32, i32, i32)> {
        let next = (from.0 + direction.0, from.1 + direction.1, from.2);
        if self.contains(next) && self.get(next).solid() {
            vec![next]
        } else {
            Vec::new()
        }
    }

    /// Every solid cell that something is holding up.
    ///
    /// Ground level carries itself. Above it, a cell is supported when a pylon stands
    /// directly beneath it, or when it can reach a supported neighbour on its own level
    /// within [`CANTILEVER_REACH`]. Support spreads outward from columns, so a platform
    /// hanging far out over the air needs a column of its own — which is the rule that
    /// makes a suspended deck a construction problem rather than a placement.
    #[must_use]
    pub fn supported(&self) -> BTreeSet<(i32, i32, i32)> {
        let mut supported: BTreeSet<(i32, i32, i32)> = BTreeSet::new();
        // Level by level upward: what holds this floor can only be below it or beside it.
        for level in 0..self.levels {
            let mut frontier: VecDeque<(i32, i32, i32)> = VecDeque::new();
            for at in self.solids().into_iter().filter(|at| at.2 == level) {
                let grounded = level == 0;
                let on_a_column = self.get((at.0, at.1, at.2 - 1)) == Space::Pylon;
                if (grounded || on_a_column) && supported.insert(at) {
                    frontier.push_back(at);
                }
            }
            // Spread sideways from whatever is already carried, up to the cantilever.
            let mut reach: BTreeMap<(i32, i32, i32), i32> =
                frontier.iter().map(|at| (*at, 0)).collect();
            while let Some(at) = frontier.pop_front() {
                let spent = reach[&at];
                if spent >= CANTILEVER_REACH {
                    continue;
                }
                for step in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let next = (at.0 + step.0, at.1 + step.1, at.2);
                    if !self.contains(next) || !self.get(next).solid() {
                        continue;
                    }
                    let better = reach.get(&next).is_none_or(|prior| spent + 1 < *prior);
                    if better {
                        reach.insert(next, spent + 1);
                        supported.insert(next);
                        frontier.push_back(next);
                    }
                }
            }
        }
        supported
    }

    /// Remove a cell and let go of everything that depended on it.
    ///
    /// Returns every cell that came down, the removal included. Repeats until the site
    /// settles, because what falls may itself have been carrying something — that chain
    /// is the mechanic, and its size is the thing worth measuring before shipping it.
    pub fn retract(&mut self, at: (i32, i32, i32)) -> BTreeSet<(i32, i32, i32)> {
        let mut fell = BTreeSet::new();
        if !self.get(at).solid() {
            return fell;
        }
        self.set(at, Space::Air);
        fell.insert(at);
        loop {
            let supported = self.supported();
            let orphans: Vec<_> = self
                .solids()
                .into_iter()
                .filter(|cell| !supported.contains(cell))
                .collect();
            if orphans.is_empty() {
                return fell;
            }
            for cell in orphans {
                self.set(cell, Space::Air);
                fell.insert(cell);
            }
        }
    }
}

#[cfg(test)]
mod tests;
