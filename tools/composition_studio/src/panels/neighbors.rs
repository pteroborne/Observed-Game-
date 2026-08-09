//! The NEIGHBOURS tab: what could stand around the selected cell.
//!
//! The panel is a list of faces, one row each, and every row answers three
//! things in the order an author asks them:
//!
//! 1. **How much freedom is here at all** — `4 of 37`, meaning four things can
//!    stand across this face given the whole facility, where thirty-seven could
//!    if only the selected tile mattered. A face reading `1 of 30` is forced by
//!    its surroundings, and that is usually a more useful fact than any bias.
//! 2. **What kind of thing, and how likely** — the archetype spread with each
//!    one's share of that cell's lottery. This is what moves when a slider
//!    moves, and it is the readout to tune against: seeing `shaft 62%` on four
//!    of six faces is the shaft bias being too high, stated as a number instead
//!    of inferred from a layout.
//! 3. **Which one you are looking at** — the previewed candidate, and whether
//!    it is what this seed actually produced.
//!
//! Shares are of the *cell's* lottery, not of the facility's. The solver
//! weights each cell separately — position and district both enter
//! `effective_weight` — so a percentage that pooled cells together would be an
//! average of things that were never compared.

use observed_facility::hex_wfc::{FaceDomain, HexArchetype};

use crate::neighbors::NeighborView;

/// Rows above this are elided. Eight faces of forty candidates each is a wall
/// of text nobody reads; the head of a weight-ordered list is where the answer
/// is, and the count says what was left out rather than pretending it was all.
const CANDIDATES_SHOWN: usize = 5;

/// The one-line summary, for the status bar.
#[must_use]
pub fn headline(view: &NeighborView) -> String {
    if let Some(refusal) = view.refusal.as_ref() {
        return format!("neighbours: {refusal}");
    }
    let Some(hood) = view.hood.as_ref() else {
        return String::from("neighbours: nothing selected");
    };
    let forced = hood.faces.iter().filter(|face| face.is_forced()).count();
    format!(
        "neighbours: {} face(s), {} forced, about {} distinct ring(s); \
         arrows pick and cycle, Space rolls one",
        hood.faces.len(),
        forced,
        hood.ring_upper_bound()
    )
}

/// The whole tab.
#[must_use]
pub fn format_neighbors_panel(view: &NeighborView) -> String {
    let mut out = String::from("WHAT COULD STAND HERE:\n\n");

    if let Some(refusal) = view.refusal.as_ref() {
        out.push_str(refusal);
        out.push('\n');
        return out;
    }
    let Some(hood) = view.hood.as_ref() else {
        return out + "Select a cell, then press N.\n";
    };

    out.push_str(&format!(
        "Centre ({},{},{}) is {:?}, and stays as solved.\n",
        hood.centre.q, hood.centre.r, hood.centre.level, hood.centre_placement.archetype
    ));
    out.push_str(
        "Solid = locked in.\n\
         Wireframe = not collapsed.\n\
         Green cage = what this seed solved.\n\
         Red cage = an alternative you are previewing.\n\n",
    );
    let report = &view.report;
    out.push_str(&format!(
        "{} ring cell(s), {} on an alternative, {} edge(s).\n",
        report.ring_cells, report.altered, report.edges
    ));
    if report.unbuildable > 0 {
        // Not a rendering note. A candidate the corpus cannot build is a cell
        // the solver may legally place and a match will then fail to load.
        out.push_str(&format!(
            "{} previewed candidate(s) HAVE NO GEOMETRY.\n\
             The solver may place them and a match would fail\n\
             at load. Check COVERAGE.\n",
            report.unbuildable
        ));
    }
    out.push('\n');

    for (index, face) in hood.faces.iter().enumerate() {
        let cursor = if index == view.cursor % hood.faces.len().max(1) {
            ">"
        } else {
            " "
        };
        out.push_str(&format!(
            "{cursor} {}. {:<6} ({},{},{}): {} of {}{}\n",
            index + 1,
            format!("{:?}", face.face),
            face.coord.q,
            face.coord.r,
            face.coord.level,
            face.candidates.len(),
            face.from_centre,
            if face.is_forced() { "  FORCED" } else { "" }
        ));
        out.push_str(&format_spread(face));
        out.push_str(&format_preview(view, face));
        out.push('\n');
    }
    out
}

/// The archetype spread of one face, as shares of that cell's own lottery.
fn format_spread(face: &FaceDomain) -> String {
    let spread = face.archetype_spread();
    if spread.is_empty() {
        return String::from("      (nothing can stand here)\n");
    }
    let mut out = String::from("      ");
    for (archetype, weight, count) in spread.iter().take(CANDIDATES_SHOWN) {
        out.push_str(&format!(
            "{} {:.0}% x{}  ",
            short(*archetype),
            share(*weight, face.total_weight) * 100.0,
            count
        ));
    }
    if spread.len() > CANDIDATES_SHOWN {
        out.push_str(&format!(
            "(+{} more kinds)",
            spread.len() - CANDIDATES_SHOWN
        ));
    }
    out.push('\n');
    out
}

/// Which candidate this face is currently showing.
fn format_preview(view: &NeighborView, face: &FaceDomain) -> String {
    let index = view.choice_for(face.coord) % face.candidates.len().max(1);
    let Some(candidate) = face.candidates.get(index) else {
        return String::new();
    };
    format!(
        "      showing {}/{}: {:?} doors {:06b} {:.1}%{}\n",
        index + 1,
        face.candidates.len(),
        candidate.placement.archetype,
        candidate.placement.doors,
        candidate.share(face.total_weight) * 100.0,
        if candidate.is_actual { "  <- seed" } else { "" }
    )
}

fn share(weight: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    {
        weight as f64 / total as f64
    }
}

/// Short archetype names, so a spread line fits the panel width.
fn short(archetype: HexArchetype) -> &'static str {
    match archetype {
        HexArchetype::Void => "void",
        HexArchetype::Room => "room",
        HexArchetype::Straight => "strt",
        HexArchetype::Corner => "turn",
        HexArchetype::Junction => "junc",
        HexArchetype::RampUp => "ramp",
        HexArchetype::RampHead => "rmph",
        HexArchetype::Shaft => "shft",
        HexArchetype::Expanse => "expn",
    }
}
