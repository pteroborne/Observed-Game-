//! The first-person match's Guardian is the rules' Guardian: where it stands, never moved
//! by the rules, and nothing released into the rules that has no body to hunt with.

use super::*;
use crate::ascent::facility::TUMBLER;
use crate::ascent::sim::{CommandRefusal, GuardianKind};

#[test]
fn the_rules_see_the_tumbler_where_it_stands_and_never_move_it() {
    let mut game = game_with(7, 1, true);
    for _ in 0..1_200 {
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
fn a_requisition_is_refused_rather_than_paid_in_a_guardian_with_no_body() {
    let mut game = game(7);
    step(&mut game, Body::Explore, SeatCommand::None);
    let refusals = step(
        &mut game,
        Body::Explore,
        SeatCommand::Architect(ArchitectCommand::Requisition),
    );
    assert_eq!(
        refusals.get(&ARCHITECT),
        Some(&Refusal::Architect(CommandRefusal::NoRelease))
    );
    assert!(
        game.rules().guardians.keys().all(|&id| id == TUMBLER),
        "nothing released"
    );
}
