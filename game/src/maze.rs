//! A per-hallway **interior maze**: the procedural labyrinth carved inside a single
//! teleport hallway piece.
//!
//! This is distinct from `observed_match::maze`, which embeds the whole room *graph* in
//! space — this generates the inside of one `from`→`to` hallway *edge*. In the teleport
//! model (see [`crate::teleport`]) a hallway always has exactly one entrance (back to
//! the room you came from, on the −Z wall) and one exit (onward, on the +Z wall). A
//! maze hallway fills the space between them with a deterministic seeded grid maze:
//!
//! - a **randomized-DFS spanning tree** (recursive backtracker) so every cell is
//!   reachable — hence the entrance and exit are *always* connected — with the long
//!   winding corridors and many dead ends that read as a labyrinth, then
//! - a light **braid** pass that opens some dead ends into loops, so there are several
//!   routes through while keeping plenty of dead ends.
//!
//! Pure logic: it returns the grid and, on request, the interior wall segments the
//! collision (`teleport::place_arena`) and presentation (`screens::rebuild_place`)
//! layers consume. The outer perimeter and the entrance/exit openings are *not* the
//! maze's job — `place_arena` builds those — so this module only ever emits the
//! interior walls between cells.

use bevy::math::Vec2;
use observed_core::SplitMix;

/// Percent chance (out of 100) that a given dead end gets braided open into a loop.
/// Low enough to keep the labyrinth full of dead ends while still offering alternate
/// routes through it.
const BRAID_PERCENT: usize = 30;

/// How many entrances (and, separately, exits) a maze of `cols` columns gets — one for
/// a narrow maze, up to three for a wide one. Multiple openings on each side make a
/// hallway feel like a real labyrinth with several ways in and out.
fn door_count(cols: usize) -> usize {
    (1 + cols / 3).clamp(1, 3)
}

/// Pick `count` distinct, spread-out door columns from `0..cols` (a random start plus
/// even spacing), so multiple entrances/exits never bunch up on one side.
fn pick_doors(rng: &mut SplitMix, cols: usize, count: usize) -> Vec<usize> {
    let count = count.clamp(1, cols);
    let start = rng.below(cols);
    let step = (cols / count).max(1);
    let mut cols_out: Vec<usize> = (0..count).map(|k| (start + k * step) % cols).collect();
    cols_out.sort_unstable();
    cols_out.dedup();
    cols_out
}

/// A generated hallway-interior maze on a `cols` × `rows` grid of square cells.
///
/// Cell `(c, r)` has column `c` in `0..cols` along +X and row `r` in `0..rows` along
/// +Z. Row `0` sits on the −Z (entrance) wall and row `rows-1` on the +Z (exit) wall.
/// Passages are stored as the open borders between adjacent cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Maze {
    pub cols: usize,
    pub rows: usize,
    /// Entrance cell columns on row `0` (−Z wall); all lead back to the prior room.
    pub entry_cols: Vec<usize>,
    /// Exit cell columns on row `rows-1` (+Z wall); all lead on to the next room.
    pub exit_cols: Vec<usize>,
    /// `open_v[r * (cols-1) + c]` — is the border between `(c, r)` and `(c+1, r)` open?
    open_v: Vec<bool>,
    /// `open_h[r * cols + c]` — is the border between `(c, r)` and `(c, r+1)` open?
    open_h: Vec<bool>,
}

impl Maze {
    /// Generate a maze on a `cols` × `rows` grid from `seed`. `cols`/`rows` must be at
    /// least 1; realistic hallways use a handful of cells in each axis.
    pub fn generate(cols: usize, rows: usize, seed: u64) -> Maze {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let mut rng = SplitMix::new(seed);
        let doors = door_count(cols);
        let entry_cols = pick_doors(&mut rng, cols, doors);
        let exit_cols = pick_doors(&mut rng, cols, doors);
        let mut maze = Maze {
            cols,
            rows,
            entry_cols,
            exit_cols,
            open_v: vec![false; cols.saturating_sub(1) * rows],
            open_h: vec![false; cols * rows.saturating_sub(1)],
        };
        maze.carve_spanning_tree(&mut rng);
        maze.braid(&mut rng);
        maze
    }

    fn cell_index(&self, c: usize, r: usize) -> usize {
        r * self.cols + c
    }

    fn v_index(&self, c: usize, r: usize) -> usize {
        r * (self.cols - 1) + c
    }

    fn h_index(&self, c: usize, r: usize) -> usize {
        r * self.cols + c
    }

    /// Is the border to the +X neighbour `(c+1, r)` open?
    fn open_east(&self, c: usize, r: usize) -> bool {
        c + 1 < self.cols && self.open_v[self.v_index(c, r)]
    }

    /// Is the border to the +Z neighbour `(c, r+1)` open?
    fn open_north(&self, c: usize, r: usize) -> bool {
        r + 1 < self.rows && self.open_h[self.h_index(c, r)]
    }

    /// Open the border between adjacent cells `(c, r)` and `(nc, nr)`.
    fn open_border(&mut self, c: usize, r: usize, nc: usize, nr: usize) {
        if nr == r && nc == c + 1 {
            let i = self.v_index(c, r);
            self.open_v[i] = true;
        } else if nr == r && c == nc + 1 {
            let i = self.v_index(nc, r);
            self.open_v[i] = true;
        } else if nc == c && nr == r + 1 {
            let i = self.h_index(c, r);
            self.open_h[i] = true;
        } else if nc == c && r == nr + 1 {
            let i = self.h_index(c, nr);
            self.open_h[i] = true;
        }
    }

    /// The in-bounds 4-neighbours of `(c, r)` as (column, row).
    fn neighbours(&self, c: usize, r: usize) -> Vec<(usize, usize)> {
        let mut out = Vec::with_capacity(4);
        if c + 1 < self.cols {
            out.push((c + 1, r));
        }
        if c > 0 {
            out.push((c - 1, r));
        }
        if r + 1 < self.rows {
            out.push((c, r + 1));
        }
        if r > 0 {
            out.push((c, r - 1));
        }
        out
    }

    /// Randomized depth-first search from the entrance cell, carving a passage to one
    /// random unvisited neighbour at a time. Every cell is visited, so the result is a
    /// spanning tree: fully connected (entrance↔exit always reachable) with dead ends.
    fn carve_spanning_tree(&mut self, rng: &mut SplitMix) {
        let count = self.cols * self.rows;
        let mut visited = vec![false; count];
        let start = (self.entry_cols[0], 0);
        visited[self.cell_index(start.0, start.1)] = true;
        let mut stack = vec![start];
        while let Some(&(c, r)) = stack.last() {
            let unvisited: Vec<(usize, usize)> = self
                .neighbours(c, r)
                .into_iter()
                .filter(|&(nc, nr)| !visited[self.cell_index(nc, nr)])
                .collect();
            if unvisited.is_empty() {
                stack.pop();
                continue;
            }
            let (nc, nr) = unvisited[rng.below(unvisited.len())];
            self.open_border(c, r, nc, nr);
            visited[self.cell_index(nc, nr)] = true;
            stack.push((nc, nr));
        }
    }

    /// The number of open borders a cell has (its passage degree).
    fn degree(&self, c: usize, r: usize) -> usize {
        let mut d = 0;
        if self.open_east(c, r) {
            d += 1;
        }
        if c > 0 && self.open_east(c - 1, r) {
            d += 1;
        }
        if self.open_north(c, r) {
            d += 1;
        }
        if r > 0 && self.open_north(c, r - 1) {
            d += 1;
        }
        d
    }

    /// Open a fraction of dead ends into a random walled neighbour. Each braid connects
    /// two already-connected cells, so it only adds loops (alternate routes) — the maze
    /// stays fully connected and most dead ends survive. A floor keeps at least one dead
    /// end (a labyrinth without any isn't a labyrinth); since one braid can clear at
    /// most two dead ends, we only braid while three or more remain.
    fn braid(&mut self, rng: &mut SplitMix) {
        for r in 0..self.rows {
            for c in 0..self.cols {
                if self.degree(c, r) != 1 {
                    continue;
                }
                if rng.below(100) >= BRAID_PERCENT {
                    continue;
                }
                if self.dead_end_count() < 3 {
                    continue;
                }
                let walled: Vec<(usize, usize)> = self
                    .neighbours(c, r)
                    .into_iter()
                    .filter(|&(nc, nr)| !self.border_open(c, r, nc, nr))
                    .collect();
                if walled.is_empty() {
                    continue;
                }
                let (nc, nr) = walled[rng.below(walled.len())];
                self.open_border(c, r, nc, nr);
            }
        }
    }

    /// Is the border between adjacent cells `(c, r)` and `(nc, nr)` open?
    fn border_open(&self, c: usize, r: usize, nc: usize, nr: usize) -> bool {
        if nr == r && nc == c + 1 {
            self.open_east(c, r)
        } else if nr == r && c == nc + 1 {
            self.open_east(nc, r)
        } else if nc == c && nr == r + 1 {
            self.open_north(c, r)
        } else if nc == c && r == nr + 1 {
            self.open_north(c, nr)
        } else {
            false
        }
    }

    /// Half-extent (XZ) of the maze's footprint for a given cell size.
    pub fn footprint_half(&self, cell: f32) -> Vec2 {
        Vec2::new(self.cols as f32 * cell * 0.5, self.rows as f32 * cell * 0.5)
    }

    /// World-space centre (XZ, footprint centred at the origin) of cell `(c, r)`.
    pub fn cell_center(&self, c: usize, r: usize, cell: f32) -> Vec2 {
        let half = self.footprint_half(cell);
        Vec2::new(
            -half.x + (c as f32 + 0.5) * cell,
            -half.y + (r as f32 + 0.5) * cell,
        )
    }

    /// The interior wall segments (centre, half-extent in XZ), centred at the origin —
    /// one per closed border between adjacent cells. Each segment's long axis is
    /// extended by `wall_t` so that perpendicular walls overlap at grid junctions
    /// (sealing corners) and edge-cell walls reach the outer perimeter. The perimeter
    /// itself and the entrance/exit openings are left to `teleport::place_arena`.
    pub fn interior_walls(&self, cell: f32, wall_t: f32) -> Vec<(Vec2, Vec2)> {
        let half = self.footprint_half(cell);
        let span = cell * 0.5 + wall_t;
        let mut walls = Vec::new();
        // Vertical walls on the borders between columns (run along Z).
        for r in 0..self.rows {
            for c in 0..self.cols.saturating_sub(1) {
                if self.open_east(c, r) {
                    continue;
                }
                let x = -half.x + (c as f32 + 1.0) * cell;
                let z = -half.y + (r as f32 + 0.5) * cell;
                walls.push((Vec2::new(x, z), Vec2::new(wall_t, span)));
            }
        }
        // Horizontal walls on the borders between rows (run along X).
        for r in 0..self.rows.saturating_sub(1) {
            for c in 0..self.cols {
                if self.open_north(c, r) {
                    continue;
                }
                let x = -half.x + (c as f32 + 0.5) * cell;
                let z = -half.y + (r as f32 + 1.0) * cell;
                walls.push((Vec2::new(x, z), Vec2::new(span, wall_t)));
            }
        }
        walls
    }

    // --- inspection (used by tests / diagnostics) --------------------------------

    /// The number of cells reachable on foot from `(c, r)` through open borders.
    pub fn reachable_from(&self, c: usize, r: usize) -> usize {
        let count = self.cols * self.rows;
        let mut seen = vec![false; count];
        let mut stack = vec![(c, r)];
        seen[self.cell_index(c, r)] = true;
        let mut n = 0;
        while let Some((cc, cr)) = stack.pop() {
            n += 1;
            for (nc, nr) in self.neighbours(cc, cr) {
                if self.border_open(cc, cr, nc, nr) && !seen[self.cell_index(nc, nr)] {
                    seen[self.cell_index(nc, nr)] = true;
                    stack.push((nc, nr));
                }
            }
        }
        n
    }

    /// Whether two cells are connected through open borders.
    pub fn connected(&self, a: (usize, usize), b: (usize, usize)) -> bool {
        let count = self.cols * self.rows;
        let mut seen = vec![false; count];
        let mut stack = vec![a];
        seen[self.cell_index(a.0, a.1)] = true;
        while let Some((cc, cr)) = stack.pop() {
            if (cc, cr) == b {
                return true;
            }
            for (nc, nr) in self.neighbours(cc, cr) {
                if self.border_open(cc, cr, nc, nr) && !seen[self.cell_index(nc, nr)] {
                    seen[self.cell_index(nc, nr)] = true;
                    stack.push((nc, nr));
                }
            }
        }
        false
    }

    /// The number of dead-end cells (passage degree exactly 1).
    pub fn dead_end_count(&self) -> usize {
        let mut n = 0;
        for r in 0..self.rows {
            for c in 0..self.cols {
                if self.degree(c, r) == 1 {
                    n += 1;
                }
            }
        }
        n
    }

    /// The total number of open borders (passages). A perfect maze has exactly
    /// `cells - 1`; anything more means the braid pass added loops.
    pub fn open_edge_count(&self) -> usize {
        self.open_v.iter().filter(|&&o| o).count() + self.open_h.iter().filter(|&&o| o).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A representative spread of hallway maze sizes.
    const SIZES: [(usize, usize); 5] = [(4, 4), (5, 6), (6, 7), (7, 5), (3, 8)];

    #[test]
    fn generation_is_deterministic() {
        for &(cols, rows) in &SIZES {
            for seed in [0u64, 1, 7, 42, 9999] {
                let a = Maze::generate(cols, rows, seed);
                let b = Maze::generate(cols, rows, seed);
                assert_eq!(a, b, "same inputs must produce the same maze");
            }
        }
    }

    #[test]
    fn every_cell_is_reachable_so_all_entrances_and_exits_connect() {
        for &(cols, rows) in &SIZES {
            for seed in 0..40u64 {
                let m = Maze::generate(cols, rows, seed);
                assert_eq!(
                    m.reachable_from(m.entry_cols[0], 0),
                    cols * rows,
                    "the spanning tree must reach every cell ({cols}x{rows}, seed {seed})"
                );
                // Every entrance reaches every exit (full connectivity), so a player can
                // pick any way in and any way out.
                for &ec in &m.entry_cols {
                    for &xc in &m.exit_cols {
                        assert!(
                            m.connected((ec, 0), (xc, m.rows - 1)),
                            "entrance {ec} must reach exit {xc} ({cols}x{rows}, seed {seed})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn wide_mazes_offer_multiple_entrances_and_exits() {
        // A wide maze should expose more than one way in and out.
        let m = Maze::generate(6, 7, 3);
        assert!(
            m.entry_cols.len() >= 2,
            "a 6-wide maze has multiple entrances"
        );
        assert!(m.exit_cols.len() >= 2, "a 6-wide maze has multiple exits");
        // The openings are distinct columns.
        let mut e = m.entry_cols.clone();
        e.dedup();
        assert_eq!(
            e.len(),
            m.entry_cols.len(),
            "entrances are distinct columns"
        );
    }

    #[test]
    fn mazes_keep_dead_ends() {
        // Dead ends are wanted; a reasonable maze always has at least one after braiding.
        for &(cols, rows) in &SIZES {
            for seed in 0..40u64 {
                let m = Maze::generate(cols, rows, seed);
                assert!(
                    m.dead_end_count() >= 1,
                    "a {cols}x{rows} maze should keep dead ends (seed {seed})"
                );
            }
        }
    }

    #[test]
    fn braiding_adds_loops_so_there_are_multiple_routes() {
        // Across seeds, the braid pass must produce at least one maze with more
        // passages than a perfect tree (cells - 1) — i.e. a loop / alternate route.
        let (cols, rows) = (6, 7);
        let cells = cols * rows;
        let any_loops =
            (0..60u64).any(|seed| Maze::generate(cols, rows, seed).open_edge_count() > cells - 1);
        assert!(any_loops, "braiding should create loops for some seed");
    }

    #[test]
    fn different_seeds_diverge() {
        let differ =
            (0..50u64).any(|seed| Maze::generate(6, 7, seed) != Maze::generate(6, 7, seed + 1));
        assert!(differ, "the seed must actually influence the layout");
    }

    #[test]
    fn interior_walls_are_within_the_footprint_and_present() {
        let cell = 3.6;
        let wall_t = 0.3;
        for &(cols, rows) in &SIZES {
            let m = Maze::generate(cols, rows, 3);
            let half = m.footprint_half(cell);
            let walls = m.interior_walls(cell, wall_t);
            assert!(
                !walls.is_empty(),
                "a maze has interior walls ({cols}x{rows})"
            );
            for (center, h) in walls {
                // The wall's solid extent may overlap the perimeter by `wall_t` (to seal
                // corners) but never strays further out than that.
                assert!(
                    center.x.abs() <= half.x + wall_t + 1e-3,
                    "wall x within footprint"
                );
                assert!(
                    center.y.abs() <= half.y + wall_t + 1e-3,
                    "wall z within footprint"
                );
                assert!(h.x > 0.0 && h.y > 0.0, "wall has real extent");
            }
        }
    }
}
