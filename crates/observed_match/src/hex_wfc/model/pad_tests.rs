//! The pad rules that a peer could disagree about.
//!
//! These target the state machine rather than a solved facility, because the
//! interesting failures — a link completing across teams, a body bouncing
//! between plates forever, a suppression clock that two machines count
//! differently — are all properties of [`HexPadState`] and none of them need a
//! world to reproduce.

use glam::Vec3;
use observed_core::{PlayerId, TeamId};
use observed_hex::HexCoord;

use super::pad::{HexPadState, PAD_REARM_TICKS, PADS_PER_PLAYER};

fn cell(q: u16, r: u16) -> HexCoord {
    HexCoord { q, r, level: 0 }
}

#[test]
fn every_player_starts_with_two_plates_because_one_links_nothing() {
    let pads = HexPadState::new([PlayerId(0), PlayerId(1)]);
    assert_eq!(pads.inventory(PlayerId(0)), PADS_PER_PLAYER);
    assert_eq!(pads.inventory(PlayerId(1)), PADS_PER_PLAYER);
    assert_eq!(PADS_PER_PLAYER, 2);
}

#[test]
fn deploying_spends_a_plate_and_refuses_when_empty() {
    let mut pads = HexPadState::new([PlayerId(0)]);
    for index in 0..PADS_PER_PLAYER {
        assert!(
            pads.deploy(PlayerId(0), TeamId(0), cell(index, 0), Vec3::ZERO)
                .is_some(),
            "plate {index} should place"
        );
    }
    assert_eq!(pads.inventory(PlayerId(0)), 0);
    assert!(
        pads.deploy(PlayerId(0), TeamId(0), cell(9, 0), Vec3::ZERO)
            .is_none(),
        "a player carrying nothing places nothing"
    );
}

#[test]
fn a_lone_plate_links_nowhere() {
    let mut pads = HexPadState::new([PlayerId(0)]);
    let first = pads
        .deploy(PlayerId(0), TeamId(0), cell(0, 0), Vec3::ZERO)
        .expect("first plate");
    assert!(
        pads.link_target(TeamId(0), first).is_none(),
        "one pad is not a pair"
    );
}

#[test]
fn a_pair_links_both_ways() {
    let mut pads = HexPadState::new([PlayerId(0)]);
    let a = pads
        .deploy(PlayerId(0), TeamId(0), cell(0, 0), Vec3::ZERO)
        .expect("plate a");
    let b = pads
        .deploy(PlayerId(0), TeamId(0), cell(4, 0), Vec3::new(8.0, 0.0, 0.0))
        .expect("plate b");
    assert_eq!(pads.link_target(TeamId(0), a).map(|pad| pad.id), Some(b));
    assert_eq!(pads.link_target(TeamId(0), b).map(|pad| pad.id), Some(a));
}

/// The behaviour the demoted model named `teleport_pads_are_keyed_to_their_team`.
#[test]
fn a_rival_plate_never_completes_your_link() {
    let mut pads = HexPadState::new([PlayerId(0), PlayerId(1)]);
    let mine = pads
        .deploy(PlayerId(0), TeamId(0), cell(0, 0), Vec3::ZERO)
        .expect("my plate");
    pads.deploy(PlayerId(1), TeamId(1), cell(4, 0), Vec3::new(8.0, 0.0, 0.0))
        .expect("their plate");
    assert!(
        pads.link_target(TeamId(0), mine).is_none(),
        "a rival's plate is not half of my pair"
    );
}

#[test]
fn a_teammates_plate_does_complete_the_link() {
    let mut pads = HexPadState::new([PlayerId(0), PlayerId(1)]);
    let mine = pads
        .deploy(PlayerId(0), TeamId(0), cell(0, 0), Vec3::ZERO)
        .expect("my plate");
    let theirs = pads
        .deploy(PlayerId(1), TeamId(0), cell(4, 0), Vec3::new(8.0, 0.0, 0.0))
        .expect("teammate plate");
    assert_eq!(
        pads.link_target(TeamId(0), mine).map(|pad| pad.id),
        Some(theirs),
        "a pair placed by two people is still a pair"
    );
}

#[test]
fn contact_needs_the_same_cell_and_the_contact_radius() {
    let mut pads = HexPadState::new([PlayerId(0)]);
    pads.deploy(PlayerId(0), TeamId(0), cell(0, 0), Vec3::ZERO)
        .expect("plate");
    assert!(
        pads.pad_under(TeamId(0), cell(0, 0), Vec3::new(0.2, 0.0, 0.0))
            .is_some(),
        "standing on it is contact"
    );
    assert!(
        pads.pad_under(TeamId(0), cell(0, 0), Vec3::new(4.0, 0.0, 0.0))
            .is_none(),
        "the far side of the cell is not contact"
    );
    assert!(
        pads.pad_under(TeamId(0), cell(1, 0), Vec3::ZERO).is_none(),
        "another cell is not contact even at the same position"
    );
}

#[test]
fn height_is_not_ignored_because_the_cell_carries_the_level() {
    let mut pads = HexPadState::new([PlayerId(0)]);
    pads.deploy(PlayerId(0), TeamId(0), cell(0, 0), Vec3::ZERO)
        .expect("plate");
    let upstairs = HexCoord {
        q: 0,
        r: 0,
        level: 1,
    };
    assert!(
        pads.pad_under(TeamId(0), upstairs, Vec3::ZERO).is_none(),
        "a plate on the floor below must not catch a body standing above it"
    );
}

#[test]
fn placing_a_plate_underfoot_suppresses_the_placer() {
    let mut pads = HexPadState::new([PlayerId(0)]);
    pads.deploy(PlayerId(0), TeamId(0), cell(0, 0), Vec3::ZERO)
        .expect("plate");
    assert!(
        pads.is_suppressed(PlayerId(0)),
        "otherwise the second plate of a pair fires the instant it is set down"
    );
}

#[test]
fn suppression_expires_after_exactly_the_rearm_window() {
    let mut pads = HexPadState::new([PlayerId(0)]);
    pads.suppress(PlayerId(0));
    for tick in 0..PAD_REARM_TICKS {
        assert!(
            pads.is_suppressed(PlayerId(0)),
            "still immune at tick {tick}"
        );
        pads.tick_suppression();
    }
    assert!(
        !pads.is_suppressed(PlayerId(0)),
        "immunity ends, or a link becomes unusable for the rest of the match"
    );
}

#[test]
fn an_expired_clock_leaves_no_entry_behind() {
    let mut pads = HexPadState::new([PlayerId(0)]);
    pads.suppress(PlayerId(0));
    for _ in 0..PAD_REARM_TICKS {
        pads.tick_suppression();
    }
    assert!(
        pads.suppressed.is_empty(),
        "a zero entry would still be folded into the digest and read as immune"
    );
}

#[test]
fn suppression_is_per_player() {
    let mut pads = HexPadState::new([PlayerId(0), PlayerId(1)]);
    pads.suppress(PlayerId(0));
    assert!(pads.is_suppressed(PlayerId(0)));
    assert!(
        !pads.is_suppressed(PlayerId(1)),
        "one player's jump must not disarm another's pads"
    );
}
