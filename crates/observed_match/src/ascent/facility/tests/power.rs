//! Floor power and recharge on the real facility: every floor's generator and station
//! stand where a body can reach them, interact at the generator switches the floor, and
//! a powered station fills a body standing at it.

use glam::Vec3;

use super::*;
use crate::ascent::economy::{MAX_CHARGE, RECHARGE_PER_BEAT};
use crate::ascent::sim::ACTOR_BEAT_TICKS;
use crate::hex_wfc::{HexActionButtons, HexBotDriver};

const INTERACT: HexActionButtons = HexActionButtons {
    interact: true,
    deploy_lantern: false,
    recover_lantern: false,
    deploy_pad: false,
    kinetic_push: false,
    kinetic_pull: false,
};

/// One tick, the body standing still and pressing `actions`; the Architect idle.
fn press(game: &mut AscentMatch, actions: HexActionButtons) {
    let tick = game.rules().tick + 1;
    let bodies = HexInputFrame {
        version: HEX_INPUT_VERSION,
        tick,
        commands: BTreeMap::from([(
            BODY,
            HexPlayerCommand {
                actions,
                ..HexPlayerCommand::default()
            },
        )]),
    };
    let seats = InputFrame {
        version: ASCENT_INPUT_VERSION,
        tick,
        commands: BTreeMap::from([(ARCHITECT, SeatCommand::None)]),
    };
    game.step(&bodies, &seats).expect("a well-formed frame");
}

/// One tick using the same body command the game and server request for a bot.
fn drive_bot(game: &mut AscentMatch, driver: &mut HexBotDriver) -> HexPlayerCommand {
    let command = game.ascent.bot_body_command(&game.physical, driver, BODY);
    let tick = game.rules().tick + 1;
    let bodies = HexInputFrame {
        version: HEX_INPUT_VERSION,
        tick,
        commands: BTreeMap::from([(BODY, command)]),
    };
    let seats = InputFrame {
        version: ASCENT_INPUT_VERSION,
        tick,
        commands: BTreeMap::from([(ARCHITECT, SeatCommand::None)]),
    };
    game.step(&bodies, &seats).expect("a well-formed bot frame");
    command
}

fn fixture(game: &AscentMatch, kind: FixtureKind, level: u8) -> Fixture {
    *game
        .ascent
        .fixtures()
        .iter()
        .find(|fixture| fixture.kind == kind && fixture.cell.level == level)
        .unwrap_or_else(|| panic!("a {kind:?} on floor {level}"))
}

/// Stand the body at `fixture`, `offset` from its point across the floor.
fn stand_at(game: &mut AscentMatch, fixture: Fixture, offset: Vec3) {
    game.physical
        .stand_body_for_tests(BODY, fixture.cell, fixture.floor + offset);
}

fn powered(game: &AscentMatch, level: u8) -> bool {
    game.rules().economy.is_powered(level)
}

#[test]
fn every_floor_has_a_generator_and_a_station_a_body_can_stand_at() {
    for seed in [3, 7, 11] {
        let game = game(seed);
        let rules = game.rules();
        assert!(
            rules.economy.pads.is_empty(),
            "the lab's pads are not sited"
        );
        for level in 0..rules.world.config.levels {
            for kind in [FixtureKind::Generator, FixtureKind::Station] {
                let fixture = fixture(&game, kind, level);
                let cell = fixture.cell;
                assert!(
                    rules.fixed_structure(cell),
                    "seed {seed}: {kind:?} can be rewritten"
                );
                assert!(
                    !rules.linked_vertically(cell),
                    "seed {seed}: {kind:?} on a stair"
                );
                assert!(
                    !rules.prison_core.contains(&cell),
                    "seed {seed}: {kind:?} in the prison"
                );
                let centre = Vec3::from_array(observed_hex::hex_origin(cell));
                assert!(
                    (fixture.floor - centre).with_y(0.0).length() <= 4.01,
                    "seed {seed}: {kind:?} is off the middle of its cell"
                );
                assert!(
                    (fixture.floor.y - centre.y - observed_hex::FLOOR_SLAB_TOP).abs() < 0.2,
                    "seed {seed}: {kind:?} is not on its floor"
                );
            }
            let generator = fixture(&game, FixtureKind::Generator, level);
            let station = fixture(&game, FixtureKind::Station, level);
            assert_ne!(generator.cell, station.cell, "seed {seed}");
            assert_eq!(rules.economy.generators[&level], generator.cell);
            assert!(rules.economy.stations.contains(&station.cell));
        }
    }
}

#[test]
fn interact_at_the_generator_switches_the_floors_power_and_back() {
    let mut game = game(7);
    let generator = fixture(&game, FixtureKind::Generator, 0);
    stand_at(&mut game, generator, Vec3::new(1.0, 0.0, 0.0));
    press(&mut game, HexActionButtons::default());
    let at = game.ascent.at_fixture(&game.physical, BODY);
    assert_eq!(
        at.map(|(fixture, at)| (fixture.kind, at)),
        Some((
            FixtureKind::Generator,
            AtFixture::Generator {
                powered: true,
                operable: true
            }
        ))
    );
    press(&mut game, INTERACT);
    assert!(!powered(&game, 0), "the floor stayed lit");
    assert!(powered(&game, 1), "the floor above went dark too");
    // Holding the key down is one press, not a toggle a tick.
    press(&mut game, HexActionButtons::default());
    assert!(!powered(&game, 0));
    press(&mut game, INTERACT);
    assert!(powered(&game, 0), "the floor did not come back");
}

#[test]
fn interact_out_of_reach_of_the_generator_changes_nothing() {
    let mut game = game(7);
    let generator = fixture(&game, FixtureKind::Generator, 0);
    let station = fixture(&game, FixtureKind::Station, 0);
    stand_at(&mut game, station, Vec3::ZERO);
    press(&mut game, INTERACT);
    assert!(powered(&game, 0));
    assert!(
        game.ascent
            .at_fixture(&game.physical, BODY)
            .is_none_or(|(fixture, _)| fixture.cell != generator.cell)
    );
}

#[test]
fn a_powered_station_fills_the_tool_of_a_body_standing_at_it() {
    let mut game = game(7);
    let observer = game.observer_for(BODY).expect("an Observer");
    let station = fixture(&game, FixtureKind::Station, 0);
    stand_at(&mut game, station, Vec3::ZERO);
    game.ascent.session.sim.economy.set_charge(observer, 0);
    for _ in 0..ACTOR_BEAT_TICKS {
        press(&mut game, HexActionButtons::default());
    }
    assert_eq!(game.rules().economy.charge(observer), RECHARGE_PER_BEAT);
    assert_eq!(
        game.ascent
            .at_fixture(&game.physical, BODY)
            .map(|(_, at)| at),
        Some(AtFixture::Station {
            powered: true,
            charge: RECHARGE_PER_BEAT
        })
    );
    for _ in 0..ACTOR_BEAT_TICKS * 8 {
        press(&mut game, HexActionButtons::default());
    }
    assert_eq!(game.rules().economy.charge(observer), MAX_CHARGE);
}

#[test]
fn a_dark_station_and_one_out_of_reach_supply_nothing() {
    let mut game = game(7);
    let observer = game.observer_for(BODY).expect("an Observer");
    let station = fixture(&game, FixtureKind::Station, 0);
    // In the station's cell, but not at it.
    stand_at(&mut game, station, Vec3::ZERO);
    let aside = [Vec3::X, Vec3::Z, Vec3::NEG_X, Vec3::NEG_Z]
        .into_iter()
        .map(|direction| station.floor + direction * (FIXTURE_REACH + 1.0))
        .find(|&feet| {
            game.physical
                .solid_along_for_tests(feet + Vec3::Y, Vec3::NEG_Y, 1.3)
                .is_some()
        });
    if let Some(feet) = aside {
        game.physical.stand_body_for_tests(BODY, station.cell, feet);
        game.ascent.session.sim.economy.set_charge(observer, 0);
        for _ in 0..ACTOR_BEAT_TICKS * 2 {
            press(&mut game, HexActionButtons::default());
        }
        assert_eq!(game.rules().economy.charge(observer), 0, "drew from afar");
    }
    stand_at(&mut game, station, Vec3::ZERO);
    game.ascent
        .session
        .sim
        .economy
        .set_powered(station.cell.level, false);
    game.ascent.session.sim.economy.set_charge(observer, 0);
    for _ in 0..ACTOR_BEAT_TICKS * 2 {
        press(&mut game, HexActionButtons::default());
    }
    assert_eq!(
        game.rules().economy.charge(observer),
        0,
        "a dark station charged"
    );
}

#[test]
fn bot_body_walks_to_a_dark_floors_generator_and_restores_power() {
    let mut game = game(7);
    let mut driver = HexBotDriver::default();
    let station = fixture(&game, FixtureKind::Station, 0);
    stand_at(&mut game, station, Vec3::ZERO);
    game.ascent.stage_power(0, false);
    let mut interacted = false;
    for _ in 0..3_000 {
        let command = drive_bot(&mut game, &mut driver);
        interacted |= command.actions.interact;
        if powered(&game, 0) {
            break;
        }
    }
    assert!(interacted, "the bot never worked the generator");
    assert!(powered(&game, 0), "the bot did not restore its floor");
    assert!(powered(&game, 1), "the bot changed the adjacent floor");
}

#[test]
fn bot_body_walks_to_a_powered_station_and_waits_until_full() {
    let mut game = game(7);
    let mut driver = HexBotDriver::default();
    let observer = game.observer_for(BODY).expect("an Observer");
    let generator = fixture(&game, FixtureKind::Generator, 0);
    stand_at(&mut game, generator, Vec3::ZERO);
    game.ascent.stage_charge(BODY, 0);
    let mut recharging = false;
    for _ in 0..3_000 {
        let command = drive_bot(&mut game, &mut driver);
        let charge = game.rules().economy.charge(observer);
        if charge > 0 && charge < MAX_CHARGE {
            recharging = true;
            assert_eq!(
                command.intent,
                player_input::PlayerIntent::default(),
                "the bot left the station before it was full"
            );
        }
        if charge == MAX_CHARGE {
            break;
        }
    }
    assert!(recharging, "the bot never waited at the station");
    assert_eq!(game.rules().economy.charge(observer), MAX_CHARGE);
}

#[test]
fn no_card_rewrites_a_fixture() {
    let game = game(7);
    for fixture in game.ascent.fixtures() {
        let hand = &game.session().hands[&TEAM].deck.hand;
        for card in hand {
            for rotation in 0..6 {
                let command = ArchitectCommand::Play {
                    card: card.id,
                    target: fixture.cell,
                    rotation,
                };
                assert_ne!(
                    game.session().architect_refusal(ARCHITECT, command),
                    None,
                    "{fixture:?} could be rewritten"
                );
            }
        }
    }
}
