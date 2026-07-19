use std::collections::BTreeMap;
use std::path::PathBuf;

use observed_authoring::{Manifest, TilePrototype};
use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexFace, HexWfcConfig, HexWfcWorld, PortClass};

use super::*;

fn tiles() -> Vec<TilePrototype> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles");
    Manifest::load(&base.join("manifest.ron"))
        .expect("manifest")
        .load_tiles(&base)
        .expect("tiles")
}

fn showcase_config(levels: u8) -> HexWfcConfig {
    HexWfcConfig {
        cols: 12,
        rows: 9,
        levels,
        min_rooms: 4,
        max_rooms: 8,
        retry_budget: 100,
        min_room_distance: 2,
    }
}

fn showcase_match(seed: u64, levels: u8, players: u8) -> HexWfcMatch {
    HexWfcMatch::new(
        seed,
        HexMatchConfig {
            players,
            wfc: showcase_config(levels),
        },
        &tiles(),
    )
    .expect("showcase match")
}

fn bot_player_command(game: &HexWfcMatch, id: PlayerId) -> HexPlayerCommand {
    HexPlayerCommand {
        intent: game.bot_command(id),
        actions: HexActionButtons::default(),
    }
}

/// Classify the vertical transitions on the solved spawn→exit route.
/// Returns `(ramp_transitions, shaft_transitions)`.
fn route_vertical_profile(world: &HexWfcWorld) -> (u32, u32) {
    let Some(route) = world.route_between(world.config.spawn(), world.config.exit()) else {
        return (0, 0);
    };
    let mut ramps = 0;
    let mut shafts = 0;
    for pair in route.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if a.level == b.level {
            continue;
        }
        let face = if b.level > a.level {
            HexFace::Up
        } else {
            HexFace::Down
        };
        let placement = &world.placements[&a];
        let class = if face == HexFace::Up {
            placement.up
        } else {
            placement.down
        };
        match class {
            PortClass::RampOpen => ramps += 1,
            PortClass::ShaftOpen => shafts += 1,
            _ => {}
        }
    }
    (ramps, shafts)
}

fn run_bot_to_exit(game: &mut HexWfcMatch, max_ticks: u64) -> Option<u64> {
    for tick in 0..max_ticks {
        let commands = game
            .players
            .keys()
            .copied()
            .filter(|id| !game.players[id].escaped)
            .map(|id| (id, bot_player_command(game, id)))
            .collect();
        game.step(&HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick,
            commands,
        });
        if game.status == HexMatchStatus::Finished {
            return Some(game.tick);
        }
    }
    None
}

#[test]
#[ignore = "seed scan for a mid-match committed relayout; run manually with --nocapture"]
fn scan_mutation_seeds() {
    for raw in 0u64..40 {
        let seed = 0xA11C_9500_0000_0000 ^ raw.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let Ok(mut game) = HexWfcMatch::new(
            seed,
            HexMatchConfig {
                players: 4,
                wfc: showcase_config(4),
            },
            &tiles(),
        ) else {
            continue;
        };
        let mut warned = None;
        let mut committed = None;
        let mut cancelled = 0u32;
        for tick in 0..2_400u64 {
            let commands = game
                .players
                .keys()
                .copied()
                .filter(|id| !game.players[id].escaped)
                .map(|id| (id, bot_player_command(&game, id)))
                .collect();
            let events = game
                .step(&HexInputFrame {
                    version: HEX_INPUT_VERSION,
                    tick,
                    commands,
                })
                .to_vec();
            for event in events {
                match event.kind {
                    HexMatchEventKind::MutationWarning => warned = warned.or(Some(event.tick)),
                    HexMatchEventKind::MutationCommitted => {
                        committed = committed.or(Some((event.tick, game.facility.generation)));
                    }
                    HexMatchEventKind::MutationCancelled => cancelled += 1,
                    _ => {}
                }
            }
            if game.status == HexMatchStatus::Finished {
                break;
            }
        }
        eprintln!(
            "seed={seed:#018x} warned={warned:?} committed={committed:?} cancelled={cancelled} final_gen={}",
            game.facility.generation
        );
    }
}

#[test]
#[ignore = "seed scan for the headless gate; run manually with --nocapture"]
fn scan_gate_seeds() {
    for &levels in &[5u8, 4u8] {
        let mut found = 0;
        for raw in 0u64..6000 {
            let seed = 0xA11C_0000_0000_0000 ^ raw.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let Ok(world) = HexWfcWorld::generate(seed, showcase_config(levels)) else {
                continue;
            };
            let (ramps, shafts) = route_vertical_profile(&world);
            if ramps >= 2 && shafts >= 1 {
                eprintln!(
                    "GATE_CANDIDATE levels={levels} seed={seed:#018x} ramps={ramps} shafts={shafts}"
                );
                found += 1;
                if found >= 12 {
                    break;
                }
            }
        }
        eprintln!("levels={levels} candidates_found={found}");
    }
}

#[test]
#[ignore = "trajectory diagnostic"]
fn diagnose_bot() {
    let seed = std::env::var("HEX_DIAG_SEED")
        .ok()
        .and_then(|value| u64::from_str_radix(value.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0xa11c000000000000u64);
    let levels: u8 = std::env::var("HEX_DIAG_LEVELS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(5);
    let world = HexWfcWorld::generate(seed, showcase_config(levels)).expect("world");
    let route = world
        .route_between_cells(world.config.spawn(), world.config.exit())
        .expect("weighted route");
    eprintln!(
        "spawn={:?} exit={:?}",
        world.config.spawn(),
        world.config.exit()
    );
    eprintln!("weighted route ({} cells):", route.cells.len());
    for c in &route.cells {
        let p = &world.placements[c];
        eprintln!(
            "  {:?} arch={:?} up={:?} down={:?} doors={:06b}",
            c, p.archetype, p.up, p.down, p.doors
        );
    }
    let mut game = showcase_match(seed, levels, 1);
    let mut last_cell = game.players[&PlayerId(0)].cell;
    for tick in 0..8_000u64 {
        let cmd = game.bot_command(PlayerId(0));
        let commands = BTreeMap::from([(
            PlayerId(0),
            HexPlayerCommand {
                intent: cmd,
                actions: HexActionButtons::default(),
            },
        )]);
        game.step(&HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick,
            commands,
        });
        let p = &game.players[&PlayerId(0)];
        if p.cell != last_cell {
            eprintln!(
                "tick={tick} cell={:?} pos=({:.1},{:.1},{:.1}) climb={:?} cmd_jump={} cmd_int={}",
                p.cell,
                p.position.x,
                p.position.y,
                p.position.z,
                p.climb_target,
                cmd.jump_pressed,
                cmd.interact_held
            );
            last_cell = p.cell;
        } else if tick % 1000 == 0 && tick > 0 {
            let b = game.bodies[&PlayerId(0)];
            eprintln!(
                "  ..stuck tick={tick} cell={:?} pos=({:.1},{:.1},{:.1}) climb={:?} transit={:?} mv=({:.2},{:.2}) look={:.2} jump={} int={} yaw={:.2} vel=({:.2},{:.2},{:.2}) grounded={}",
                p.cell,
                p.position.x,
                p.position.y,
                p.position.z,
                p.climb_target,
                p.transit_target,
                cmd.movement.x,
                cmd.movement.y,
                cmd.look.x,
                cmd.jump_pressed,
                cmd.interact_held,
                b.yaw,
                b.velocity.x,
                b.velocity.y,
                b.velocity.z,
                b.grounded
            );
        }
        if p.escaped {
            eprintln!("ESCAPED at tick {tick}");
            break;
        }
    }
    let p = &game.players[&PlayerId(0)];
    eprintln!(
        "final cell={:?} pos=({:.1},{:.1},{:.1})",
        p.cell, p.position.x, p.position.y, p.position.z
    );
}

/// Pinned headless gate seed (found via `scan_gate_seeds`). Its solved 12×9×5
/// showcase route crosses two ramp levels and two shafts.
const GATE_SEED: u64 = 0xa11c_0000_0000_0000;
const GATE_LEVELS: u8 = 5;

/// Phase 94 success criterion 1 — the headless gate. On a pinned seed whose
/// solved route crosses ≥2 ramp levels and ≥1 shaft, an objective bot completes
/// spawn→exit, and it does so deterministically: two independent runs reach the
/// exit on the identical tick and end on the identical snapshot digest.
#[test]
fn headless_gate_bot_crosses_ramps_and_shaft_deterministically() {
    let world = HexWfcWorld::generate(GATE_SEED, showcase_config(GATE_LEVELS)).expect("world");
    let (ramps, shafts) = route_vertical_profile(&world);
    assert!(
        ramps >= 2 && shafts >= 1,
        "gate route must cross >=2 ramp levels and >=1 shaft, got ramps={ramps} shafts={shafts}"
    );

    let mut first = showcase_match(GATE_SEED, GATE_LEVELS, 1);
    let a = run_bot_to_exit(&mut first, 40_000).expect("gate bot completes spawn->exit (run A)");
    let mut second = showcase_match(GATE_SEED, GATE_LEVELS, 1);
    let b = run_bot_to_exit(&mut second, 40_000).expect("gate bot completes spawn->exit (run B)");

    assert_eq!(
        a, b,
        "gate must be deterministic: run A escaped on tick {a}, run B on {b}"
    );
    assert_eq!(
        first.snapshot().digest,
        second.snapshot().digest,
        "identical inputs must yield identical final snapshot digests"
    );
}

#[test]
fn compiled_catalog_hash_participates_in_network_snapshot_identity() {
    let first = showcase_match(GATE_SEED, GATE_LEVELS, 1);
    let mut mismatched = showcase_match(GATE_SEED, GATE_LEVELS, 1);
    mismatched.bind_simulation_content_hash([0xA5; 32]);

    assert_ne!(first.snapshot().digest, mismatched.snapshot().digest);
    assert_eq!(mismatched.snapshot().simulation_content_hash, [0xA5; 32]);
}

/// Phase 94 success criterion 2 — the bot-stall soak (Arc K standard). Every bot
/// of a four-bot cohort must reach the exit on every generated showcase layout;
/// a single stall is a failure. Layouts that fail to generate or expose no
/// spawn→exit route are skipped (not stalls), and the test asserts that a
/// meaningful number of real layouts were exercised.
#[test]
fn bot_soak_has_no_stalls() {
    let mut exercised = 0;
    for raw in 0u64..14 {
        let seed = 0x50A6_0000_0000_0000 ^ raw.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        for &levels in &[4u8, 5u8] {
            let Ok(world) = HexWfcWorld::generate(seed, showcase_config(levels)) else {
                continue;
            };
            if world
                .route_between(world.config.spawn(), world.config.exit())
                .is_none()
            {
                continue;
            }
            let mut game = showcase_match(seed, levels, 4);
            let finished = run_bot_to_exit(&mut game, 40_000);
            assert!(
                finished.is_some(),
                "STALL: seed={seed:#018x} levels={levels}: not all four bots escaped within budget \
                 (escaped={:?}, players={:?}, stuck={:?}, guardian={:?})",
                game.escape_order,
                game.players,
                game.stuck_ticks,
                game.guardian
            );
            // A robust route must never fling a body out of the world.
            assert!(
                !game
                    .recent_events
                    .iter()
                    .any(|event| event.kind == HexMatchEventKind::PlayerRecovered),
                "seed={seed:#018x} levels={levels}: a body needed fall recovery"
            );
            exercised += 1;
        }
    }
    assert!(
        exercised >= 12,
        "soak exercised too few layouts: {exercised}"
    );
}

/// Phase 94 success criterion 3 — every open blueprint door port is a two-way
/// traversable threshold: if a room cell opens a lateral door, the corridor cell
/// on the far side opens the matching reciprocal face, so a body can cross it in
/// both directions. Checked across a spread of generated facilities.
#[test]
fn every_open_blueprint_door_is_two_way_traversable() {
    let mut checked = 0;
    for raw in 0u64..10 {
        let seed = 0x7D00_0000_0000_0000 ^ raw.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let Ok(game) = HexWfcMatch::new(
            seed,
            HexMatchConfig {
                players: 1,
                wfc: showcase_config(5),
            },
            &tiles(),
        ) else {
            continue;
        };
        let grid = game.facility.config.grid();
        for door in game.door_states().into_iter().filter(|door| door.open) {
            assert!(
                door.face.is_lateral(),
                "blueprint door {:?} is on a non-lateral face {:?}",
                door.key,
                door.face
            );
            let near = &game.facility.placements[&door.room_cell];
            assert!(
                near.is_open(door.face),
                "door {:?} reports open but the room cell is sealed",
                door.key
            );
            let neighbor = grid
                .neighbor(door.room_cell, door.face)
                .expect("an open door must have an in-grid neighbor");
            let far = game
                .facility
                .placements
                .get(&neighbor)
                .expect("an open door's neighbor must be placed");
            assert!(
                far.is_open(door.face.opposite()),
                "door {:?} is one-way: {:?} opens {:?} but neighbor {:?} seals {:?}",
                door.key,
                door.room_cell,
                door.face,
                neighbor,
                door.face.opposite()
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "no open blueprint doors were exercised");
}

/// Phase 94 success criterion 4 — movement determinism. The step loop is pure,
/// so replaying one scripted input sequence on two independent matches yields
/// bit-identical per-tick snapshot digests (the headless == interactive
/// guarantee the Phase 95 shell relies on; both drive the same `step`).
#[test]
fn movement_is_deterministic_on_a_scripted_input_sequence() {
    let script = scripted_inputs(600);
    let digests = |()| {
        let mut game = showcase_match(GATE_SEED, GATE_LEVELS, 3);
        let mut trace = Vec::with_capacity(script.len());
        for frame in &script {
            game.step(frame);
            trace.push(game.snapshot().digest);
        }
        trace
    };
    assert_eq!(
        digests(()),
        digests(()),
        "scripted movement must be deterministic"
    );
}

/// Phase 94 deliverable 7 — ordinary inter-level drops are survivable-by-design.
/// The gate route descends and climbs 8 m between levels; running it must never
/// raise a `PlayerRecovered` (which is reserved for a body leaving the world).
#[test]
fn ordinary_drops_do_not_trigger_recovery_on_the_gate_route() {
    let mut game = showcase_match(GATE_SEED, GATE_LEVELS, 1);
    let mut recoveries = 0;
    for tick in 0..40_000u64 {
        let commands = game
            .players
            .keys()
            .copied()
            .filter(|id| !game.players[id].escaped)
            .map(|id| (id, bot_player_command(&game, id)))
            .collect();
        let events = game.step(&HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick,
            commands,
        });
        recoveries += events
            .iter()
            .filter(|event| event.kind == HexMatchEventKind::PlayerRecovered)
            .count();
        if game.status == HexMatchStatus::Finished {
            break;
        }
    }
    assert_eq!(
        recoveries, 0,
        "8 m gate-route drops must be survivable, not recovered"
    );
    assert_eq!(
        game.status,
        HexMatchStatus::Finished,
        "gate bot must finish"
    );
}

/// A mid-match observation-safe local relayout actually fires in play at the
/// deterministic 8--12 second cadence. Two
/// independent runs with the identical scripted bot inputs must reproduce the
/// same generation timeline and the same final snapshot digest byte-for-byte.
///
/// The fixture's first warned pocket commits on its first scheduled attempt.
const MUTATION_SEED: u64 = 0xA11C_9500_0000_0000;

#[test]
fn observed_relayout_commits_mid_match_deterministically() {
    fn run() -> (Vec<(u64, u32)>, Vec<u32>, u64) {
        let mut game = showcase_match(MUTATION_SEED, 4, 4);
        let first_commit = mutation::scheduled_mutation_tick(MUTATION_SEED, 0);
        let mut committed = Vec::new();
        let mut generations = Vec::new();
        let mut warned = false;
        for tick in 0..first_commit + 30 {
            let commands = game
                .players
                .keys()
                .copied()
                .filter(|id| !game.players[id].escaped)
                .map(|id| (id, bot_player_command(&game, id)))
                .collect();
            let events = game
                .step(&HexInputFrame {
                    version: HEX_INPUT_VERSION,
                    tick,
                    commands,
                })
                .to_vec();
            for event in events {
                match event.kind {
                    HexMatchEventKind::MutationWarning => warned = true,
                    HexMatchEventKind::MutationCommitted => {
                        committed.push((event.tick, game.facility.generation));
                    }
                    _ => {}
                }
            }
            generations.push(game.facility.generation);
        }
        assert!(warned, "a relayout warning must precede the commit");
        (committed, generations, game.snapshot().digest)
    }

    let (committed, generations, digest) = run();
    let first_commit = mutation::scheduled_mutation_tick(MUTATION_SEED, 0);
    assert_eq!(
        committed,
        vec![(first_commit, 1)],
        "the observed local relayout must commit at its deterministic scheduled tick"
    );
    assert_eq!(
        generations.first().copied(),
        Some(0),
        "the facility starts at generation 0"
    );
    assert_eq!(
        generations.last().copied(),
        Some(1),
        "the committed relayout leaves the match at generation 1"
    );

    // Determinism: identical seed + scripted bot inputs reproduce the timeline.
    let (committed_b, generations_b, digest_b) = run();
    assert_eq!(
        committed, committed_b,
        "commit timeline must be deterministic"
    );
    assert_eq!(
        generations, generations_b,
        "the per-tick generation timeline must be deterministic"
    );
    assert_eq!(
        digest, digest_b,
        "the final snapshot digest must be deterministic"
    );
}

/// A deterministic, varied scripted input sequence for one to three players:
/// walking, turning, sprinting, jumping, and interacting, so the determinism
/// check exercises every branch of the step loop rather than a null intent.
fn scripted_inputs(ticks: u64) -> Vec<HexInputFrame> {
    (0..ticks)
        .map(|tick| {
            let mut commands = BTreeMap::new();
            for raw in 0u16..3 {
                let phase = tick.wrapping_add(u64::from(raw) * 37);
                let intent = PlayerIntent {
                    movement: glam::Vec2::new(
                        ((phase % 5) as f32 - 2.0) / 2.0,
                        ((phase / 5) % 3) as f32 - 1.0,
                    ),
                    look: glam::Vec2::new(((phase % 7) as f32 - 3.0) * 0.1, 0.0),
                    jump_pressed: phase % 23 == 0,
                    sprint_held: phase % 2 == 0,
                    interact_held: phase % 17 == 0,
                    ..PlayerIntent::default()
                };
                commands.insert(
                    PlayerId(raw),
                    HexPlayerCommand {
                        intent,
                        actions: HexActionButtons::default(),
                    },
                );
            }
            HexInputFrame {
                version: HEX_INPUT_VERSION,
                tick,
                commands,
            }
        })
        .collect()
}
