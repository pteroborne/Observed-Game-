//! Floor power and Architect-placed recharge on the real facility.

use glam::Vec3;

use super::*;
use crate::ascent::economy::{MAX_CHARGE, RECHARGE_PER_BEAT};
use crate::ascent::sim::{ACTOR_BEAT_TICKS, CardKind};
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

fn open_site(game: &AscentMatch, level: u8) -> Fixture {
    let (&cell, &floor) = game
        .ascent
        .station_points
        .iter()
        .find(|(cell, _)| cell.level == level && !game.rules().fixed_structure(**cell))
        .expect("a mutable, standable station site");
    Fixture {
        kind: FixtureKind::Station,
        cell,
        floor,
    }
}

/// Stage a station card and use the real Architect seat once a known site is legal.
fn install_station(game: &mut AscentMatch) -> Fixture {
    assert!(
        game.ascent
            .session
            .hands
            .get_mut(&TEAM)
            .unwrap()
            .deck
            .stage_kind(CardKind::Station),
        "the loyal deck deals stations"
    );
    let command = (0..6_000)
        .find_map(|_| {
            // Held through the whole walk: the live-hand rule never gives up a station.
            let card = game.session().hands[&TEAM]
                .deck
                .hand
                .iter()
                .find(|card| card.kind == CardKind::Station)
                .expect("the station card stays in hand")
                .id;
            let legal = game
                .ascent
                .station_points
                .keys()
                .copied()
                .filter(|cell| cell.level == 0 && !game.rules().fixed_structure(*cell))
                .map(|target| ArchitectCommand::Play {
                    card,
                    target,
                    rotation: 0,
                })
                .find(|&command| {
                    game.session()
                        .architect_refusal(ARCHITECT, command)
                        .is_none()
                });
            if legal.is_none() {
                step(game, Body::Explore, SeatCommand::None);
            }
            legal
        })
        .expect("exploration found a legal station site");
    let ArchitectCommand::Play { target, .. } = command else {
        unreachable!()
    };
    assert!(step(game, Body::Turn(0.0), SeatCommand::Architect(command)).is_empty());
    let station = fixture(game, FixtureKind::Station, target.level);
    assert_eq!(station.cell, target);
    assert!(game.rules().economy.stations.contains(&target));
    station
}

#[test]
fn every_floor_has_a_generator_and_standable_station_sites_but_no_free_station() {
    for seed in [3, 7, 11] {
        let game = game(seed);
        let rules = game.rules();
        assert!(
            rules.economy.pads.is_empty(),
            "the lab's pads are not sited"
        );
        for level in 0..rules.world.config.levels {
            {
                let fixture = fixture(&game, FixtureKind::Generator, level);
                let cell = fixture.cell;
                assert!(
                    rules.fixed_structure(cell),
                    "seed {seed}: generator can be rewritten"
                );
                assert!(
                    !rules.linked_vertically(cell),
                    "seed {seed}: generator on a stair"
                );
                assert!(
                    !rules.prison_core.contains(&cell),
                    "seed {seed}: generator in the prison"
                );
                let centre = Vec3::from_array(observed_hex::hex_origin(cell));
                assert!(
                    (fixture.floor - centre).with_y(0.0).length() <= 4.01,
                    "seed {seed}: generator is off the middle of its cell"
                );
                assert!(
                    (fixture.floor.y - centre.y - observed_hex::FLOOR_SLAB_TOP).abs() < 0.2,
                    "seed {seed}: generator is not on its floor"
                );
            }
            let generator = fixture(&game, FixtureKind::Generator, level);
            assert_eq!(rules.economy.generators[&level], generator.cell);
            assert!(
                game.ascent
                    .station_points
                    .keys()
                    .any(|cell| cell.level == level)
            );
        }
        assert!(rules.economy.stations.is_empty(), "stations must be played");
        assert!(
            game.ascent
                .fixtures()
                .iter()
                .all(|fixture| fixture.kind == FixtureKind::Generator)
        );
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
    let station = open_site(&game, 0);
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
    let station = install_station(&mut game);
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
    let station = install_station(&mut game);
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

/// A standable site on `level` whose way to the floor's generator stays on the floor, or,
/// with `on_floor` false, one whose only way there leaves it.
fn site_reaching_the_generator(game: &AscentMatch, level: u8, on_floor: bool) -> Fixture {
    let generator = fixture(game, FixtureKind::Generator, level).cell;
    let facility = &game.physical().facility;
    let (&cell, &floor) = game
        .ascent
        .station_points
        .iter()
        .filter(|(cell, _)| cell.level == level && **cell != generator)
        .find(|(cell, _)| {
            facility
                .route_between_cells(**cell, generator)
                .is_some_and(|route| route.cells.iter().all(|at| at.level == level) == on_floor)
        })
        .expect("such a site");
    Fixture {
        kind: FixtureKind::Station,
        cell,
        floor,
    }
}

/// A dark generator the body could only reach by climbing off its floor is no errand: the
/// floor it would arrive on is not the dark one, and it would give up halfway.
#[test]
fn a_generator_reachable_only_off_the_floor_is_not_an_errand() {
    let mut game = game(7);
    let mut driver = HexBotDriver::default();
    let site = site_reaching_the_generator(&game, 0, false);
    stand_at(&mut game, site, Vec3::ZERO);
    let generator = fixture(&game, FixtureKind::Generator, 0).cell;
    assert!(
        driver
            .route_len_to(game.physical(), BODY, generator)
            .is_some()
    );
    assert_eq!(
        driver.floor_route_len_to(game.physical(), BODY, generator),
        None
    );
}

#[test]
fn bot_body_walks_to_a_dark_floors_generator_and_restores_power() {
    let mut game = game(7);
    let mut driver = HexBotDriver::default();
    let station = site_reaching_the_generator(&game, 0, true);
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
    let station = install_station(&mut game);
    // A few cells off, somewhere the station is an errand: reachable without leaving
    // its floor, as the bot requires.
    let facility = &game.physical().facility;
    let (&cell, &floor) = game
        .ascent
        .station_points
        .iter()
        .filter(|(cell, _)| cell.level == station.cell.level && **cell != station.cell)
        .find(|(cell, _)| {
            facility
                .route_between_cells(**cell, station.cell)
                .is_some_and(|route| {
                    route.cells.len() > 2 && route.cells.iter().all(|at| at.level == cell.level)
                })
        })
        .expect("somewhere the station is an errand from");
    stand_at(
        &mut game,
        Fixture {
            kind: FixtureKind::Station,
            cell,
            floor,
        },
        Vec3::ZERO,
    );
    assert!(
        driver
            .floor_route_len_to(game.physical(), BODY, station.cell)
            .is_some(),
        "the bot can reach the station on its floor"
    );
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
fn no_card_rewrites_a_generator() {
    let game = game(7);
    for fixture in game
        .ascent
        .fixtures()
        .iter()
        .filter(|fixture| fixture.kind == FixtureKind::Generator)
    {
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

/// A bot Architect deals its team a station: with none on the floor its Observers stand on,
/// it plays one from its mixed hand onto a site the team has found, and the station stands
/// in the facility where a body can work it.
#[test]
fn a_bot_architect_deploys_a_station_on_its_teams_floor() {
    let mut game = game_seated(7, 1, false, true);
    let mut deployed = None;
    for _ in 0..6_000 {
        step(&mut game, Body::Explore, SeatCommand::None);
        if let Some(&cell) = game.rules().economy.stations.first() {
            deployed = Some(cell);
            break;
        }
    }
    let cell = deployed.expect("the bot Architect never deployed a station");
    let station = game
        .ascent
        .fixtures()
        .iter()
        .find(|fixture| fixture.kind == FixtureKind::Station && fixture.cell == cell)
        .expect("the deployed station stands in the facility");
    assert_eq!(station.floor, game.ascent.station_points[&cell]);
    assert!(
        game.rules()
            .observers
            .values()
            .any(|observer| observer.cell.level == cell.level),
        "deployed off the team's floor"
    );
}

/// A station is equipment on its tile, not structure: rewrite the tile and the station goes
/// with it, from the rules and from the facility.
#[test]
fn a_rewritten_tile_takes_its_station_with_it() {
    let mut game = game(7);
    let station = install_station(&mut game);
    let placement = game.rules().world.placements[&station.cell];
    game.ascent.session.sim.rewrite(placement);
    step(&mut game, Body::Turn(0.0), SeatCommand::None);
    assert!(!game.rules().economy.stations.contains(&station.cell));
    assert!(
        !game
            .ascent
            .fixtures()
            .iter()
            .any(|fixture| fixture.kind == FixtureKind::Station && fixture.cell == station.cell),
        "the station still stands in the facility"
    );
}

/// A linked pair of the body's team's plates, one where the body stands on `from_level`'s
/// open site and one on `to_level`'s.
fn plates(game: &mut AscentMatch, from_level: u8, to_level: u8) -> (Fixture, Fixture) {
    let from = open_site(game, from_level);
    let to = game
        .ascent
        .station_points
        .iter()
        .find(|(cell, _)| cell.level == to_level && **cell != from.cell)
        .map(|(&cell, &floor)| Fixture {
            kind: FixtureKind::Station,
            cell,
            floor,
        })
        .expect("a second site");
    let team = game.physical().players[&BODY].team;
    for site in [from, to] {
        game.physical
            .pads
            .deploy(BODY, team, site.cell, site.floor)
            .expect("a plate to deploy");
    }
    stand_at(game, from, Vec3::ZERO);
    (from, to)
}

/// Whether standing still on a plate, past the re-arm a deployed plate gives its owner,
/// carries the body anywhere.
fn carried(game: &mut AscentMatch) -> bool {
    (0..u64::from(crate::hex_wfc::PAD_REARM_TICKS) + 30).any(|_| {
        press(game, HexActionButtons::default());
        game.physical()
            .recent_events
            .iter()
            .any(|event| event.kind == HexMatchEventKind::PadTraversed)
    })
}

/// A dark floor's plates are inert (design section 5): a body standing on one goes
/// nowhere, and nor does one whose link ends on a dark floor. With the power back, the
/// same plate carries it.
#[test]
fn a_floor_without_power_leaves_its_plates_inert() {
    let mut game = game(7);
    plates(&mut game, 0, 0);
    game.ascent.stage_power(0, false);
    assert!(!carried(&mut game), "a dark floor's plate carried the body");

    game.ascent.stage_power(0, true);
    assert!(
        carried(&mut game),
        "the restored plate did not carry the body"
    );

    let mut game = super::game(7);
    plates(&mut game, 0, 1);
    game.ascent.stage_power(1, false);
    assert!(
        !carried(&mut game),
        "a plate linked to a dark floor carried the body"
    );
}

/// The real facility is climbed by walked stairs and ramps, which no power failure stops,
/// so the rules' routes keep a dark floor's way up and down, as the bodies do. (A lab
/// board's ascent rooms are powered lifts, and a dark floor stops them.)
#[test]
fn a_dark_floor_keeps_its_stairs_in_the_rules_routes() {
    let mut game = game(7);
    let rules = game.rules();
    let (foot, head) = rules
        .world
        .placements
        .keys()
        .filter(|cell| cell.level == 0)
        .find_map(|&cell| {
            rules
                .exits(cell)
                .into_iter()
                .find(|next| next.level != cell.level)
                .map(|next| (cell, next))
        })
        .expect("a way up from the ground floor");
    for (dark, why) in [(0, "the foot's floor dark"), (1, "the head's floor dark")] {
        game.ascent.stage_power(dark, false);
        assert!(
            game.rules().exits(foot).contains(&head),
            "the way up is gone with {why}"
        );
        game.ascent.stage_power(dark, true);
    }
}
