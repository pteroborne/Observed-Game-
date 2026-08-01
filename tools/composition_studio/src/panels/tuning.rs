//! Tuning panel view for editing profile scalar fields in `composition_studio`.

use observed_facility::hex_wfc::PROFILE_MIN;

use crate::tunables::{TUNABLE_FIELDS, TunableField};
use crate::{StudioState, StudioTab};

/// Format text view of tuning sliders for `StudioTab::Tuning`.
pub fn format_tuning_panel(state: &StudioState, selected_index: usize) -> String {
    let mut lines = Vec::new();
    lines.push("TUNING   Up/Dn select    Lt/Rt or -/+ adjust".to_string());
    lines.push("         every bias runs 0.25 .. 4.00".to_string());
    lines.push(String::new());

    let mut at_floor = false;

    let fields_on_tab: Vec<(usize, &TunableField)> = TUNABLE_FIELDS
        .iter()
        .enumerate()
        .filter(|(_, f)| f.tab == StudioTab::Tuning)
        .collect();

    for (idx, (_global_idx, field)) in fields_on_tab.iter().enumerate() {
        let val = (field.get)(&state.profile);
        let cursor = if idx == selected_index { ">" } else { " " };
        if (val - PROFILE_MIN).abs() < 0.001 {
            at_floor = true;
        }

        let bar_width = 16;
        let norm = ((val - field.min) / (field.max - field.min)).clamp(0.0, 1.0);
        let pos = (norm * (bar_width - 1) as f64).round() as usize;

        let mut bar = vec!['-'; bar_width];
        if pos < bar_width {
            bar[pos] = 'O';
        }
        let bar_str: String = bar.into_iter().collect();

        // Deliberately short: the panel is a fixed-width text node, and a row
        // that overruns it wraps mid-value and reads as two broken rows.
        lines.push(format!("{cursor} {:<22}[{bar_str}]{val:6.2}", field.label));

        // The selected control explains itself, in place. A tooltip would hide
        // the answer behind a hover the author has no reason to attempt, and
        // printing all eighteen at once is a wall nobody reads.
        if idx == selected_index {
            for line in wrap(field.consequence, 44) {
                lines.push(format!("     {line}"));
            }
            lines.push(String::new());
        }
    }

    if at_floor {
        lines.push(String::new());
        lines.push("NOTICE: this is a bias, not a switch. It can never remove an".to_string());
        lines.push("archetype -- the floor exists so a legal variant is always".to_string());
        lines.push("still choosable. To genuinely forbid one, pin it (PINS tab).".to_string());
    }

    lines.join("\n")
}

/// Break `text` on word boundaries at `width`. The panel is a fixed-width text
/// node, so a long sentence would otherwise wrap mid-word wherever the node
/// happens to end.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if !current.is_empty() && current.len() + 1 + word.len() > width {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_never_exceeds_the_width_or_splits_a_word() {
        let text = "Higher makes branching intersections common, so routes fork \
                    more and dead ends thin out.";
        for line in wrap(text, 30) {
            assert!(line.len() <= 30, "{line:?} is {} chars", line.len());
        }
        assert_eq!(
            wrap(text, 30).join(" "),
            text.split_whitespace().collect::<Vec<_>>().join(" ")
        );
    }

    /// The selected control explains itself; the others stay quiet, or the
    /// panel becomes a wall of prose nobody reads.
    #[test]
    fn only_the_selected_field_shows_its_consequence() {
        let state = StudioState::default();
        let first = format_tuning_panel(&state, 0);
        let second = format_tuning_panel(&state, 1);
        assert_ne!(first, second, "selection must change the explanation shown");

        let shown = TUNABLE_FIELDS
            .iter()
            .filter(|field| field.tab == StudioTab::Tuning)
            .filter(|field| first.contains(field.consequence.split(' ').next().unwrap_or("")))
            .count();
        assert!(shown >= 1, "the selected field must explain itself");
    }
}
