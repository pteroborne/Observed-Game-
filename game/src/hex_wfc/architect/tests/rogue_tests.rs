//! The Rogue board: a player whose body fell into true void plays for the Rogue, from a
//! Rogue hand of their own, on the facility's truth, through their own seat, and sends the
//! major Guardians where they point.

use super::*;

/// A body that falls into true void takes a seat at the Rogue board, and a play made there
/// is the Rogue's: from the hand they joined with, on the facility's truth, through the
/// player's own seat.
#[test]
fn a_corrupted_player_plays_for_the_rogue_from_their_own_seat() {
    let local = PlayerId(0);
    let mut runtime = runtime_as_body(local);
    runtime.match_state.drop_into_void(local);
    let mut desk = ArchitectDesk::rogue(local, AscentTeam(TEAM.0), 0);
    tick_rogue(&mut runtime, &mut desk);
    let ascent = runtime.ascent.as_ref().expect("an Ascent match");
    let id = ascent.observer_for(local).expect("an Observer");
    assert_eq!(
        ascent.rules().observers[&id].state,
        observed_match::ascent::sim::ObserverState::Corrupted
    );
    // The desk reads the Rogue's hand and the facility's truth, never the team's.
    let knowledge = desk
        .knowledge(ascent.rules())
        .expect("the Rogue board's view");
    assert_eq!(knowledge.cells.len(), ascent.rules().world.placements.len());
    let hand = desk.hand(ascent.session()).expect("the Rogue's hand");
    assert_eq!(
        hand.deck.hand,
        ascent.session().rogue_hands[&local].deck.hand,
        "the hand they joined with, not the bot Rogue's"
    );
    assert!(desk.team_requests_are_none(ascent.session()));

    // The first play the rules would take from this seat, played.
    let mut play = None;
    for _ in 0..3_000 {
        let ascent = runtime.ascent.as_ref().expect("an Ascent match");
        let knowledge = desk.knowledge(ascent.rules()).expect("view");
        let hand = desk.hand(ascent.session()).expect("hand");
        play = hand.deck.hand.iter().find_map(|card| {
            knowledge.cells.keys().find_map(|&target| {
                (0..6).find_map(|rotation| {
                    let play = ArchitectCommand::Play {
                        card: card.id,
                        target,
                        rotation,
                    };
                    ascent
                        .session()
                        .architect_refusal(desk.seat, play)
                        .is_none()
                        .then_some(play)
                })
            })
        });
        if play.is_some() {
            break;
        }
        tick_rogue(&mut runtime, &mut desk);
    }
    let play = play.expect("the Rogue has a legal play");
    let before = runtime
        .ascent
        .as_ref()
        .expect("rules")
        .rules()
        .command_log
        .len();
    desk.pending = Some(play);
    tick_rogue(&mut runtime, &mut desk);
    assert_eq!(
        desk.last_refusal, None,
        "the rules refused the Rogue's play"
    );
    let log = &runtime.ascent.as_ref().expect("rules").rules().command_log;
    assert!(log.len() > before && log.iter().any(|&(_, command)| command == play));
}

/// At the Rogue board the answer key sends the major Guardians to the cell pointed at: the
/// rules take the directive from the player's own seat, on the player's own clock, and the
/// Guardians' bodies are told where to walk.
#[test]
fn the_rogue_board_sends_the_guardians_where_it_points() {
    let local = PlayerId(0);
    let mut runtime = runtime_as_body(local);
    runtime.match_state.drop_into_void(local);
    let mut desk = ArchitectDesk::rogue(local, AscentTeam(TEAM.0), 0);
    tick_rogue(&mut runtime, &mut desk);

    let ascent = runtime.ascent.as_ref().expect("an Ascent match");
    let target = desk
        .knowledge(ascent.rules())
        .expect("the Rogue board's view")
        .cells
        .keys()
        .copied()
        .find(|&cell| {
            ascent
                .session()
                .architect_refusal(desk.seat, ArchitectCommand::Direct { target: cell })
                .is_none()
        })
        .expect("somewhere to send the Guardians");
    let bot_clock = ascent.rules().cooldown;
    desk.look_at(target.level);
    desk.hovered = Some(target);
    super::super::input::direct_guardians(&mut desk, &runtime);
    assert_eq!(desk.pending, Some(ArchitectCommand::Direct { target }));
    tick_rogue(&mut runtime, &mut desk);

    let ascent = runtime.ascent.as_ref().expect("an Ascent match");
    assert_eq!(desk.pending, None, "the desk handed the directive over");
    assert_eq!(
        ascent.rules().directed.map(|directive| directive.cell),
        Some(target)
    );
    assert_eq!(runtime.match_state.guardian_directive(), Some(target));
    assert!(desk.hand(ascent.session()).expect("hand").cooldown > 0);
    assert!(
        ascent.rules().cooldown <= bot_clock,
        "the bot Rogue's clock is not the player's"
    );
}

/// A match with the local player walking as a body, every seat of the rules a bot's.
fn runtime_as_body(local: PlayerId) -> HexWfcRuntime {
    assert_eq!(local, PlayerId(0));
    runtime_seated(None)
}

/// One tick at the Rogue board: bodies walked by the bot, the lost one standing, the
/// board's view read again as the desk does each frame.
fn tick_rogue(runtime: &mut HexWfcRuntime, desk: &mut ArchitectDesk) {
    tick(runtime, desk);
    let rules = runtime.ascent.as_ref().expect("rules").rules();
    desk.rogue_sight = Some((rules.tick, rules.rogue_view()));
}
