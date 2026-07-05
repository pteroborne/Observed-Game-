//! Game-side adapter for `observed_facility::wfc`'s hallway-interior generator
//! (Phase 47 / Arc D5).
//!
//! `crate::maze` is the shipping DFS+braid labyrinth generator for a teleport
//! hallway's interior; this module is the WFC alternative revived from the archived
//! `labs/wfc_proc_gen_lab/src/hallway_wfc.rs` (moved into `observed_facility::wfc`
//! behind the `wfc` feature in this same phase). The selection between the two lives
//! in `crate::teleport::geom::hallway_geom_with_slots`, keyed off the edge's
//! [`observed_facility::map_spec::CorridorRole`] — see that module's doc comment for
//! the mapping. This module is only the pure conversion: `InteriorSeg` (the facility
//! crate's plain-data wall segment) to `teleport::WallSeg` (the game's), plus picking
//! the same door columns `crate::maze` would for the same seed so both generators
//! read as the same kind of hallway for a given edge.

use bevy::math::Vec2;
use observed_facility::wfc::{InteriorSeg, WfcInteriorError, generate_interior_walls};

use crate::maze;
use crate::teleport::WallSeg;

/// A WFC-generated hallway interior: the wall segments plus the entry/exit door
/// columns used to build them (so the caller can place gaps consistently with
/// `crate::maze::Maze`'s door-column contract).
pub struct WfcInterior {
    pub walls: Vec<WallSeg>,
    pub entry_cols: Vec<usize>,
    pub exit_cols: Vec<usize>,
}

/// Attempt to generate a WFC hallway interior on a `cols x rows` grid from `seed`.
/// Picks the same entry/exit door columns [`crate::maze::Maze::generate`] would for
/// the same `(cols, seed)` (via `maze::door_columns`), so a fallback to the DFS maze
/// on [`WfcInteriorError`] presents the same door layout the WFC attempt tried to
/// fill. Returns `Err` (carrying the attempt count) if no connected layout converges
/// within the retry budget — the caller is expected to fall back to
/// `maze::Maze::generate` for that hallway.
pub fn generate(
    cols: usize,
    rows: usize,
    seed: u64,
    cell: f32,
    wall_t: f32,
) -> Result<WfcInterior, WfcInteriorError> {
    let (entry_cols, exit_cols) = maze::door_columns(cols, seed);
    let segs = generate_interior_walls(cols, rows, &entry_cols, &exit_cols, seed, cell, wall_t)?;
    Ok(WfcInterior {
        walls: segs.into_iter().map(to_wall_seg).collect(),
        entry_cols,
        exit_cols,
    })
}

fn to_wall_seg(seg: InteriorSeg) -> WallSeg {
    WallSeg {
        center: Vec2::new(seg.center.x, seg.center.y),
        half: Vec2::new(seg.half.x, seg.half.y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_picks_the_same_door_columns_as_the_dfs_maze_for_the_same_seed() {
        let seed = 7u64;
        let (cols, rows) = (6, 7);
        let dfs = maze::Maze::generate(cols, rows, seed);
        let wfc = generate(cols, rows, seed, 2.2, 0.12).expect("seed 7 converges");
        assert_eq!(
            wfc.entry_cols, dfs.entry_cols,
            "WFC and DFS pick the same entry door columns for the same seed"
        );
        assert_eq!(
            wfc.exit_cols, dfs.exit_cols,
            "WFC and DFS pick the same exit door columns for the same seed"
        );
    }

    #[test]
    fn generation_is_deterministic_for_the_same_seed() {
        let a = generate(6, 7, 3, 2.2, 0.12).expect("seed 3 converges");
        let b = generate(6, 7, 3, 2.2, 0.12).expect("seed 3 regenerates");
        assert_eq!(a.walls.len(), b.walls.len());
        for (x, y) in a.walls.iter().zip(b.walls.iter()) {
            assert_eq!(x.center, y.center);
            assert_eq!(x.half, y.half);
        }
    }

    /// Representative pinned seeds across every maze-template grid size the game
    /// actually uses (`4x4`, `5x6`, `6x7`, `7x5`, `4x8` — see `hallway::TEMPLATES`'
    /// `Maze` entries) converge without needing the DFS fallback. Found by scanning
    /// seeds `0..80` for each size: every one of them converges on the first attempt
    /// (0 retries), so `0`/`1`/`2` are safe, cheap, representative picks for every
    /// size — this pins that fact so a future change to the WFC model/retry budget
    /// that regresses convergence on real hallway sizes fails loudly here.
    #[test]
    fn pinned_seeds_converge_without_fallback_on_every_template_grid_size() {
        let template_grid_sizes = [(4usize, 4usize), (5, 6), (6, 7), (7, 5), (4, 8)];
        for (cols, rows) in template_grid_sizes {
            for seed in [0u64, 1, 2] {
                generate(cols, rows, seed, 2.2, 0.12).unwrap_or_else(|e| {
                    panic!("{cols}x{rows} seed {seed} should converge without fallback, got {e:?}")
                });
            }
        }
    }
}
