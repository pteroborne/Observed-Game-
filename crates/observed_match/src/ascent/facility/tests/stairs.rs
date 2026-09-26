//! Stair cards on the real facility: the one play that builds the way up.

use observed_facility::hex_wfc::HexArchetype;

use super::*;
use crate::ascent::sim::CardKind;

/// A stair play the rules would take from the Architect's seat now, if there is one.
fn a_legal_stair(game: &AscentMatch) -> Option<ArchitectCommand> {
    let hand = &game.session().hands.get(&TEAM)?.deck.hand;
    let known = &game.rules().team_knowledge.get(&TEAM)?.cells;
    hand.iter()
        .filter(|card| card.kind == CardKind::Stair)
        .find_map(|card| {
            known.keys().find_map(|&target| {
                (0..6).find_map(|rotation| {
                    let play = ArchitectCommand::Play {
                        card: card.id,
                        target,
                        rotation,
                    };
                    game.session()
                        .architect_refusal(ARCHITECT, play)
                        .is_none()
                        .then_some(play)
                })
            })
        })
}

#[test]
fn an_architect_deals_stairs_and_a_stair_play_builds_the_climb() {
    let mut game = game(7);
    let deck_has_stairs = game.session().hands[&TEAM]
        .deck
        .hand
        .iter()
        .any(|card| card.kind == CardKind::Stair);
    let mut play = None;
    for _ in 0..3_000 {
        play = a_legal_stair(&game);
        if play.is_some() {
            break;
        }
        // Play a requisition now and then, so a hand with no stair draws again.
        let requisition = game.rules().tick % 600 == 599;
        step(
            &mut game,
            Body::Explore,
            if requisition {
                SeatCommand::Architect(ArchitectCommand::Requisition)
            } else {
                SeatCommand::None
            },
        );
    }
    let play = play.unwrap_or_else(|| {
        panic!("a stair was never legal (dealt one at the start: {deck_has_stairs})")
    });
    let ArchitectCommand::Play { target, .. } = play else {
        unreachable!()
    };
    let refusals = step(&mut game, Body::Turn(0.0), SeatCommand::Architect(play));
    assert!(refusals.is_empty(), "the rules took it: {refusals:?}");

    // Both halves stand in the rules and in the physical facility.
    let head = HexCoord {
        level: target.level + 1,
        ..target
    };
    for (cell, archetype) in [
        (target, HexArchetype::RampUp),
        (head, HexArchetype::RampHead),
    ] {
        assert_eq!(game.rules().world.placements[&cell].archetype, archetype);
        assert_eq!(
            game.physical().facility.placements[&cell].archetype,
            archetype,
            "the physical match built {cell:?}"
        );
    }
    assert!(
        game.physical()
            .geometry
            .pieces
            .iter()
            .any(|piece| piece.source_cell == target),
        "the ramp has geometry"
    );
    // The way up runs through it: the foot's exits reach the head, when the lights are on.
    if game.rules().economy.is_powered(target.level) && game.rules().economy.is_powered(head.level)
    {
        assert!(game.rules().exits(target).contains(&head));
    }
    // And it is fixed now: no tile play can take a stair away.
    assert!(game.rules().fixed_structure(target) && game.rules().fixed_structure(head));
    assert_geometry_is_fresh(&game);
}
