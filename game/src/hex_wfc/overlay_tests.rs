//! Tests for the canonical match overlays and their simulation policy (see `overlay.rs`).

use super::*;

#[test]
fn offline_pause_stops_but_online_pause_advances_neutrally() {
    let paused = MatchOverlayState::Pause(PausePage::Root);
    assert_eq!(
        simulation_policy(paused, false, false),
        SimulationPolicy::Stop
    );
    assert_eq!(
        simulation_policy(paused, true, false),
        SimulationPolicy::ContinueNeutral
    );
    assert!(simulation_policy(paused, true, false).sends_neutral_input());
    assert_eq!(
        simulation_policy(MatchOverlayState::SurvivorMap, false, false),
        SimulationPolicy::ContinueNeutral,
        "the survivor map is not a free offline pause"
    );
    assert_eq!(
        simulation_policy(MatchOverlayState::Playing, false, true),
        SimulationPolicy::Stop,
        "offline onboarding gives a first-time player time to read"
    );
    assert_eq!(
        simulation_policy(MatchOverlayState::Playing, true, true),
        SimulationPolicy::ContinueNeutral,
        "one LAN client cannot pause the authoritative match"
    );
}

#[test]
fn leave_requires_request_then_explicit_confirmation() {
    let mut state = MatchOverlayState::Pause(PausePage::Root);
    assert_eq!(
        reduce_action(&mut state, OverlayAction::RequestLeave),
        OverlayEffect::None
    );
    assert_eq!(state, MatchOverlayState::Pause(PausePage::ConfirmLeave));

    let mut cancelled = state;
    assert_eq!(
        reduce_action(&mut cancelled, OverlayAction::Back),
        OverlayEffect::None
    );
    assert_eq!(cancelled, MatchOverlayState::Pause(PausePage::Root));

    assert_eq!(
        reduce_action(&mut state, OverlayAction::ConfirmLeave),
        OverlayEffect::LeaveMatch
    );
}

#[test]
fn semantic_back_owns_nested_pages_and_pause_outranks_map() {
    let nested = MatchOverlayState::Pause(PausePage::Settings);
    assert_eq!(
        reduce_hotkeys(
            nested,
            OverlayHotkeys {
                pause: true,
                map: true,
                back: true,
            }
        ),
        MatchOverlayState::Pause(PausePage::Root),
        "Escape/B backs out one level before any overlapping hotkey"
    );
    assert_eq!(
        reduce_hotkeys(
            MatchOverlayState::Playing,
            OverlayHotkeys {
                pause: true,
                map: true,
                back: false,
            }
        ),
        MatchOverlayState::Pause(PausePage::Root),
        "pause wins when the player binds pause and map to the same key"
    );
}

#[test]
fn escape_can_open_default_pause_but_never_leaves_the_match() {
    assert_eq!(
        reduce_hotkeys(
            MatchOverlayState::Playing,
            OverlayHotkeys {
                pause: true,
                back: true,
                ..Default::default()
            }
        ),
        MatchOverlayState::Pause(PausePage::Root)
    );
    assert_eq!(
        reduce_hotkeys(
            MatchOverlayState::Pause(PausePage::ConfirmLeave),
            OverlayHotkeys {
                pause: true,
                back: true,
                ..Default::default()
            }
        ),
        MatchOverlayState::Pause(PausePage::Root)
    );
}

#[test]
fn pause_settings_reuse_the_standalone_row_model_without_bindings_or_back() {
    let rows = pause_setting_rows();
    assert!(!rows.is_empty());
    assert!(
        rows.iter()
            .all(|row| !matches!(row, SettingsRow::Binding(_) | SettingsRow::Back))
    );
}
