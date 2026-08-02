//! Header and whole-tab notices for `StudioTab::Tuning`.
//!
//! The eighteen controls themselves are real slider entities and live in
//! [`crate::tuning_widgets`]. What remains here is the text that describes the
//! tab rather than any one control: the key hints, and the floor notice.

use observed_facility::hex_wfc::PROFILE_MIN;

use crate::tunables::{TUNABLE_FIELDS, TunableField};
use crate::{StudioState, StudioTab};

/// Format the text portion of the Tuning tab.
pub fn format_tuning_panel(state: &StudioState) -> String {
    let mut lines = vec![
        "TUNING   Up/Dn select   Lt/Rt adjust   or drag".to_string(),
        "         every bias runs 0.25 .. 4.00".to_string(),
        String::new(),
    ];

    let at_floor = TUNABLE_FIELDS
        .iter()
        .filter(|field: &&TunableField| field.tab == StudioTab::Tuning)
        .any(|field| ((field.get)(&state.profile) - PROFILE_MIN).abs() < 0.001);

    if at_floor {
        lines.push("NOTICE: this is a bias, not a switch. It can never remove an".to_string());
        lines.push("archetype -- the floor exists so a legal variant is always".to_string());
        lines.push("still choosable. To genuinely forbid one, pin it (PINS tab).".to_string());
        lines.push(String::new());
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_facility::hex_wfc::PROFILE_MAX;

    /// The floor is the one place the tool has to explain itself unprompted: an
    /// author who drags a bias to its minimum expecting the archetype to vanish
    /// has a wrong model, and the value alone will not correct it.
    #[test]
    fn the_floor_notice_appears_only_when_a_field_is_at_the_floor() {
        let mut state = StudioState::default();
        for field in TUNABLE_FIELDS
            .iter()
            .filter(|field| field.tab == StudioTab::Tuning)
        {
            (field.set)(&mut state.profile, PROFILE_MAX.min(field.max));
        }
        assert!(
            !format_tuning_panel(&state).contains("NOTICE"),
            "nothing is at the floor, so nothing should be explained"
        );

        let first = TUNABLE_FIELDS
            .iter()
            .find(|field| field.tab == StudioTab::Tuning)
            .expect("the Tuning tab has fields");
        (first.set)(&mut state.profile, PROFILE_MIN);
        assert!(
            format_tuning_panel(&state).contains("pin it"),
            "a field at the floor must point at the mechanism that actually forbids"
        );
    }

    /// The header has to name every way in, or the mouse path stays invisible -
    /// which was the original complaint about this tab.
    #[test]
    fn the_header_names_both_the_keyboard_and_the_mouse() {
        let text = format_tuning_panel(&StudioState::default());
        assert!(text.contains("Up/Dn"), "keyboard selection unadvertised");
        assert!(text.contains("drag"), "dragging unadvertised");
    }
}
