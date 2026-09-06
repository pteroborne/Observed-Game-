//! The lattice: a hexagon of cells whose **edges** carry passability.
//!
//! Every cell is floor. What changes is the wall between two cells, which is
//! the single most important modelling decision in this lab and was got wrong
//! first time round. An earlier draft made a whole cell `Open` or `Sealed`, so
//! a mutation deleted floor: brutal to play, wrong about the system being
//! modelled — where architecture *rewires* rather than decays — and, it turned
//! out, bad for legibility, because passability could only be read as colour.
//!
//! Drawn as geometry, a wall is a wall and a doorway is a gap. That carries the
//! most important information on screen with no hue at all, which is what the
//! Legibility Contract asks for and what `observed_style::outline` already says
//! in as many words: colour is never the only channel carrying meaning.
//!
//! `observed_hex` bounds are rhombic, so a radius-`n` hexagon is a `(2n+1)`
//! square grid plus a mask — a `Vec<bool>` here rather than a change to the
//! shared crate the solver and importer also use.

use observed_hex::coords::{HexCoord, HexGridSize, lateral_distance};
use observed_hex::faces::HexFace;
use observed_hex::ports::PortClass;

use crate::sim::prng::Prng;

/// One wall between two cells, named from one side.
///
/// The same physical boundary can be named twice — from either endpoint — so
/// [`Edge::canonical`] picks one, and the simulation only ever stores that.
/// Without it a telegraph could mark one wall twice and "two edges will change"
/// would sometimes mean one.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Edge {
    pub cell: HexCoord,
    pub face: HexFace,
}

impl Edge {
    #[must_use]
    pub fn canonical(self, size: HexGridSize) -> Self {
        match size.neighbor(self.cell, self.face) {
            Some(other) if size.index(other) < size.index(self.cell) => Self {
                cell: other,
                face: self.face.opposite(),
            },
            _ => self,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Board {
    size: HexGridSize,
    radius: u16,
    centre: HexCoord,
    in_board: Vec<bool>,
    /// Six lateral ports per cell, indexed by [`HexFace::index`]. Both sides of
    /// a boundary are always kept in agreement; see [`Board::set_port`].
    ports: Vec<[PortClass; 6]>,
}

impl Board {
    /// A hexagon of the given radius, every interior boundary a doorway.
    ///
    /// Radius 3 is 37 cells: `3r^2 + 3r + 1`.
    #[must_use]
    pub fn hexagon(radius: u16) -> Self {
        let span = radius * 2 + 1;
        let size = HexGridSize {
            cols: span,
            rows: span,
            levels: 1,
        };
        let centre = HexCoord {
            q: radius,
            r: radius,
            level: 0,
        };
        let in_board: Vec<bool> = (0..size.cell_count())
            .map(|i| lateral_distance(centre, size.coord(i)) <= u32::from(radius))
            .collect();

        let mut board = Self {
            size,
            radius,
            centre,
            in_board,
            ports: vec![[PortClass::Sealed; 6]; size.cell_count()],
        };
        // Every interior boundary starts open; the rim stays sealed because it
        // has nothing behind it.
        let interior: Vec<Edge> = board.interior_edges();
        for edge in interior {
            board.set_port(edge, PortClass::Door);
        }
        board
    }

    /// A hexagon with `walls` interior boundaries closed, never disconnecting
    /// it. Deterministic given the generator.
    #[must_use]
    pub fn hexagon_with_walls(radius: u16, walls: usize, rng: &mut Prng) -> Self {
        let mut board = Self::hexagon(radius);
        let mut candidates = board.interior_edges();
        rng.shuffle(&mut candidates);
        let mut closed = 0;
        for edge in candidates {
            if closed >= walls {
                break;
            }
            board.set_port(edge, PortClass::Sealed);
            if board.fully_connected() {
                closed += 1;
            } else {
                // Putting this wall up would strand part of the board. A lab
                // that can deal an unwinnable match wastes the tester's time.
                board.set_port(edge, PortClass::Door);
            }
        }
        board
    }

    #[must_use]
    pub const fn size(&self) -> HexGridSize {
        self.size
    }

    #[must_use]
    pub const fn radius(&self) -> u16 {
        self.radius
    }

    #[must_use]
    pub const fn centre(&self) -> HexCoord {
        self.centre
    }

    /// Whether the coordinate is part of the hexagon at all.
    #[must_use]
    pub fn on_board(&self, coord: HexCoord) -> bool {
        self.size.contains(coord) && self.in_board[self.size.index(coord)]
    }

    /// Every cell of the hexagon, in index order. Iteration order is part of
    /// the determinism contract.
    pub fn cells(&self) -> impl Iterator<Item = HexCoord> + '_ {
        (0..self.size.cell_count())
            .filter(|&i| self.in_board[i])
            .map(|i| self.size.coord(i))
    }

    /// Every boundary with a cell on both sides, canonically named, in index
    /// order.
    #[must_use]
    pub fn interior_edges(&self) -> Vec<Edge> {
        let mut edges: Vec<Edge> = Vec::new();
        for cell in self.cells() {
            for face in HexFace::LATERAL {
                let Some(other) = self.size.neighbor(cell, face) else {
                    continue;
                };
                if self.on_board(other) {
                    edges.push(Edge { cell, face }.canonical(self.size));
                }
            }
        }
        edges.sort_unstable();
        edges.dedup();
        edges
    }

    #[must_use]
    pub fn port(&self, edge: Edge) -> PortClass {
        if !self.on_board(edge.cell) {
            return PortClass::Sealed;
        }
        self.ports[self.size.index(edge.cell)][edge.face.index()]
    }

    /// Set both sides of a boundary at once. Ports that disagree across a
    /// boundary are the classic WFC bug, and `ports_compatible` says a bond
    /// exists only when both faces offer the same class — so the two sides are
    /// never allowed to drift apart here.
    pub fn set_port(&mut self, edge: Edge, class: PortClass) {
        if self.on_board(edge.cell) {
            let index = self.size.index(edge.cell);
            self.ports[index][edge.face.index()] = class;
        }
        if let Some(other) = self.size.neighbor(edge.cell, edge.face)
            && self.on_board(other)
        {
            let index = self.size.index(other);
            self.ports[index][edge.face.opposite().index()] = class;
        }
    }

    /// Whether a pawn may cross this boundary.
    #[must_use]
    pub fn passable(&self, cell: HexCoord, face: HexFace) -> bool {
        let Some(other) = self.size.neighbor(cell, face) else {
            return false;
        };
        self.on_board(cell)
            && self.on_board(other)
            && self.port(Edge { cell, face }) == PortClass::Door
    }

    /// Neighbours reachable through a doorway, in face order.
    pub fn open_neighbours(
        &self,
        cell: HexCoord,
    ) -> impl Iterator<Item = (HexFace, HexCoord)> + '_ {
        HexFace::LATERAL.into_iter().filter_map(move |face| {
            self.passable(cell, face)
                .then(|| self.size.neighbor(cell, face).map(|next| (face, next)))
                .flatten()
        })
    }

    /// How many doorways this cell has. Two is a corridor, four or more is a
    /// room — the rooms-versus-corridors distinction the model could not
    /// express at all while cells were merely open or sealed.
    #[must_use]
    pub fn doorway_count(&self, cell: HexCoord) -> usize {
        self.open_neighbours(cell).count()
    }

    /// Flood fill across doorways, indexed by [`HexGridSize::index`].
    #[must_use]
    pub fn reachable_from(&self, start: HexCoord) -> Vec<bool> {
        let mut seen = vec![false; self.size.cell_count()];
        if !self.on_board(start) {
            return seen;
        }
        seen[self.size.index(start)] = true;
        let mut frontier = vec![start];
        while let Some(cell) = frontier.pop() {
            for (_, next) in self.open_neighbours(cell) {
                let index = self.size.index(next);
                if !seen[index] {
                    seen[index] = true;
                    frontier.push(next);
                }
            }
        }
        seen
    }

    #[must_use]
    pub fn connected(&self, start: HexCoord, goal: HexCoord) -> bool {
        self.on_board(goal) && self.reachable_from(start)[self.size.index(goal)]
    }

    /// Whether every cell can still reach every other.
    #[must_use]
    pub fn fully_connected(&self) -> bool {
        let Some(first) = self.cells().next() else {
            return true;
        };
        let seen = self.reachable_from(first);
        self.cells().all(|cell| seen[self.size.index(cell)])
    }

    /// Steps from every cell to `goal` across doorways, `u32::MAX` where
    /// unreachable.
    #[must_use]
    pub fn distance_field(&self, goal: HexCoord) -> Vec<u32> {
        let mut distance = vec![u32::MAX; self.size.cell_count()];
        if !self.on_board(goal) {
            return distance;
        }
        distance[self.size.index(goal)] = 0;
        let mut queue = std::collections::VecDeque::from([goal]);
        while let Some(cell) = queue.pop_front() {
            let here = distance[self.size.index(cell)];
            for (_, next) in self.open_neighbours(cell) {
                let index = self.size.index(next);
                if distance[index] == u32::MAX {
                    distance[index] = here + 1;
                    queue.push_back(next);
                }
            }
        }
        distance
    }

    /// One step from `start` towards `goal`. Ties break on face order, which is
    /// why `HexFace::LATERAL`'s order is a determinism contract.
    #[must_use]
    pub fn step_towards(&self, start: HexCoord, goal: HexCoord) -> Option<HexCoord> {
        if start == goal || !self.on_board(start) {
            return None;
        }
        let distance = self.distance_field(goal);
        self.open_neighbours(start)
            .map(|(_, next)| (distance[self.size.index(next)], next))
            .filter(|&(d, _)| d != u32::MAX)
            .min_by_key(|&(d, _)| d)
            .map(|(_, next)| next)
    }
}
