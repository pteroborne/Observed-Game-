//! Tests for the hex simulation runtime and its input tick (see `sim.rs`).

use super::*;

fn showcase_match(seed: u64) -> HexWfcMatch {
    let prototypes = load_prototypes();
    HexWfcMatch::new(seed, HexMatchConfig::default(), &prototypes).expect("showcase solves")
}

#[test]
fn map_browsing_visits_only_discovered_levels() {
    let discovered = BTreeSet::from([
        HexCoord {
            q: 1,
            r: 1,
            level: 0,
        },
        HexCoord {
            q: 1,
            r: 1,
            level: 3,
        },
        HexCoord {
            q: 1,
            r: 1,
            level: 7,
        },
    ]);
    assert_eq!(browsed_level(&discovered, 0, 1), 3);
    assert_eq!(browsed_level(&discovered, 3, 1), 7);
    assert_eq!(browsed_level(&discovered, 7, 1), 7);
    assert_eq!(browsed_level(&discovered, 7, -1), 3);
    assert_eq!(browsed_level(&discovered, 0, -1), 0);
}

#[test]
fn presentation_cursor_selects_only_cells_with_new_revisions() {
    let a = HexCoord {
        q: 1,
        r: 1,
        level: 0,
    };
    let b = HexCoord {
        q: 2,
        r: 1,
        level: 0,
    };
    let c = HexCoord {
        q: 3,
        r: 1,
        level: 0,
    };
    let live = BTreeMap::from([(a, 0), (b, 2), (c, 1)]);
    let presented = BTreeMap::from([(a, 0), (b, 1), (c, 1)]);
    assert_eq!(changed_revisions(&live, &presented), vec![(b, 2)]);
}

#[test]
fn offline_pause_stops_the_authoritative_system_tick() {
    let game = showcase_match(44);
    let local_player = LOCAL_PLAYER;
    let map_level = game.players[&local_player].cell.level;
    let presented_revisions = game.facility.cell_revisions.clone();
    let tick = game.tick;
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<crate::GameState>()
        .insert_resource(HexWfcRuntime {
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
        })
        .insert_resource(HexWfcIntent {
            intent: PlayerIntent {
                movement: Vec2::Y,
                look: Vec2::X,
                jump_pressed: true,
                sprint_held: true,
                ..Default::default()
            },
            actions: HexActionButtons {
                interact: true,
                deploy_lantern: true,
                recover_lantern: true,
            },
            browse_map_level: 1,
        })
        .insert_resource(super::super::overlay::MatchOverlayState::Pause(
            super::super::overlay::PausePage::Root,
        ))
        .insert_resource(super::super::HexOnboardingGate::default())
        .insert_resource(crate::lan::LanRuntime::new())
        .add_systems(Update, step_runtime);

    app.update();

    let runtime = app.world().resource::<HexWfcRuntime>();
    assert_eq!(runtime.match_state.tick, tick);
    let intent = app.world().resource::<HexWfcIntent>();
    assert!(intent.intent.is_neutral());
    assert_eq!(intent.actions, HexActionButtons::default());
}

#[test]
fn continuing_neutrally_clears_held_input_between_fixed_ticks() {
    let mut intent = HexWfcIntent {
        intent: PlayerIntent {
            movement: Vec2::ONE,
            sprint_held: true,
            interact_held: true,
            ..Default::default()
        },
        actions: HexActionButtons {
            interact: true,
            ..Default::default()
        },
        browse_map_level: 0,
    };
    finish_input_tick(&mut intent, SimulationPolicy::ContinueNeutral);
    assert!(intent.intent.is_neutral());
    assert_eq!(intent.actions, HexActionButtons::default());
}

#[test]
fn all_non_local_actors_cross_the_same_command_boundary() {
    let game = showcase_match(44);
    assert!(game.players.len() >= 2);
    assert!(
        game.players
            .keys()
            .filter(|&&id| id != LOCAL_PLAYER)
            .all(|&id| {
                let _intent = game.bot_command(id);
                true
            })
    );
}

#[test]
fn hex_replay_records_the_versioned_simulation() {
    let mut game = showcase_match(44);
    let mut driver = HexBotDriver::new();
    game.bind_simulation_content_hash([0x5A; 32]);
    let local = PlayerId(2);
    let mut replay = crate::sim::replay::ReplayTape::new_hex_wfc_for_player(&game, local);
    let players = game.players.keys().copied().collect::<Vec<_>>();
    let commands = players
        .into_iter()
        .map(|id| (id, driver.command(&game, id)))
        .collect();
    game.step(&HexInputFrame {
        tick: 1,
        commands,
        ..Default::default()
    });
    replay.record_hex_wfc(&game);
    assert_eq!(
        replay.input_version,
        observed_match::hex_wfc::HEX_INPUT_VERSION
    );
    assert_eq!(replay.map_name, "hex_wfc_v3");
    assert_eq!(replay.simulation_content_hash, [0x5A; 32]);
    assert_eq!(replay.actors.len(), game.players.len());
    assert_eq!(replay.samples[0].actors.len(), game.players.len());
    assert_eq!(
        replay.samples[0].actors[local.index()].actor,
        crate::sim::replay::ReplayActorId::LocalPlayer
    );
}
