//! The COVERAGE tab: what this layout asks the catalog for, and what is missing.
//!
//! ASCII only — the tool ships no font asset, so anything outside Bevy's default
//! subset renders as a blank box.

use crate::coverage::{CoverageReport, ProjectorVerdict, Supply};

/// The whole-catalog seam audit. Slow (it recompiles every `.map`), so it runs
/// on demand rather than on every solve.
#[derive(Clone, Debug, Default)]
pub enum SeamAudit {
    #[default]
    NotRun,
    Running,
    Done {
        valid: usize,
        mismatched: usize,
        report: String,
    },
    Failed(String),
}

impl SeamAudit {
    fn summary(&self) -> String {
        match self {
            Self::NotRun => String::from("not run - press A"),
            Self::Running => String::from("running..."),
            Self::Done {
                valid, mismatched, ..
            } => {
                if *mismatched == 0 {
                    format!("{valid} seam(s) agree")
                } else {
                    format!("{mismatched} MISMATCHED of {} seam(s)", valid + mismatched)
                }
            }
            Self::Failed(detail) => format!("failed: {detail}"),
        }
    }
}

#[must_use]
pub fn format_coverage_panel(coverage: &CoverageReport, seams: &SeamAudit) -> String {
    let mut out = String::new();
    out.push_str("CATALOG COVERAGE\n\n");

    // The authoritative line first: prediction is only useful next to the
    // verdict it is predicting.
    let verdict = match &coverage.verdict {
        Some(ProjectorVerdict::Projected) => String::from("projector: OK (this layout would load)"),
        Some(ProjectorVerdict::Failed(detail)) => format!("projector: FAILED {detail}"),
        Some(ProjectorVerdict::CorpusUnavailable) | None => {
            String::from("projector: corpus unavailable")
        }
    };
    out.push_str(&verdict);
    out.push_str("\n\n");

    if coverage.demanded.is_empty() {
        out.push_str("no per-cell demands in this layout.\n");
        return out;
    }

    // Missing first: it is the only class that stops a match from loading.
    if coverage.unmet.is_empty() {
        out.push_str("every demand is covered.\n\n");
    } else {
        out.push_str(&format!("UNCOVERED ({}):\n", coverage.unmet.len()));
        for row in &coverage.unmet {
            out.push_str(&format!(
                "  {:<20} {:<16} sig {:#06x}  {} cell(s)  eg ({},{},{})\n",
                row.archetype,
                row.register,
                row.signature.0,
                row.cells,
                row.example.q,
                row.example.r,
                row.example.level,
            ));
        }
        out.push('\n');
    }

    let exact = count(coverage, Supply::Exact);
    let generic = count(coverage, Supply::GenericFallback);
    out.push_str(&format!(
        "DEMANDS: {} distinct\n  {exact} authored for their own district\n  \
         {generic} on the shared generic kit\n",
        coverage.demanded.len()
    ));
    out.push_str(
        "  (generic is legal, but that district has\n   no geometry of its own there)\n\n",
    );

    // The heaviest generic fallbacks are the best next authoring targets.
    let mut fallbacks: Vec<_> = coverage
        .demanded
        .iter()
        .filter(|row| row.supply == Supply::GenericFallback)
        .collect();
    fallbacks.sort_by_key(|row| std::cmp::Reverse(row.cells));
    if !fallbacks.is_empty() {
        out.push_str("heaviest generic fallbacks:\n");
        for row in fallbacks.iter().take(6) {
            out.push_str(&format!(
                "  {:<20} {:<16} {} cell(s)\n",
                row.archetype, row.register, row.cells
            ));
        }
        out.push('\n');
    }

    out.push_str("ROOM VARIETY   modules -> prototypes / registers\n");
    for row in &coverage.room_variety {
        let mark = if row.is_thin() { "THIN" } else { "ok" };
        out.push_str(&format!(
            "  {:<18} {:>2} -> {:>3} / {:<2}  {mark}\n",
            row.role, row.modules, row.prototypes, row.registers
        ));
    }
    let thin = coverage
        .room_variety
        .iter()
        .filter(|row| row.is_thin())
        .count();
    if thin > 0 {
        out.push_str(&format!(
            "  {thin} role(s) sit on ONE authored module.\n\
             \x20 Register expansion makes copies, not variety:\n\
             \x20 that room is the same room in every district.\n\
             \x20 (bug_backlog #25)\n"
        ));
    }
    out.push('\n');

    out.push_str(&format!(
        "CATALOG: {} cell prototype(s)\n  {} unused in this layout\n  {} at weight 0\n",
        coverage.never_placed.corpus_cells,
        coverage.never_placed.unused_here,
        coverage.never_placed.zero_weight.len(),
    ));
    for key in coverage.never_placed.zero_weight.iter().take(5) {
        out.push_str(&format!("  weight 0, will never place: {key}\n"));
    }

    out.push_str(&format!("\nSEAMS: {}\n", seams.summary()));
    if let SeamAudit::Done {
        mismatched, report, ..
    } = seams
        && *mismatched > 0
    {
        for line in report.lines().take(12) {
            out.push_str(&format!("  {line}\n"));
        }
    }

    out
}

fn count(coverage: &CoverageReport, supply: Supply) -> usize {
    coverage
        .demanded
        .iter()
        .filter(|row| row.supply == supply)
        .count()
}
