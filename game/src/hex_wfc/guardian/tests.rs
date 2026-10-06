use observed_guardian::form::State;
use observed_match::hex_wfc::HexGuardianStatus;

use super::{Transition, state_for, transition};

#[test]
fn stone_friction_follows_movement_and_stops_when_frozen() {
    let at = bevy::prelude::Vec3::ZERO;
    let next = bevy::prelude::Vec3::X;
    assert_eq!(super::slide_gain(State::Hunting, Some(at), at, 0.1), 0.0);
    assert!(super::slide_gain(State::Hunting, Some(at), next, 0.1) > 0.0);
    assert_eq!(
        super::slide_gain(State::FrozenBySight, Some(at), next, 0.1),
        0.0
    );
    assert_eq!(
        super::slide_gain(State::Hunting, Some(at), next * 100.0, 0.1),
        0.0
    );
}

#[test]
fn observation_stops_an_unfinished_glide() {
    let at = bevy::prelude::Vec3::ZERO;
    let destination = bevy::prelude::Vec3::X * 14.0;
    for state in [State::FrozenBySight, State::FrozenByAnchor] {
        assert_eq!(super::drawn_position(Some(at), destination, state, 0.1), at);
    }
    assert!(super::drawn_position(Some(at), destination, State::Hunting, 0.1).x > 0.0);
}

#[test]
fn every_status_is_drawn_as_its_own_state() {
    assert_eq!(state_for(HexGuardianStatus::Active), State::Hunting);
    assert_eq!(
        state_for(HexGuardianStatus::FrozenByPlayer),
        State::FrozenBySight
    );
    assert_eq!(
        state_for(HexGuardianStatus::FrozenByAnchor),
        State::FrozenByAnchor
    );
}

/// Each change of state that means something to a player has a sound, and the rest
/// do not: being seen latches, an anchor clamps, being let go unwinds.
#[test]
fn a_change_of_state_is_heard() {
    use State::{FrozenByAnchor, FrozenBySight, Hunting};
    assert_eq!(transition(Hunting, FrozenBySight), Some(Transition::Latch));
    assert_eq!(transition(Hunting, FrozenByAnchor), Some(Transition::Clamp));
    assert_eq!(
        transition(FrozenBySight, FrozenByAnchor),
        Some(Transition::Clamp)
    );
    assert_eq!(
        transition(FrozenBySight, Hunting),
        Some(Transition::Release)
    );
    assert_eq!(
        transition(FrozenByAnchor, Hunting),
        Some(Transition::Release)
    );
    // Anchored and then also seen: already locked, nothing new to hear.
    assert_eq!(transition(FrozenByAnchor, FrozenBySight), None);
    assert_eq!(transition(Hunting, Hunting), None);
}
