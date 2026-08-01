//! "What will my next click do, right now?"
//!
//! The tool has a lot of modes — paint, inspect, pan, and four modifier
//! combinations on top — and every one of them was invisible state you had to
//! remember. This turns that into something continuously on screen.
//!
//! It updates with held modifiers rather than only describing the resting
//! state: hold shift and the shift row becomes the active one. That is the
//! difference between a legend (which you read once and forget) and an
//! indicator (which answers the question at the moment you have it).
//!
//! ASCII only — the tool ships no font asset.

use crate::StudioState;
use crate::chrome::LabMenuState;

/// One row of the bar: a chord, what it does, and whether it is live right now.
struct Row {
    chord: &'static str,
    action: String,
    active: bool,
}

/// Marker for the row that the current modifiers select.
const ACTIVE: &str = ">";
const INACTIVE: &str = " ";

/// Render the bar for the current input state.
///
/// `shift` and `ctrl` are the *held* modifiers, so the caller passes live
/// keyboard state rather than a snapshot taken at some earlier edit.
#[must_use]
pub fn format_action_bar(
    state: &StudioState,
    menu: &LabMenuState,
    shift: bool,
    ctrl: bool,
) -> String {
    if menu.confirm.is_some() {
        return String::from(
            "CONFIRM REQUIRED\n\n\
             > [Enter]  promote to corpus\n  [Esc]    cancel",
        );
    }
    if menu.is_open {
        // Honest about the current modality rather than describing viewport
        // actions that cannot fire. Slice 3.6b docks the panel and this branch
        // becomes "panel has keyboard focus" instead.
        return String::from(
            "MENU HAS THE KEYBOARD          [F2] close\n\n\
             > [Up/Dn]     select a control\n\
               [Lt/Rt]     adjust it\n\
               [Tab]       next tab\n\
               [Ctrl+S]    save to working path",
        );
    }

    let mode = if shift {
        format!("inspect (hold shift) - brush {}", state.brush.label())
    } else {
        format!("paint - {}", state.brush.label())
    };

    let target = match state.hovered {
        Some(coord) => format!("over ({},{},{})", coord.q, coord.r, coord.level),
        None => String::from("over nothing"),
    };

    let rows = [
        Row {
            chord: "LMB",
            action: format!("paint {}", state.brush.label()),
            active: !shift && !ctrl,
        },
        Row {
            chord: "SHIFT+LMB",
            action: String::from("inspect only, no edit"),
            active: shift,
        },
        Row {
            chord: "DEL",
            action: String::from("unpin selected"),
            active: !ctrl,
        },
        Row {
            chord: "CTRL+DEL",
            action: String::from("clear every pin"),
            active: ctrl,
        },
    ];

    let mut out = format!("MODE  {mode}\n      {target}\n\n");
    for row in rows {
        let mark = if row.active { ACTIVE } else { INACTIVE };
        out.push_str(&format!("{mark} {:<11}{}\n", row.chord, row.action));
    }
    out.push_str("\n  RMB/MMB    pan          WHEEL  zoom\n");
    out.push_str("  , .        brush        Q E    rotate view\n");
    out.push_str("  F          detail       C      cutaway\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> StudioState {
        StudioState::default()
    }

    fn closed() -> LabMenuState {
        LabMenuState {
            is_open: false,
            ..LabMenuState::default()
        }
    }

    /// The whole point: the bar must change when a modifier is held. A static
    /// legend would pass a "renders something" test while teaching nothing.
    #[test]
    fn holding_shift_changes_which_row_is_active() {
        let state = state();
        let menu = closed();
        let resting = format_action_bar(&state, &menu, false, false);
        let shifted = format_action_bar(&state, &menu, true, false);
        assert_ne!(resting, shifted, "held modifiers must be visible");
        assert!(shifted.contains("inspect"), "{shifted}");
        assert!(
            shifted.contains("> SHIFT+LMB"),
            "the shift row must become the active one:\n{shifted}"
        );
        assert!(
            resting.contains("> LMB"),
            "at rest, plain left-click is the active row:\n{resting}"
        );
    }

    #[test]
    fn holding_ctrl_promotes_the_destructive_row() {
        let state = state();
        let menu = closed();
        let held = format_action_bar(&state, &menu, false, true);
        assert!(held.contains("> CTRL+DEL"), "{held}");
        assert!(held.contains("clear every pin"), "{held}");
    }

    /// With the panel modal, describing viewport actions that cannot fire would
    /// be worse than saying nothing.
    #[test]
    fn an_open_menu_says_so_instead_of_listing_viewport_actions() {
        let bar = format_action_bar(&state(), &LabMenuState::default(), false, false);
        assert!(bar.contains("MENU HAS THE KEYBOARD"), "{bar}");
        assert!(!bar.contains("RMB/MMB"), "{bar}");
    }

    #[test]
    fn a_pending_confirmation_shows_only_its_two_answers() {
        let menu = LabMenuState {
            confirm: Some(crate::chrome::PendingConfirm {
                prompt: String::new(),
            }),
            ..LabMenuState::default()
        };
        let bar = format_action_bar(&state(), &menu, false, false);
        assert!(bar.contains("[Enter]"), "{bar}");
        assert!(bar.contains("[Esc]"), "{bar}");
        assert!(!bar.contains("LMB"), "nothing else is live:\n{bar}");
    }

    #[test]
    fn the_hovered_cell_is_named() {
        let mut state = state();
        state.hovered = Some(observed_hex::HexCoord {
            q: 3,
            r: 4,
            level: 0,
        });
        let bar = format_action_bar(&state, &closed(), false, false);
        assert!(bar.contains("over (3,4,0)"), "{bar}");
    }
}
