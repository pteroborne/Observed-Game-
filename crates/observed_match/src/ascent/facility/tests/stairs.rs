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
    game.ascent
        .stage_card(ARCHITECT, CardKind::Stair)
        .expect("the finite deck deals stairs");
    let mut play = None;
    for _ in 0..3_000 {
        play = a_legal_stair(&game);
        if play.is_some() {
            break;
        }
        step(&mut game, Body::Explore, SeatCommand::None);
    }
    let play = play.expect("the body discovers a legal stair footprint");
    let ArchitectCommand::Play {
        target, rotation, ..
    } = play
    else {
        unreachable!()
    };
    // A stair is a climb composition: a foot, a mid and a high cell along the
    // heading, and the landing above the last.
    let built = game.rules().played_stair(target, rotation);
    let refusals = step(&mut game, Body::Turn(0.0), SeatCommand::Architect(play));
    assert!(refusals.is_empty(), "the rules took it: {refusals:?}");

    // All four cells stand in the rules and in the physical facility, with geometry.
    for placement in built {
        let cell = placement.coord;
        assert!(matches!(placement.archetype, HexArchetype::Climb { .. }));
        assert_eq!(
            game.rules().world.placements[&cell].archetype,
            placement.archetype
        );
        assert_eq!(
            game.physical().facility.placements[&cell].archetype,
            placement.archetype,
            "the physical match built {cell:?}"
        );
        assert!(
            game.physical()
                .geometry
                .pieces
                .iter()
                .any(|piece| piece.source_cell == cell),
            "{cell:?} has geometry"
        );
    }
    let [foot, _, high, landing] = built.map(|placement| placement.coord);
    assert_eq!(foot, target, "the play's target is the foot");
    // The way up runs through it: the high cell's exits reach the landing, when the
    // lights are on.
    if game.rules().economy.is_powered(high.level) && game.rules().economy.is_powered(landing.level)
    {
        assert!(game.rules().exits(high).contains(&landing));
    }
    // And it is fixed now: no tile play can take any of it away.
    assert!(
        built
            .iter()
            .all(|placement| game.rules().fixed_structure(placement.coord))
    );
    assert_geometry_is_fresh(&game);
}

#[test]
fn a_turned_stair_can_be_played_and_built() {
    let mut game = game(7);
    let mut play = None;
    for _ in 0..3_000 {
        let hand = game.session().hands.get(&TEAM).map(|h| &h.deck.hand);
        let known = game.rules().team_knowledge.get(&TEAM).map(|k| &k.cells);
        if let (Some(hand), Some(known)) = (hand, known) {
            play = hand
                .iter()
                .filter(|card| card.kind == CardKind::Stair)
                .find_map(|card| {
                    known.keys().find_map(|&target| {
                        (6..120).find_map(|rotation| {
                            let p = ArchitectCommand::Play {
                                card: card.id,
                                target,
                                rotation,
                            };
                            game.session()
                                .architect_refusal(ARCHITECT, p)
                                .is_none()
                                .then_some(p)
                        })
                    })
                });
        }
        if play.is_some() {
            break;
        }
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
    let play = play.expect("a turned stair becomes legal as the team maps the ground floor");
    let ArchitectCommand::Play {
        target, rotation, ..
    } = play
    else {
        unreachable!()
    };
    assert!(
        rotation >= 6,
        "must be a turned stair (rotation {rotation})"
    );
    let (_heading, turn, exit) = observed_facility::hex_wfc::stair_shape(rotation);
    assert!(
        turn != observed_facility::hex_wfc::ClimbTurn::Ahead
            || exit != observed_facility::hex_wfc::ClimbTurn::Ahead
    );
    let built = game.rules().played_stair(target, rotation);
    let refusals = step(&mut game, Body::Turn(0.0), SeatCommand::Architect(play));
    assert!(refusals.is_empty(), "the rules took it: {refusals:?}");
    for placement in built {
        let cell = placement.coord;
        assert!(matches!(placement.archetype, HexArchetype::Climb { .. }));
        assert_eq!(
            game.rules().world.placements[&cell].archetype,
            placement.archetype
        );
        assert_eq!(
            game.physical().facility.placements[&cell].archetype,
            placement.archetype
        );
    }
    assert_geometry_is_fresh(&game);
}
