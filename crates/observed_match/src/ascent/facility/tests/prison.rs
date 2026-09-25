//! The prison: a lobby in the facility, and a maze in a space of its own.

use glam::Vec3;
use observed_hex::{FLOOR_SLAB_TOP, hex_origin};

use super::*;
use crate::ascent::session::Role;
use crate::ascent::sim::{ACTOR_BEAT_TICKS, ObserverState};
use crate::hex_wfc::HexBodyPlace;
use crate::hex_wfc::LOBBY_HOLD_TICKS;

const OTHER: PlayerId = PlayerId(1);
/// The team as the physical match numbers it.
const SIDE: observed_core::TeamId = observed_core::TeamId(0);

fn place(game: &AscentMatch, player: PlayerId) -> HexBodyPlace {
    game.physical().players[&player].place
}

fn state(game: &AscentMatch, player: PlayerId) -> ObserverState {
    game.rules().observers[&ObserverId(player.0)].state
}

fn events(game: &AscentMatch, kind: HexMatchEventKind) -> Vec<PlayerId> {
    game.physical()
        .recent_events
        .iter()
        .filter(|event| event.kind == kind)
        .filter_map(|event| event.player)
        .collect()
}

fn lobby(game: &AscentMatch) -> BTreeSet<HexCoord> {
    game.physical()
        .prison
        .as_ref()
        .expect("an Ascent match has a prison")
        .lobby
        .clone()
}

/// Stand a body in `cell`, as a test's staging rather than as play.
fn stage(game: &mut AscentMatch, player: PlayerId, cell: HexCoord) {
    let state = game.physical.players.get_mut(&player).expect("player");
    state.cell = cell;
    state.position = Vec3::from_array(hex_origin(cell)) + Vec3::Y * (FLOOR_SLAB_TOP + 1.2);
}

/// Every body stands and turns slowly.
fn idle(game: &mut AscentMatch) {
    let moves: Vec<_> = game
        .physical()
        .players
        .keys()
        .map(|&player| (player, Body::Turn(0.02)))
        .collect();
    step_bodies(game, &moves, SeatCommand::None);
}

#[test]
fn a_guardian_catch_jails_a_lone_body_and_the_rogue_wins() {
    let mut game = game_with(7, 1, true);
    for _ in 0..30 {
        idle(&mut game);
    }
    // The Guardian, close behind the body where it cannot be seen.
    let body = &game.physical().players[&BODY];
    let forward = Vec3::new(body.yaw.sin(), 0.0, -body.yaw.cos());
    let (cell, behind) = (body.cell, body.position - forward);
    game.physical.guardian.cell = cell;
    game.physical.guardian.position = behind;

    idle(&mut game);
    assert_eq!(events(&game, HexMatchEventKind::GuardianCatch), vec![BODY]);
    assert_eq!(events(&game, HexMatchEventKind::PlayerJailed), vec![BODY]);
    assert_eq!(place(&game, BODY), HexBodyPlace::Prison);
    let maze = &game.physical().prison.as_ref().unwrap().mazes[&SIDE];
    assert_eq!(game.physical().players[&BODY].cell, maze.entry());
    assert_eq!(state(&game, BODY), ObserverState::Jailed);

    // Every loyal Observer is jailed, which the rules resolve on the beat.
    for _ in 0..ACTOR_BEAT_TICKS {
        if game.rules().outcome != MatchOutcome::Running {
            break;
        }
        idle(&mut game);
    }
    assert_eq!(game.rules().outcome, MatchOutcome::RogueVictory);
}

#[test]
fn a_jailed_body_walks_its_maze_out_into_the_lobby() {
    let mut game = game_with(7, 2, false);
    idle(&mut game);
    game.physical.jail(BODY);
    idle(&mut game);
    assert_eq!(state(&game, BODY), ObserverState::Jailed);
    let known = game.rules().team_knowledge[&TEAM].discovered_cells.clone();

    let mut ticks = 0u32;
    while place(&game, BODY) == HexBodyPlace::Prison {
        step_bodies(
            &mut game,
            &[(BODY, Body::Explore), (OTHER, Body::Turn(0.0))],
            SeatCommand::None,
        );
        ticks += 1;
        assert!(ticks < 90 * 60, "still in the maze after ninety seconds");
        if place(&game, BODY) == HexBodyPlace::Prison {
            assert_eq!(state(&game, BODY), ObserverState::Jailed);
            // Nothing a jailed body passes in its maze becomes knowledge of the facility.
            assert_eq!(game.rules().team_knowledge[&TEAM].discovered_cells, known);
        }
    }
    assert_eq!(events(&game, HexMatchEventKind::PlayerReleased), vec![BODY]);
    assert!(lobby(&game).contains(&game.physical().players[&BODY].cell));
    assert_eq!(state(&game, BODY), ObserverState::Active);
    // The shortest way out, walked by a body that knows it, is well inside the budget a
    // lost player gets.
    assert!(
        ticks >= 15 * 60,
        "out in {ticks} ticks: the maze is too small"
    );
}

#[test]
fn holding_the_lobby_breaks_the_team_out_and_leaving_it_resets_the_hold() {
    let mut game = game_with(7, 2, false);
    idle(&mut game);
    game.physical.jail(BODY);
    idle(&mut game);
    let lobby_anchor = game.physical().prison.as_ref().unwrap().lobby_anchor;
    let outside = game.physical().facility.config.spawn();
    assert!(!lobby(&game).contains(&outside));

    // Most of a hold, then out again: nothing happens and the hold starts over.
    stage(&mut game, OTHER, lobby_anchor);
    for _ in 0..LOBBY_HOLD_TICKS - 20 {
        idle(&mut game);
    }
    stage(&mut game, OTHER, outside);
    idle(&mut game);
    assert_eq!(place(&game, BODY), HexBodyPlace::Prison);
    assert_eq!(
        game.physical().prison.as_ref().unwrap().lobby_hold[&SIDE],
        0
    );

    stage(&mut game, OTHER, lobby_anchor);
    let mut ticks = 0;
    while place(&game, BODY) == HexBodyPlace::Prison {
        idle(&mut game);
        ticks += 1;
        assert!(
            ticks <= LOBBY_HOLD_TICKS + 2,
            "the hold never broke them out"
        );
    }
    assert!(
        ticks >= LOBBY_HOLD_TICKS - 1,
        "broke out after only {ticks} ticks"
    );
    assert_eq!(events(&game, HexMatchEventKind::Jailbreak), vec![BODY]);
    assert!(lobby(&game).contains(&game.physical().players[&BODY].cell));
    assert_eq!(state(&game, BODY), ObserverState::Active);
}

#[test]
fn a_catch_joins_an_occupied_maze_and_an_empty_prison_carves_a_new_one() {
    let mut game = game_with(7, 2, false);
    idle(&mut game);
    let maze = |game: &AscentMatch| {
        game.physical().prison.as_ref().unwrap().mazes[&SIDE]
            .world
            .placements
            .clone()
    };
    game.physical.jail(BODY);
    let first = maze(&game);
    idle(&mut game);
    game.physical.jail(OTHER);
    assert_eq!(maze(&game), first, "a second prisoner joins the first");
    assert_eq!(
        game.physical().players[&OTHER].cell,
        game.physical().prison.as_ref().unwrap().mazes[&SIDE].entry()
    );

    // Empty the prison through the lobby, then catch again.
    let lobby_anchor = game.physical().prison.as_ref().unwrap().lobby_anchor;
    game.physical.players.get_mut(&OTHER).unwrap().place = HexBodyPlace::Facility;
    stage(&mut game, OTHER, lobby_anchor);
    while place(&game, BODY) == HexBodyPlace::Prison {
        idle(&mut game);
    }
    for _ in 0..10 {
        idle(&mut game);
    }
    game.physical.jail(BODY);
    assert_ne!(maze(&game), first, "an empty prison carves a fresh maze");
}

#[test]
fn the_guardian_waits_at_the_lobby_door() {
    let mut game = game_with(7, 2, true);
    idle(&mut game);
    // One body jailed, so the Guardian hunts the other alone; that one stands in the lobby.
    game.physical.jail(OTHER);
    let lobby_cells = lobby(&game);
    let anchor = game.physical().prison.as_ref().unwrap().lobby_anchor;
    stage(&mut game, BODY, anchor);
    for _ in 0..2_400 {
        let moves = [(BODY, Body::Turn(0.0)), (OTHER, Body::Turn(0.0))];
        step_bodies(&mut game, &moves, SeatCommand::None);
        assert!(
            !lobby_cells.contains(&game.physical().guardian.cell),
            "the Guardian entered the lobby"
        );
        assert!(events(&game, HexMatchEventKind::GuardianCatch).is_empty());
    }
    assert_eq!(
        game.physical().guardian.target,
        Some(BODY),
        "it was hunting"
    );
}

#[test]
fn a_fall_through_the_facility_is_lost_to_the_void_and_corrupts() {
    let mut game = game_with(7, 2, false);
    idle(&mut game);
    let state_before = state(&game, BODY);
    assert_eq!(state_before, ObserverState::Active);
    game.physical.players.get_mut(&BODY).unwrap().position = Vec3::new(0.0, -50.0, 0.0);
    idle(&mut game);
    assert_eq!(events(&game, HexMatchEventKind::PlayerLost), vec![BODY]);
    assert_eq!(place(&game, BODY), HexBodyPlace::Void);
    assert_eq!(state(&game, BODY), ObserverState::Corrupted);
    assert_eq!(game.session().seats()[&BODY].role, Role::Rogue);
    // It does not come back.
    for _ in 0..120 {
        idle(&mut game);
    }
    assert_eq!(place(&game, BODY), HexBodyPlace::Void);
    assert_eq!(state(&game, BODY), ObserverState::Corrupted);
}
