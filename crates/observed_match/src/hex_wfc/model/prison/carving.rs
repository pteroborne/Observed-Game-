//! Prison mazes carved ahead of the catch that needs them.
//!
//! A fresh maze is a braided lattice (0.1 ms), its geometry projected from the corpus
//! (about 13 ms) and a physics scene of its colliders (about 13 ms): a whole frame and
//! more, which used to be paid on the tick of a Guardian's catch, the one moment the
//! player is watching most closely.
//!
//! None of it depends on the catch. A team's `n`th maze is fixed by the match's seed,
//! the team and `n` ([`maze_seed`]), so the next one is carved on a thread of its own as
//! soon as the last is used - at the prison's opening for the first - and the catch only
//! takes it. The result is the same function of the same seed whichever thread made it
//! and whenever it finished, so determinism does not depend on timing: a catch that
//! comes before its maze is ready waits for the rest of it, and one whose carving failed,
//! or on a target without threads, carves it on the spot as before.

use std::sync::{Arc, OnceLock};

use observed_core::TeamId;
use observed_facility::hex_wfc::maze::braided_maze;

use super::super::super::content::HexMatchContent;
use super::super::super::geometry::HexWfcGeometrySnapshot;
use super::{HexPrisonMaze, MAZE_COLS, MAZE_REGISTER, MAZE_ROWS};

/// The seed of `team`'s `ordinal`th maze in a match seeded `seed`.
#[must_use]
pub(in crate::hex_wfc::model) fn maze_seed(seed: u64, team: TeamId, ordinal: u32) -> u64 {
    seed ^ u64::from(team.0).wrapping_mul(0xA24B_AED4_963E_E407)
        ^ u64::from(ordinal + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Carve the maze for `seed`: lattice, geometry and colliders.
#[must_use]
pub(in crate::hex_wfc::model) fn carve(seed: u64, content: &HexMatchContent) -> HexPrisonMaze {
    let world = braided_maze(seed, MAZE_COLS, MAZE_ROWS, MAZE_REGISTER);
    let geometry =
        HexWfcGeometrySnapshot::project_with_rooms(&world, content.cells(), content.rooms())
            .expect("every prison hall is one the corpus is required to build");
    let physics = geometry.rapier_scene();
    HexPrisonMaze {
        world,
        geometry,
        physics,
    }
}

/// A team's next maze, being carved ahead.
#[derive(Clone, Debug)]
pub(in crate::hex_wfc::model) struct Carving {
    /// Which of the team's mazes this is.
    pub ordinal: u32,
    seed: u64,
    /// Set once by the carving thread: the maze, or nothing if carving it failed.
    maze: Arc<OnceLock<Option<HexPrisonMaze>>>,
    /// Whether a thread is carving it; without one it is carved when taken.
    threaded: bool,
}

impl Carving {
    /// Start carving `team`'s `ordinal`th maze in a match seeded `seed`.
    pub(in crate::hex_wfc::model) fn start(
        seed: u64,
        team: TeamId,
        ordinal: u32,
        content: &Arc<HexMatchContent>,
    ) -> Self {
        let seed = maze_seed(seed, team, ordinal);
        let maze = Arc::new(OnceLock::new());
        let threaded = spawn(seed, content, &maze);
        Self {
            ordinal,
            seed,
            maze,
            threaded,
        }
    }

    /// The maze: waiting for its carving if it is not done, carving it here if it failed
    /// or had no thread. The same maze either way.
    #[must_use]
    pub(in crate::hex_wfc::model) fn take(self, content: &HexMatchContent) -> HexPrisonMaze {
        let carved = if self.threaded {
            self.maze.wait();
            // Moved out when this is the only holder, which it is unless the match has
            // been cloned since; copied otherwise.
            match Arc::try_unwrap(self.maze) {
                Ok(lock) => lock.into_inner().flatten(),
                Err(shared) => shared.get().cloned().flatten(),
            }
        } else {
            None
        };
        carved.unwrap_or_else(|| carve(self.seed, content))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn(
    seed: u64,
    content: &Arc<HexMatchContent>,
    maze: &Arc<OnceLock<Option<HexPrisonMaze>>>,
) -> bool {
    let (content, into) = (Arc::clone(content), Arc::clone(maze));
    std::thread::Builder::new()
        .name("prison maze".to_owned())
        .spawn(move || {
            let carved =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| carve(seed, &content)));
            // Always set, so a wait never outlives a failed carving.
            let _ = into.set(carved.ok());
        })
        .is_ok()
}

#[cfg(target_arch = "wasm32")]
fn spawn(
    _seed: u64,
    _content: &Arc<HexMatchContent>,
    _maze: &Arc<OnceLock<Option<HexPrisonMaze>>>,
) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Arc<HexMatchContent> {
        Arc::new(HexMatchContent::from_runtime_catalog(
            crate::hex_wfc::test_catalog().clone(),
        ))
    }

    #[test]
    fn a_maze_carved_ahead_is_the_maze_carved_on_the_spot() {
        let content = content();
        let ahead = Carving::start(7, TeamId(1), 2, &content);
        assert!(ahead.threaded, "carved on a thread of its own");
        let taken = ahead.take(&content);
        let here = carve(maze_seed(7, TeamId(1), 2), &content);
        assert_eq!(taken.world, here.world);
        assert_eq!(taken.geometry.pieces.len(), here.geometry.pieces.len());
    }

    #[test]
    fn every_team_s_every_maze_has_its_own_seed() {
        let seeds: std::collections::BTreeSet<u64> = (0..4)
            .flat_map(|team| (0..16).map(move |n| maze_seed(99, TeamId(team), n)))
            .collect();
        assert_eq!(seeds.len(), 64);
    }
}
