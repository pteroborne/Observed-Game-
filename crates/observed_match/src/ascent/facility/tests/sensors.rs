//! The Rogue's sensors on the real facility: installed from a Rogue seat, hung where a body
//! can reach them, seeing for the Rogue, and taken down by a body with interact.

use super::*;
use crate::ascent::sim::CardKind;
use crate::hex_wfc::HexActionButtons;

const ROGUE: PlayerId = PlayerId(41);

/// One team of one body and a bot Architect, and a player at the Rogue board.
fn game_with_a_rogue() -> AscentMatch {
    let config = HexMatchConfig {
        teams: 1,
        members_per_team: 1,
        guardian: false,
        wfc: HexWfcConfig {
            levels: 2,
            ..HexWfcConfig::default()
        },
    };
    let physical = HexWfcMatch::new_with_content(
        7,
        config,
        crate::hex_wfc::compatibility_test_content().clone(),
    )
    .expect("a two-level facility solves");
    let mut seats = super::super::architect_seats(&physical, None);
    seats.insert(
        ROGUE,
        Seat {
            role: Role::Rogue,
            bot: false,
        },
    );
    AscentMatch::new(physical, 7, seats).expect("the match's seats and a Rogue")
}

/// One tick: the body stands pressing `interact` or not, and the Rogue sends `command`.
fn tick(
    game: &mut AscentMatch,
    interact: bool,
    command: SeatCommand,
) -> BTreeMap<PlayerId, Refusal> {
    let tick = game.rules().tick + 1;
    let bodies = HexInputFrame {
        version: HEX_INPUT_VERSION,
        tick,
        commands: BTreeMap::from([(
            BODY,
            HexPlayerCommand {
                actions: HexActionButtons {
                    interact,
                    ..HexActionButtons::default()
                },
                ..HexPlayerCommand::default()
            },
        )]),
    };
    let seats = InputFrame {
        version: ASCENT_INPUT_VERSION,
        tick,
        commands: BTreeMap::from([(ROGUE, command)]),
    };
    game.step(&bodies, &seats).expect("a well-formed frame")
}

#[test]
fn a_sensor_the_rogue_installs_hangs_in_the_facility_and_sees_the_body() {
    let mut game = game_with_a_rogue();
    tick(&mut game, false, SeatCommand::None);
    let id = game.observer_for(BODY).expect("the body is an Observer");
    let body = game.rules().observers[&id].cell;
    assert!(!game.rules().rogue_view().known_observers.contains_key(&id));
    // Somewhere out of the body's sight whose own sight reaches the body.
    let probe = rogue_play(&mut game, CardKind::Sensor, body);
    let ArchitectCommand::Play { card, .. } = probe else {
        unreachable!("a card play")
    };
    let sense = |target| ArchitectCommand::Play {
        card,
        target,
        rotation: 0,
    };
    let target = game
        .rules()
        .world
        .placements
        .keys()
        .copied()
        .filter(|&cell| game.rules().sensor_sight(cell).contains(&body))
        .find(|&target| {
            game.session()
                .architect_refusal(ROGUE, sense(target))
                .is_none()
        })
        .expect("a hidden cell that sees the body");
    let refusals = tick(&mut game, false, SeatCommand::Architect(sense(target)));
    assert!(refusals.is_empty(), "{refusals:?}");
    assert!(game.physical().sensors().any(|(cell, _)| cell == target));
    assert!(game.rules().sensor_watching(target));
    assert!(
        game.rules().rogue_view().known_observers.contains_key(&id),
        "the Rogue sees the body through its sensor"
    );
}

#[test]
fn a_body_under_a_sensor_takes_it_down_with_interact() {
    let mut game = game_with_a_rogue();
    tick(&mut game, false, SeatCommand::None);
    // The body's own cell, unless its floor's generator stands there: interact at the
    // generator switches the floor's power, which comes before a sensor.
    let own = game.physical().players[&BODY].cell;
    let generator = game.rules().economy.generators.get(&own.level).copied();
    let cell = if generator == Some(own) {
        let grid = game.physical().facility.config.grid();
        observed_hex::HexFace::LATERAL
            .into_iter()
            .filter_map(|face| grid.neighbor(own, face))
            .find(|&next| Some(next) != generator && game.physical().standing_point(next).is_some())
            .expect("a neighbour without the generator")
    } else {
        own
    };
    game.ascent.stage_sensor(cell);
    tick(&mut game, false, SeatCommand::None);
    let (_, at) = game
        .physical()
        .sensors()
        .find(|&(hung, _)| hung == cell)
        .expect("the staged sensor hangs");
    let feet = game
        .physical()
        .standing_point(cell)
        .expect("somewhere to stand in the body's own cell");
    assert!(
        game.physical.stage_body_facing(BODY, cell, feet, at),
        "the body stands under the sensor"
    );
    let under = game
        .ascent
        .at_sensor(game.physical(), BODY)
        .expect("the body is under the sensor");
    assert_eq!(under.cell, cell);
    assert!(under.dismantlable);

    tick(&mut game, true, SeatCommand::None);
    assert!(game.rules().sensors.is_empty(), "the rules took it down");
    assert_eq!(game.physical().sensors().count(), 0, "and the facility");
}
