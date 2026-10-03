//! The bot Rogue every Ascent match seats: it plays on the real facility, and only against
//! an Observer it has detected, by a Guardian's sight or by a sensor.

use super::*;

#[test]
fn every_match_seats_a_bot_rogue() {
    let team = observed_core::TeamId(0);
    let game = game_with(7, 1, true);
    let seats = super::super::architect_seats(game.physical(), Some(team));
    assert_eq!(
        seats.get(&super::super::ROGUE_SEAT),
        Some(&Seat {
            role: Role::Rogue,
            bot: true,
        })
    );
    // The human's team keeps its own Architect seat; the Rogue's is not among them.
    assert_eq!(
        seats.get(&architect_seat(team)),
        Some(&Seat {
            role: Role::Architect(TeamId(team.0)),
            bot: false,
        })
    );
}

#[test]
fn the_bot_rogue_plays_only_against_a_detected_observer() {
    let config = HexMatchConfig {
        teams: 1,
        members_per_team: 2,
        guardian: true,
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
    let seats = super::super::architect_seats(&physical, None);
    let mut game = AscentMatch::new(physical, 7, seats).expect("the match's own seats");
    let bodies: Vec<PlayerId> = game.physical().players.keys().copied().collect();

    let mut rogue_plays = 0;
    for _ in 0..3_000 {
        // Directed Guardians can jail both bodies before the loop is out: that is the match.
        if game.rules().outcome != crate::ascent::sim::MatchOutcome::Running {
            break;
        }
        let before = game.rules().command_log.len();
        // Who it could have seen when it chose: the Rogue decides inside the tick, and its
        // own play can cut a Guardian's view of its prey before the tick is out.
        let detected_before: Vec<(crate::ascent::sim::ObserverId, HexCoord)> = {
            let rules = game.rules();
            rules
                .rogue_detected()
                .iter()
                .map(|id| (*id, rules.observers[id].cell))
                .collect()
        };
        let tick = game.rules().tick + 1;
        let frame = HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick,
            commands: bodies
                .iter()
                .map(|&player| (player, game.physical().bot_player_command(player)))
                .collect(),
        };
        let seats = InputFrame {
            version: ASCENT_INPUT_VERSION,
            tick,
            commands: BTreeMap::new(),
        };
        game.step(&frame, &seats).expect("a well-formed frame");
        assert_eq!(
            game.rules().world.placements,
            game.physical().facility.placements,
            "tick {tick}: the rules and the bodies disagree about the facility"
        );
        let selected = game
            .rules()
            .traces
            .get("Architect")
            .and_then(|trace| trace.selected);
        if game.rules().command_log.len() > before
            && matches!(selected, Some("close the hunt" | "undermine"))
        {
            rogue_plays += 1;
            // The Rogue decides on the beat, after the bodies have moved: what it saw is
            // what the rules saw as the tick began, or what they see now.
            let rules = game.rules();
            let (_, command) = rules.command_log[before];
            let ArchitectCommand::Play { target, .. } = command else {
                panic!("tick {tick}: the Rogue plays tiles, not {command:?}");
            };
            let near_prey = rules
                .rogue_detected()
                .iter()
                .map(|id| rules.observers[id].cell)
                .chain(detected_before.iter().map(|&(_, cell)| cell))
                .any(|cell| observed_hex::travel_distance(cell, target) <= 3);
            assert!(
                near_prey,
                "tick {tick}: the Rogue played at {target:?}, near no detected Observer"
            );
        }
    }
    assert!(rogue_plays > 0, "the bot Rogue never played");
    assert_geometry_is_fresh(&game);
}

/// Where the Tumbler has `steps` of walk to go, far from every body.
fn a_walk_from_the_tumbler(game: &AscentMatch, steps: usize) -> HexCoord {
    let physical = game.physical();
    let from = physical.guardian.cell;
    let rules = game.rules();
    physical
        .facility
        .placements
        .iter()
        .filter(|(cell, placement)| {
            cell.level == from.level
                && placement.space.built()
                && !rules.prison.cells.contains(cell)
                && physical
                    .players
                    .values()
                    .all(|body| observed_hex::travel_distance(body.cell, **cell) >= 6)
        })
        .filter_map(|(&cell, _)| {
            let route = physical.facility.route_between_cells(from, cell)?;
            (route.cells.len() == steps + 1).then_some(cell)
        })
        .min()
        .expect("somewhere a few steps from the Tumbler, away from the bodies")
}

#[test]
fn a_directed_tumbler_walks_where_the_rogue_sent_it() {
    const ROGUE: PlayerId = PlayerId(41);
    let config = HexMatchConfig {
        teams: 1,
        members_per_team: 2,
        guardian: true,
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
    let mut game = AscentMatch::new(physical, 7, seats).expect("the match's seats and a Rogue");
    let bodies: Vec<PlayerId> = game.physical().players.keys().copied().collect();
    let tick_with = |game: &mut AscentMatch, command: SeatCommand| {
        let tick = game.rules().tick + 1;
        // The bodies stand, turning slowly, where the match put them.
        let frame = HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick,
            commands: bodies
                .iter()
                .map(|&player| (player, HexPlayerCommand::default()))
                .collect(),
        };
        let seats = InputFrame {
            version: ASCENT_INPUT_VERSION,
            tick,
            commands: BTreeMap::from([(ROGUE, command)]),
        };
        game.step(&frame, &seats).expect("a well-formed frame")
    };
    tick_with(&mut game, SeatCommand::None);
    let goal = a_walk_from_the_tumbler(&game, 4);
    let direct = rogue_play(&mut game, crate::ascent::sim::CardKind::Directive, goal);
    let refusals = tick_with(&mut game, SeatCommand::Architect(direct));
    assert!(refusals.is_empty(), "{refusals:?}");
    assert_eq!(game.physical().guardian_directive(), Some(goal));

    let mut left = game
        .physical()
        .facility
        .route_between_cells(game.physical().guardian.cell, goal)
        .map_or(usize::MAX, |route| route.cells.len());
    let mut ticks = 0;
    while game.rules().directed.is_some() {
        tick_with(&mut game, SeatCommand::None);
        ticks += 1;
        let tumbler = game.physical().guardian.cell;
        let now = game
            .physical()
            .facility
            .route_between_cells(tumbler, goal)
            .map_or(usize::MAX, |route| route.cells.len());
        assert!(
            now <= left,
            "tick {ticks}: the Tumbler turned away from {goal:?}"
        );
        left = now;
    }
    assert_eq!(
        game.physical().guardian.cell,
        goal,
        "the directive is spent by arriving, not by running out"
    );
    assert!(ticks < crate::ascent::sim::DIRECTIVE_TICKS as usize);
    // Spent, it lets the Tumbler go back to hunting.
    tick_with(&mut game, SeatCommand::None);
    assert_eq!(game.physical().guardian_directive(), None);
}

#[test]
fn a_bot_rogue_with_nobody_detected_watches_the_way_up() {
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
    let seats = super::super::architect_seats(&physical, None);
    let mut game = AscentMatch::new(physical, 7, seats).expect("the match's own seats");
    for _ in 0..600 {
        step(&mut game, Body::Turn(0.0), SeatCommand::None);
        if !game.rules().sensors.is_empty() {
            break;
        }
    }
    let rules = game.rules();
    let (&cell, _) = rules
        .sensors
        .iter()
        .next()
        .expect("the bot Rogue installed a sensor");
    assert_ne!(
        rules.world.placements[&cell].up,
        observed_hex::PortClass::Sealed,
        "at the foot of a climb"
    );
    assert_eq!(
        rules
            .traces
            .get("Architect")
            .and_then(|trace| trace.selected),
        Some("watch the way up")
    );
    assert!(game.physical().sensors().any(|(hung, _)| hung == cell));
}
