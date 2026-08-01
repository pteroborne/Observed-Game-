//! The PINS tab: authored intent, and everything wrong with it.
//!
//! ASCII only — the tool ships no font asset.

use observed_facility::hex_wfc::PinDiagnostic;

use crate::StudioState;
use crate::brush::{self, Brush};

#[must_use]
pub fn format_pins_panel(state: &StudioState) -> String {
    let mut out = String::new();
    out.push_str("AUTHORED PINS\n\n");
    out.push_str(
        "A bias leans the lottery and can never zero a legal variant, so it can\n\
         say \"prefer shafts\" but never \"no shafts\". A pin says it and is checked.\n\n\
         left-drag paints   shift+left inspects   Delete unpins   Ctrl+Delete clears\n\
         , / . cycle brush\n\n",
    );

    out.push_str("BRUSH:\n");
    for entry in Brush::ALL {
        let mark = if entry == state.brush { ">" } else { " " };
        out.push_str(&format!("  {mark} {}\n", entry.label()));
    }
    out.push('\n');

    let pinned = brush::pinned_coords(&state.profile);
    out.push_str(&format!("PINNED CELLS: {}\n", pinned.len()));
    for coord in pinned.iter().take(12) {
        let intent = brush::intent_at(&state.profile, *coord)
            .map(brush::describe)
            .unwrap_or_default();
        out.push_str(&format!(
            "  ({:>2},{:>2},{})  {intent}\n",
            coord.q, coord.r, coord.level
        ));
    }
    if pinned.len() > 12 {
        out.push_str(&format!("  ... and {} more\n", pinned.len() - 12));
    }
    out.push('\n');

    if state.pin_diagnostics.is_empty() {
        if pinned.is_empty() {
            out.push_str("nothing pinned yet.\n");
        } else {
            out.push_str("every pin is satisfiable.\n");
        }
        return out;
    }

    out.push_str(&format!("DIAGNOSTICS ({}):\n", state.pin_diagnostics.len()));
    for diagnostic in state.pin_diagnostics.iter().take(10) {
        // A collision is information, not an error: the blueprint legitimately
        // won. Everything else is something to go and fix.
        let severity = match diagnostic {
            PinDiagnostic::BlueprintCollision { .. } | PinDiagnostic::Shadowed { .. } => "note",
            _ => "ERROR",
        };
        out.push_str(&format!("  [{severity}] {diagnostic}\n"));
    }
    if state.pin_diagnostics.len() > 10 {
        out.push_str(&format!(
            "  ... and {} more\n",
            state.pin_diagnostics.len() - 10
        ));
    }
    out
}
