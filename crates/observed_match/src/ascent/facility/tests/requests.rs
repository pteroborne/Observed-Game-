//! Requests on the real facility: a body a bot drives asks its Architect for help, and a
//! bot Architect acknowledges and answers.

use super::*;
use crate::ascent::session::{RequestKind, STALL_BEATS};
use crate::ascent::sim::ACTOR_BEAT_TICKS;

/// A facility the body has walked for a while, with a bot Architect, the body voiced.
fn walked(ticks: usize) -> AscentMatch {
    let mut game = game_seated(7, 1, false, true);
    game.voice([BODY]);
    for _ in 0..ticks {
        step(&mut game, Body::Explore, SeatCommand::None);
    }
    game
}

#[test]
fn a_body_left_standing_asks_for_a_route_and_its_bot_architect_answers_it() {
    let mut game = walked(600);
    let stand = game.rules().observers[&ObserverId(BODY.0)].cell;
    let beat = u64::from(ACTOR_BEAT_TICKS);
    let mut asked = None;
    for _ in 0..(STALL_BEATS + 3) * beat {
        step(&mut game, Body::Turn(0.0), SeatCommand::None);
        if let Some(request) = game.session().requests.get(&BODY) {
            asked = Some(*request);
            break;
        }
    }
    let request = asked.expect("a body going nowhere asks for help");
    assert_eq!(
        (request.kind, request.target),
        (RequestKind::Route, stand),
        "for a route, where it stands"
    );
    assert!(
        game.physical().players[&BODY].cell == stand,
        "it asked only once it had stood still for {STALL_BEATS} beats"
    );

    // The bot Architect acknowledges on its beat, and builds toward the ask.
    let mut acknowledged = false;
    let mut answered = None;
    for _ in 0..beat * 12 {
        step(&mut game, Body::Turn(0.0), SeatCommand::None);
        acknowledged |= game
            .session()
            .requests
            .get(&BODY)
            .is_some_and(|request| request.acknowledged_by == Some(ARCHITECT));
        if game
            .rules()
            .traces
            .get("Architect 0")
            .and_then(|trace| trace.selected)
            == Some("answer a request")
        {
            answered = game.rules().command_log.last().map(|(_, command)| *command);
            break;
        }
    }
    assert!(acknowledged, "the bot Architect acknowledged the ask");
    let Some(ArchitectCommand::Play { target, .. }) = answered else {
        panic!("the bot Architect never answered the ask with a play");
    };
    assert!(
        observed_hex::travel_distance(stand, target) <= 3,
        "{target:?} answers an ask at {stand:?}"
    );
}

#[test]
fn a_body_the_player_drives_is_not_asked_for() {
    let mut game = game_seated(7, 1, false, true);
    for _ in 0..600 {
        step(&mut game, Body::Explore, SeatCommand::None);
    }
    let beat = u64::from(ACTOR_BEAT_TICKS);
    for _ in 0..(STALL_BEATS + 3) * beat {
        step(&mut game, Body::Turn(0.0), SeatCommand::None);
    }
    assert!(
        game.session().requests.is_empty(),
        "a player asks for themselves"
    );
}
