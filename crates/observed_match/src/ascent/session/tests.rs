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
