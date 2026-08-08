use std::collections::BTreeMap;

use observed_authoring::RoomSocketKind;
use observed_authoring::TilePrototype;
use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexFace, HexWfcConfig, HexWfcWorld, PortClass};

use super::*;

fn tiles() -> Vec<TilePrototype> {
    crate::hex_wfc::test_tiles()
}

/// The committed catalog's whole-room modules.
///
/// Bot tests used to run with **no** room prototypes, so every multi-hex room
/// fragmented into per-cell fallback tiles — geometry the game never ships, and
/// whose interior furniture (1.3 m tall, against a 0.45 m autostep) physically
/// trapped bots. `tiles()` deliberately keeps the compatibility hall kit while
/// adding only the strict authored towers; rooms come from the real catalog.
fn rooms() -> &'static [observed_authoring::RoomPrototype] {
    crate::hex_wfc::compatibility_test_content().rooms()
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
    HexWfcMatch::new_with_rooms(
        seed,
        HexMatchConfig {
            guardian: true,
            teams: players,
            members_per_team: 1,
            wfc: showcase_config(levels),
        },
        &tiles(),
        rooms(),
    )
    .expect("showcase match")
}

fn bot_player_command(
    driver: &mut HexBotDriver,
    game: &HexWfcMatch,
    id: PlayerId,
) -> HexPlayerCommand {
    HexPlayerCommand {
        intent: driver.command(game, id).intent,
        actions: HexActionButtons::default(),
    }
}

#[test]
fn default_roster_is_two_stable_teams_of_two() {
    let game =
        HexWfcMatch::new(44, HexMatchConfig::default(), &tiles()).expect("default showcase match");
    assert_eq!(game.teams.len(), 2);
    assert_eq!(
        game.teams[&observed_core::TeamId(0)].members,
        vec![PlayerId(0), PlayerId(1)]
    );
    assert_eq!(
        game.teams[&observed_core::TeamId(1)].members,
        vec![PlayerId(2), PlayerId(3)]
    );
    assert_eq!(game.players[&PlayerId(1)].team, observed_core::TeamId(0));
    assert_eq!(game.players[&PlayerId(2)].team, observed_core::TeamId(1));
}

#[test]
fn match_retains_the_shared_immutable_content_value() {
    let content = std::sync::Arc::clone(crate::hex_wfc::compatibility_test_content());
    let game = HexWfcMatch::new_with_content(44, HexMatchConfig::default(), content.clone())
        .expect("default showcase match");

    assert!(std::ptr::eq(game.content(), content.as_ref()));
    assert_eq!(
        game.simulation_content_hash,
        content.simulation_content_hash()
    );
}

#[test]
fn complete_bot_commands_reuse_routes_until_the_facility_changes() {
    let mut game = showcase_match(44, 3, 1);
    let mut driver = HexBotDriver::new();
    let player = PlayerId(0);

    let _ = driver.command(&game, player);
    assert!(driver.has_cached_route(player));
    let cached_generation = driver.cached_route_generation(player);

    let _ = driver.command(&game, player);
    assert_eq!(
        driver.cached_route_generation(player),
        cached_generation,
        "an unchanged cell, objective, and generation reuse the route"
    );

    game.facility.generation = game.facility.generation.wrapping_add(1);
    let _ = driver.command(&game, player);
    assert_eq!(
        driver.cached_route_generation(player),
        Some(game.facility.generation),
        "a relayout generation invalidates the derived route"
    );
}

#[test]
fn keystones_and_distinct_station_sockets_gate_team_escape() {
    let mut game = HexWfcMatch::new_with_rooms(
        44,
        HexMatchConfig {
            guardian: false,
            teams: 1,
            members_per_team: 2,
            wfc: showcase_config(4),
        },
        &tiles(),
        rooms(),
    )
    .expect("objective showcase");
    let cells = game
        .facility
        .route_between(game.facility.config.spawn(), game.facility.config.exit())
        .expect("spawn-exit route");
    let a_cell = cells[0];
    let b_cell = *cells.get(1).expect("route has a second cell");
    let point = |cell| glam::Vec3::from_array(observed_hex::hex_origin(cell)) + glam::Vec3::Y;
    game.geometry.sockets = vec![
        super::super::geometry::HexRoomSocket {
            room_generation_key: 10,
            room_role: observed_facility::map_spec::RoomRole::Keystone,
            id: "key_a".to_string(),
            kind: RoomSocketKind::Keystone,
            cell: a_cell,
            position: point(a_cell),
            yaw_degrees: 0.0,
        },
        super::super::geometry::HexRoomSocket {
            room_generation_key: 11,
            room_role: observed_facility::map_spec::RoomRole::Keystone,
            id: "key_b".to_string(),
            kind: RoomSocketKind::Keystone,
            cell: a_cell,
            position: point(a_cell),
            yaw_degrees: 0.0,
        },
        super::super::geometry::HexRoomSocket {
            room_generation_key: 20,
            room_role: observed_facility::map_spec::RoomRole::DualStation,
            id: "station_a".to_string(),
            kind: RoomSocketKind::StationA,
            cell: a_cell,
            position: point(a_cell),
            yaw_degrees: 0.0,
        },
        super::super::geometry::HexRoomSocket {
            room_generation_key: 20,
            room_role: observed_facility::map_spec::RoomRole::DualStation,
            id: "station_b".to_string(),
            kind: RoomSocketKind::StationB,
            cell: b_cell,
            position: point(b_cell),
            yaw_degrees: 180.0,
        },
    ];
    game.objectives = HexObjectiveState::new(&game);
    assert!(game.objectives.enabled);

    let interact = |commands: &mut BTreeMap<PlayerId, HexPlayerCommand>, player| {
        commands.insert(
            player,
            HexPlayerCommand {
                actions: HexActionButtons {
                    interact: true,
                    ..HexActionButtons::default()
                },
                ..HexPlayerCommand::default()
            },
        );
    };
    game.players.get_mut(&PlayerId(0)).expect("player").cell = a_cell;
    game.players.get_mut(&PlayerId(0)).expect("player").position = point(a_cell);
    let mut commands = BTreeMap::new();
    interact(&mut commands, PlayerId(0));
    game.step_objectives(&HexInputFrame {
        commands: commands.clone(),
        ..HexInputFrame::default()
    });
    game.step_objectives(&HexInputFrame {
        commands: commands.clone(),
        ..HexInputFrame::default()
    });
    let team = observed_core::TeamId(0);
    assert_eq!(game.teams[&team].objectives.keystones, 2);

    let exit = game.facility.config.exit();
    for player in [PlayerId(0), PlayerId(1)] {
        game.players.get_mut(&player).expect("player").cell = exit;
    }
    game.resolve_escapes();
    assert!(
        !game.teams[&team].escaped,
        "unfinished station seals the exit"
    );

    game.players.get_mut(&PlayerId(0)).expect("player").cell = a_cell;
    game.players.get_mut(&PlayerId(0)).expect("player").position = point(a_cell);
    game.players.get_mut(&PlayerId(1)).expect("player").cell = b_cell;
    game.players.get_mut(&PlayerId(1)).expect("player").position = point(b_cell);
    interact(&mut commands, PlayerId(1));
    for _ in 0..120 {
        game.step_objectives(&HexInputFrame {
            commands: commands.clone(),
            ..HexInputFrame::default()
        });
    }
    assert!(game.teams[&team].objectives.dual_station_complete);

    for player in [PlayerId(0), PlayerId(1)] {
        game.players.get_mut(&player).expect("player").cell = exit;
    }
    game.resolve_escapes();
    assert!(game.teams[&team].escaped);
}

#[test]
fn a_team_finishes_only_after_both_members_escape() {
    let mut game = HexWfcMatch::new(
        44,
        HexMatchConfig {
            guardian: true,
            teams: 1,
            members_per_team: 2,
            wfc: showcase_config(4),
        },
        &tiles(),
    )
    .expect("two-member showcase");
    let exit = game.facility.config.exit();
    game.players.get_mut(&PlayerId(0)).expect("p1").cell = exit;
    game.resolve_escapes();
    assert!(
        !game.players[&PlayerId(0)].escaped,
        "the first teammate waits at the team exit"
    );
    assert!(!game.teams[&observed_core::TeamId(0)].escaped);
    assert!(game.escape_order.is_empty());

    game.players.get_mut(&PlayerId(1)).expect("p2").cell = exit;
    game.resolve_escapes();
    assert!(game.players[&PlayerId(0)].escaped);
    assert!(game.players[&PlayerId(1)].escaped);
    assert!(game.teams[&observed_core::TeamId(0)].escaped);
    assert_eq!(game.escape_order, vec![observed_core::TeamId(0)]);
    assert_eq!(game.status, HexMatchStatus::Finished);
}

#[test]
fn teammate_observations_share_one_survivor_map() {
    let mut game = HexWfcMatch::new(
        44,
        HexMatchConfig {
            guardian: true,
            teams: 1,
            members_per_team: 2,
            wfc: showcase_config(4),
        },
        &tiles(),
    )
    .expect("two-member showcase");
    let teammate_cell = game
        .facility
        .placements
        .keys()
        .copied()
        .find(|cell| *cell != game.facility.config.spawn())
        .expect("second cell");
    game.players.get_mut(&PlayerId(1)).expect("p2").cell = teammate_cell;
    game.update_map_knowledge();
    let team_map = game.team_map(observed_core::TeamId(0)).expect("team map");
    assert!(team_map.cells.contains_key(&game.facility.config.spawn()));
    assert!(team_map.cells.contains_key(&teammate_cell));
    assert_eq!(game.player_map(PlayerId(0)), game.player_map(PlayerId(1)));
}

/// Classify the vertical transitions on the solved spawn→exit route.
/// Returns `(ramp_transitions, stair_transitions)`.
fn route_vertical_profile(world: &HexWfcWorld) -> (u32, u32) {
    let Some(route) = world.route_between(world.config.spawn(), world.config.exit()) else {
        return (0, 0);
    };
    let mut ramps = 0;
    let mut stairs = 0;
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
            PortClass::ShaftOpen => stairs += 1,
            _ => {}
        }
    }
    (ramps, stairs)
}

fn run_bot_to_exit(game: &mut HexWfcMatch, max_ticks: u64) -> Option<u64> {
    let mut driver = HexBotDriver::new();
    for tick in 0..max_ticks {
        let commands = game
            .players
            .keys()
            .copied()
            .filter(|id| !game.players[id].escaped)
            .map(|id| (id, bot_player_command(&mut driver, game, id)))
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
                guardian: true,
                teams: 2,
                members_per_team: 2,
                wfc: showcase_config(4),
            },
            &tiles(),
        ) else {
            continue;
        };
        let mut warned = None;
        let mut committed = None;
        let mut cancelled = 0u32;
        let mut driver = HexBotDriver::new();
        for tick in 0..2_400u64 {
            let commands = game
                .players
                .keys()
                .copied()
                .filter(|id| !game.players[id].escaped)
                .map(|id| (id, bot_player_command(&mut driver, &game, id)))
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
            let (ramps, stairs) = route_vertical_profile(&world);
            if ramps >= 2 && stairs >= 1 {
                eprintln!(
                    "GATE_CANDIDATE levels={levels} seed={seed:#018x} ramps={ramps} stairs={stairs}"
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
    let max_ticks: u64 = std::env::var("HEX_DIAG_TICKS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8_000);
    let players: u8 = std::env::var("HEX_DIAG_PLAYERS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    let focus = PlayerId(
        std::env::var("HEX_DIAG_PLAYER")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
    );
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
    let mut game = showcase_match(seed, levels, players);
    if std::env::var("HEX_DIAG_PIN_GUARDIAN").is_ok() {
        pin_guardian_for_path_soak(&mut game);
    }
    let mut last_cell = game.players[&focus].cell;
    let mut driver = HexBotDriver::new();
    for tick in 0..max_ticks {
        let cmd = game.bot_command(focus);
        let commands = game
            .players
            .keys()
            .copied()
            .filter(|id| !game.players[id].escaped)
            .map(|id| (id, bot_player_command(&mut driver, &game, id)))
            .collect();
        game.step(&HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick,
            commands,
        });
        let p = &game.players[&focus];
        if p.cell != last_cell {
            eprintln!(
                "tick={tick} cell={:?} pos=({:.1},{:.1},{:.1}) cmd_jump={} cmd_int={}",
                p.cell,
                p.position.x,
                p.position.y,
                p.position.z,
                cmd.jump_pressed,
                cmd.interact_held
            );
            last_cell = p.cell;
        } else if tick % 1000 == 0 && tick > 0 {
            let b = game.bodies[&focus];
            eprintln!(
                "  ..stuck tick={tick} cell={:?} pos=({:.1},{:.1},{:.1}) mv=({:.2},{:.2}) look={:.2} jump={} int={} yaw={:.2} vel=({:.2},{:.2},{:.2}) grounded={}",
                p.cell,
                p.position.x,
                p.position.y,
                p.position.z,
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
    let p = &game.players[&focus];
    eprintln!(
        "final cell={:?} pos=({:.1},{:.1},{:.1})",
        p.cell, p.position.x, p.position.y, p.position.z
    );
    let half_height = game
        .content()
        .traversal_profile()
        .requirements()
        .capsule_half_height;
    let body_min_y = p.position.y - half_height;
    let body_max_y = p.position.y + half_height;
    for piece in &game.geometry.pieces {
        let observed_traversal::ColliderShape::ConvexHull { points } = &piece.shape else {
            continue;
        };
        let min = points
            .iter()
            .fold(Vec3::splat(f32::INFINITY), |min, point| {
                min.min(*point + piece.center)
            });
        let max = points
            .iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), |max, point| {
                max.max(*point + piece.center)
            });
        if p.position.x >= min.x - 1.0
            && p.position.x <= max.x + 1.0
            && p.position.z >= min.z - 1.0
            && p.position.z <= max.z + 1.0
            && body_max_y >= min.y
            && body_min_y <= max.y
        {
            eprintln!(
                "  nearby collider source={:?} tile={:?} bounds=({:.1},{:.1},{:.1})..({:.1},{:.1},{:.1})",
                piece.source_cell, piece.tile, min.x, min.y, min.z, max.x, max.y, max.z
            );
        }
    }
}

/// Pinned headless gate seed (found via `scan_gate_seeds`). Its solved 12×9×5
/// showcase route crosses two ramp levels and two physical stair towers.
///
/// Re-pinned after Arc P's room/open-volume topology changes: the previous seed's
/// route retained stairs but no longer crossed a ramp. That is expected — the gate
/// asserts the *bot* can walk a route with both vertical kinds on it, not that one
/// particular seed produces one.
/// Any arc that touches weighting should expect to re-run `scan_gate_seeds`.
const GATE_SEED: u64 = 0xd9c1_e6e5_fd29_f054;
const GATE_LEVELS: u8 = 5;

/// Phase 94 success criterion 1 — the headless gate. On a pinned seed whose
/// solved route crosses ≥2 ramp levels and ≥1 stair tower, an objective bot completes
/// spawn→exit, and it does so deterministically: two independent runs reach the
/// exit on the identical tick and end on the identical snapshot digest.
#[test]
fn headless_gate_bot_walks_ramps_and_stairs_deterministically() {
    let world = HexWfcWorld::generate(GATE_SEED, showcase_config(GATE_LEVELS)).expect("world");
    let (ramps, stairs) = route_vertical_profile(&world);
    assert!(
        ramps >= 2 && stairs >= 1,
        "gate route must cross >=2 ramp levels and >=1 stair tower, got ramps={ramps} stairs={stairs}"
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
    eprintln!(
        "headless gate completion_tick={a} digest={:#018x}",
        first.snapshot().digest
    );
    // Moved twice on purpose, both times by putting a shape on its own declared
    // route instead of on an inferred one.
    //
    // TR-11 (5,596 -> 5,511) put every *annotated* module on a graph leg.
    // TR-10's ramp spine (5,511 -> 6,589) annotated the last shape that had
    // none, and that direction is the expensive one: a ramp used to be walked
    // by `ramp_walk_dir` at full movement with sprint held, and a declared
    // climb is followed at the profile's climb tuning — 1.61 m/s against 7.0.
    // The 1,078 ticks are that 4.35x, and they buy consistency: the authored
    // perimeter ramps have always been followed at this speed, because they
    // have always shipped a spine. Whether a *ramp* should be tuned like a
    // staircase is a real question, and a separate one — `StairSpine` carries
    // no mode, so answering it is schema work.
    //
    // Determinism, which is what this gate actually guards, is asserted above
    // and is unaffected by either move.
    assert_eq!(a, 6_589, "TR-10 pins the declared-ramp completion tick");
    assert_eq!(
        first.snapshot().digest,
        0x02dd_ea8d_c8d2_ac4a,
        "TR-10 pins the declared-ramp final snapshot digest"
    );
}

fn mix_trace(digest: &mut u64, value: u64) {
    *digest ^= value;
    *digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
}

/// Pin the current shape-specific bot follower against a real projected
/// perimeter tower and the production Rapier controller. TR-1 can run the
/// extracted follower against this exact intent/body trace before deleting
/// the current `climb_command` implementation.
#[test]
fn perimeter_tower_local_intent_and_body_trace_is_pinned() {
    let mut game = showcase_match(GATE_SEED, GATE_LEVELS, 1);
    let tower_cells = game
        .geometry
        .pieces
        .iter()
        .filter_map(|piece| {
            piece
                .tile
                .as_ref()
                .filter(|tile| tile.archetype == "stair_tower")
                .map(|tile| (piece.source_cell, tile.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut trace = 0xcbf2_9ce4_8422_2325u64;
    let mut traced_tower = None;
    let mut traced_ticks = 0u64;
    let mut completion = None;
    let id = PlayerId(0);
    let mut driver = HexBotDriver::new();
    let half_height = game
        .content()
        .traversal_profile()
        .requirements()
        .capsule_half_height;
    for tick in 0..40_000u64 {
        let feet = game.bodies[&id].position - Vec3::Y * half_height;
        let touching = tower_cells.keys().copied().find(|cell| {
            game.geometry.climbs.get(cell).is_some_and(|spine| {
                !spine.is_empty()
                    && spine.distance(feet).is_some_and(|distance| distance <= 1.6)
                    && !spine.has_arrived(feet)
            })
        });
        let command = driver.command(&game, id);
        if let Some(cell) = touching {
            traced_tower.get_or_insert(cell);
            if traced_tower == Some(cell) {
                traced_ticks += 1;
                for bits in [
                    command.intent.movement.x.to_bits(),
                    command.intent.movement.y.to_bits(),
                    command.intent.look.x.to_bits(),
                    command.intent.look.y.to_bits(),
                ] {
                    mix_trace(&mut trace, u64::from(bits));
                }
                mix_trace(&mut trace, u64::from(command.intent.sprint_held));
            }
        }
        game.step(&HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick,
            commands: [(id, command)].into_iter().collect(),
        });
        if touching.is_some_and(|cell| traced_tower == Some(cell)) {
            let body = game.bodies[&id];
            for bits in [
                body.position.x.to_bits(),
                body.position.y.to_bits(),
                body.position.z.to_bits(),
                body.velocity.x.to_bits(),
                body.velocity.y.to_bits(),
                body.velocity.z.to_bits(),
                body.yaw.to_bits(),
            ] {
                mix_trace(&mut trace, u64::from(bits));
            }
            mix_trace(&mut trace, u64::from(body.grounded));
            let feet = body.position - Vec3::Y * half_height;
            let cell = traced_tower.expect("touching sets the tower");
            if game.geometry.climbs[&cell].has_arrived(feet) {
                completion = Some(tick + 1);
                break;
            }
        }
    }
    let cell = traced_tower.expect("gate bot reaches a perimeter tower");
    let tile = tower_cells[&cell].clone();
    let body = game.bodies[&id];
    eprintln!(
        "tower cell={cell:?} tile={tile:?} completion={completion:?} traced_ticks={traced_ticks} trace={trace:#018x} body={body:?}"
    );
    assert_eq!(
        cell,
        HexCoord {
            q: 1,
            r: 0,
            level: 0
        }
    );
    assert_eq!(tile.archetype, "stair_tower");
    assert_eq!(tile.register, "megastructure");
    assert_eq!(tile.variant, 360);
    // TR-11 moved this trace on purpose, and it is the only pin in that packet
    // permitted to move: the tower is now climbed by a graph leg instead of by
    // the compatibility follower beside it.
    //
    // The climb still *completes*, which is the property worth having. It
    // arrives on tick 1,075 against the 1,066 pinned before — nine ticks, 0.15
    // s — and the body ends 0.17 m from where it used to, at the same height,
    // on the same tread. The graph follower picks its next target slightly
    // differently along the same authored spine; nothing here is a regression.
    //
    // Selection is not steering: the catalog hash, the composition profile, the
    // simulation hash and both spectator selection digests are unmoved.
    assert_eq!(completion, Some(1_075));
    assert_eq!(traced_ticks, 973);
    assert_eq!(trace, 0x5adc_2eb9_81ea_1880);
    assert_eq!(
        [
            body.position.x.to_bits(),
            body.position.y.to_bits(),
            body.position.z.to_bits(),
            body.velocity.x.to_bits(),
            body.velocity.y.to_bits(),
            body.velocity.z.to_bits(),
            body.yaw.to_bits(),
        ],
        [
            1_096_826_245,
            1_091_997_307,
            1_079_991_628,
            3_173_502_237,
            0,
            3_217_949_542,
            1_086_865_250,
        ]
    );
}

/// Keep the Guardian out of geometry/pathfinding soaks. Its competitive
/// setbacks have their own tests and can otherwise turn a walkability failure
/// into repeated, valid returns to a recovery room.
fn pin_guardian_for_path_soak(game: &mut HexWfcMatch) {
    let blueprint = game
        .facility
        .blueprints
        .iter()
        .find(|blueprint| blueprint.cells.contains(&game.guardian.cell))
        .expect("Guardian belongs to a blueprint");
    let threshold = HexThresholdKey {
        room_generation_key: blueprint.generation_key(),
        port: "path-soak-pin",
    };
    game.lanterns
        .deploy(PlayerId(0), threshold, blueprint.anchor, Vec3::ZERO)
        .expect("path soak starts with one anchor lantern");
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
            pin_guardian_for_path_soak(&mut game);
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
                guardian: true,
                teams: 1,
                members_per_team: 1,
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
    let mut driver = HexBotDriver::new();
    for tick in 0..40_000u64 {
        let commands = game
            .players
            .keys()
            .copied()
            .filter(|id| !game.players[id].escaped)
            .map(|id| (id, bot_player_command(&mut driver, &game, id)))
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
const MUTATION_SEED: u64 = 0x7BBA_F82C_7DDF_743F;

#[test]
fn observed_relayout_commits_mid_match_deterministically() {
    fn run() -> (Vec<(u64, u32)>, Vec<u32>, u64) {
        let mut game = showcase_match(MUTATION_SEED, 4, 4);
        let first_commit = mutation::scheduled_mutation_tick(MUTATION_SEED, 0);
        let mut committed = Vec::new();
        let mut generations = Vec::new();
        let mut warned = false;
        let mut driver = HexBotDriver::new();
        for tick in 0..first_commit + 30 {
            let commands = game
                .players
                .keys()
                .copied()
                .filter(|id| !game.players[id].escaped)
                .map(|id| (id, bot_player_command(&mut driver, &game, id)))
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

/// A bot that cannot route to the exit used to return `PlayerIntent::default()`
/// and stand still forever. Nothing caught it: the stall soak skips any layout
/// whose spawn cannot reach the exit, so the branch never ran. It now falls back
/// to Explore and keeps moving, which is what lets a bot leave a pocket or a
/// cell orphaned by a relayout.
#[test]
fn a_bot_with_no_route_explores_instead_of_freezing() {
    use super::bot::BotBehaviour;

    let mut game = showcase_match(0xB07B_0000_0000_0001, 4, 1);
    let id = PlayerId(0);
    // Wall the objective off so no route to it can exist, while leaving the
    // rest of the facility connected — the bot must still have somewhere to go.
    let exit = game.facility.config.exit();
    let grid = game.facility.config.grid();
    for face in HexFace::LATERAL {
        if let Some(neighbour) = grid.neighbor(exit, face)
            && let Some(placement) = game.facility.placements.get_mut(&neighbour)
        {
            placement.doors &= !(1 << face.opposite().index());
        }
    }
    if let Some(placement) = game.facility.placements.get_mut(&exit) {
        placement.doors = 0;
        placement.up = PortClass::Sealed;
        placement.down = PortClass::Sealed;
    }
    // The bot is somewhere that cannot reach the exit ...
    assert_eq!(
        game.bot_behaviour(id),
        BotBehaviour::Explore,
        "a bot with no route should be exploring"
    );
    // ... and it must still be trying to move.
    let intent = game.bot_command(id);
    assert!(
        intent.movement.length_squared() > 0.0,
        "Explore must emit movement, not freeze the bot"
    );
}

/// `spawn_to_exit_cost` is a cache, so the only way it can be wrong is by going stale.
/// Drive a match until a relayout actually commits and assert it still equals a fresh
/// computation — before, during, and after the facility changing shape.
#[test]
fn cached_spawn_to_exit_cost_survives_a_committed_relayout() {
    let fresh = |game: &HexWfcMatch| {
        let config = game.facility.config;
        game.facility
            .route_between_cells(config.spawn(), config.exit())
            .map_or(1, |route| route.cost_millis.max(1))
    };

    let mut game = HexWfcMatch::new(
        0xA11C_9500_0000_0000,
        HexMatchConfig {
            guardian: true,
            teams: 2,
            members_per_team: 2,
            wfc: showcase_config(4),
        },
        &tiles(),
    )
    .expect("fixture seed builds");
    assert_eq!(
        game.spawn_to_exit_cost,
        fresh(&game),
        "cache must be primed at construction"
    );

    let mut generations = 0u32;
    let mut driver = HexBotDriver::new();
    for tick in 0..2_400u64 {
        let commands = game
            .players
            .keys()
            .copied()
            .filter(|id| !game.players[id].escaped)
            .map(|id| (id, bot_player_command(&mut driver, &game, id)))
            .collect();
        let committed = game
            .step(&HexInputFrame {
                version: HEX_INPUT_VERSION,
                tick,
                commands,
            })
            .iter()
            .any(|event| event.kind == HexMatchEventKind::MutationCommitted);
        if committed {
            generations += 1;
        }
        assert_eq!(
            game.spawn_to_exit_cost,
            fresh(&game),
            "cache went stale at tick {tick} (committed this tick: {committed})"
        );
        if game.status == HexMatchStatus::Finished {
            break;
        }
    }
    assert!(
        generations > 0,
        "fixture must actually commit a relayout, or this proves nothing"
    );
}

/// Sixteen seats actually run, which the simulation refused before Phase 112.
///
/// The roster guard was 8, below what the widened wire can carry, so a lobby
/// could fill to sixteen and the match would then fail to start. The soak proves
/// bodies at that scale still route; this proves they can exist at all, and that
/// the guard and the wire agree on where the ceiling is.
#[test]
fn a_sixteen_seat_match_runs_and_seventeen_is_refused() {
    let mut game = HexWfcMatch::new_with_rooms(
        0xA11C_E3D0_0000_0011,
        HexMatchConfig {
            guardian: true,
            teams: 4,
            members_per_team: 4,
            wfc: showcase_config(4),
        },
        &tiles(),
        rooms(),
    )
    .expect("sixteen seats is within the roster guard");
    assert_eq!(game.players.len(), 16);
    assert_eq!(usize::from(MAX_ROSTER), 16);

    let mut driver = HexBotDriver::new();
    for tick in 0..240 {
        let commands = game
            .players
            .keys()
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .map(|id| (id, bot_player_command(&mut driver, &game, id)))
            .collect();
        game.step(&HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick,
            commands,
        });
    }
    assert!(
        game.players
            .values()
            .all(|player| player.position.is_finite()),
        "sixteen bodies must all stay in the world"
    );

    assert!(
        HexWfcMatch::new_with_rooms(
            0xA11C_E3D0_0000_0011,
            HexMatchConfig {
                guardian: true,
                teams: 17,
                members_per_team: 1,
                wfc: showcase_config(4),
            },
            &tiles(),
            rooms(),
        )
        .is_err(),
        "seventeen seats exceeds what the wire can carry and must be refused"
    );
}

/// Co-op is one team, and the escape condition is the team's.
///
/// The simulation already had the semantics — team completion and shared map
/// knowledge are keyed by team — so this pins that a single team behaves rather
/// than adding machinery to make it.
#[test]
fn a_single_team_shares_its_map_and_finishes_together() {
    let game = HexWfcMatch::new_with_rooms(
        0xA11C_E3D0_0000_0012,
        HexMatchConfig {
            guardian: false,
            teams: 1,
            members_per_team: 4,
            wfc: showcase_config(4),
        },
        &tiles(),
        rooms(),
    )
    .expect("a single team is a valid roster");
    assert_eq!(game.players.len(), 4);
    let teams: std::collections::BTreeSet<_> =
        game.players.values().map(|player| player.team).collect();
    assert_eq!(teams.len(), 1, "co-op puts everyone on one team");

    // Everyone reads the same sketch, because knowledge is team-scoped.
    let first = game.players.keys().next().copied().expect("a player");
    for id in game.players.keys().copied() {
        assert_eq!(
            game.player_map(id).map(|map| map.cells.len()),
            game.player_map(first).map(|map| map.cells.len()),
            "teammates must share one map"
        );
    }
}

/// The Guardian toggle actually stops it hunting.
#[test]
fn a_match_without_a_guardian_leaves_it_where_it_started() {
    let build = |guardian: bool| {
        HexWfcMatch::new_with_rooms(
            0xA11C_E3D0_0000_0013,
            HexMatchConfig {
                guardian,
                teams: 2,
                members_per_team: 2,
                wfc: showcase_config(4),
            },
            &tiles(),
            rooms(),
        )
        .expect("match")
    };
    let mut off = build(false);
    let mut on = build(true);
    let mut off_driver = HexBotDriver::new();
    let mut on_driver = HexBotDriver::new();
    let start = off.guardian.cell;
    for tick in 0..600 {
        for (game, driver) in [(&mut off, &mut off_driver), (&mut on, &mut on_driver)] {
            let commands = game
                .players
                .keys()
                .copied()
                .collect::<Vec<_>>()
                .into_iter()
                .map(|id| (id, bot_player_command(driver, game, id)))
                .collect();
            game.step(&HexInputFrame {
                version: HEX_INPUT_VERSION,
                tick,
                commands,
            });
        }
    }
    assert_eq!(
        off.guardian.cell, start,
        "a disabled Guardian must not move"
    );
    assert_ne!(
        on.guardian.cell, start,
        "an enabled Guardian should have hunted, or this test proves nothing"
    );
}
