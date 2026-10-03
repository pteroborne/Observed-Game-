use std::collections::{BTreeMap, BTreeSet};

use glam::Vec2;
use observed_facility::hex_wfc::HexWfcConfig;
use observed_hex::HexCoord;
use player_input::PlayerIntent;

use super::*;
use crate::ascent::session::SeatCommand;
use crate::ascent::sim::{ArchitectCommand, CommandRefusal, RETRACTION_TICKS};
use crate::hex_wfc::{
    HEX_INPUT_VERSION, HexMatchConfig, HexMatchEventKind, HexPlayerCommand, HexWfcGeometrySnapshot,
};

const ARCHITECT: PlayerId = PlayerId(40);
const BODY: PlayerId = PlayerId(0);
const TEAM: TeamId = TeamId(0);

fn game(seed: u64) -> AscentMatch {
    game_with(seed, 1, false)
}

/// One team of `members` bodies and its Architect, on a two-level facility.
fn game_with(seed: u64, members: u8, guardian: bool) -> AscentMatch {
    game_seated(seed, members, guardian, false)
}

/// As [`game_with`], with the Architect's seat held by a bot when `bot` is set.
fn game_seated(seed: u64, members: u8, guardian: bool, bot: bool) -> AscentMatch {
    let config = HexMatchConfig {
        teams: 1,
        members_per_team: members,
        guardian,
        wfc: HexWfcConfig {
            levels: 2,
            ..HexWfcConfig::default()
        },
    };
    let physical = HexWfcMatch::new_with_content(
        seed,
        config,
        crate::hex_wfc::compatibility_test_content().clone(),
    )
    .expect("a two-level facility solves");
    AscentMatch::new(
        physical,
        seed,
        BTreeMap::from([(
            ARCHITECT,
            Seat {
                role: Role::Architect(TEAM),
                bot,
            },
        )]),
    )
    .expect("a team and its Architect")
}

/// How the body moves this tick.
#[derive(Clone, Copy)]
enum Body {
    /// Stands and turns at this rate.
    Turn(f32),
    /// Walks the facility the way the game's own bot does.
    Explore,
}

/// One tick: the body moves, and the Architect sends `command`.
fn step(game: &mut AscentMatch, body: Body, command: SeatCommand) -> BTreeMap<PlayerId, Refusal> {
    step_bodies(game, &[(BODY, body)], command)
}

/// One tick: each listed body moves, and the Architect sends `command`.
fn step_bodies(
    game: &mut AscentMatch,
    moves: &[(PlayerId, Body)],
    command: SeatCommand,
) -> BTreeMap<PlayerId, Refusal> {
    let tick = game.rules().tick + 1;
    let commands = moves
        .iter()
        .map(|&(player, body)| {
            let intent = match body {
                Body::Turn(rate) => HexPlayerCommand {
                    intent: PlayerIntent {
                        look: Vec2::new(rate, 0.0),
                        ..PlayerIntent::default()
                    },
                    ..HexPlayerCommand::default()
                },
                Body::Explore => game.physical().bot_player_command(player),
            };
            (player, intent)
        })
        .collect();
    let bodies = HexInputFrame {
        version: HEX_INPUT_VERSION,
        tick,
        commands,
    };
    let seats = InputFrame {
        version: ASCENT_INPUT_VERSION,
        tick,
        commands: BTreeMap::from([(ARCHITECT, command)]),
    };
    let refusals = game.step(&bodies, &seats).expect("a well-formed frame");
    assert_eq!(
        game.rules().world.placements,
        game.physical().facility.placements,
        "tick {tick}: the rules and the bodies disagree about the facility"
    );
    refusals
}

/// A play the Architect may make now, preferring one that `wanted` accepts.
fn find_play(
    game: &AscentMatch,
    mut wanted: impl FnMut(&AscentMatch, HexCoord, u8, usize) -> bool,
) -> Option<ArchitectCommand> {
    let hand = &game.session().hands[&TEAM].deck.hand;
    let known = &game.rules().team_knowledge[&TEAM].discovered_cells;
    for (index, card) in hand.iter().enumerate() {
        for &target in known {
            for rotation in 0..6 {
                let command = ArchitectCommand::Play {
                    card: card.id,
                    target,
                    rotation,
                };
                if game
                    .session()
                    .architect_refusal(ARCHITECT, command)
                    .is_none()
                    && wanted(game, target, rotation, index)
                {
                    return Some(command);
                }
            }
        }
    }
    None
}

/// Walk the body until the Architect has a play `wanted` accepts. Every so often it stops
/// and looks round, as a player does: the rules know only what a body has seen, and a body
/// the bot has parked sees little beyond the cells it is warding.
fn explore_until_playable(
    game: &mut AscentMatch,
    wanted: impl Fn(&AscentMatch, HexCoord, u8, usize) -> bool + Copy,
) -> ArchitectCommand {
    for tick in 0..6_000 {
        if tick % 30 == 0
            && let Some(command) = find_play(game, wanted)
        {
            return command;
        }
        let body = if (tick / 300) % 3 == 2 {
            Body::Turn(0.6)
        } else {
            Body::Explore
        };
        step(game, body, SeatCommand::None);
    }
    panic!("walking the facility never uncovered a legal play");
}

/// A card of `kind` from the seated Rogue's hand (the rules' own), played on `target`:
/// staged into the hand, as a capture stages what it needs.
fn rogue_play(
    game: &mut AscentMatch,
    kind: crate::ascent::sim::CardKind,
    target: HexCoord,
) -> ArchitectCommand {
    let deck = &mut game.ascent.session.sim.deck;
    assert!(deck.stage_kind(kind), "the Rogue's deck deals {kind:?}");
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

fn target_of(command: ArchitectCommand) -> HexCoord {
    match command {
        ArchitectCommand::Play { target, .. } => target,
        ArchitectCommand::Requisition => {
            unreachable!("plays only")
        }
    }
}

/// Every piece and collider the physical match holds is the one a fresh projection
/// of its facility would build.
fn assert_geometry_is_fresh(game: &AscentMatch) {
    let content = crate::hex_wfc::compatibility_test_content();
    let physical = game.physical();
    let fresh = HexWfcGeometrySnapshot::project_with_rooms(
        &physical.facility,
        content.cells(),
        content.rooms(),
    )
    .expect("the facility projects");
    let pieces = |snapshot: &HexWfcGeometrySnapshot| {
        snapshot
            .pieces
            .iter()
            .map(|piece| (piece.id, piece.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    let colliders = |snapshot: &HexWfcGeometrySnapshot| {
        snapshot
            .arena
            .colliders
            .iter()
            .map(|collider| (collider.id, collider.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(pieces(&physical.geometry), pieces(&fresh));
    assert_eq!(colliders(&physical.geometry), colliders(&fresh));
}

#[test]
fn a_card_play_is_built_into_the_facility_the_bodies_walk_in() {
    let mut game = game(7);
    let command = explore_until_playable(&mut game, |_, _, _, _| true);
    let target = target_of(command);
    let before = game.physical().facility.placements[&target];
    let generation = game.physical().facility.generation;

    let refusals = step(&mut game, Body::Turn(0.0), SeatCommand::Architect(command));

    assert!(refusals.is_empty(), "{refusals:?}");
    let after = game.physical().facility.placements[&target];
    assert_ne!(after, before, "the play changed the cell");
    assert!(after.space.built());
    assert_eq!(game.physical().facility.generation, generation + 1);
    assert!(
        game.physical()
            .last_relayout_delta
            .as_ref()
            .is_some_and(|delta| delta.changed_cells.contains(&target))
    );
    assert!(
        game.physical()
            .geometry
            .pieces
            .iter()
            .any(|piece| piece.source_cell == target),
        "the new tile has pieces"
    );
    assert_geometry_is_fresh(&game);
}

#[test]
fn a_contradiction_retracts_out_of_the_physical_facility() {
    let mut game = game(7);
    // A play that leaves an open door against a built neighbour's wall.
    let contradicts = |game: &AscentMatch, target: HexCoord, rotation: u8, index: usize| {
        let mut probe = game.rules().clone();
        let card = game.session().hands[&TEAM].deck.hand[index];
        probe.deck.hand = vec![card];
        probe.cooldown = 0;
        probe.known.insert(target);
        probe
            .submit(ArchitectCommand::Play {
                card: card.id,
                target,
                rotation,
            })
            .is_ok()
            && !probe.contradictions.is_empty()
    };
    let command = explore_until_playable(&mut game, contradicts);
    step(&mut game, Body::Turn(0.0), SeatCommand::Architect(command));
    assert!(!game.rules().contradictions.is_empty());

    // Look away, and let the front run.
    let mut retracted = BTreeSet::new();
    for _ in 0..RETRACTION_TICKS * 4 {
        step(&mut game, Body::Turn(1.0), SeatCommand::None);
        retracted.extend(game.rules().retracted.iter().copied());
        if !retracted.is_empty() {
            break;
        }
    }
    let &cell = retracted
        .iter()
        .next()
        .expect("an unwatched contradiction retracts");
    assert!(game.physical().facility.placements[&cell].space.unbuilt());
    assert!(
        !game
            .physical()
            .geometry
            .pieces
            .iter()
            .any(|piece| piece.source_cell == cell),
        "a retracted tile leaves nothing to stand on"
    );
    assert_geometry_is_fresh(&game);
}

#[test]
fn a_tile_ahead_of_the_body_is_held_by_its_sight() {
    let mut game = game(7);
    // Explore until the body sees past its own cell: a body hemmed in by walls sees
    // nothing else, and fairly.
    for tick in 0..6_000 {
        step(&mut game, Body::Explore, SeatCommand::None);
        let rules = game.rules();
        let body = &rules.observers[&ObserverId(BODY.0)];
        if tick >= 600
            && rules.observed.iter().any(|&at| {
                at != body.cell && rules.team_knowledge[&TEAM].discovered_cells.contains(&at)
            })
        {
            break;
        }
    }
    let rules = game.rules();
    let body = &rules.observers[&ObserverId(BODY.0)];
    let (cell, facing) = game
        .physical()
        .body_cell_and_facing(BODY)
        .expect("the body exists");
    assert_eq!(
        (body.cell, body.facing),
        (cell, facing),
        "the rules follow the body"
    );
    let ahead = rules
        .observed
        .iter()
        .copied()
        .find(|&at| at != body.cell && rules.team_knowledge[&TEAM].discovered_cells.contains(&at))
        .expect("the body wards something beyond its own cell");
    let card = game.session().hands[&TEAM]
        .deck
        .hand
        .iter()
        .find(|card| card.district == Some(game.rules().district(ahead.level)))
        .copied()
        .expect("a card for this floor");
    assert_eq!(
        game.session().architect_refusal(
            ARCHITECT,
            ArchitectCommand::Play {
                card: card.id,
                target: ahead,
                rotation: 0,
            },
        ),
        Some(Refusal::Architect(CommandRefusal::Observed))
    );
}

#[test]
fn rooms_and_stairs_are_refused_whole() {
    let mut game = game(7);
    let fixed = |game: &AscentMatch| {
        let rules = game.rules();
        rules.team_knowledge[&TEAM]
            .discovered_cells
            .iter()
            .copied()
            .find(|&cell| rules.fixed_structure(cell) && !rules.observed.contains(&cell))
    };
    let mut target = fixed(&game);
    for _ in 0..6_000 {
        if target.is_some() {
            break;
        }
        step(&mut game, Body::Explore, SeatCommand::None);
        target = fixed(&game);
    }
    let target = target.expect("the body has seen a room or a stair and looked away");
    let district = game.rules().district(target.level);
    let card = game.session().hands[&TEAM]
        .deck
        .hand
        .iter()
        .find(|card| card.district == Some(district))
        .copied()
        .expect("a hand holds a card for each district it can reach");
    for rotation in 0..6 {
        assert_eq!(
            game.session().architect_refusal(
                ARCHITECT,
                ArchitectCommand::Play {
                    card: card.id,
                    target,
                    rotation,
                },
            ),
            Some(Refusal::Architect(CommandRefusal::FixedStructure))
        );
    }
}

#[test]
fn a_directed_facility_changes_only_when_an_architect_changes_it() {
    let mut game = game(7);
    let generation = game.physical().facility.generation;
    // Well past the director's latest scheduled relayout.
    for _ in 0..crate::hex_wfc::MAX_MUTATION_TICKS + 120 {
        step(&mut game, Body::Turn(0.3), SeatCommand::None);
        assert!(!game.physical().recent_events.iter().any(|event| matches!(
            event.kind,
            HexMatchEventKind::MutationWarning | HexMatchEventKind::MutationCommitted
        )));
    }
    assert_eq!(game.physical().facility.generation, generation);
}

#[test]
fn the_same_commands_build_the_same_facility() {
    let run = || {
        // Seed 11's bot parks its body in the start rooms, and a body sees only what is in
        // front of it: rooms and a stair, which no card may touch.
        let mut game = game(7);
        let command = explore_until_playable(&mut game, |_, _, _, _| true);
        step(&mut game, Body::Turn(0.0), SeatCommand::Architect(command));
        for _ in 0..RETRACTION_TICKS * 2 {
            step(&mut game, Body::Turn(0.5), SeatCommand::None);
        }
        (
            game.physical().facility.placements.clone(),
            game.physical().facility.cell_revisions.clone(),
            game.rules().tick,
            game.physical().geometry.pieces.len(),
        )
    };
    assert_eq!(run(), run());
}

#[test]
fn no_legal_play_redraws_what_a_body_is_watching() {
    let mut game = game(7);
    let mut checked = 0;
    for tick in 0..3_000 {
        if tick % 150 == 0 {
            let watched = game.rules().observed.clone();
            // What is drawn of each watched cell, as a set: a room beside a play is
            // re-projected whole, and may come back in another order with nothing moved -
            // the presentation redraws by revision, and those are not bumped.
            let drawn = |game: &AscentMatch| {
                let mut pieces = game
                    .physical()
                    .geometry
                    .pieces
                    .iter()
                    .filter(|piece| watched.contains(&piece.source_cell))
                    .cloned()
                    .collect::<Vec<_>>();
                pieces.sort_by_key(|piece| piece.id);
                pieces
            };
            let before = drawn(&game);
            let mut plays = Vec::new();
            let _ = find_play(&game, |_, target, rotation, index| {
                if plays.len() < 12 {
                    plays.push((target, rotation, index));
                }
                false
            });
            for (target, rotation, index) in plays {
                let mut probe = game.clone();
                let card = probe.session().hands[&TEAM].deck.hand[index].id;
                let command = ArchitectCommand::Play {
                    card,
                    target,
                    rotation,
                };
                assert!(
                    step(&mut probe, Body::Turn(0.0), SeatCommand::Architect(command)).is_empty()
                );
                assert_eq!(drawn(&probe), before, "{command:?} redrew a watched cell");
                checked += 1;
            }
        }
        step(&mut game, Body::Explore, SeatCommand::None);
    }
    assert!(checked >= 20, "only {checked} plays were checked");
}

/// A retraction can take a built cell beside a room a body is standing in, which would
/// open that room's window in front of it. The rules ward that neighbour.
#[test]
fn a_retraction_cannot_open_a_window_beside_a_watched_room() {
    use observed_facility::hex_wfc::exposure::follows_neighbours;
    use observed_facility::hex_wfc::{HexArchetype, HexPlacement, HexSpace};
    use observed_hex::{HexFace, PortClass};

    let redrawn_by = |game: &AscentMatch, watched: HexCoord, neighbour: HexCoord| {
        let drawn = |physical: &HexWfcMatch| {
            physical
                .geometry
                .pieces
                .iter()
                .filter(|piece| piece.source_cell == watched)
                .cloned()
                .collect::<Vec<_>>()
        };
        let mut probe = game.physical().clone();
        let before = drawn(&probe);
        let rock = HexPlacement {
            coord: neighbour,
            space: HexSpace::Void,
            archetype: HexArchetype::Void,
            doors: 0,
            up: PortClass::Sealed,
            down: PortClass::Sealed,
        };
        probe
            .apply_directed_change(BTreeMap::from([(neighbour, rock)]))
            .is_ok()
            && drawn(&probe) != before
    };
    // Over a few seeds, because whether a body's walk takes it into such a room is the
    // facility's to say: seed 7's stopped doing so when every floor became one district.
    let warded = [7, 3, 11, 13, 5].into_iter().find(|&seed| {
        let mut game = game(seed);
        for _ in 0..6_000 {
            let rules = game.rules();
            let body = &rules.observers[&ObserverId(BODY.0)];
            let grid = rules.world.config.grid();
            if rules
                .world
                .placements
                .get(&body.cell)
                .is_some_and(follows_neighbours)
            {
                let exposed = HexFace::LATERAL
                    .into_iter()
                    .filter(|&face| face != body.facing)
                    .filter_map(|face| grid.neighbor(body.cell, face))
                    .find(|&next| {
                        rules.world.placements[&next].space.built()
                            && !rules.fixed_structure(next)
                            && redrawn_by(&game, body.cell, next)
                    });
                if let Some(next) = exposed {
                    assert!(rules.observed.contains(&next), "seed {seed}");
                    assert!(rules.retraction_protected(next), "seed {seed}");
                    return true;
                }
            }
            step(&mut game, Body::Explore, SeatCommand::None);
        }
        false
    });
    assert!(
        warded.is_some(),
        "no body stood where a neighbour's retraction would redraw it"
    );
}

mod anchors;
mod doors;
mod guardians;
mod kinetic;
mod power;
mod prison;
mod requests;
mod rogue;
mod sensors;
mod sight;
mod stairs;

#[test]
fn a_bot_architect_repairs_what_the_rogue_breaks_through_the_human_path() {
    const ROGUE: PlayerId = PlayerId(41);
    let config = HexMatchConfig {
        teams: 1,
        members_per_team: 1,
        guardian: false,
        wfc: HexWfcConfig {
            levels: 2,
            ..HexWfcConfig::default()
        },
    };
    // Seed 11: on seed 7's facility, since one district a floor, a minor wave catches the
    // lone body and ends the match before the repair this is about can be made.
    let seed = 11;
    let physical = HexWfcMatch::new_with_content(
        seed,
        config,
        crate::hex_wfc::compatibility_test_content().clone(),
    )
    .expect("a two-level facility solves");
    let seats = BTreeMap::from([
        (
            ARCHITECT,
            Seat {
                role: Role::Architect(TEAM),
                bot: true,
            },
        ),
        (
            ROGUE,
            Seat {
                role: Role::Rogue,
                bot: false,
            },
        ),
    ]);
    let mut game = AscentMatch::new(physical, seed, seats).expect("a bot Architect and a Rogue");
    for _ in 0..600 {
        step(&mut game, Body::Explore, SeatCommand::None);
    }
    // The Rogue plays a contradiction near the body, somewhere it is not looking but its
    // team has mapped: an Architect can repair only what the team knows is there.
    let body = game.rules().observers[&ObserverId(BODY.0)].cell;
    let mapped = game.rules().team_knowledge[&TEAM].discovered_cells.clone();
    let sabotage = game
        .rules()
        .deck
        .hand
        .iter()
        .flat_map(|card| {
            game.rules()
                .mutable_targets()
                .into_iter()
                .flat_map(move |target| {
                    (0..6).map(move |rotation| ArchitectCommand::Play {
                        card: card.id,
                        target,
                        rotation,
                    })
                })
        })
        .filter(|command| {
            let ArchitectCommand::Play { target, .. } = *command else {
                return false;
            };
            observed_hex::travel_distance(target, body) <= 2
                && mapped.contains(&target)
                && game.session().architect_refusal(ROGUE, *command).is_none()
        })
        .find(|&command| {
            let mut probe = game.rules().clone();
            probe.submit(command).is_ok() && !probe.contradictions.is_empty()
        })
        .expect("the Rogue has a contradiction to play near the body");
    let seats = InputFrame {
        version: ASCENT_INPUT_VERSION,
        tick: game.rules().tick + 1,
        commands: BTreeMap::from([(ROGUE, SeatCommand::Architect(sabotage))]),
    };
    let bodies = HexInputFrame {
        version: HEX_INPUT_VERSION,
        tick: game.rules().tick + 1,
        commands: BTreeMap::new(),
    };
    assert!(game.step(&bodies, &seats).expect("well formed").is_empty());
    assert!(!game.rules().contradictions.is_empty());

    let mut repaired = false;
    let mut slowest = std::time::Duration::ZERO;
    for _ in 0..600 {
        let started = std::time::Instant::now();
        step(&mut game, Body::Turn(0.0), SeatCommand::None);
        slowest = slowest.max(started.elapsed());
        repaired |= game
            .rules()
            .traces
            .get("Architect 0")
            .and_then(|trace| trace.selected)
            == Some("repair a contradiction");
        if repaired && game.rules().contradictions.is_empty() {
            break;
        }
    }
    eprintln!("slowest tick with a bot Architect deciding: {slowest:?}");
    assert!(repaired, "the bot never repaired the Rogue's contradiction");
    assert_geometry_is_fresh(&game);
}

/// Evidence, not regression cover: how many minors a production match collects. Ten
/// minutes of two teams of two, bot bodies, bot Architects and the bot Rogue, counting
/// the minors alive on each floor, the waves, and how many minors those waves would have
/// released without `MINORS_PER_FLOOR`. The bots never shove, so every minor that goes is
/// one that fell or walked off something on its own.
#[test]
#[ignore = "ten minutes of production play (about a minute); prints the crowd, asserts nothing"]
fn production_minor_crowd() {
    use crate::ascent::economy::MINORS_PER_FLOOR;
    let content = std::sync::Arc::new(crate::hex_wfc::HexMatchContent::from_runtime_catalog(
        crate::hex_wfc::test_catalog().clone(),
    ));
    for seed in [1u64, 2] {
        let config = HexMatchConfig {
            teams: 2,
            members_per_team: 2,
            guardian: true,
            wfc: HexWfcConfig::arc_default(),
        };
        let physical = HexWfcMatch::new_with_content(seed, config, content.clone()).unwrap();
        let seats = super::architect_seats(&physical, None);
        let mut game = AscentMatch::new(physical, seed, seats).unwrap();
        let (mut released, mut lost, mut peak_total) = (0usize, 0usize, 0usize);
        // Bodies that fell into true void: the price of an unrailed edge, to them too.
        let mut fell = Vec::new();
        let mut peak_floor: BTreeMap<u8, usize> = BTreeMap::new();
        let mut at_minute = Vec::new();
        for tick in 1..=36_000u64 {
            let commands = game
                .physical()
                .players
                .keys()
                .map(|&p| (p, game.physical().bot_player_command(p)))
                .collect();
            let bodies = HexInputFrame {
                version: HEX_INPUT_VERSION,
                tick,
                commands,
            };
            let seats = InputFrame {
                version: ASCENT_INPUT_VERSION,
                tick,
                commands: BTreeMap::new(),
            };
            if game.step(&bodies, &seats).is_err() {
                eprintln!("seed {seed}: the match ended at tick {tick}");
                break;
            }
            for event in &game.physical().recent_events {
                match event.kind {
                    HexMatchEventKind::GuardianReleased => released += 1,
                    HexMatchEventKind::GuardianLost => lost += 1,
                    HexMatchEventKind::PlayerLost => fell.push((tick, event.cell)),
                    _ => {}
                }
            }
            let mut per_floor: BTreeMap<u8, usize> = BTreeMap::new();
            for guardian in game.physical().released.values() {
                if guardian.kind() == HexReleasedKind::Minor {
                    *per_floor.entry(guardian.cell().level).or_default() += 1;
                }
            }
            let total: usize = per_floor.values().sum();
            peak_total = peak_total.max(total);
            for (&level, &count) in &per_floor {
                let peak = peak_floor.entry(level).or_default();
                *peak = (*peak).max(count);
            }
            if tick % 3_600 == 0 {
                at_minute.push(total);
            }
            if game.rules().outcome != crate::ascent::sim::MatchOutcome::Running {
                eprintln!(
                    "seed {seed}: decided at tick {tick}: {:?}",
                    game.rules().outcome
                );
                break;
            }
        }
        let economy = &game.rules().economy;
        let uncapped: u32 = economy
            .wave_counts
            .iter()
            .map(|(&level, &waves)| waves * (1 + u32::from(level)))
            .sum();
        eprintln!(
            "seed {seed}: waves {:?}; released {released} (uncapped the waves would \
             have released {uncapped}); lost {lost}; alive each minute {at_minute:?}; \
             peak alive {peak_total}, peak per floor {peak_floor:?} (ceiling {MINORS_PER_FLOOR}); \
             bodies lost to the void (tick, cell) {fell:?}",
            economy.wave_counts,
        );
    }
}

/// Evidence, not regression cover: a production facility with two bot Architects,
/// printing tick times. Measured 2026-09-26: median about 0.1 ms, a bot's decision beat
/// about 3 ms, and a Guardian catch's tick about 3-6 ms, its maze carved ahead
/// (`hex_wfc::model::prison::carving`); it was 35-40 ms when the maze's geometry and
/// colliders were built on the tick of the catch.
#[test]
#[ignore = "two minutes of production play (about 10 s); prints timings, asserts nothing"]
fn production_ascent_tick_times() {
    let content = std::sync::Arc::new(crate::hex_wfc::HexMatchContent::from_runtime_catalog(
        crate::hex_wfc::test_catalog().clone(),
    ));
    let config = HexMatchConfig {
        teams: 2,
        members_per_team: 2,
        guardian: true,
        wfc: HexWfcConfig::arc_default(),
    };
    let physical = HexWfcMatch::new_with_content(1, config, content).unwrap();
    // The seats every Ascent match has: a bot Architect a team, and the bot Rogue.
    let seats = super::architect_seats(&physical, None);
    let started = std::time::Instant::now();
    let mut game = AscentMatch::new(physical, 1, seats).unwrap();
    eprintln!("construct {:?}", started.elapsed());
    let mut times = Vec::new();
    // The ticks a body was caught on, which carve a maze: what the prison costs.
    let mut catches = Vec::new();
    for _ in 0..7_200 {
        let tick = game.rules().tick + 1;
        let commands = game
            .physical()
            .players
            .keys()
            .map(|&p| (p, game.physical().bot_player_command(p)))
            .collect();
        let bodies = HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick,
            commands,
        };
        let seats = InputFrame {
            version: ASCENT_INPUT_VERSION,
            tick,
            commands: BTreeMap::new(),
        };
        let started = std::time::Instant::now();
        // The rules may decide the match before the soak ends; what decided it is printed.
        if game.step(&bodies, &seats) == Err(Refusal::MatchFinished) {
            break;
        }
        times.push(started.elapsed());
        if game
            .physical()
            .recent_events
            .iter()
            .any(|event| event.kind == HexMatchEventKind::PlayerJailed)
        {
            catches.push(started.elapsed());
        }
        if started.elapsed() > std::time::Duration::from_millis(8) {
            eprintln!(
                "slow tick {} {:?}: {:?}",
                tick,
                started.elapsed(),
                game.physical()
                    .recent_events
                    .iter()
                    .map(|e| e.kind)
                    .collect::<Vec<_>>()
            );
        }
    }
    eprintln!(
        "outcome {:?} at tick {}",
        game.rules().outcome,
        game.rules().tick
    );
    let mut sorted = times.clone();
    sorted.sort();
    let plays = game.rules().command_log.len();
    let traces: Vec<_> = game
        .rules()
        .traces
        .iter()
        // "Architect" alone is the Rogue's; "Architect <team>" each loyal bot's.
        .filter(|(k, _)| k.starts_with("Architect"))
        .map(|(k, t)| (k.clone(), t.selected))
        .collect();
    eprintln!(
        "catches {} slowest catch tick {:?}",
        catches.len(),
        catches.iter().max()
    );
    eprintln!(
        "median {:?} p95 {:?} max {:?} plays {plays} traces {traces:?}",
        sorted[sorted.len() / 2],
        sorted[sorted.len() * 95 / 100],
        sorted.last().unwrap()
    );
    let beats: Vec<_> = times
        .iter()
        .enumerate()
        .filter(|(i, _)| (i + 1) % 60 == 0)
        .map(|(_, t)| *t)
        .collect();
    eprintln!("beat ticks: max {:?}", beats.iter().max());
}

/// A LAN match's peers each build the rules from the launch and step the server's
/// frames. They must stay identical tick for tick - bot Architects, Guardian catches,
/// mazes carved on other threads and all - or every client desyncs.
#[test]
fn two_peers_stepping_the_same_frames_stay_in_step() {
    // Seed 15: a seed whose match reaches a catch, which is the part of the match this is
    // here to cover. 13's facility, since the climb compositions, ran 12,000 ticks without
    // one; 15's catches by tick 900 and ends by 3,400.
    const SEED: u64 = 15;
    let config = HexMatchConfig {
        teams: 2,
        members_per_team: 2,
        guardian: true,
        wfc: HexWfcConfig {
            levels: 2,
            ..HexWfcConfig::default()
        },
    };
    let peer = || {
        let physical = HexWfcMatch::new_with_content(
            SEED,
            config,
            crate::hex_wfc::compatibility_test_content().clone(),
        )
        .expect("a two-level facility solves");
        let seats = super::architect_seats(&physical, None);
        AscentMatch::new(physical, SEED, seats).expect("bot Architects for both teams")
    };
    let (mut server, mut client) = (peer(), peer());
    let mut caught = 0;
    for _ in 0..12_000 {
        let tick = server.rules().tick + 1;
        let commands: BTreeMap<_, _> = server
            .physical()
            .players
            .keys()
            .map(|&player| (player, server.physical().bot_player_command(player)))
            .collect();
        let bodies = HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick,
            commands,
        };
        let seats = InputFrame {
            version: ASCENT_INPUT_VERSION,
            tick,
            commands: BTreeMap::new(),
        };
        let _ = server.step(&bodies, &seats);
        let _ = client.step(&bodies, &seats);
        assert_eq!(
            server.physical().snapshot().digest,
            client.physical().snapshot().digest,
            "the peers parted at tick {tick}"
        );
        assert_eq!(server.rules().command_log, client.rules().command_log);
        caught += server
            .physical()
            .recent_events
            .iter()
            .filter(|event| event.kind == HexMatchEventKind::PlayerJailed)
            .count();
        if server.rules().outcome != crate::ascent::sim::MatchOutcome::Running {
            break;
        }
    }
    assert!(
        caught > 0,
        "a catch, so mazes carved on other threads were covered"
    );
}

/// Playing a multi-tile Cistern wonder card updates the physical facility, geometry, and
/// colliders atomically across all 3 cells of the room.
#[test]
fn cistern_card_play_updates_physical_geometry_and_colliders() {
    use crate::ascent::sim::CardKind;
    use observed_facility::hex_wfc::{HexArchetype, HexSpace};

    let mut game = game(7);
    assert!(
        game.ascent
            .session
            .hands
            .get_mut(&TEAM)
            .unwrap()
            .deck
            .stage_kind(CardKind::Cistern)
    );

    let mut play = None;
    for _ in 0..3_000 {
        play = find_play(&game, |g, _target, _rotation, index| {
            g.session().hands[&TEAM].deck.hand[index].kind == CardKind::Cistern
        });
        if play.is_some() {
            break;
        }
        step(&mut game, Body::Explore, SeatCommand::None);
    }
    let play = play.expect("found a legal cistern site after exploration");
    let ArchitectCommand::Play {
        target, rotation, ..
    } = play
    else {
        unreachable!()
    };
    let placements = game.rules().played_cistern(target, rotation);
    let cells: Vec<HexCoord> = placements.iter().map(|p| p.coord).collect();

    let refusals = step(&mut game, Body::Turn(0.0), SeatCommand::Architect(play));
    assert!(refusals.is_empty(), "command accepted");

    for cell in &cells {
        let physical_p = game.physical().facility.placements[cell];
        assert_eq!(physical_p.space, HexSpace::Hall);
        assert_eq!(physical_p.archetype, HexArchetype::Expanse);
        let has_colliders = game
            .physical()
            .geometry
            .pieces
            .iter()
            .any(|piece| piece.source_cell == *cell && piece.part.collides());
        assert!(
            has_colliders,
            "physical match generated colliders for cistern cell {cell:?}"
        );
    }
}
