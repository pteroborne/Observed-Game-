//! Synthetic completion facts over a real prepared hex room trace. These pictures
//! verify presentation; they do not establish human match completion or a LAN handshake.
use super::{Shot, shot};
use crate::play_setup::{ActivePlaySession, PlayPreset, PlayRules, PlaySeat, PlaySetupDraft};
use crate::sim::replay::ReplayTape;
use crate::{
    GameState,
    flow::{AscentResult, AscentResultRole, Career},
};
use bevy::prelude::*;
use observed_core::{PlayerId, TeamId};
use observed_match::ascent::sim::MatchOutcome;

pub(super) fn sweep() -> Vec<Shot> {
    [
        ("00_observer_summit", GameState::Results),
        ("01_observer_other_team", GameState::Results),
        ("02_architect_summit", GameState::Results),
        ("03_architect_rogue_loss", GameState::Results),
        ("04_rogue_victory", GameState::Results),
        ("05_rogue_summit_loss", GameState::Results),
        ("06_spectator_summit", GameState::Results),
        ("07_spectator_rogue", GameState::Results),
        ("08_rogue_darkness", GameState::Results),
        ("09_replay_absent", GameState::Results),
        ("10_replay_empty", GameState::Results),
        ("11_replay_observer", GameState::Replay),
        ("12_replay_architect", GameState::Replay),
        ("13_replay_rogue", GameState::Replay),
        ("14_replay_spectator", GameState::Replay),
        ("15_lan_results", GameState::Results),
        ("16_lan_replay", GameState::Replay),
        ("17_race_results", GameState::Results),
    ]
    .into_iter()
    .enumerate()
    .map(|(case, (label, state))| Shot {
        completion: Some(case),
        ..shot(label, state)
    })
    .collect()
}

pub(super) fn stage(world: &mut World, case: usize) {
    if case == 17 {
        let (_, result, _, tape) = super::super::scenarios::staged_results_case(0);
        let mut career = world.resource_mut::<Career>();
        *career = Career::default();
        career.record(result);
        world.insert_resource(tape);
        let setup = PlaySetupDraft {
            preset: PlayPreset::Custom,
            teams: 4,
            members_per_team: 1,
            fill_empty_seats: true,
            guardian: true,
            rules: PlayRules::Race,
            seat: PlaySeat::Observer,
        };
        world.insert_resource(crate::play_setup::LaunchedPlaySetup(setup.clone()));
        world.insert_resource(setup);
        world.insert_resource(ActivePlaySession::from_launch(
            observed_match::hex_wfc::HexMatchConfig {
                teams: 4,
                members_per_team: 1,
                ..default()
            },
            false,
            false,
        ));
        world.resource_mut::<crate::lan::LanRuntime>().leave();
        return;
    }
    use AscentResultRole::*;
    use MatchOutcome::*;
    let (role, outcome, winner) = match case {
        1 => (Observer, LoyalVictory, Some(TeamId(1))),
        2 | 12 => (Architect, LoyalVictory, Some(TeamId(0))),
        3 => (Architect, RogueVictory, None),
        4 | 8 | 13 => (Rogue, RogueVictory, None),
        5 => (Rogue, LoyalVictory, Some(TeamId(1))),
        6 | 14 => (Spectator, LoyalVictory, Some(TeamId(0))),
        7 => (Spectator, RogueVictory, None),
        _ => (Observer, LoyalVictory, Some(TeamId(0))),
    };
    let setup = PlaySetupDraft {
        rules: PlayRules::Ascent,
        seat: if role == Architect {
            PlaySeat::Architect
        } else {
            PlaySeat::Observer
        },
        guardian: false,
        ..PlaySetupDraft::for_preset(if role == Spectator {
            PlayPreset::Spectate
        } else {
            PlayPreset::TeamRace
        })
    };
    let config = crate::hex_wfc::sim::runtime_config_for(&setup);
    let prepared = crate::hex_wfc::launch::prepare(crate::hex_wfc::launch::HexLaunchSpec {
        requested_seed: 0xF011_FAC1_1177,
        config,
        seed_policy: crate::hex_wfc::launch::HexSeedPolicy::Nearby,
    })
    .expect("capture seed prepares");
    let mut tape = ReplayTape::new_hex_wfc_for_player(&prepared.match_state, PlayerId(0));
    tape.record_hex_wfc(&prepared.match_state);
    let facts = AscentResult {
        role,
        outcome,
        winner,
        local_team: TeamId(0),
        loyal: if role == Rogue { 3 } else { 4 },
        jailed: if outcome == RogueVictory && case != 8 {
            if role == Rogue { 3 } else { 4 }
        } else {
            0
        },
        corrupted: usize::from(role == Rogue),
        rogue_by_capture: case != 8,
    };
    tape.ascent_result = Some(facts);
    for actor in &mut tape.actors {
        if actor.id == crate::sim::replay::ReplayActorId::LocalPlayer
            && matches!(role, Architect | Spectator)
        {
            actor.label = "Team 1 Observer (bot)".to_string();
        }
    }
    for pose in &mut tape.samples[0].actors {
        pose.status =
            if role == Rogue && pose.actor == crate::sim::replay::ReplayActorId::LocalPlayer {
                "Rogue"
            } else if facts.jailed > 0 {
                "jailed"
            } else {
                "loyal"
            }
            .to_string();
        pose.task = if pose.status == "Rogue" {
            "disrupt loyal teams"
        } else if pose.status == "jailed" {
            "escape prison or await rescue"
        } else {
            "ascend with the team"
        }
        .to_string();
        if pose.status != "loyal" {
            pose.room = None;
        }
    }
    let local_won =
        (role == Rogue && outcome == RogueVictory) || (role != Rogue && winner == Some(TeamId(0)));
    let result = crate::flow::MatchResult {
        local_team: TeamId(0),
        local_won,
        winner,
        placement: local_won.then_some(1),
        escaped: usize::from(winner.is_some()),
        absorbed: usize::from(winner.is_none()),
    };
    tape.result = Some(result.clone());
    let mut career = world.resource_mut::<Career>();
    *career = Career::default();
    career.record(result);
    career.last_ascent_result = Some(facts);
    world.insert_resource(ActivePlaySession::from_launch(
        config,
        role == Spectator,
        matches!(case, 15 | 16),
    ));
    world.insert_resource(crate::play_setup::LaunchedPlaySetup(setup.clone()));
    world.insert_resource(setup);
    world.resource_mut::<crate::lan::LanRuntime>().leave();
    if matches!(case, 15 | 16) {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("local capture socket");
        let client = observed_net::lan::LanClient::connect(
            socket.local_addr().unwrap(),
            1,
            None,
            None,
            [0; 32],
        )
        .expect("local capture client");
        world.resource_mut::<crate::lan::LanRuntime>().client = Some(client);
    }
    if case == 9 {
        world.remove_resource::<ReplayTape>();
    } else {
        if case == 10 {
            tape.samples.clear();
        }
        world.insert_resource(tape);
    }
}
