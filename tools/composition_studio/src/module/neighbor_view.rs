//! Which neighbour you are looking at, and when the ring is rebuilt.
//!
//! Split from [`super::neighbors`] so the query stays Bevy-free and testable
//! without an `App`: that file answers *what could sit here*, this one holds
//! *which of those you are currently previewing*.

use std::collections::BTreeMap;

use bevy::prelude::*;
use observed_core::SplitMix;

use crate::module::app::ModuleState;
use crate::module::neighbors::{Candidate, NeighbourRing, RingCell, build_ring};

/// How far the camera pulls back in the mode.
///
/// One cell is 14 m across flats, so a centre with a neighbour on each side is
/// 42 m — three cells. Opening cropped would make the first action of every
/// session a zoom-out, which is the same argument the cutaway default already
/// makes for itself.
pub const RING_FRAMING: f32 = 3.0;

/// What the last ring pass drew, for the panel and the status strip.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RingReport {
    pub cells: usize,
    /// Cells that drew a candidate's geometry.
    pub drawn: usize,
    /// Cells with nothing in the corpus that fits. An authoring hole.
    pub empty: usize,
    pub edges: usize,
    /// Ring cells whose previewed candidates disagree with *each other*.
    pub broken_seams: usize,
}

/// The neighbour ring, and the choice of what is previewed in each cell.
#[derive(Default, Resource)]
pub struct NeighbourView {
    pub on: bool,
    /// Which ring cell the keys act on.
    pub cursor: usize,
    /// Candidate index per ring cell. Absent means the heaviest one.
    pub choice: BTreeMap<(i32, i32), usize>,
    /// How many rolls have happened, doubling as the seed so a roll is
    /// reproducible from its count.
    pub rolls: u64,
    pub ring: NeighbourRing,
    pub report: RingReport,
    /// The `(selected module, corpus generation)` the ring was built from.
    built_from: (usize, u64),
}

impl NeighbourView {
    /// Open the mode from the start, for a headless capture.
    #[must_use]
    pub fn opened(on: bool) -> Self {
        Self {
            on,
            ..Self::default()
        }
    }

    /// The ring cell the keys act on.
    #[must_use]
    pub fn selected(&self) -> Option<&RingCell> {
        if self.ring.cells.is_empty() {
            return None;
        }
        self.ring.cells.get(self.cursor % self.ring.cells.len())
    }

    /// Which candidate index is previewed in `cell`.
    #[must_use]
    pub fn index_for(&self, cell: &RingCell) -> usize {
        let count = cell.candidates.len().max(1);
        self.choice.get(&cell.coord).copied().unwrap_or(0) % count
    }

    /// The candidate previewed in `cell`, if anything fits there at all.
    #[must_use]
    pub fn previewed<'a>(&self, cell: &'a RingCell) -> Option<&'a Candidate> {
        cell.candidates.get(self.index_for(cell))
    }

    /// Point the keys at a ring cell by its panel number.
    pub fn select(&mut self, index: usize) -> Option<String> {
        let cell = self.ring.cells.get(index)?;
        self.cursor = index;
        let count = cell.candidates.len();
        Some(format!(
            "{:?}: {count} option(s) here",
            cell.face().unwrap_or(observed_hex::HexFace::East)
        ))
    }

    /// Move the previewed candidate on the cell under the cursor.
    ///
    /// Wraps: the lists are short, and an author cycling through them should
    /// not have to notice which end they are at.
    pub fn step_candidate(&mut self, delta: isize) -> Option<String> {
        let cell = self.selected()?;
        let coord = cell.coord;
        let count = cell.candidates.len();
        let face = cell.face();
        if count == 0 {
            return Some(String::from("nothing in the corpus meets this face"));
        }
        if count == 1 {
            return Some(format!(
                "{:?} is forced: one module in the corpus fits",
                face.unwrap_or(observed_hex::HexFace::East)
            ));
        }
        let current = self.index_for(cell);
        #[allow(clippy::cast_possible_wrap)]
        let next = (current as isize + delta).rem_euclid(count as isize) as usize;
        self.choice.insert(coord, next);
        let candidate = &self.ring.cells[self.cursor % self.ring.cells.len()].candidates[next];
        Some(format!(
            "{:?}: {}/{count} - {} turn {}",
            face.unwrap_or(observed_hex::HexFace::East),
            next + 1,
            candidate.name,
            candidate.turn
        ))
    }

    /// Draw a fresh combination across the whole ring.
    ///
    /// Each cell is drawn independently, because the ring's cells constrain
    /// each other and this mode does not yet solve that — which is exactly why
    /// disagreeing pairs are drawn in the alert colour instead of being
    /// avoided. Seeing the contradiction is the point; hiding it would be the
    /// bug.
    pub fn roll(&mut self) -> String {
        self.rolls += 1;
        let mut rng = SplitMix::new(self.rolls);
        let mut rolled = 0;
        for cell in &self.ring.cells {
            let count = cell.candidates.len();
            if count <= 1 {
                continue;
            }
            #[allow(clippy::cast_possible_truncation)]
            let pick = (rng.next_u64() % count as u64) as usize;
            self.choice.insert(cell.coord, pick);
            rolled += 1;
        }
        format!("roll {}: {rolled} cell(s) redrawn", self.rolls)
    }

    /// Back to the heaviest candidate everywhere.
    pub fn reset(&mut self) {
        self.choice.clear();
    }

    fn rebuild(&mut self, state: &ModuleState) {
        self.ring = match state.current() {
            Some(diagnosis) => build_ring(diagnosis, &state.diagnoses),
            None => NeighbourRing::default(),
        };
        self.choice.clear();
        self.cursor = 0;
        self.rolls = 0;
        self.built_from = (state.selected, state.built_from);
    }
}

/// Rebuild the ring when the mode is open and the module under it moved.
///
/// Runs between `rebuild_diagnoses` and the render pass, so a watcher-driven
/// reparse and a keyboard-driven selection both reach the ring in the same
/// frame they reach the centre.
pub fn refresh_ring(mut state: ResMut<ModuleState>, mut view: ResMut<NeighbourView>) {
    if !view.on {
        return;
    }
    if view.built_from == (state.selected, state.built_from) {
        return;
    }
    view.rebuild(&state);
    state.dirty = true;
}

/// How far back the camera sits: pulled out in the mode, unchanged otherwise.
#[must_use]
pub fn ring_scale(view: &NeighbourView) -> f32 {
    if view.on { RING_FRAMING } else { 1.0 }
}

/// Open or close the mode, rebuilding on the way in.
///
/// Rebuilding here rather than waiting for `refresh_ring` matters: the mode is
/// meaningless without a ring, and a first frame that drew nothing would read
/// as broken.
pub fn toggle(state: &mut ModuleState, view: &mut NeighbourView) -> String {
    view.on = !view.on;
    state.dirty = true;
    if !view.on {
        return String::from("neighbours off");
    }
    // Close the cutaway on the way in, and say so.
    //
    // The mode's entire legend is *solid means settled, wireframe means it
    // could be otherwise*, and a cut-away centre is a thin shell of far walls
    // and floor - which, ringed by six full wireframe cages, reads as a hole
    // rather than as the subject. `C` still opens it, and inside the mode that
    // is a deliberate act rather than the state you arrive in.
    state.cutaway = false;
    view.rebuild(state);
    match view.ring.refusal.as_ref() {
        Some(refusal) => format!("neighbours: {refusal}"),
        None => format!(
            "neighbours: {} cell(s) around this module, {} with nothing that fits",
            view.ring.cells.len(),
            view.ring.empty_cells()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::neighbors::build_ring;

    fn corpus() -> Vec<crate::module::diagnose::Diagnosis> {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles/authored");
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
            .expect("the authored corpus is committed")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "map"))
            .collect();
        paths.sort();
        paths
            .iter()
            .map(|path| crate::module::diagnose::diagnose_file(path))
            .collect()
    }

    fn view_over(name: &str) -> (NeighbourView, Vec<crate::module::diagnose::Diagnosis>) {
        let corpus = corpus();
        let subject = corpus
            .iter()
            .find(|diagnosis| diagnosis.name() == name)
            .expect("fixture is committed");
        let mut view = NeighbourView::opened(true);
        view.ring = build_ring(subject, &corpus);
        (view, corpus)
    }

    /// A roll must be reproducible from its count, or a capture of a rolled
    /// ring is not evidence of anything.
    #[test]
    fn a_roll_is_reproducible_from_its_count() {
        let (mut first, _) = view_over("hall_straight");
        let (mut second, _) = view_over("hall_straight");
        for _ in 0..5 {
            first.roll();
            second.roll();
        }
        assert_eq!(first.choice, second.choice);
        assert_eq!(first.rolls, 5);
    }

    /// Cycling must stay inside the cell's own candidate list. Walking off the
    /// end would preview a tile that does not fit, which is the one thing this
    /// mode may not do.
    #[test]
    fn cycling_never_previews_something_that_does_not_fit() {
        let (mut view, _) = view_over("hall_straight");
        for index in 0..view.ring.cells.len() {
            view.cursor = index;
            for delta in [1isize, 1, 1, -1, -1, -1, -1, 1] {
                view.step_candidate(delta);
                let cell = &view.ring.cells[index];
                let Some(previewed) = view.previewed(cell) else {
                    assert!(cell.candidates.is_empty());
                    continue;
                };
                assert!(cell.candidates.contains(previewed));
            }
        }
    }

    /// Resetting returns every cell to the heaviest option, which is what the
    /// mode opens on.
    #[test]
    fn resetting_returns_every_cell_to_its_heaviest_candidate() {
        let (mut view, _) = view_over("hall_straight");
        view.roll();
        view.reset();
        for cell in &view.ring.cells {
            assert_eq!(view.index_for(cell), 0);
        }
    }

    /// Opening the mode must build a ring in the same call.
    ///
    /// Waiting for `refresh_ring` would leave the first frame drawing nothing,
    /// which for a mode whose whole content is the ring reads as broken. It
    /// also closes the cutaway, which is what makes the centre read as solid.
    #[test]
    fn opening_the_mode_builds_a_ring_and_closes_the_cutaway() {
        let mut state = ModuleState {
            diagnoses: corpus(),
            cutaway: true,
            ..ModuleState::default()
        };
        state.selected = state
            .diagnoses
            .iter()
            .position(|diagnosis| diagnosis.name() == "hall_straight")
            .expect("fixture is committed");
        let mut view = NeighbourView::default();

        let note = toggle(&mut state, &mut view);
        assert!(view.on);
        assert!(!state.cutaway, "the centre must read as solid");
        assert_eq!(view.ring.cells.len(), 6, "the ring is built on the way in");
        assert!(state.dirty, "opening must ask for a redraw");
        assert!(note.contains("6 cell"), "{note}");

        // Closing leaves the cutaway where the author last had it rather than
        // silently restoring a value they may have changed inside the mode.
        let note = toggle(&mut state, &mut view);
        assert!(!view.on);
        assert_eq!(note, "neighbours off");
    }

    /// The framing must move with the mode, or the ring opens off-screen.
    #[test]
    fn the_camera_pulls_back_only_in_the_mode() {
        let mut view = NeighbourView::default();
        assert!((ring_scale(&view) - 1.0).abs() < f32::EPSILON);
        view.on = true;
        assert!((ring_scale(&view) - RING_FRAMING).abs() < f32::EPSILON);
    }
}
