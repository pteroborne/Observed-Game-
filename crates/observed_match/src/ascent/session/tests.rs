use super::*;
use crate::ascent::sim::{ArchitectMode, GuardianKind};

fn session() -> AscentSession {
    let sim = ArchitectLab::for_mode(ArchitectMode::Pocket).unwrap();
    let seats = BTreeMap::from([
        (
            PlayerId(0),
            Seat {
                role: Role::Architect(TeamId(0)),
                bot: false,
            },
        ),
        (
            PlayerId(1),
            Seat {
                role: Role::Observer(ObserverId(0)),
                bot: false,
            },
        ),
        (
            PlayerId(2),
            Seat {
                role: Role::Observer(ObserverId(1)),
                bot: true,
            },
        ),
        (
            PlayerId(3),
            Seat {
                role: Role::Rogue,
                bot: false,
            },
        ),
    ]);
    AscentSession::new(sim, 42, seats).unwrap()
}

fn frame(session: &AscentSession, player: PlayerId, command: SeatCommand) -> InputFrame {
    InputFrame {
        version: ASCENT_INPUT_VERSION,
        tick: session.sim.tick + 1,
        commands: BTreeMap::from([(player, command)]),
    }
}

#[test]
fn observer_cannot_submit_an_architect_command() {
    let mut session = session();
    let deck = session.sim.deck.clone();
    let input = frame(
        &session,
        PlayerId(1),
        SeatCommand::Architect(ArchitectCommand::Requisition),
    );
    assert_eq!(
        session.advance(&input).unwrap()[&PlayerId(1)],
        Refusal::WrongRole
    );
    assert_eq!(session.sim.deck, deck);
    assert_eq!(session.sim.requisition.count, 0);
}

#[test]
fn duplicate_frame_does_not_repeat_requisition() {
    let mut session = session();
    let input = frame(
        &session,
        PlayerId(0),
        SeatCommand::Architect(ArchitectCommand::Requisition),
    );
    assert!(session.advance(&input).unwrap().is_empty());
    let count = session.sim.guardians.len();
    assert_eq!(session.advance(&input), Err(Refusal::Tick));
    assert_eq!(session.sim.guardians.len(), count);
    assert_eq!(session.sim.requisition.count, 1);
}

#[test]
fn loyal_requisition_does_not_spend_the_rogue_hand_or_cooldown() {
    let mut session = session();
    session.sim.cooldown = 80;
    let deck = session.sim.deck.clone();
    let before = session
        .sim
        .guardians
        .values()
        .filter(|g| g.kind == GuardianKind::Major)
        .count();
    let input = frame(
        &session,
        PlayerId(0),
        SeatCommand::Architect(ArchitectCommand::Requisition),
    );
    assert!(session.advance(&input).unwrap().is_empty());
    assert_eq!(session.sim.deck, deck);
    assert_eq!(session.sim.cooldown, 79);
    assert_eq!(
        session
            .sim
            .guardians
            .values()
            .filter(|g| g.kind == GuardianKind::Major)
            .count(),
        before + 1
    );
}

#[test]
fn requests_require_team_knowledge_and_acknowledgments_match_the_request() {
    let mut session = session();
    let target = session.sim.observers[&ObserverId(0)].cell;
    let input = frame(
        &session,
        PlayerId(1),
        SeatCommand::Request {
            kind: RequestKind::Route,
            target,
        },
    );
    assert!(session.advance(&input).unwrap().is_empty());
    let created_at = session.requests[&PlayerId(1)].created_at;
    let rogue_ack = frame(
        &session,
        PlayerId(3),
        SeatCommand::Acknowledge {
            author: PlayerId(1),
            created_at,
        },
    );
    assert_eq!(
        session.advance(&rogue_ack).unwrap()[&PlayerId(3)],
        Refusal::WrongRole
    );
    let ack = frame(
        &session,
        PlayerId(0),
        SeatCommand::Acknowledge {
            author: PlayerId(1),
            created_at,
        },
    );
    assert!(session.advance(&ack).unwrap().is_empty());
    assert_eq!(
        session.requests[&PlayerId(1)].acknowledged_by,
        Some(PlayerId(0))
    );
    let hidden = *session
        .sim
        .world
        .placements
        .keys()
        .find(|cell| {
            !session
                .sim
                .team_knowledge(TeamId(0))
                .discovered_cells
                .contains(cell)
        })
        .unwrap();
    let input = frame(
        &session,
        PlayerId(1),
        SeatCommand::Request {
            kind: RequestKind::Route,
            target: hidden,
        },
    );
    assert_eq!(
        session.advance(&input).unwrap()[&PlayerId(1)],
        Refusal::UnknownTarget
    );
    assert_eq!(session.requests[&PlayerId(1)].target, target);
}

#[test]
fn corruption_changes_the_seat_and_an_eliminated_teams_architect_spectates() {
    let mut session = session();
    for observer in session.sim.observers.values_mut() {
        observer.state = ObserverState::Corrupted;
    }
    let input = frame(&session, PlayerId(1), SeatCommand::None);
    session.advance(&input).unwrap();
    assert_eq!(session.seats()[&PlayerId(1)].role, Role::Rogue);
    assert_eq!(
        session.seats()[&PlayerId(0)].role,
        Role::Spectator(TeamId(0))
    );
}

#[test]
fn no_two_seats_can_control_the_same_observer() {
    let session = session();
    let mut seats = session.seats.clone();
    seats.insert(PlayerId(4), seats[&PlayerId(1)]);
    assert!(matches!(
        AscentSession::new(session.sim, 42, seats),
        Err(Refusal::Roster)
    ));
}

#[test]
fn snapshots_expose_only_the_seats_own_hand_and_known_cells() {
    let session = session();
    let architect = session.snapshot(PlayerId(0)).unwrap();
    let observer = session.snapshot(PlayerId(1)).unwrap();
    let rogue = session.snapshot(PlayerId(3)).unwrap();
    assert_eq!(architect.hand, session.hands[&TeamId(0)].deck.hand);
    assert!(observer.hand.is_empty());
    assert_eq!(observer.cells, architect.cells);
    assert!(rogue.cells.len() > architect.cells.len());
    assert_eq!(rogue.hand, session.sim.deck.hand);
    assert_eq!(session.snapshot(PlayerId(99)), Err(Refusal::UnknownSeat));
}

#[test]
fn rejected_unknown_placement_leaves_both_faction_hands_and_clocks_intact() {
    let mut session = session();
    let before = session.hands[&TeamId(0)].deck.clone();
    let rogue = session.sim.deck.clone();
    let target = *session
        .sim
        .world
        .placements
        .keys()
        .find(|cell| {
            !session
                .sim
                .team_knowledge(TeamId(0))
                .discovered_cells
                .contains(cell)
        })
        .unwrap();
    let input = frame(
        &session,
        PlayerId(0),
        SeatCommand::Architect(ArchitectCommand::Play {
            card: before.hand[0].id,
            target,
            rotation: 0,
        }),
    );
    assert_eq!(
        session.advance(&input).unwrap()[&PlayerId(0)],
        Refusal::Architect(CommandRefusal::UnknownTarget)
    );
    assert_eq!(session.hands[&TeamId(0)].deck, before);
    assert_eq!(session.hands[&TeamId(0)].cooldown, 0);
    assert_eq!(session.sim.deck, rogue);
    assert_eq!(session.sim.cooldown, 0);
}

#[test]
fn card_previews_match_commit_refusals_without_mutating_any_seat() {
    let mut session = session();
    session.sim.cooldown = 5;
    let before: Vec<_> = session
        .seats()
        .keys()
        .map(|&id| session.snapshot(id).unwrap())
        .collect();
    for player in [PlayerId(0), PlayerId(3)] {
        for card in session.snapshot(player).unwrap().hand {
            for &target in session.sim.world.placements.keys() {
                for rotation in 0..6 {
                    let command = ArchitectCommand::Play {
                        card: card.id,
                        target,
                        rotation,
                    };
                    let preview = session.architect_refusal(player, command);
                    let commit = session
                        .clone()
                        .accept(player, SeatCommand::Architect(command))
                        .err();
                    assert_eq!(
                        preview, commit,
                        "seat {player:?}, card {card:?}, target {target:?}"
                    );
                }
            }
        }
    }
    let after: Vec<_> = session
        .seats()
        .keys()
        .map(|&id| session.snapshot(id).unwrap())
        .collect();
    assert_eq!(before, after);
}

#[test]
fn request_expiration_and_invalid_protocol_frames_are_authoritative() {
    let mut session = session();
    session.sim.guardians.clear();
    for seat in session.seats.values_mut() {
        seat.bot = false;
    }
    let target = session.sim.observers[&ObserverId(0)].cell;
    let input = frame(
        &session,
        PlayerId(1),
        SeatCommand::Request {
            kind: RequestKind::Rescue,
            target,
        },
    );
    let mut invalid = input.clone();
    invalid.version += 1;
    assert_eq!(session.advance(&invalid), Err(Refusal::Version));
    assert_eq!(session.sim.tick, 0);
    assert!(session.requests.is_empty());
    session.advance(&input).unwrap();
    for _ in 1..REQUEST_LIFETIME_TICKS {
        let input = frame(&session, PlayerId(1), SeatCommand::None);
        session.advance(&input).unwrap();
    }
    assert!(session.requests.is_empty());
    let input = frame(
        &session,
        PlayerId(0),
        SeatCommand::Acknowledge {
            author: PlayerId(1),
            created_at: 0,
        },
    );
    assert_eq!(
        session.advance(&input).unwrap()[&PlayerId(0)],
        Refusal::RequestExpired
    );
}

/// A tick just before a beat, when bot seats decide.
fn at_a_beat(session: &mut AscentSession, beats: u64) {
    let beat = u64::from(crate::ascent::sim::ACTOR_BEAT_TICKS);
    session.sim.tick = beat * beats - 1;
}

#[test]
fn a_bot_observer_names_only_trouble_a_body_there_would_know() {
    let mut session = session();
    let id = ObserverId(1);
    let cell = session.sim.observers[&id].cell;
    session.sim.economy.set_powered(cell.level, true);
    at_a_beat(&mut session, 2);
    session.stalls.insert(id, (cell, session.sim.tick));
    assert_eq!(session.trouble(id), None, "just arrived, lights on");

    at_a_beat(&mut session, 2 + STALL_BEATS);
    assert_eq!(
        session.trouble(id),
        Some((RequestKind::Route, cell)),
        "going nowhere"
    );
    session.sim.economy.set_powered(cell.level, false);
    assert_eq!(session.trouble(id), Some((RequestKind::Power, cell)));
    session.sim.observers.get_mut(&id).expect("observer").state = ObserverState::Jailed;
    assert_eq!(session.trouble(id), Some((RequestKind::Rescue, cell)));
}

#[test]
fn a_bot_observer_asks_and_withdraws_but_never_for_a_human() {
    let mut session = session();
    let cell = session.sim.observers[&ObserverId(1)].cell;
    for id in [ObserverId(0), ObserverId(1)] {
        session.sim.observers.get_mut(&id).expect("observer").state = ObserverState::Jailed;
    }
    at_a_beat(&mut session, 3);
    session.run_bot_requests();
    let asked = session.requests.get(&PlayerId(2)).expect("the bot asks");
    assert_eq!((asked.kind, asked.target), (RequestKind::Rescue, cell));
    assert!(
        !session.requests.contains_key(&PlayerId(1)),
        "a human asks for themselves"
    );

    // Free, lit, and just arrived: the trouble has passed, and so has the ask.
    session
        .sim
        .observers
        .get_mut(&ObserverId(1))
        .expect("observer")
        .state = ObserverState::Active;
    session.sim.economy.set_powered(cell.level, true);
    at_a_beat(&mut session, 4);
    session
        .stalls
        .insert(ObserverId(1), (cell, session.sim.tick));
    session.run_bot_requests();
    assert!(!session.requests.contains_key(&PlayerId(2)));
}

#[test]
fn a_bot_architect_acknowledges_its_team_and_is_asked_for_routes_oldest_first() {
    let mut session = session();
    session
        .seats
        .get_mut(&PlayerId(0))
        .expect("the Architect")
        .bot = true;
    let cells: Vec<HexCoord> = session
        .sim
        .team_knowledge(TeamId(0))
        .discovered_cells
        .into_iter()
        .take(2)
        .collect();
    for (author, (&target, created_at)) in [PlayerId(1), PlayerId(2)]
        .into_iter()
        .zip(cells.iter().zip([5, 3]))
    {
        session.requests.insert(
            author,
            TeamRequest {
                author,
                team: TeamId(0),
                kind: RequestKind::Route,
                target,
                created_at,
                acknowledged_by: None,
            },
        );
    }
    session.sim.tick = 10;
    session.acknowledge_as_bot(TeamId(0));
    assert!(
        session
            .requests
            .values()
            .all(|request| request.acknowledged_by == Some(PlayerId(0)))
    );
    assert_eq!(session.asked_routes(TeamId(0)), vec![cells[1], cells[0]]);
}

#[test]
fn a_rescue_may_point_at_the_lobby_every_team_knows_and_nothing_else_may() {
    let mut session = session();
    let lobby = *session.sim.prison.cells.iter().next().expect("a lobby");
    if let Some(known) = session.sim.team_knowledge.get_mut(&TeamId(0)) {
        known.discovered_cells.remove(&lobby);
        known.cells.remove(&lobby);
    }
    assert!(
        !session
            .sim
            .team_knowledge(TeamId(0))
            .discovered_cells
            .contains(&lobby),
        "the team has not found the lobby"
    );
    let route = SeatCommand::Request {
        kind: RequestKind::Route,
        target: lobby,
    };
    assert_eq!(
        session.accept(PlayerId(1), route),
        Err(Refusal::UnknownTarget)
    );
    let rescue = SeatCommand::Request {
        kind: RequestKind::Rescue,
        target: lobby,
    };
    assert_eq!(session.accept(PlayerId(1), rescue), Ok(None));
    assert_eq!(session.requests[&PlayerId(1)].target, lobby);
}

#[test]
fn a_body_asks_for_what_its_place_says_it_needs() {
    let mut session = session();
    let id = ObserverId(0);
    let (cell, facing) = {
        let observer = &session.sim.observers[&id];
        (observer.cell, observer.facing)
    };
    session.sim.economy.set_powered(cell.level, true);
    let ahead = session.sim.world.config.grid().neighbor(cell, facing);
    let known = session.sim.team_knowledge(TeamId(0)).discovered_cells;
    let expected = ahead.filter(|cell| known.contains(cell)).unwrap_or(cell);
    assert_eq!(
        session.ask_for_help(PlayerId(1)),
        Some((RequestKind::Route, expected)),
        "a route on from where it faces, if found, else from where it stands"
    );

    // Facing a cell the team has not found: from where it stands.
    if let Some(ahead) = ahead
        && let Some(knowledge) = session.sim.team_knowledge.get_mut(&TeamId(0))
    {
        knowledge.discovered_cells.remove(&ahead);
        knowledge.cells.remove(&ahead);
        assert_eq!(
            session.ask_for_help(PlayerId(1)),
            Some((RequestKind::Route, cell))
        );
    }

    session.sim.economy.set_powered(cell.level, false);
    assert_eq!(
        session.ask_for_help(PlayerId(1)),
        Some((RequestKind::Power, cell))
    );
    session.sim.observers.get_mut(&id).expect("observer").state = ObserverState::Jailed;
    assert_eq!(
        session.ask_for_help(PlayerId(1)),
        Some((RequestKind::Rescue, cell))
    );
    assert_eq!(
        session.ask_for_help(PlayerId(0)),
        None,
        "an Architect is no body"
    );
}

#[test]
fn a_hand_with_nothing_for_its_team_s_floor_draws_something_that_is() {
    use crate::ascent::sim::{CardKind, District};
    let mut session = session();
    let floor = session
        .sim
        .district(session.sim.observers[&ObserverId(0)].cell.level);
    // Any other district's: the fixture is a single floor, so the sky's will do.
    let other = District::for_floor(7, 8);
    assert_ne!(other, floor);
    // A dead hand: the other district's tiles and doors only.
    let hand = session.hands.get_mut(&TeamId(0)).expect("a hand");
    for card in &mut hand.deck.hand {
        if matches!(card.kind, CardKind::Tile(_)) {
            card.district = Some(other);
        }
    }
    assert!(!session.hands[&TeamId(0)].deck.has_tile_for(floor));
    session.sim.tick = 3 * u64::from(crate::ascent::sim::ACTOR_BEAT_TICKS) + 1;
    session.keep_hands_live();
    assert!(
        !session.hands[&TeamId(0)].deck.has_tile_for(floor),
        "only on the beat"
    );
    session.sim.tick = 3 * u64::from(crate::ascent::sim::ACTOR_BEAT_TICKS);
    session.keep_hands_live();
    assert!(session.hands[&TeamId(0)].deck.has_tile_for(floor));
}

#[test]
fn seated_and_joined_rogue_hands_draw_a_tile_for_a_placeable_floor() {
    use crate::ascent::sim::{CardKind, District};

    let mut session = session();
    session.sim.observers.get_mut(&ObserverId(0)).unwrap().state = ObserverState::Corrupted;
    let input = frame(&session, PlayerId(1), SeatCommand::None);
    session.advance(&input).unwrap();
    assert_eq!(session.seats()[&PlayerId(1)].role, Role::Rogue);
    assert!(session.sim.mutable_targets().iter().any(|c| c.level == 0));

    let upper = District::for_floor(7, 8);
    assert_ne!(upper, District::GROUND);
    // Both Rogue hands have only the other district's tiles and districtless cards.
    // Neither player can repair the one-floor facility with this hand.
    for deck in [
        &mut session.sim.deck,
        &mut session.rogue_hands.get_mut(&PlayerId(1)).unwrap().deck,
    ] {
        for card in &mut deck.hand {
            if matches!(card.kind, CardKind::Tile(_)) {
                card.district = Some(upper);
            }
        }
        assert!(!deck.has_tile_for(District::GROUND));
    }

    session.sim.tick = 3 * u64::from(crate::ascent::sim::ACTOR_BEAT_TICKS);
    session.keep_hands_live();
    assert!(session.sim.deck.has_tile_for(District::GROUND));
    assert!(
        session.rogue_hands[&PlayerId(1)]
            .deck
            .has_tile_for(District::GROUND)
    );
}

/// A built cell of the facility no Guardian stands on and the prison does not hold.
fn open_ground(session: &AscentSession) -> HexCoord {
    let sim = &session.sim;
    *sim.world
        .placements
        .iter()
        .find(|(cell, placement)| {
            placement.space.built()
                && !sim.prison_core.contains(cell)
                && !sim.prison.cells.contains(cell)
                && !sim.guardians.values().any(|g| g.cell == **cell)
        })
        .expect("the facility has open ground")
        .0
}

/// The Rogue's own deck in the hand `player` plays from: a seated Rogue's is the rules',
/// a joined player's their own, an Architect's their team's.
fn deal_rogue_deck(session: &mut AscentSession, player: PlayerId) {
    let deck = crate::ascent::sim::Deck::rogue(
        7,
        session.sim.world.config.levels,
        &crate::ascent::sim::TileShape::ALL,
    );
    if let Some(hand) = session.rogue_hands.get_mut(&player) {
        hand.deck = deck;
    } else if let Some(team) = session.team(player) {
        session.hands.get_mut(&team).expect("a team's hand").deck = deck;
    } else {
        session.sim.deck = deck;
    }
}

/// `player` playing a card of `kind` on `target`, staged into their hand.
fn order(
    session: &mut AscentSession,
    player: PlayerId,
    kind: crate::ascent::sim::CardKind,
    target: HexCoord,
) -> ArchitectCommand {
    let deck = if let Some(hand) = session.rogue_hands.get_mut(&player) {
        &mut hand.deck
    } else if let Some(team) = session.team(player) {
        &mut session.hands.get_mut(&team).expect("a team's hand").deck
    } else {
        &mut session.sim.deck
    };
    assert!(deck.stage_kind(kind), "the deck deals {kind:?}");
    let card = deck
        .hand
        .iter()
        .find(|card| card.kind == kind)
        .expect("staged")
        .id;
    ArchitectCommand::Play {
        card,
        target,
        rotation: 0,
    }
}

#[test]
fn only_the_rogue_directs_the_guardians() {
    use crate::ascent::sim::CardKind;
    let mut session = session();
    deal_rogue_deck(&mut session, PlayerId(0));
    deal_rogue_deck(&mut session, PlayerId(3));
    let target = open_ground(&session);
    let loyal = order(&mut session, PlayerId(0), CardKind::Directive, target);
    assert_eq!(
        session.architect_refusal(PlayerId(0), loyal),
        Some(Refusal::Architect(CommandRefusal::RogueOnly))
    );
    let input = frame(&session, PlayerId(0), SeatCommand::Architect(loyal));
    assert_eq!(
        session.advance(&input).unwrap()[&PlayerId(0)],
        Refusal::Architect(CommandRefusal::RogueOnly)
    );
    assert_eq!(session.sim.directed, None);

    let direct = order(&mut session, PlayerId(3), CardKind::Directive, target);
    let ArchitectCommand::Play { card, .. } = direct else {
        unreachable!("a card play")
    };
    let input = frame(&session, PlayerId(3), SeatCommand::Architect(direct));
    assert!(session.advance(&input).unwrap().is_empty());
    let directive = session.sim.directed.expect("the Rogue's directive stands");
    assert_eq!(directive.cell, target);
    assert!(
        session.sim.deck.hand.iter().all(|held| held.id != card),
        "a directive spends its card"
    );
    assert!(session.sim.cooldown > 0, "a directive starts the cooldown");
    let again = order(&mut session, PlayerId(3), CardKind::Directive, target);
    let input = frame(&session, PlayerId(3), SeatCommand::Architect(again));
    assert_eq!(
        session.advance(&input).unwrap()[&PlayerId(3)],
        Refusal::Architect(CommandRefusal::Cooldown)
    );
}

#[test]
fn only_the_rogue_can_play_an_instability_surge() {
    use crate::ascent::sim::CardKind;
    let mut session = session();
    deal_rogue_deck(&mut session, PlayerId(0));
    deal_rogue_deck(&mut session, PlayerId(3));
    let target = open_ground(&session);
    let loyal = order(&mut session, PlayerId(0), CardKind::Surge, target);
    assert_eq!(
        session.architect_refusal(PlayerId(0), loyal),
        Some(Refusal::Architect(CommandRefusal::RogueOnly))
    );
    let before = session.sim.economy.disturbance(target.level);
    let rogue = order(&mut session, PlayerId(3), CardKind::Surge, target);
    let input = frame(&session, PlayerId(3), SeatCommand::Architect(rogue));
    assert!(session.advance(&input).unwrap().is_empty());
    assert_eq!(session.sim.economy.disturbance(target.level), before + 50);
    assert!(session.sim.cooldown > 0);
}

#[test]
fn a_directive_nobody_reaches_runs_out() {
    let mut session = session();
    // Out of every Guardian's reach: nothing hunts in this one's rules.
    session.sim.guardians.clear();
    deal_rogue_deck(&mut session, PlayerId(3));
    let target = open_ground(&session);
    let direct = order(
        &mut session,
        PlayerId(3),
        crate::ascent::sim::CardKind::Directive,
        target,
    );
    let input = frame(&session, PlayerId(3), SeatCommand::Architect(direct));
    assert!(session.advance(&input).unwrap().is_empty());
    for _ in 1..crate::ascent::sim::DIRECTIVE_TICKS {
        assert!(session.sim.directed.is_some(), "tick {}", session.sim.tick);
        let input = frame(&session, PlayerId(3), SeatCommand::None);
        if session.advance(&input).is_err() {
            return; // The match ended first.
        }
    }
    let input = frame(&session, PlayerId(3), SeatCommand::None);
    if session.advance(&input).is_ok() {
        assert_eq!(session.sim.directed, None);
    }
}

#[test]
fn a_player_who_joins_the_rogue_plays_their_own_hand_on_their_own_clock() {
    let mut session = session();
    session.sim.observers.get_mut(&ObserverId(0)).unwrap().state = ObserverState::Corrupted;
    let input = frame(&session, PlayerId(1), SeatCommand::None);
    session.advance(&input).unwrap();
    assert_eq!(session.seats()[&PlayerId(1)].role, Role::Rogue);
    let joined = session.rogue_hands[&PlayerId(1)].clone();
    assert_eq!(joined.cooldown, 0);
    assert_eq!(
        session.snapshot(PlayerId(1)).unwrap().hand,
        joined.deck.hand,
        "the board shows the hand they joined with"
    );

    deal_rogue_deck(&mut session, PlayerId(1));
    deal_rogue_deck(&mut session, PlayerId(3));
    let target = open_ground(&session);
    let direct = order(
        &mut session,
        PlayerId(1),
        crate::ascent::sim::CardKind::Directive,
        target,
    );
    let input = frame(&session, PlayerId(1), SeatCommand::Architect(direct));
    assert!(session.advance(&input).unwrap().is_empty());
    assert!(session.rogue_hands[&PlayerId(1)].cooldown > 0);
    assert_eq!(
        session.sim.cooldown, 0,
        "the rules' own Rogue clock is untouched"
    );
    // The seated Rogue's clock is its own: nothing waits on the joined player's.
    let other = order(
        &mut session,
        PlayerId(3),
        crate::ascent::sim::CardKind::Sensor,
        target,
    );
    assert_ne!(
        session.architect_refusal(PlayerId(3), other),
        Some(Refusal::Architect(CommandRefusal::Cooldown))
    );
}

/// A cell an active Observer of `session` can be seen from along an open line, and that
/// Observer: where a sensor would watch it.
fn beside_an_observer(session: &AscentSession) -> (ObserverId, HexCoord) {
    let sim = &session.sim;
    sim.observers
        .values()
        .filter(|o| o.state == ObserverState::Active)
        .find_map(|o| {
            sim.exits(o.cell)
                .into_iter()
                .find(|next| next.level == o.cell.level && !sim.prison.cells.contains(next))
                .map(|next| (o.id, next))
        })
        .expect("an Observer with a way out")
}

#[test]
fn a_live_sensor_shows_the_rogue_who_it_sees_and_a_dark_one_does_not() {
    let mut session = session();
    session.sim.guardians.clear();
    let (id, beside) = beside_an_observer(&session);
    assert!(
        !session
            .sim
            .rogue_knowledge()
            .known_observers
            .contains_key(&id),
        "nothing detects it yet"
    );
    session.sim.sensors.insert(beside, session.sim.tick);
    assert!(session.sim.sensor_watching(beside));
    assert!(
        session
            .sim
            .rogue_knowledge()
            .known_observers
            .contains_key(&id)
    );
    // The Rogue seat's own snapshot reads the same knowledge.
    assert!(
        session
            .snapshot(PlayerId(3))
            .unwrap()
            .observers
            .iter()
            .any(|o| o.id == id)
    );

    session.sim.economy.set_powered(beside.level, false);
    assert!(!session.sim.sensor_live(beside));
    assert!(
        !session
            .sim
            .rogue_knowledge()
            .known_observers
            .contains_key(&id),
        "a dark floor blinds its sensors"
    );
}

#[test]
fn only_the_rogue_installs_sensors_out_of_sight_and_keeps_four() {
    let mut session = session();
    let seen = *session
        .sim
        .observed
        .iter()
        .find(|cell| session.sim.world.placements[cell].space.built())
        .expect("an Observer holds something in view");
    use crate::ascent::sim::CardKind;
    deal_rogue_deck(&mut session, PlayerId(0));
    deal_rogue_deck(&mut session, PlayerId(3));
    let loyal = order(&mut session, PlayerId(0), CardKind::Sensor, seen);
    assert_eq!(
        session.architect_refusal(PlayerId(0), loyal),
        Some(Refusal::Architect(CommandRefusal::RogueOnly))
    );
    let watched = order(&mut session, PlayerId(3), CardKind::Sensor, seen);
    assert_eq!(
        session.architect_refusal(PlayerId(3), watched),
        Some(Refusal::Architect(CommandRefusal::Observed))
    );

    let unseen: Vec<HexCoord> = session
        .sim
        .world
        .placements
        .iter()
        .filter(|(cell, placement)| {
            placement.space.built()
                && !session.sim.observed.contains(cell)
                && !session.sim.prison.cells.contains(cell)
                && !session.sim.prison_core.contains(cell)
        })
        .map(|(&cell, _)| cell)
        .take(crate::ascent::sim::MAX_SENSORS + 1)
        .collect();
    assert_eq!(unseen.len(), crate::ascent::sim::MAX_SENSORS + 1);
    for &target in &unseen {
        session.sim.cooldown = 0;
        let sense = order(&mut session, PlayerId(3), CardKind::Sensor, target);
        let input = frame(&session, PlayerId(3), SeatCommand::Architect(sense));
        let refusals = session.advance(&input).unwrap();
        assert!(refusals.is_empty(), "{target:?}: {refusals:?}");
        assert!(session.sim.cooldown > 0, "a sensor spends the cooldown");
    }
    assert_eq!(session.sim.sensors.len(), crate::ascent::sim::MAX_SENSORS);
    assert!(
        !session.sim.sensors.contains_key(&unseen[0]),
        "the oldest retires past the cap"
    );
    assert!(
        session
            .sim
            .sensors
            .contains_key(&unseen[crate::ascent::sim::MAX_SENSORS])
    );
}

#[test]
fn an_observer_beside_a_sensor_dismantles_it_and_one_away_cannot() {
    use crate::ascent::sim::{ObserverAction, ObserverRefusal};
    let mut session = session();
    let (id, beside) = beside_an_observer(&session);
    let far = *session
        .sim
        .world
        .placements
        .keys()
        .find(|cell| observed_hex::travel_distance(**cell, session.sim.observers[&id].cell) > 3)
        .expect("somewhere far");
    session.sim.sensors.insert(beside, 0);
    session.sim.sensors.insert(far, 0);
    let dismantle = |cell| ObserverCommand {
        facing: None,
        action: ObserverAction::Dismantle(cell),
    };
    assert_eq!(
        session.sim.submit_observer(id, dismantle(far)),
        Err(ObserverRefusal::OutOfReach)
    );
    assert_eq!(session.sim.submit_observer(id, dismantle(beside)), Ok(()));
    assert!(!session.sim.sensors.contains_key(&beside));
    assert!(session.sim.sensors.contains_key(&far));
}
