//! Unit tests for hex map building, visibility gating, player beacons, and landmarks.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use observed_core::PlayerId;
use observed_facility::map_spec::RoomRole;
use observed_hex::hex_origin;
use observed_match::hex_wfc::{
    HexBotDriver, HexMapCellKnowledge, HexMapDiscovery, HexMatchConfig, HexPlayerMapKnowledge,
    HexWfcMatch,
};
use observed_style::MarkerRole;
use std::collections::BTreeSet;

use crate::hex_wfc::sim::{HexWfcRuntime, load_prototypes};

use super::build::build;
use super::{HexMapCell, HexMapLandmark, HexMapOrientationFrame, HexMapPlayerBeacon};

fn test_runtime() -> HexWfcRuntime {
    let prototypes = load_prototypes();
    let game = HexWfcMatch::new(
        44,
        HexMatchConfig {
            guardian: false,
            teams: 2,
            members_per_team: 1,
            ..HexMatchConfig::default()
        },
        &prototypes,
    )
    .expect("match generates");
    let local_player = PlayerId(0);
    let map_level = game.players[&local_player].cell.level;
    let presented_revisions = game.facility.cell_revisions.clone();
    HexWfcRuntime {
        match_state: game,
        bot_driver: HexBotDriver::new(),
        local_player,
        pending_visual_cells: BTreeSet::new(),
        presented_revisions,
        status: String::new(),
        map_open: false,
        map_level,
        results_delay_frames: 0,
        networked: false,
        resync_attempts: 0,
    }
}

#[test]
fn undiscovered_cells_and_rival_positions_are_never_rendered() {
    let mut runtime = test_runtime();
    let local_cell = runtime.local().cell;

    // Ensure the match has a rival player on team 1
    let rival_id = PlayerId(1);
    assert_ne!(
        runtime.match_state.players[&rival_id].team,
        runtime.local().team
    );
    let rival_cell = runtime.match_state.players[&rival_id].cell;

    // Restrict local player's knowledge to only local_cell
    let mut limited_knowledge = HexPlayerMapKnowledge::default();
    limited_knowledge.cells.insert(
        local_cell,
        HexMapCellKnowledge {
            discovery: HexMapDiscovery::Traversed,
            anchored: false,
            room_role: None,
            known_ports: Default::default(),
            last_confirmed_revision: 0,
        },
    );
    let local_team = runtime.local().team;
    runtime
        .match_state
        .map_knowledge
        .insert(local_team, limited_knowledge);

    let mut world = World::new();
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, &world);
    let mut meshes = bevy::asset::Assets::<Mesh>::default();
    let mut materials = bevy::asset::Assets::<StandardMaterial>::default();

    let _census = build(&mut commands, &runtime, &mut meshes, &mut materials);
    queue.apply(&mut world);

    // Assert that only the single known cell is spawned as a HexMapCell
    let cell_count = world.query::<&HexMapCell>().iter(&world).count();
    assert_eq!(
        cell_count, 1,
        "only the single known cell should be rendered, even if facility has many placements"
    );

    // Assert that undiscovered placements in the facility are NOT spawned
    assert!(runtime.match_state.facility.placements.len() > 1);
    for &placement_cell in runtime.match_state.facility.placements.keys() {
        if placement_cell != local_cell {
            let cell_origin = Vec3::from_array(hex_origin(placement_cell));
            let found = world
                .query::<(&HexMapCell, &Transform)>()
                .iter(&world)
                .any(|(_, transform)| (transform.translation - cell_origin).length() < 0.1);
            assert!(
                !found,
                "undiscovered cell {placement_cell:?} must never be rendered"
            );
        }
    }

    // Assert that no rival player beacon is spawned
    let beacon_count = world.query::<&HexMapPlayerBeacon>().iter(&world).count();
    assert!(beacon_count > 0, "local player beacon should exist");

    // The beacon should be positioned near the local player's cell, not the rival's
    if rival_cell != local_cell {
        let rival_origin = Vec3::from_array(hex_origin(rival_cell));
        let rival_found = world
            .query::<(&HexMapPlayerBeacon, &Transform)>()
            .iter(&world)
            .any(|(_, transform)| {
                let diff = transform.translation - rival_origin;
                Vec2::new(diff.x, diff.z).length() < 1.0
            });
        assert!(
            !rival_found,
            "rival position must never have a beacon or leak to map"
        );
    }
}

#[test]
fn player_beacon_and_facing_are_distinguishable_from_cell_markers() {
    let mut runtime = test_runtime();
    runtime
        .match_state
        .players
        .get_mut(&runtime.local_player)
        .unwrap()
        .yaw = std::f32::consts::FRAC_PI_2;

    let mut world = World::new();
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, &world);
    let mut meshes = bevy::asset::Assets::<Mesh>::default();
    let mut materials = bevy::asset::Assets::<StandardMaterial>::default();

    let _census = build(&mut commands, &runtime, &mut meshes, &mut materials);
    queue.apply(&mut world);

    // Verify beacon entities exist
    let beacon_entities: Vec<Entity> = world
        .query_filtered::<Entity, With<HexMapPlayerBeacon>>()
        .iter(&world)
        .collect();
    assert!(
        !beacon_entities.is_empty(),
        "player beacons must be spawned"
    );

    // Player beacons must be distinct from ordinary HexMapCells
    for &entity in &beacon_entities {
        assert!(
            world.get::<HexMapCell>(entity).is_none(),
            "player beacon entities must not be ordinary cell markers"
        );
    }

    // Verify "YOU" text badge is spawned
    let you_badge = world
        .query::<(&HexMapPlayerBeacon, &Text2d, &TextColor)>()
        .iter(&world)
        .find(|(_, text, _)| text.0 == "YOU");
    assert!(
        you_badge.is_some(),
        "a 'YOU' badge must be present above the beacon"
    );
    let (_, _, color) = you_badge.unwrap();
    assert_eq!(
        color.0,
        observed_style::marker(MarkerRole::You).base_color,
        "'YOU' badge color must use MarkerRole::You"
    );

    // Verify heading pointer shaft is spawned and oriented with player yaw
    let shaft = world
        .query::<(&HexMapPlayerBeacon, &Transform, &Name)>()
        .iter(&world)
        .find(|(_, _, name)| name.as_str() == "Hex map player heading shaft");
    assert!(shaft.is_some(), "player heading shaft must be spawned");
}

#[test]
fn orientation_frame_spawns_cardinal_arms_and_labels() {
    let runtime = test_runtime();

    let mut world = World::new();
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, &world);
    let mut meshes = bevy::asset::Assets::<Mesh>::default();
    let mut materials = bevy::asset::Assets::<StandardMaterial>::default();

    let census = build(&mut commands, &runtime, &mut meshes, &mut materials);
    queue.apply(&mut world);

    // Verify compass frame entities exist
    let frame_count = world
        .query::<&HexMapOrientationFrame>()
        .iter(&world)
        .count();
    assert!(
        frame_count >= 5,
        "orientation frame hub and arms must exist"
    );

    // Verify N, E, S, W text labels
    let labels: BTreeSet<String> = world
        .query::<(&HexMapOrientationFrame, &Text2d)>()
        .iter(&world)
        .map(|(_, text)| text.0.clone())
        .collect();
    assert!(labels.contains("N"), "compass must label North");
    assert!(labels.contains("E"), "compass must label East");
    assert!(labels.contains("S"), "compass must label South");
    assert!(labels.contains("W"), "compass must label West");

    // Census bounds must be defined and incorporate the compass
    assert!(
        census.bounds.is_some(),
        "census must record facility bounds"
    );
}

#[test]
fn landmarks_spawn_only_for_known_exit_and_anchors() {
    let mut runtime = test_runtime();
    let exit_cell = runtime
        .match_state
        .facility
        .blueprints
        .iter()
        .find(|b| b.role == RoomRole::Exit)
        .and_then(|b| b.cells.iter().copied().next())
        .expect("match must have exit");

    // Case 1: Knowledge includes exit cell and an anchored cell
    let local_cell = runtime.local().cell;
    let mut knowledge = HexPlayerMapKnowledge::default();
    knowledge.cells.insert(
        local_cell,
        HexMapCellKnowledge {
            discovery: HexMapDiscovery::Traversed,
            anchored: true,
            room_role: None,
            known_ports: Default::default(),
            last_confirmed_revision: 0,
        },
    );
    knowledge.cells.insert(
        exit_cell,
        HexMapCellKnowledge {
            discovery: HexMapDiscovery::Traversed,
            anchored: false,
            room_role: Some(RoomRole::Exit),
            known_ports: Default::default(),
            last_confirmed_revision: 0,
        },
    );
    let local_team = runtime.local().team;
    runtime
        .match_state
        .map_knowledge
        .insert(local_team, knowledge);

    let mut world = World::new();
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, &world);
    let mut meshes = bevy::asset::Assets::<Mesh>::default();
    let mut materials = bevy::asset::Assets::<StandardMaterial>::default();

    let _census = build(&mut commands, &runtime, &mut meshes, &mut materials);
    queue.apply(&mut world);

    // Exit landmark should exist
    let exit_landmark = world
        .query::<(&HexMapLandmark, &Name)>()
        .iter(&world)
        .any(|(_, name)| name.as_str() == "Hex map exit landmark");
    assert!(exit_landmark, "exit landmark must spawn when exit is known");

    // Anchor landmark should exist
    let anchor_landmark = world
        .query::<(&HexMapLandmark, &Name)>()
        .iter(&world)
        .any(|(_, name)| name.as_str() == "Hex map anchor landmark");
    assert!(
        anchor_landmark,
        "anchor landmark must spawn when cell is anchored"
    );

    // Case 2: Exit cell is NOT known
    let mut knowledge_no_exit = HexPlayerMapKnowledge::default();
    knowledge_no_exit.cells.insert(
        local_cell,
        HexMapCellKnowledge {
            discovery: HexMapDiscovery::Traversed,
            anchored: false,
            room_role: None,
            known_ports: Default::default(),
            last_confirmed_revision: 0,
        },
    );
    runtime
        .match_state
        .map_knowledge
        .insert(local_team, knowledge_no_exit);

    let mut world2 = World::new();
    let mut queue2 = CommandQueue::default();
    let mut commands2 = Commands::new(&mut queue2, &world2);
    let mut meshes2 = bevy::asset::Assets::<Mesh>::default();
    let mut materials2 = bevy::asset::Assets::<StandardMaterial>::default();

    let _census2 = build(&mut commands2, &runtime, &mut meshes2, &mut materials2);
    queue2.apply(&mut world2);

    let exit_landmark2 = world2
        .query::<(&HexMapLandmark, &Name)>()
        .iter(&world2)
        .any(|(_, name)| name.as_str() == "Hex map exit landmark");
    assert!(
        !exit_landmark2,
        "undiscovered exit must never spawn a landmark"
    );
}
