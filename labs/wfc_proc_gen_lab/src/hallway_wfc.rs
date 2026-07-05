//! **Ported** hallway-interior WFC generator — Phase 47 / Arc D5 (2026-07-04).
//!
//! This used to be the game's Wave-Function-Collapse alternative for carving a
//! teleport hallway's interior walls, archived here (refactor Arc G1, 2026-07-02)
//! when nothing in the game called it. Phase 47 is exactly the moment the archive
//! doc comment predicted: the generator has now been ported onto the current
//! place-geometry API and lives in `observed_facility::wfc` (behind the `wfc`
//! feature, alongside the Phase 45 topology generator), where
//! `game::wfc_interior::generate` (the game-side adapter) and
//! `game::teleport::geom::hallway_geom_with_slots_and_role` (the role-driven WFC vs.
//! DFS-maze selection) consume it for real.
//!
//! This module is now just a thin re-export so the lab's own smoke test below keeps
//! exercising the **live** production code rather than a second, drifting copy.
//! `WallSeg` is kept as a local alias (rather than re-exporting
//! `observed_facility::wfc::InteriorSeg` directly) so any lab code still spelling
//! `hallway_wfc::WallSeg` keeps compiling unchanged.

pub use observed_facility::wfc::{
    InteriorSeg as WallSeg, WfcInteriorError, generate_interior_walls,
};

/// The archived generator's entry point, kept under its old name for any lab code
/// still calling it: forwards straight to the live
/// `observed_facility::wfc::generate_interior_walls`, returning an empty `Vec` (this
/// module's old no-solution contract) instead of propagating
/// [`WfcInteriorError`] on retry-budget exhaustion.
pub fn generate_wfc_interior_walls(
    cols: usize,
    rows: usize,
    entry_cols: &[usize],
    exit_cols: &[usize],
    seed: u64,
    cell: f32,
    wall_t: f32,
) -> Vec<WallSeg> {
    generate_interior_walls(cols, rows, entry_cols, exit_cols, seed, cell, wall_t)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ported generator (now living in `observed_facility::wfc`) still produces a
    /// consistent, connected interior through this lab's re-export: walls come back
    /// non-empty for a feasible seed, and the connectivity gate (every entrance
    /// reaches every exit, no orphaned walkable cells) held during generation.
    #[test]
    fn ported_hallway_wfc_still_generates_connected_interiors() {
        let walls = generate_wfc_interior_walls(7, 5, &[1], &[5], 1, 2.2, 0.12);
        assert!(
            !walls.is_empty(),
            "a 7x5 hallway with one entry and one exit should solve within the retry budget"
        );
    }
}
