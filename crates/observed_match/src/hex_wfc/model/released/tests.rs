//! Released Guardians in a physical match: a minor hunts on a body, catches into the
//! prison, is destroyed only by falling out of the facility, and steps identically on
//! every peer.

use std::collections::BTreeMap;

use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexSpace, HexWfcConfig};
use observed_hex::{HexCoord, PortClass};

use super::super::{
    HEX_INPUT_VERSION, HexBodyPlace, HexInputFrame, HexMatchConfig, HexMatchEventKind, HexWfcMatch,
};
use super::{HexReleasedGuardian, HexReleasedKind, MINOR_SIGHT_STEPS};

const BODY: PlayerId = PlayerId(0);

fn prison_match(seed: u64) -> HexWfcMatch {
    let config = HexMatchConfig {
        teams: 1,
        members_per_team: 1,
        guardian: false,
        wfc: HexWfcConfig {
            levels: 2,
            ..HexWfcConfig::default()
        },
    };
    let mut game = HexWfcMatch::new_with_content(
        seed,
        config,
        crate::hex_wfc::compatibility_test_content().clone(),
    )
    .expect("a two-level facility solves");
    game.send_catches_to_prison();
    game
}

/// A built cell on the body's floor, `steps` doors from it through that floor, and out of
/// the prison lobby.
fn cell_near_body(game: &HexWfcMatch, steps: usize) -> HexCoord {
    let body = game.players[&BODY].cell;
    let lobby = &game.prison.as_ref().expect("a prison").lobby;
    let mut cells: Vec<HexCoord> = game.facility.placements.keys().copied().collect();
    cells.sort();
    cells
        .into_iter()
        .find(|&cell| {
            !lobby.contains(&cell)
                && game
                    .facility
                    .route_between(body, cell)
                    .is_some_and(|route| {
                        route.len() == steps + 1 && route.iter().all(|at| at.level == body.level)
                    })
        })
        .expect("a cell that many doors from the body on its floor")
}

fn step(game: &mut HexWfcMatch, walk: bool) -> Vec<HexMatchEventKind> {
    let commands = if walk {
        BTreeMap::from([(BODY, game.bot_player_command(BODY))])
    } else {
        BTreeMap::new()
    };
    let frame = HexInputFrame {
        version: HEX_INPUT_VERSION,
        tick: game.tick + 1,
        commands,
    };
    game.step(&frame).iter().map(|event| event.kind).collect()
}

fn minor(game: &HexWfcMatch, id: u16) -> Option<&super::HexMinorState> {
    match game.released.get(&id)? {
        HexReleasedGuardian::Minor(minor) => Some(minor),
        HexReleasedGuardian::Major(_) => None,
    }
}

#[test]
fn a_released_minor_hunts_a_body_down_on_its_floor_and_jails_it() {
    let mut game = prison_match(7);
    let from = cell_near_body(&game, 3);
    assert!(game.release_guardian(1, HexReleasedKind::Minor, from));
    let lobby = game.prison.as_ref().expect("a prison").lobby.clone();
    let mut caught = false;
    for _ in 0..3_000 {
        let events = step(&mut game, false);
        let minor = minor(&game, 1).expect("the minor is still in the facility");
        assert!(
            !lobby.contains(&minor.cell),
            "a minor never enters the lobby"
        );
        if events.contains(&HexMatchEventKind::GuardianCatch) {
            caught = true;
            break;
        }
    }
    assert!(caught, "the minor never reached the body");
    assert_eq!(game.players[&BODY].place, HexBodyPlace::Prison);
}

#[test]
fn a_minor_hunts_only_what_it_can_detect() {
    let mut game = prison_match(7);
    let far = game
        .facility
        .placements
        .iter()
        .filter(|(cell, placement)| {
            placement.space.built()
                && game
                    .facility
                    .route_between(game.players[&BODY].cell, **cell)
                    .is_none_or(|route| route.len() > MINOR_SIGHT_STEPS + 3)
        })
        .map(|(&cell, _)| cell)
        .min()
        .expect("a cell out of a minor's sight");
    assert!(game.release_guardian(1, HexReleasedKind::Minor, far));
    for _ in 0..600 {
        step(&mut game, false);
    }
    let minor = minor(&game, 1).expect("still here");
    assert_eq!(minor.target, None, "nobody within sight");
    assert_eq!(minor.cell, far, "and so it holds where it was released");
}

#[test]
fn a_minor_whose_floor_is_retracted_falls_out_of_the_facility_and_is_gone() {
    let mut game = prison_match(7);
    // Somewhere on the ground floor with nothing beneath it, out of the body's reach, so
    // it stands idle until its floor goes.
    let body = game.players[&BODY].cell;
    let spot = game
        .facility
        .placements
        .iter()
        .filter(|(cell, placement)| {
            cell.level == 0
                && placement.space.built()
                && !game
                    .facility
                    .blueprints
                    .iter()
                    .any(|blueprint| blueprint.cells.contains(cell))
                && game
                    .facility
                    .route_between(body, **cell)
                    .is_none_or(|route| route.len() > MINOR_SIGHT_STEPS + 3)
        })
        .map(|(&cell, _)| cell)
        .min()
        .expect("a ground-floor hall out of sight");
    assert!(game.release_guardian(4, HexReleasedKind::Minor, spot));
    for _ in 0..30 {
        step(&mut game, false);
    }
    let mut retracted = game.facility.placements[&spot];
    retracted.space = HexSpace::Void;
    retracted.doors = 0;
    retracted.up = PortClass::Sealed;
    retracted.down = PortClass::Sealed;
    game.apply_directed_change(BTreeMap::from([(spot, retracted)]))
        .expect("a retraction builds");
    let mut lost = false;
    for _ in 0..600 {
        if step(&mut game, false).contains(&HexMatchEventKind::GuardianLost) {
            lost = true;
            break;
        }
    }
    assert!(lost, "the minor never fell out of the facility");
    assert!(!game.released.contains_key(&4));
}

#[test]
fn released_guardians_step_identically_on_every_peer() {
    let mut a = prison_match(11);
    let from = cell_near_body(&a, 2);
    let mut b = a.clone();
    for game in [&mut a, &mut b] {
        assert!(game.release_guardian(1, HexReleasedKind::Minor, from));
        assert!(game.release_guardian(2, HexReleasedKind::Major, from));
    }
    for _ in 0..900 {
        step(&mut a, true);
        step(&mut b, true);
        assert_eq!(a.snapshot().digest, b.snapshot().digest, "tick {}", a.tick);
    }
}

#[test]
fn a_release_is_refused_twice_under_one_id_and_onto_nothing_built() {
    let mut game = prison_match(7);
    let from = cell_near_body(&game, 2);
    assert!(game.release_guardian(1, HexReleasedKind::Minor, from));
    assert!(!game.release_guardian(1, HexReleasedKind::Major, from));
    let unbuilt = game
        .facility
        .placements
        .iter()
        .find(|(_, placement)| !placement.space.built())
        .map(|(&cell, _)| cell)
        .expect("somewhere unbuilt");
    assert!(!game.release_guardian(2, HexReleasedKind::Minor, unbuilt));
    assert!(game.remove_released(1));
    assert!(game.released.is_empty());
}
