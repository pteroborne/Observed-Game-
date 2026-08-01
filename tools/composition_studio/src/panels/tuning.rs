//! Tuning panel view for editing profile scalar fields in `composition_studio`.

use observed_facility::hex_wfc::PROFILE_MIN;

use crate::tunables::{TUNABLE_FIELDS, TunableField};
use crate::{StudioState, StudioTab};

/// Format text view of tuning sliders for `StudioTab::Tuning`.
pub fn format_tuning_panel(state: &StudioState, selected_index: usize) -> String {
    let mut lines = Vec::new();
    lines.push(
        "TUNING SLIDERS (Use Up/Down to navigate, Left/Right or -/+ to adjust bias):".to_string(),
    );
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

        lines.push(format!(
            "{cursor} {:<24} [{bar_str}] {:5.2} (bias range: {:4.2}..{:4.2})",
            field.label, val, field.min, field.max
        ));
    }

    if at_floor {
        lines.push(String::new());
        lines.push(
            "NOTICE: Bias only -- never removes an archetype. To forbid one, pin it (Slice 3)."
                .to_string(),
        );
    }

    lines.join("\n")
}
