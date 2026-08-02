//! The candidate ladder for `StudioTab::Solve`.
//!
//! A multi-candidate search costs one full solve per candidate, so the question
//! an author actually has is not "which layout won" but "was the extra time
//! worth it". That is only answerable from the **spread**: if the winner scores
//! barely above the seed alone, the search is buying nothing and the candidate
//! count should come back down. So the losers are printed too, and the gap over
//! candidate 0 is stated outright rather than left to be worked out from a
//! column of numbers.

use observed_facility::hex_wfc::CandidateOutcome;

/// Render the ladder. Returns an empty string when no search happened, so the
/// Solve tab does not grow a section that always says "1 candidate".
pub fn format_candidate_ladder(candidates: &[CandidateOutcome]) -> String {
    if candidates.len() < 2 {
        return String::new();
    }

    let mut lines = vec![format!("CANDIDATE LADDER   {} solved", candidates.len())];

    for candidate in candidates {
        let marker = if candidate.winner { ">" } else { " " };
        let index = candidate.index;
        // Only the low bits: the full 64-bit seed is 18 characters and would
        // push the score column off the panel, and the derived seeds differ in
        // their low bits anyway.
        let seed = candidate.seed & 0xffff_ffff;
        match &candidate.score {
            Some(score) => lines.push(format!(
                "{marker} {index}  ..{seed:08x}  {:7.2}{}",
                score.total,
                if candidate.winner { "  WINNER" } else { "" }
            )),
            // A hole in the ladder is a solvability signal, not noise: it says
            // this profile is near the edge of what the lattice can satisfy.
            None => lines.push(format!("{marker} {index}  ..{seed:08x}   FAILED")),
        }
    }

    let scored: Vec<f64> = candidates
        .iter()
        .filter_map(|c| c.score.as_ref().map(|s| s.total))
        .collect();
    if let (Some(best), Some(first)) = (
        scored.iter().copied().fold(None, |acc: Option<f64>, v| {
            Some(acc.map_or(v, |a: f64| a.max(v)))
        }),
        candidates
            .first()
            .and_then(|c| c.score.as_ref())
            .map(|s| s.total),
    ) {
        let gain = best - first;
        lines.push(String::new());
        if gain.abs() < 0.005 {
            lines.push(
                "the seed alone already won -- these extra solves bought nothing".to_string(),
            );
        } else {
            lines.push(format!(
                "+{gain:.2} over the seed alone, for {}x the solve time",
                candidates.len()
            ));
        }
    }

    // Two blanks: the caller appends the score breakdown directly after this,
    // and one newline runs the two sections together into a single block.
    lines.push(String::new());
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_facility::hex_wfc::LayoutScore;

    fn scored(index: u32, total: f64, winner: bool) -> CandidateOutcome {
        CandidateOutcome {
            index,
            seed: u64::from(index) + 0x1000,
            score: Some(LayoutScore {
                connectivity: 0.0,
                elevation: 0.0,
                room_wholeness: 0.0,
                archetype_variety: 0.0,
                junction_rhythm: 0.0,
                total,
            }),
            winner,
        }
    }

    /// A single candidate is not a search, and a "ladder" of one is noise that
    /// makes the tab look like it did work it did not do.
    #[test]
    fn a_lone_candidate_renders_no_ladder() {
        assert!(format_candidate_ladder(&[scored(0, 5.0, true)]).is_empty());
        assert!(format_candidate_ladder(&[]).is_empty());
    }

    /// The gain over candidate 0 is the whole justification for the extra
    /// solves, so it must be stated, and stated as a real difference.
    #[test]
    fn the_ladder_states_what_the_search_actually_bought() {
        let text = format_candidate_ladder(&[
            scored(0, 10.0, false),
            scored(1, 13.5, true),
            scored(2, 11.0, false),
        ]);
        assert!(text.contains("+3.50"), "gain not stated: {text}");
        assert!(text.contains("WINNER"));
        assert!(text.contains("3 solved"));
    }

    /// When the seed already wins, say so plainly. Printing "+0.00" invites the
    /// author to read it as a small win rather than as "turn this back down".
    #[test]
    fn a_search_that_bought_nothing_says_so() {
        let text = format_candidate_ladder(&[scored(0, 10.0, true), scored(1, 9.0, false)]);
        assert!(text.contains("bought nothing"), "{text}");
    }

    /// A failed candidate must stay visible: a ladder that silently drops it
    /// looks like a smaller search that went fine.
    #[test]
    fn a_failed_candidate_is_shown_rather_than_dropped() {
        let text = format_candidate_ladder(&[
            scored(0, 10.0, true),
            CandidateOutcome {
                index: 1,
                seed: 0x2000,
                score: None,
                winner: false,
            },
        ]);
        assert!(text.contains("FAILED"), "{text}");
        assert!(text.contains("2 solved"));
    }
}
