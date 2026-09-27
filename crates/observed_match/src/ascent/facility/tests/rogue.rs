//! The bot Rogue every Ascent match seats: it plays on the real facility, and only against
//! an Observer a Guardian has detected.

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
        let before = game.rules().command_log.len();
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
            // what the rules see now.
            let rules = game.rules();
            let (_, command) = rules.command_log[before];
            let ArchitectCommand::Play { target, .. } = command else {
                panic!("tick {tick}: the Rogue plays tiles, not {command:?}");
            };
            let near_prey = rules
                .detected_observers()
                .iter()
                .any(|id| observed_hex::travel_distance(rules.observers[id].cell, target) <= 3);
            assert!(
                near_prey,
                "tick {tick}: the Rogue played at {target:?}, near no detected Observer"
            );
        }
    }
    assert!(rogue_plays > 0, "the bot Rogue never played");
    assert_geometry_is_fresh(&game);
}
