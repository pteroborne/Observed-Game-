//! What the turn-based rules promise.
//!
//! These run against real solved facilities, not fixtures. A seed corpus is used
//! wherever a claim is about the *rules* rather than about one layout, because a
//! single seed proving relayout safety proves only that one pocket happened to
//! land somewhere harmless.

use std::collections::BTreeSet;

use observed_core::{PlayerId, TeamId};
use observed_facility::map_spec::RoomRole;
use observed_hex::HexCoord;

use crate::settings::{
    ActionCosts, BoardSize, GuardianSetting, MatchSettings, Objectives, PRESETS, ShiftCadence,
    Sight,
};

use super::action::{Refusal, TacticsAction};
use super::unit::PLAYER_TEAM;
use super::{MatchStatus, TacticsGame, relayout::ShiftOutcome, vision};

/// Enough seeds to catch a rule that only holds for a lucky layout, few enough
/// that the suite stays quick at Compact scale.
const SEEDS: [u64; 6] = [
    0x0000_0000_000c_0ffe,
    0x0000_0000_0000_0b0b,
    0x0000_0000_000d_00d0,
    0x5eed_0000_0000_0001,
    0xa11c_e3d0_0000_0008,
    0x0000_0000_dead_beef,
];

fn compact(seed: u64) -> MatchSettings {
    MatchSettings {
        seed,
        board: BoardSize::Compact,
        ..MatchSettings::standard()
    }
}

fn game(seed: u64) -> TacticsGame {
    TacticsGame::new(compact(seed)).expect("a compact facility solves at every corpus seed")
}

/// Spend a unit's whole turn walking wherever it can, deterministically: always
/// the first legal move. Enough to move the squad off the spawn without a route
/// planner in the tests.
fn wander(game: &mut TacticsGame) {
    let ids: Vec<PlayerId> = game.units.keys().copied().collect();
    for id in ids {
        while let Some(&action) = game
            .legal_actions(id)
            .iter()
            .find(|action| matches!(action, TacticsAction::Move(_)))
        {
            if game.apply(id, action).is_err() {
                break;
            }
        }
    }
}

#[test]
fn every_preset_produces_a_playable_match() {
    for preset in PRESETS {
        let name = preset.name;
        let mut settings = (preset.build)();
        // Presets pin their own seed; the corpus checks the preset's *shape*
        // solves generally, not that one seed is lucky.
        for seed in SEEDS {
            settings.seed = seed;
            let game = TacticsGame::new(settings).unwrap_or_else(|error| {
                panic!("{name} failed to solve at seed {seed:#x}: {error:?}")
            });
            let spawn = game.world.config.spawn();
            let exit = game.world.config.exit();
            assert!(
                game.world.route_between(spawn, exit).is_some(),
                "{name} at seed {seed:#x} solved with no route from spawn to exit"
            );
            assert_eq!(
                game.units
                    .values()
                    .filter(|unit| unit.team == PLAYER_TEAM)
                    .count(),
                usize::from(settings.squad_size)
            );
        }
    }
}

#[test]
fn a_move_preview_is_the_route_execution_uses() {
    let mut game = game(SEEDS[0]);
    let id = PlayerId(0);
    let destination = game.world.config.exit();
    let preview = game.preview_move(id, destination);
    assert!(preview.can_move());
    assert_eq!(
        preview.action_point_cost,
        u16::try_from(preview.affordable_steps).expect("compact route")
            * u16::from(game.settings.costs.move_step)
    );
    for step in preview.executable_steps() {
        game.apply(id, TacticsAction::Move(step.face))
            .expect("a previewed step is legal");
        assert_eq!(game.units[&id].cell, step.to);
    }
    assert_eq!(game.units[&id].cell, preview.stopping_cell);
}

#[test]
fn action_availability_names_ap_refusals_without_mutating() {
    let mut game = game(SEEDS[0]);
    let id = PlayerId(0);
    let before = game.digest();
    let available = game.action_availability(id, TacticsAction::DeployAnchor);
    assert!(available.enabled);
    assert_eq!(available.cost, game.settings.costs.anchor);
    assert_eq!(
        game.digest(),
        before,
        "an availability query mutated the match"
    );

    game.units.get_mut(&id).expect("unit").action_points = 0;
    let unavailable = game.action_availability(id, TacticsAction::DeployAnchor);
    assert!(!unavailable.enabled);
    assert_eq!(unavailable.refusal, Some(Refusal::NotEnoughActionPoints));
}

#[test]
fn a_squad_cannot_spend_more_than_its_action_points() {
    let mut game = game(SEEDS[0]);
    let id = PlayerId(0);
    let budget = game.settings.action_points;
    let mut spent = 0u8;
    while let Some(&action) = game.legal_actions(id).first() {
        let cost = action.cost(&game.settings.costs);
        assert!(game.apply(id, action).is_ok());
        spent += cost;
        assert!(spent <= budget, "spent {spent} of a {budget} point budget");
    }
    assert_eq!(game.units[&id].action_points, budget - spent);
    // With nothing affordable left, every action is refused for the same reason.
    let exhausted = game.units[&id].action_points;
    for face in observed_hex::HexFace::ALL {
        if exhausted < game.settings.costs.move_step {
            assert_eq!(
                game.apply(id, TacticsAction::Move(face)),
                Err(Refusal::NotEnoughActionPoints)
            );
        }
    }
}

#[test]
fn ending_a_turn_restores_the_whole_squad_to_full_points() {
    let mut game = game(SEEDS[0]);
    wander(&mut game);
    assert!(
        game.units
            .values()
            .any(|unit| unit.action_points < game.settings.action_points),
        "the wander should have spent something"
    );
    game.end_turn();
    for unit in game.units.values() {
        assert_eq!(unit.action_points, game.settings.action_points);
    }
    assert_eq!(game.turn, 2);
}

#[test]
fn a_unit_never_steps_through_a_sealed_face() {
    for seed in SEEDS {
        let mut game = game(seed);
        let id = PlayerId(0);
        for _ in 0..12 {
            let from = game.units[&id].cell;
            let legal: BTreeSet<_> = vision::exits(&game.world, from)
                .into_iter()
                .map(|(face, _)| face)
                .collect();
            for face in observed_hex::HexFace::ALL {
                if legal.contains(&face) {
                    continue;
                }
                assert_eq!(
                    game.apply(id, TacticsAction::Move(face)),
                    Err(Refusal::Blocked),
                    "seed {seed:#x}: a sealed {face:?} face accepted a step"
                );
                assert_eq!(game.units[&id].cell, from, "a refused move still moved");
            }
            let Some(&step) = game
                .legal_actions(id)
                .iter()
                .find(|action| matches!(action, TacticsAction::Move(_)))
            else {
                break;
            };
            if game.apply(id, step).is_err() {
                break;
            }
            if game.units[&id].action_points < game.settings.costs.move_step {
                game.end_turn();
            }
        }
    }
}

/// A move must land somewhere real. The port pairing is what guarantees it, and
/// a one-sided check would let a unit walk into a shaft it cannot climb.
#[test]
fn every_legal_move_lands_in_solid_space_reachable_from_both_sides() {
    for seed in SEEDS {
        let game = game(seed);
        for unit in game.units.values() {
            for (face, destination) in vision::exits(&game.world, unit.cell) {
                assert!(vision::is_solid_space(&game.world, destination));
                assert_eq!(
                    vision::step_through(&game.world, destination, face.opposite()),
                    Some(unit.cell),
                    "seed {seed:#x}: {face:?} is passable one way only"
                );
            }
        }
    }
}

/// The headline safety property. Whatever the pocket contains, committing it
/// must never rewrite ground the squad is standing on, watching, or holding.
#[test]
fn a_shift_never_touches_an_occupied_observed_or_anchored_cell() {
    for seed in SEEDS {
        let mut game = game(seed);
        for _ in 0..8 {
            wander(&mut game);
            let occupied: BTreeSet<HexCoord> = game.units.values().map(|unit| unit.cell).collect();
            let observed = game.observed_cells();
            let anchored = game.anchored_cells();
            let before = game.world.placements.clone();

            game.end_turn();

            for (cell, placement) in &game.world.placements {
                let protected =
                    occupied.contains(cell) || observed.contains(cell) || anchored.contains(cell);
                if protected {
                    assert_eq!(
                        before.get(cell),
                        Some(placement),
                        "seed {seed:#x}: protected cell {cell:?} was rewritten"
                    );
                }
            }
        }
    }
}

/// A shift must never strand the exit. `commit_relayout_delta` revalidates
/// routes and rolls back rather than accepting a layout that breaks one, and
/// this is the property a player actually depends on.
#[test]
fn the_exit_stays_reachable_across_many_shifts() {
    for seed in SEEDS {
        let mut game = game(seed);
        for turn in 0..12 {
            wander(&mut game);
            game.end_turn();
            let spawn = game.world.config.spawn();
            let exit = game.world.config.exit();
            assert!(
                game.world.route_between(spawn, exit).is_some(),
                "seed {seed:#x}: no route to the exit after turn {turn}"
            );
        }
    }
}

/// The guard under several of the tests above. If the relayout never actually
/// committed — because a Compact board is mostly rooms and protective halo, say —
/// then "a shift never touches a protected cell" would pass by doing nothing, and
/// the mechanic this lab exists to tune would be untested.
#[test]
fn the_facility_really_does_shift() {
    let mut committed = 0;
    let mut changed = 0;
    for seed in SEEDS {
        let mut game = game(seed);
        for _ in 0..10 {
            wander(&mut game);
            let before = game.world.placements.clone();
            if game.end_turn() == ShiftOutcome::Committed {
                committed += 1;
                changed += before
                    .iter()
                    .filter(|(cell, placement)| {
                        game.world.placements.get(*cell) != Some(*placement)
                    })
                    .count();
            }
        }
    }
    assert!(
        committed > 0,
        "no seed ever committed a shift; the relayout is inert"
    );
    assert!(
        changed > 0,
        "shifts committed but rewrote nothing, so nothing was being tested"
    );
}

/// Standing in the telegraphed pocket is the mechanic. If the squad occupies it,
/// the commit must be refused and reported as held.
#[test]
fn holding_the_telegraphed_pocket_refuses_the_shift() {
    let mut held_at_least_once = false;
    for seed in SEEDS {
        let mut game = game(seed);
        for _ in 0..10 {
            let Some(pocket) = game.telegraph.as_ref().map(|t| t.cells().clone()) else {
                game.end_turn();
                continue;
            };
            // Put a unit in the pocket by fiat. Walking there is a routing
            // problem, not the property under test.
            let Some(&target) = pocket
                .iter()
                .find(|cell| vision::is_solid_space(&game.world, **cell))
            else {
                game.end_turn();
                continue;
            };
            let id = PlayerId(0);
            game.units.get_mut(&id).expect("unit").cell = target;
            let before = game.world.placements.clone();

            let outcome = game.end_turn();

            if outcome == ShiftOutcome::Held {
                held_at_least_once = true;
                assert_eq!(
                    before, game.world.placements,
                    "a held shift still changed cells"
                );
            }
        }
    }
    assert!(
        held_at_least_once,
        "occupying the telegraphed pocket never once held it"
    );
}

/// Switching the shift off must leave the facility genuinely static — the
/// control case a player compares against.
#[test]
fn a_match_with_shift_off_never_changes_a_cell() {
    let mut game = TacticsGame::new(MatchSettings {
        shift: ShiftCadence::Off,
        ..compact(SEEDS[0])
    })
    .expect("solves");
    let before = game.world.placements.clone();
    let generation = game.world.generation;
    for _ in 0..10 {
        wander(&mut game);
        assert_eq!(game.end_turn(), ShiftOutcome::NothingToShift);
    }
    assert_eq!(before, game.world.placements);
    assert_eq!(generation, game.world.generation);
}

#[test]
fn knowledge_never_contains_a_cell_the_squad_has_not_reached() {
    for seed in SEEDS {
        let mut game = game(seed);
        for _ in 0..6 {
            wander(&mut game);
            game.end_turn();
            let reachable: BTreeSet<HexCoord> = game
                .units
                .values()
                .filter(|unit| unit.team == PLAYER_TEAM)
                .flat_map(|unit| {
                    vision::observed_from(&game.world, unit.cell, game.settings.sight.hops())
                })
                .collect();
            let known = &game.knowledge[&PLAYER_TEAM].cells;
            // Everything currently in sight must be known. The converse is not a
            // rule — knowledge is memory, so it outlives the sight that made it.
            for cell in &reachable {
                assert!(
                    known.contains_key(cell),
                    "seed {seed:#x}: {cell:?} is in sight but unknown"
                );
            }
        }
    }
}

/// A remembered cell that the facility rewrote must read as stale, so the map
/// can show the player which of their notes are no longer trustworthy.
#[test]
fn a_rewritten_cell_reads_stale_until_it_is_seen_again() {
    for seed in SEEDS {
        let mut game = game(seed);
        for _ in 0..10 {
            wander(&mut game);
            let before: Vec<HexCoord> =
                game.knowledge[&PLAYER_TEAM].cells.keys().copied().collect();
            game.end_turn();
            let observed = game.observed_cells();
            let map = &game.knowledge[&PLAYER_TEAM];
            for cell in before {
                let Some(known) = map.cells.get(&cell) else {
                    continue;
                };
                if known.is_stale(&game.world, cell) {
                    assert!(
                        !observed.contains(&cell),
                        "seed {seed:#x}: {cell:?} is in sight and still reads stale"
                    );
                }
            }
        }
    }
}

/// Full Map is a drawing option. If it changed the simulation, the whole-board
/// view would be measuring a different game than the fogged one.
#[test]
fn seeing_the_whole_map_changes_nothing_the_solver_protects() {
    for seed in SEEDS {
        let fogged = MatchSettings {
            sight: Sight::Corridor,
            ..compact(seed)
        };
        let revealed = MatchSettings {
            sight: Sight::FullMap,
            ..compact(seed)
        };
        let mut a = TacticsGame::new(fogged).expect("solves");
        let mut b = TacticsGame::new(revealed).expect("solves");
        for turn in 0..8 {
            wander(&mut a);
            wander(&mut b);
            a.end_turn();
            b.end_turn();
            assert_eq!(
                a.digest(),
                b.digest(),
                "seed {seed:#x}: Full Map diverged from Corridor at turn {turn}"
            );
        }
    }
}

#[test]
fn the_exit_refuses_a_squad_that_has_not_finished_its_objectives() {
    let mut game = TacticsGame::new(MatchSettings {
        objectives: Objectives::Keystones,
        keystones_required: 1,
        ..compact(SEEDS[0])
    })
    .expect("solves");
    assert!(!game.team_authorized(PLAYER_TEAM));

    let exit = game.world.config.exit();
    for unit in game.units.values_mut() {
        unit.cell = exit;
    }
    game.end_turn();
    assert_eq!(
        game.status,
        MatchStatus::Running,
        "an unauthorized squad escaped"
    );
    assert!(game.units.values().all(|unit| !unit.escaped));

    // Grant the keystones the way the match does, then try again.
    game.objectives.team_mut(PLAYER_TEAM).keystones = 1;
    assert!(game.team_authorized(PLAYER_TEAM));
    for unit in game.units.values_mut() {
        unit.cell = exit;
    }
    let ids: Vec<PlayerId> = game.units.keys().copied().collect();
    for id in ids {
        game.apply(id, TacticsAction::Interact).ok();
    }
    game.end_turn();
    assert_eq!(game.status, MatchStatus::Escaped);
}

#[test]
fn a_station_needs_two_units_held_for_the_configured_turns() {
    let settings = MatchSettings {
        objectives: Objectives::Full,
        station_hold_turns: 2,
        squad_size: 3,
        ..compact(SEEDS[0])
    };
    let Ok(mut game) = TacticsGame::new(settings) else {
        return;
    };
    let Some(station) = game
        .world
        .blueprints
        .iter()
        .find(|blueprint| blueprint.role == RoomRole::DualStation)
        .map(|blueprint| blueprint.cells.clone())
    else {
        // Not every compact solve places one; the rule is tested where it exists.
        return;
    };

    // One unit is not enough, however long it waits.
    game.units.get_mut(&PlayerId(0)).expect("unit").cell = station[0];
    for _ in 0..4 {
        game.end_turn();
    }
    assert_eq!(game.objectives.team(PLAYER_TEAM).station_turns, 0);
    assert!(!game.objectives.team(PLAYER_TEAM).station_complete);

    // Two units, held for the configured number of turns.
    game.units.get_mut(&PlayerId(1)).expect("unit").cell = station[0];
    game.end_turn();
    assert_eq!(game.objectives.team(PLAYER_TEAM).station_turns, 1);
    game.units.get_mut(&PlayerId(0)).expect("unit").cell = station[0];
    game.units.get_mut(&PlayerId(1)).expect("unit").cell = station[0];
    game.end_turn();
    assert!(game.objectives.team(PLAYER_TEAM).station_complete);
}

/// Leaving the station resets the count. Without this the room is a two-turn
/// tax rather than a coordination problem.
#[test]
fn breaking_a_station_pairing_resets_its_progress() {
    let settings = MatchSettings {
        objectives: Objectives::Full,
        station_hold_turns: 3,
        squad_size: 3,
        ..compact(SEEDS[0])
    };
    let Ok(mut game) = TacticsGame::new(settings) else {
        return;
    };
    let Some(station) = game
        .world
        .blueprints
        .iter()
        .find(|blueprint| blueprint.role == RoomRole::DualStation)
        .map(|blueprint| blueprint.cells[0])
    else {
        return;
    };
    game.units.get_mut(&PlayerId(0)).expect("unit").cell = station;
    game.units.get_mut(&PlayerId(1)).expect("unit").cell = station;
    game.end_turn();
    assert_eq!(game.objectives.team(PLAYER_TEAM).station_turns, 1);

    game.units.get_mut(&PlayerId(1)).expect("unit").cell = game.world.config.spawn();
    game.end_turn();
    assert_eq!(
        game.objectives.team(PLAYER_TEAM).station_turns,
        0,
        "walking away left the station counting"
    );
}

#[test]
fn an_anchor_holds_its_cell_and_gives_it_back_when_recovered() {
    let mut game = game(SEEDS[0]);
    let id = PlayerId(0);
    let cell = game.units[&id].cell;
    let carried = game.anchors.inventory(id);
    assert!(carried > 0, "a squad starts with something to place");

    assert!(
        game.legal_actions(id)
            .contains(&TacticsAction::DeployAnchor)
    );
    game.apply(id, TacticsAction::DeployAnchor)
        .expect("deploys");
    assert_eq!(game.anchor_in(cell), Some(id));
    assert_eq!(game.anchors.inventory(id), carried - 1);
    assert!(game.anchored_cells().contains(&cell));
    // A second anchor in the same cell is not on offer.
    assert!(
        !game
            .legal_actions(id)
            .contains(&TacticsAction::DeployAnchor)
    );

    game.apply(id, TacticsAction::RecoverAnchor)
        .expect("recovers");
    assert_eq!(game.anchor_in(cell), None);
    assert_eq!(game.anchors.inventory(id), carried);
    assert!(!game.anchored_cells().contains(&cell));
}

/// An anchored cell survives a shift even with nobody watching it. That is what
/// separates an anchor from standing there yourself.
#[test]
fn an_anchor_holds_a_cell_the_squad_has_walked_away_from() {
    for seed in SEEDS {
        let mut game = game(seed);
        let id = PlayerId(0);
        game.apply(id, TacticsAction::DeployAnchor)
            .expect("deploys");
        let anchored = game.units[&id].cell;
        let before = game.world.placements[&anchored];
        // Move the whole squad far away so nothing observes the anchor.
        let exit = game.world.config.exit();
        for unit in game.units.values_mut() {
            unit.cell = exit;
        }
        for _ in 0..6 {
            game.end_turn();
            assert_eq!(
                game.world.placements[&anchored], before,
                "seed {seed:#x}: an unwatched anchored cell was rewritten"
            );
        }
    }
}

#[test]
fn a_pad_pair_links_and_a_single_pad_does_not() {
    let mut game = TacticsGame::new(MatchSettings {
        pads: true,
        ..compact(SEEDS[0])
    })
    .expect("solves");
    let id = PlayerId(0);
    let first = game.units[&id].cell;
    game.apply(id, TacticsAction::DeployPad).expect("deploys");
    assert_eq!(game.units[&id].cell, first, "one pad linked to nothing");

    // Walk to another cell and drop the second plate.
    let Some(&step) = game
        .legal_actions(id)
        .iter()
        .find(|action| matches!(action, TacticsAction::Move(_)))
    else {
        return;
    };
    game.apply(id, step).expect("steps");
    let second = game.units[&id].cell;
    assert_ne!(first, second);
    if !game.legal_actions(id).contains(&TacticsAction::DeployPad) {
        return;
    }
    game.apply(id, TacticsAction::DeployPad).expect("deploys");
    // Placing under your own feet suppresses the link, exactly as the shipped
    // rule does — otherwise the second plate fires the instant it lands.
    assert_eq!(game.units[&id].cell, second);
    assert!(game.pads.is_suppressed(id));

    // Next turn clears the re-arm, and stepping back onto a plate links.
    game.end_turn();
    assert!(!game.pads.is_suppressed(id));
}

#[test]
fn the_guardian_freezes_when_the_squad_is_watching_it() {
    let mut game = TacticsGame::new(MatchSettings {
        guardian: GuardianSetting::Hunting,
        ..compact(SEEDS[0])
    })
    .expect("solves");
    let guardian = game.guardian.expect("a hunting match has one").cell;
    // Stand a unit on it: the strongest form of observation there is.
    game.units.get_mut(&PlayerId(0)).expect("unit").cell = guardian;
    game.end_turn();
    let after = game.guardian.expect("still there");
    assert_eq!(after.cell, guardian, "an observed Guardian moved");
    assert_eq!(
        after.status,
        observed_match::hex_wfc::HexGuardianStatus::FrozenByPlayer
    );
}

#[test]
fn switching_the_guardian_off_leaves_no_guardian_at_all() {
    let game = TacticsGame::new(MatchSettings {
        guardian: GuardianSetting::Off,
        ..compact(SEEDS[0])
    })
    .expect("solves");
    assert!(game.guardian.is_none());
    assert!(game.guardian_status().is_none());
}

/// The property every other test quietly depends on: the same settings and the
/// same actions produce the same match, every time.
#[test]
fn a_match_replays_identically_from_its_settings_and_action_log() {
    for seed in SEEDS {
        let settings = compact(seed);
        let mut first = TacticsGame::new(settings).expect("solves");
        for _ in 0..8 {
            wander(&mut first);
            first.end_turn();
        }
        let log = first.log.clone();

        let mut second = TacticsGame::new(settings).expect("solves");
        let mut turn = 1;
        for entry in &log {
            while turn < entry.turn {
                second.end_turn();
                turn += 1;
            }
            second
                .apply(entry.unit, entry.action)
                .expect("a logged action replays");
        }
        while turn < first.turn {
            second.end_turn();
            turn += 1;
        }
        assert_eq!(
            first.digest(),
            second.digest(),
            "seed {seed:#x}: replaying the log produced a different match"
        );
    }
}

#[test]
fn a_squad_size_outside_the_offered_range_is_refused() {
    for squad_size in [0, 1, crate::settings::MAX_SQUAD + 1] {
        assert_eq!(
            TacticsGame::new(MatchSettings {
                squad_size,
                ..compact(SEEDS[0])
            })
            .err(),
            Some(super::TacticsError::InvalidSquad),
            "a squad of {squad_size} was accepted"
        );
    }
}

/// A free step would let every "spend until you cannot" loop run forever. The
/// configuration is refused rather than each loop being defended separately.
#[test]
fn a_match_where_stepping_is_free_is_refused() {
    assert_eq!(
        TacticsGame::new(MatchSettings {
            costs: ActionCosts {
                move_step: 0,
                ..ActionCosts::default()
            },
            ..compact(SEEDS[0])
        })
        .err(),
        Some(super::TacticsError::FreeMovement)
    );
}

#[test]
fn a_match_cannot_require_more_keystones_than_its_facility_contains() {
    let settings = MatchSettings {
        objectives: crate::settings::Objectives::Keystones,
        keystones_required: u8::MAX,
        ..compact(SEEDS[0])
    };
    assert!(matches!(
        TacticsGame::new(settings),
        Err(super::TacticsError::InsufficientKeystones {
            required: u8::MAX,
            ..
        })
    ));
}

/// Costs are settings, so a match that prices moves differently must actually
/// afford differently.
#[test]
fn action_costs_come_from_the_settings() {
    let mut game = TacticsGame::new(MatchSettings {
        action_points: 4,
        costs: ActionCosts {
            move_step: 2,
            ..ActionCosts::default()
        },
        ..compact(SEEDS[0])
    })
    .expect("solves");
    let id = PlayerId(0);
    let mut steps = 0;
    while let Some(&action) = game
        .legal_actions(id)
        .iter()
        .find(|action| matches!(action, TacticsAction::Move(_)))
    {
        game.apply(id, action).expect("steps");
        steps += 1;
    }
    assert_eq!(steps, 2, "four points at two a step is two steps");
}

/// Rival units belong to a different team, so their IDs must not collide with
/// the player's however the squad size setting moves.
#[test]
fn rival_units_never_share_an_id_with_the_player_squad() {
    for squad_size in crate::settings::MIN_SQUAD..=crate::settings::MAX_SQUAD {
        let game = TacticsGame::new(MatchSettings {
            squad_size,
            rival_team: true,
            ..compact(SEEDS[0])
        })
        .expect("solves");
        let players: BTreeSet<PlayerId> = game
            .units
            .values()
            .filter(|unit| unit.team == PLAYER_TEAM)
            .map(|unit| unit.id)
            .collect();
        let rivals: BTreeSet<PlayerId> = game
            .units
            .values()
            .filter(|unit| unit.team == TeamId(1))
            .map(|unit| unit.id)
            .collect();
        assert_eq!(players.len(), usize::from(squad_size));
        assert_eq!(rivals.len(), usize::from(squad_size));
        assert!(players.is_disjoint(&rivals));
    }
}
