//! Two native clients enter through semantic Join/Ready actions and the real LAN worker.
//! A separate mode exercises the largest live lobby with transport-only role fixtures.
use super::{Shot, shot};
use crate::{GameState, lan::LanRuntime};
use bevy::{prelude::*, ui_widgets::Activate};
use observed_core::{TeamId, cosmetics::CosmeticLook, lan::LanRole};
use observed_net::lan::{LanClient, WireSeatOccupant};
use std::path::PathBuf;

#[derive(Resource)]
pub(crate) struct DeskCapture {
    name: &'static str,
    coord: PathBuf,
    peers: Vec<LanClient>,
    saw_loading: bool,
}
fn name() -> &'static str {
    match std::env::var("OBSERVED2_CAPTURE_DESK_PEER").as_deref() {
        Ok("architect") => "architect",
        Ok("roster") => "roster",
        Ok("observer") => "observer",
        _ => panic!("desk capture needs architect, observer or roster peer"),
    }
}
pub(super) fn sweep() -> Vec<Shot> {
    let large = name() == "roster";
    [
        ("00_browser", GameState::LanBrowser),
        ("01_full_lobby", GameState::Lobby),
        (
            if large {
                "02_roster_page_two"
            } else {
                "02_in_match"
            },
            if large {
                GameState::Lobby
            } else {
                GameState::HexWfc
            },
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(case, (label, state))| Shot {
        desks: Some(case),
        extra_settle: if case == 2 && !large { 3.0 } else { 0.0 },
        ..shot(label, state)
    })
    .collect()
}
pub(super) fn stage(world: &mut World) {
    let name = name();
    let coord =
        PathBuf::from(std::env::var("OBSERVED2_CAPTURE_DESK_COORD").expect("coord directory"));
    world
        .resource_mut::<crate::settings::Settings>()
        .complete_onboarding();
    let mut setup = crate::play_setup::PlaySetupDraft::default();
    setup.select_preset(crate::play_setup::PlayPreset::CoOp);
    world.insert_resource(setup);
    let hash = crate::hex_wfc::sim::simulation_content_hash();
    let mut peers = Vec::new();
    let address = if name != "observer" {
        let large = name == "roster";
        let args = vec![
            "observed-evidence".to_string(),
            "--bind".into(),
            "127.0.0.1:0".into(),
            "--ascent".into(),
            "--teams".into(),
            if large { "16" } else { "1" }.into(),
            "--team-size".into(),
            if large { "1" } else { "3" }.into(),
            "--min-humans".into(),
            if large { "32" } else { "4" }.into(),
            "--no-discovery".into(),
            "--no-guardian".into(),
            "--seed".into(),
            "0xF011FAC11177".into(),
        ];
        let handle = observed_server::ServerHandle::spawn(
            observed_server::ServerConfig::from_args(args).unwrap(),
        )
        .unwrap();
        let address = handle.address;
        std::fs::write(coord.join("address"), address.to_string()).unwrap();
        world.resource_mut::<LanRuntime>().listen_server = Some(handle);
        if large {
            for team in 0..16 {
                peers.push(
                    LanClient::connect_with_role(
                        address,
                        1000 + u16::from(team),
                        Some(TeamId(team)),
                        None,
                        hash,
                        CosmeticLook::default(),
                        LanRole::Architect,
                    )
                    .unwrap(),
                );
                if team > 0 {
                    peers.push(
                        LanClient::connect(
                            address,
                            1100 + u16::from(team),
                            Some(TeamId(team)),
                            None,
                            hash,
                        )
                        .unwrap(),
                    );
                }
            }
        } else {
            for account in 932..934 {
                peers.push(
                    LanClient::connect(address, account, Some(TeamId(0)), None, hash).unwrap(),
                );
            }
        }
        address
    } else {
        std::fs::read_to_string(coord.join("address"))
            .unwrap()
            .parse()
            .unwrap()
    };
    let mut lan = world.resource_mut::<LanRuntime>();
    lan.direct_address = address.to_string();
    lan.requested_team = Some(TeamId(0));
    world.insert_resource(DeskCapture {
        name,
        coord,
        peers,
        saw_loading: false,
    });
}
fn activate_browser(world: &mut World, action: crate::screens::lan::LanBrowserAction) {
    let entity = world
        .query::<(Entity, &crate::screens::lan::LanBrowserAction)>()
        .iter(world)
        .find_map(|(entity, candidate)| (*candidate == action).then_some(entity))
        .expect("real browser action");
    world.trigger(Activate { entity });
}
fn activate_lobby(world: &mut World, action: crate::screens::lobby::LobbyAction) {
    let entity = world
        .query::<(Entity, &crate::screens::lobby::LobbyAction)>()
        .iter(world)
        .find_map(|(entity, candidate)| (*candidate == action).then_some(entity))
        .expect("real lobby action");
    world.trigger(Activate { entity });
}
pub(super) fn pose(world: &mut World, case: usize) {
    let name = world.resource::<DeskCapture>().name;
    if case == 0 && name == "architect" {
        activate_browser(world, crate::screens::lan::LanBrowserAction::ToggleRole);
    }
    if case == 2 && name == "roster" {
        activate_lobby(world, crate::screens::lobby::LobbyAction::NextRoster);
    }
}
pub(crate) fn poll(mut capture: Option<ResMut<DeskCapture>>, state: Res<State<GameState>>) {
    let Some(capture) = capture.as_deref_mut() else {
        return;
    };
    capture.saw_loading |= *state.get() == GameState::Loading;
    if capture.name == "roster" {
        for peer in &mut capture.peers {
            peer.poll();
        }
        return;
    }
    for peer in &mut capture.peers {
        peer.poll();
        if peer.token.is_some() {
            peer.set_ready(true).unwrap();
        }
        if let Some(launch) = peer.launch {
            // These two peers are transport fixtures. The graphical peers run real preparation.
            peer.mark_launch_ready(launch.match_number).unwrap();
            peer.take_ready_frames(observed_net::lan::FRAME_WINDOW);
        }
    }
}
pub(super) fn ready(
    case: usize,
    lan: &LanRuntime,
    runtime: Option<&crate::hex_wfc::sim::HexWfcRuntime>,
) -> bool {
    if case == 0 {
        return true;
    }
    let Some(client) = lan.client.as_ref() else {
        return false;
    };
    let Some(lobby) = client.lobby.as_ref() else {
        return false;
    };
    let humans = lobby
        .seats
        .iter()
        .filter(|s| s.occupant == WireSeatOccupant::Human)
        .count()
        + lobby
            .architect_seats
            .iter()
            .filter(|s| s.occupant == WireSeatOccupant::Human)
            .count();
    if name() == "roster" {
        return humans == 32;
    }
    if case == 1 {
        return humans == 4;
    }
    runtime.is_some_and(|runtime| {
        runtime.networked && runtime.match_state.tick >= 30 && runtime.resync_attempts == 0
    })
}
pub(super) fn evidence(world: &mut World, case: usize) {
    let capture = world.resource::<DeskCapture>();
    let lan = world.resource::<LanRuntime>();
    let client = lan.client.as_ref();
    let runtime = world.get_resource::<crate::hex_wfc::sim::HexWfcRuntime>();
    let tape = world.get_resource::<crate::sim::replay::ReplayTape>();
    let metadata = serde_json::json!({ "peer": capture.name, "case": case,
        "assignment": client.and_then(|c| c.assignment).map(|s| format!("{s:?}")),
        "embodied_player": client.and_then(|c| c.player).map(|p| p.0),
        "observer_slots": client.and_then(|c| c.lobby.as_ref()).map(|l| l.seats.len()),
        "architect_slots": client.and_then(|c| c.lobby.as_ref()).map(|l| l.architect_seats.len()),
        "saw_real_loading": capture.saw_loading,
        "simulation_tick": runtime.map(|r| r.match_state.tick),
        "simulation_digest": runtime.map(|r| format!("{:016x}", r.match_state.snapshot().digest)),
        "resync_attempts": runtime.map(|r| r.resync_attempts),
        "rendered_body_count": runtime.map(|r| r.match_state.players.len()),
        "replay_has_owned_body": tape.map(|t| t.actors.iter().any(|a| a.id == crate::sim::replay::ReplayActorId::LocalPlayer)),
        "fixture": "real graphical Join/Ready and Loading; two neutral-input transport peers; Guardian off; returning-player help state" });
    std::fs::write(
        capture.coord.join(format!("{}-{case}.json", capture.name)),
        serde_json::to_vec_pretty(&metadata).unwrap(),
    )
    .unwrap();
}
pub(super) fn after_shot(world: &mut World, case: usize) {
    let capture = world.resource::<DeskCapture>();
    if case == 0 {
        activate_browser(world, crate::screens::lan::LanBrowserAction::JoinDirect);
    } else if case == 1 && capture.name != "roster" {
        activate_lobby(world, crate::screens::lobby::LobbyAction::ToggleReady);
    } else if case == 2 {
        std::fs::write(capture.coord.join(format!("{}-done", capture.name)), "done").unwrap();
    }
}
pub(super) fn can_exit() -> bool {
    let coord = PathBuf::from(std::env::var("OBSERVED2_CAPTURE_DESK_COORD").unwrap());
    if name() == "roster" {
        return coord.join("roster-done").exists();
    }
    coord.join("architect-done").exists() && coord.join("observer-done").exists()
}
