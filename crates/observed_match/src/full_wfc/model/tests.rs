use super::*;

fn game(seed: u64) -> FullWfcMatch {
    FullWfcMatch::new(seed, FullWfcMatchConfig::default()).expect("match")
}

fn place_player(game: &mut FullWfcMatch, id: PlayerId, cell: CellCoord, position: Vec3) {
    let yaw = game.players[&id].yaw;
    let player = game.players.get_mut(&id).expect("player");
    player.cell = cell;
    player.position = position;
    game.bodies.insert(id, FpsBody::spawned(position, yaw));
}

#[test]
fn default_match_is_four_teams_of_two_with_eight_physical_keys() {
    let game = game(7);
    assert_eq!(game.teams.len(), 4);
    assert_eq!(game.players.len(), 8);
    assert_eq!(game.available_keystones.len(), 8);
    assert!(game.teams.values().all(|team| team.members.len() == 2));
}

#[test]
fn identical_frames_produce_identical_snapshots() {
    let mut a = game(13);
    let mut b = game(13);
    for tick in 0..400 {
        let mut frame = InputFrame {
            tick,
            ..Default::default()
        };
        frame.commands.insert(
            PlayerId(0),
            PlayerCommand {
                intent: PlayerIntent {
                    movement: Vec2::Y,
                    sprint_held: tick % 2 == 0,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        a.step(&frame);
        b.step(&frame);
        assert_eq!(a.snapshot(), b.snapshot());
    }
}

#[test]
fn incompatible_input_frame_version_is_rejected_without_advancing() {
    let mut game = game(17);
    let before = game.snapshot();
    game.step(&InputFrame {
        version: FULL_WFC_INPUT_VERSION + 1,
        ..Default::default()
    });
    assert_eq!(game.snapshot(), before);
}

#[test]
fn guardian_destination_is_farthest_by_live_weighted_route() {
    let game = game(19);
    let room = game
        .facility
        .farthest_room_from_exit(&BTreeSet::new())
        .expect("destination");
    let cost = game
        .facility
        .route(game.facility.rooms[&room].coord)
        .unwrap()
        .cost_millis;
    assert!(game.facility.rooms.values().all(|candidate| {
        candidate.role == RoomRole::Exit
            || game
                .facility
                .route(candidate.coord)
                .is_none_or(|route| route.cost_millis <= cost)
    }));
}

#[test]
fn keystone_is_a_single_physical_pickup() {
    let mut game = game(29);
    let room = *game.available_keystones.first().expect("key room");
    let cell = game.facility.rooms[&room].coord;
    let pickup = cell_origin(cell) + Vec3::Y * 1.15;
    for id in [PlayerId(0), PlayerId(2)] {
        place_player(&mut game, id, cell, pickup);
    }
    let commands = BTreeMap::from([(
        PlayerId(0),
        PlayerCommand {
            actions: ActionButtons {
                interact: true,
                ..Default::default()
            },
            ..Default::default()
        },
    )]);
    game.step(&InputFrame {
        tick: 1,
        commands,
        ..Default::default()
    });
    assert_eq!(game.teams[&TeamId(0)].keystones, 1);
    assert!(!game.available_keystones.contains(&room));
    let commands = BTreeMap::from([(
        PlayerId(2),
        PlayerCommand {
            actions: ActionButtons {
                interact: true,
                ..Default::default()
            },
            ..Default::default()
        },
    )]);
    game.step(&InputFrame {
        tick: 2,
        commands,
        ..Default::default()
    });
    assert_eq!(game.teams[&TeamId(1)].keystones, 0);
}

#[test]
fn dual_station_requires_both_team_commands_for_two_seconds() {
    let mut game = game(37);
    let station = game
        .facility
        .rooms
        .values()
        .find(|room| room.role == RoomRole::DualStation)
        .expect("station")
        .coord;
    let position = cell_origin(station) + Vec3::Y * 0.9;
    for id in [PlayerId(0), PlayerId(1)] {
        place_player(&mut game, id, station, position);
    }
    let held = PlayerCommand {
        actions: ActionButtons {
            interact: true,
            ..Default::default()
        },
        ..Default::default()
    };
    for tick in 1..120 {
        game.step(&InputFrame {
            tick,
            commands: BTreeMap::from([(PlayerId(0), held)]),
            ..Default::default()
        });
    }
    assert!(!game.teams[&TeamId(0)].dual_station_complete);
    for tick in 120..240 {
        game.step(&InputFrame {
            tick,
            commands: BTreeMap::from([(PlayerId(0), held), (PlayerId(1), held)]),
            ..Default::default()
        });
    }
    assert!(
        game.teams[&TeamId(0)].dual_station_complete,
        "ticks={} cells={:?}/{:?} positions={:?}/{:?}",
        game.teams[&TeamId(0)].dual_station_ticks,
        game.players[&PlayerId(0)].cell,
        game.players[&PlayerId(1)].cell,
        game.players[&PlayerId(0)].position,
        game.players[&PlayerId(1)].position,
    );
}

#[test]
fn unseen_guardian_catches_to_weighted_farthest_room() {
    let mut game = game(41);
    for (&id, other) in &mut game.players {
        other.escaped = id != PlayerId(0);
    }
    let player = game.players[&PlayerId(0)].clone();
    let forward = Vec3::new(player.yaw.sin(), 0.0, -player.yaw.cos());
    game.guardian.cell = player.cell;
    game.guardian.room = game.facility.room_at(player.cell).expect("spawn room");
    game.guardian.position = player.position - forward * 0.4;
    game.guardian.target_team = Some(player.team);
    game.step(&InputFrame::default());
    assert_ne!(game.players[&PlayerId(0)].cell, player.cell);
    assert!(
        game.recent_events
            .iter()
            .any(|event| event.kind == GameplayEventKind::GuardianCatch)
    );
}

#[test]
#[ignore = "long-running deterministic local-match soak"]
fn objective_bots_can_complete_the_local_match_through_input_frames() {
    let mut game = game(23);
    for tick in 0..50_000 {
        let commands = game
            .players
            .keys()
            .copied()
            .map(|id| (id, game.bot_command(id)))
            .collect();
        game.step(&InputFrame {
            tick,
            commands,
            ..Default::default()
        });
        if game.status == MatchStatus::Finished {
            break;
        }
    }
    assert!(
        !game.escape_order.is_empty(),
        "at least one team must solve keys, the dual station, and the exit"
    );
}
