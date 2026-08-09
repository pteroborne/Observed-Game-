//! The neighbours section of the module studio panel.
//!
//! One row per ring cell, answering three things in the order an author asks
//! them: how much freedom is on this face, what is currently drawn there, and
//! whether anything is wrong.
//!
//! The section leads with what the numbers *are*. This is the tile grammar —
//! two ports agreeing across one seam — and nothing else. A facility narrows it
//! much further, and saying so is not modesty: an author who read these counts
//! as "what the game will build here" would tune against variety that cannot
//! occur. `observed_facility::hex_wfc::neighborhood` calls this same number the
//! flattering one, and the sibling tool's NEIGHBOURS tab is where the narrowed
//! one lives.

use observed_hex::PortClass;

use crate::module::neighbor_render::{broken_pairs, describe};
use crate::module::neighbor_view::NeighbourView;
use crate::module::neighbors::{RingCell, turns_offered};

/// Rows past this are summarised rather than listed. A room's ring runs to a
/// dozen cells and the panel does not scroll.
const ROWS_SHOWN: usize = 9;

/// The neighbours block, or the reason there is not one.
#[must_use]
pub fn format_neighbours(view: &NeighbourView) -> String {
    let mut lines = vec![
        String::new(),
        String::from("WHAT COULD SIT HERE"),
        String::from("Solid = locked in.  Wireframe = not collapsed."),
        String::from("Amber = the cell the keys are on."),
        String::from("Red = a hole, or a seam that does not meet."),
        String::new(),
    ];

    if let Some(refusal) = view.ring.refusal.as_ref() {
        lines.push(refusal.clone());
        return lines.join("\n");
    }
    if view.ring.cells.is_empty() {
        lines.push(String::from("no ring: this module has no footprint."));
        return lines.join("\n");
    }

    lines.push(String::from(
        "Counts are what the TILE GRAMMAR permits - two ports",
    ));
    lines.push(String::from(
        "agreeing across one seam. A facility narrows it much",
    ));
    lines.push(String::from(
        "further; composition-studio's NEIGHBOURS tab has that",
    ));
    lines.push(String::from("number. Lateral faces only: a ramp's"));
    lines.push(String::from("partner is above it and is not previewed."));
    lines.push(String::new());

    let report = &view.report;
    lines.push(format!(
        "{} cell(s), {} drawn, {} with nothing that fits.",
        report.cells, report.drawn, report.empty
    ));
    if report.broken_seams > 0 {
        // Not a rendering note. These cells are each legal against the centre
        // and illegal against each other, which is precisely the thing a
        // controlled solve would have to fix.
        lines.push(format!(
            "{} seam(s) between neighbours do not meet.",
            report.broken_seams
        ));
    }
    lines.push(String::new());

    let cursor = view.cursor % view.ring.cells.len();
    for (index, cell) in view.ring.cells.iter().enumerate().take(ROWS_SHOWN) {
        lines.push(row(view, cell, index, cursor));
        lines.push(format!("       {}", describe(view, cell)));
    }
    if view.ring.cells.len() > ROWS_SHOWN {
        lines.push(format!(
            "  (+{} more cell(s); digits reach the first {ROWS_SHOWN})",
            view.ring.cells.len() - ROWS_SHOWN
        ));
    }

    for (a, b, face) in broken_pairs(view) {
        lines.push(format!(
            "  BROKEN SEAM: cell {} and cell {} across {:?}",
            a + 1,
            b + 1,
            face
        ));
    }
    lines.join("\n")
}

/// One cell's headline row.
fn row(view: &NeighbourView, cell: &RingCell, index: usize, cursor: usize) -> String {
    let marker = if index == cursor { ">" } else { " " };
    let face = cell
        .face()
        .map_or_else(|| String::from("?"), |face| format!("{face:?}"));
    let class = if cell.is_sealed() { "Sealed" } else { "Door" };
    let count = cell.candidates.len();
    let turns = view
        .previewed(cell)
        .map(|candidate| turns_offered(cell, &candidate.name))
        .unwrap_or(0);
    let seams = if cell.seams.len() > 1 {
        format!("  ({} seams)", cell.seams.len())
    } else {
        String::new()
    };
    let note = if count == 0 {
        String::from("  NOTHING FITS")
    } else if cell.is_forced() {
        String::from("  FORCED")
    } else if turns > 1 {
        format!("  x{turns} turns")
    } else {
        String::new()
    };
    format!(
        "{marker} {}. {face:<10} {class:<7} {}/{count}{seams}{note}",
        index + 1,
        view.index_for(cell) + 1
    )
}

/// The one-line summary for the status strip.
#[must_use]
pub fn headline(view: &NeighbourView) -> String {
    if let Some(refusal) = view.ring.refusal.as_ref() {
        return format!("neighbours: {refusal}");
    }
    let doors = view
        .ring
        .cells
        .iter()
        .filter(|cell| !cell.is_sealed())
        .count();
    format!(
        "neighbours: {} cell(s), {doors} open, {} empty, {} broken seam(s)",
        view.ring.cells.len(),
        view.report.empty,
        view.report.broken_seams
    )
}

/// Whether a cell's first seam is a doorway, for callers that want the class
/// without reasoning about multi-seam cells.
#[must_use]
pub fn is_doorway(cell: &RingCell) -> bool {
    cell.seams
        .first()
        .is_some_and(|seam| seam.class == PortClass::Door)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::diagnose::{Diagnosis, diagnose_file};
    use crate::module::neighbor_view::NeighbourView;
    use crate::module::neighbors::build_ring;

    fn corpus() -> Vec<Diagnosis> {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles/authored");
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
            .expect("the authored corpus is committed")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "map"))
            .collect();
        paths.sort();
        paths.iter().map(|path| diagnose_file(path)).collect()
    }

    fn view_over(name: &str) -> NeighbourView {
        let corpus = corpus();
        let subject = corpus
            .iter()
            .find(|diagnosis| diagnosis.name() == name)
            .expect("fixture is committed");
        let mut view = NeighbourView::opened(true);
        view.ring = build_ring(subject, &corpus);
        view
    }

    /// The tool ships no font asset, so Bevy's default ASCII subset is all it
    /// can draw. The crate-wide ratchet checks the source; this checks the
    /// string that is actually built, which is where a module name could smuggle
    /// one in.
    #[test]
    fn the_neighbours_panel_stays_ascii() {
        let text = format_neighbours(&view_over("hall_straight"));
        assert!(text.is_ascii(), "non-ASCII in the panel: {text}");
    }

    /// Every ring cell must appear, or a face silently stops being inspectable.
    #[test]
    fn every_ring_cell_gets_a_row() {
        let view = view_over("hall_straight");
        let text = format_neighbours(&view);
        for index in 1..=view.ring.cells.len().min(ROWS_SHOWN) {
            assert!(
                text.contains(&format!(" {index}. ")),
                "cell {index} has no row:\n{text}"
            );
        }
    }

    /// An empty cell is an authoring hole and has to be stated, not left as a
    /// row that merely looks bare.
    #[test]
    fn a_cell_with_nothing_that_fits_says_so() {
        let mut view = view_over("hall_straight");
        // Empty one cell's list, exactly as a corpus hole would.
        view.ring.cells[0].candidates.clear();
        let text = format_neighbours(&view);
        assert!(text.contains("NOTHING FITS"), "{text}");
        assert!(
            text.contains("NOTHING IN THE CORPUS MEETS THIS FACE"),
            "{text}"
        );
    }

    /// The honesty note is the difference between a useful readout and one an
    /// author would over-trust. It is not decoration.
    #[test]
    fn the_panel_says_this_is_the_grammar_not_the_solver() {
        let text = format_neighbours(&view_over("hall_straight"));
        assert!(text.contains("TILE GRAMMAR"), "{text}");
        assert!(text.contains("NEIGHBOURS tab"), "{text}");
        assert!(text.contains("Lateral faces only"), "{text}");
    }

    /// A module that does not parse must produce the refusal, not a plausible
    /// list of faces.
    #[test]
    fn a_refusal_replaces_the_rows() {
        let corpus = corpus();
        let broken = crate::module::diagnose::diagnose_text(
            std::path::Path::new("not_a_module.map"),
            "this is not a map file",
        );
        let mut view = NeighbourView::opened(true);
        view.ring = build_ring(&broken, &corpus);
        let text = format_neighbours(&view);
        assert!(text.contains("does not parse"), "{text}");
        assert!(
            !text.contains(" 1. "),
            "a refusal must not list cells:\n{text}"
        );
    }
}
