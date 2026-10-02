//! The first-person match's Guardian is the rules' Guardian: where it stands, never moved
//! by the rules, and nothing released into the rules that has no body to hunt with.

use super::*;
use crate::ascent::facility::TUMBLER;
use crate::ascent::sim::{GuardianId, GuardianKind};
use crate::hex_wfc::HexReleasedKind;

#[test]
fn the_rules_see_the_tumbler_where_it_stands_and_never_move_it() {
    let mut game = game_with(7, 1, true);
    // Until the match ends: a lone body the Tumbler catches is every loyal Observer
    // jailed, which is the Rogue's win.
    for _ in 0..1_200 {
        if !running(&game) {
            break;
        }
        step(&mut game, Body::Explore, SeatCommand::None);
        let physical = game.physical().guardian.cell;
        let rules = game
            .rules()
            .guardians
            .get(&TUMBLER)
            .expect("the Tumbler is in the rules");
        assert_eq!(rules.cell, physical, "at tick {}", game.rules().tick);
        assert_eq!(rules.kind, GuardianKind::Major);
    }
}

#[test]
fn a_match_without_a_guardian_gives_the_rules_none() {
    let mut game = game_with(7, 1, false);
    for _ in 0..60 {
        step(&mut game, Body::Explore, SeatCommand::None);
    }
    assert!(game.rules().guardians.is_empty());
}

#[test]
fn a_requisition_releases_a_major_the_facility_gives_a_body() {
    let mut game = game(7);
    step(&mut game, Body::Explore, SeatCommand::None);
    let refusals = step(
        &mut game,
        Body::Explore,
        SeatCommand::Architect(ArchitectCommand::Requisition),
    );
    assert!(refusals.is_empty(), "{refusals:?}");
    assert_eq!(game.rules().deck.hand.len(), 5, "the hand is refilled");
    let (&id, major) = game
        .physical()
        .released
        .iter()
        .next()
        .expect("the price is paid in a Guardian with a body");
    assert_eq!(major.kind(), HexReleasedKind::Major);
    step(&mut game, Body::Explore, SeatCommand::None);
    let rules = &game.rules().guardians[&GuardianId(id)];
    assert_eq!(rules.kind, GuardianKind::Major);
    assert_eq!(rules.cell, game.physical().released[&id].cell());
}

#[test]
fn a_disturbance_wave_releases_minors_the_rules_follow_and_never_move() {
    let mut game = game_with(7, 1, false);
    step(&mut game, Body::Explore, SeatCommand::None);
    let level = game.physical().players[&BODY].cell.level;
    let wave = game.ascent.session.sim.add_disturbance(level, 100);
    assert!(!wave.is_empty(), "a threshold crossed releases a wave");
    assert!(
        wave.iter()
            .all(|id| !game.rules().guardians.contains_key(id)),
        "nothing hunts in the rules before it has a body"
    );
    step(&mut game, Body::Explore, SeatCommand::None);
    for id in &wave {
        assert_eq!(
            game.physical().released.get(&id.0).map(|g| g.kind()),
            Some(HexReleasedKind::Minor),
            "{id:?} has a body"
        );
    }
    for _ in 0..600 {
        // Until the match ends: the wave may catch the only body, the Rogue's win.
        if !running(&game) {
            break;
        }
        step(&mut game, Body::Explore, SeatCommand::None);
        for (&id, body) in &game.physical().released {
            let rules = &game.rules().guardians[&GuardianId(id)];
            assert_eq!(rules.kind, GuardianKind::Minor);
            assert_eq!(rules.cell, body.cell(), "tick {}", game.rules().tick);
        }
        for id in game.rules().guardians.keys() {
            assert!(
                *id == TUMBLER || game.physical().released.contains_key(&id.0),
                "{id:?} hunts in the rules with no body"
            );
        }
    }
}

#[test]
fn a_collapsed_floor_takes_its_minors_out_of_the_facility_and_the_rules() {
    let mut game = game_with(7, 1, false);
    step(&mut game, Body::Explore, SeatCommand::None);
    let level = game.physical().players[&BODY].cell.level;
    let wave = game.ascent.session.sim.add_disturbance(level, 100);
    step(&mut game, Body::Explore, SeatCommand::None);
    step(&mut game, Body::Explore, SeatCommand::None);
    assert!(
        wave.iter()
            .all(|id| game.rules().guardians.contains_key(id))
    );
    game.ascent.session.sim.collapsed_floors.insert(level);
    step(&mut game, Body::Explore, SeatCommand::None);
    step(&mut game, Body::Explore, SeatCommand::None);
    for id in &wave {
        assert!(!game.physical().released.contains_key(&id.0));
        assert!(!game.rules().guardians.contains_key(id));
    }
}

fn running(game: &AscentMatch) -> bool {
    game.rules().outcome == crate::ascent::sim::MatchOutcome::Running
}
