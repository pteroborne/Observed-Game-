//! Native cosmetics and loading layout fixtures. These do not prove a live LAN launch.
use super::{Shot, shot};
use crate::{
    GameState,
    flow::Career,
    hex_wfc::{
        launch::{HexLaunchSpec, HexSeedPolicy},
        loading::{
            HexLaunchRequest, HexLaunchRequestSequence, HexLaunchWorker, HexLoadingError,
            HexLoadingPhase, HexLoadingState,
        },
    },
    play_setup::{LaunchContext, PlayRules, PlaySeat},
};
use bevy::prelude::*;

pub(super) fn sweep() -> Vec<Shot> {
    [
        "00_equipped_defaults",
        "01_locked_ember",
        "02_cobalt_comparison",
        "03_comet_comparison",
        "04_champion_comparison",
        "05_equipped_combination",
        "06_observer_loading",
        "07_architect_loading",
        "08_race_loading",
        "09_spectator_loading",
        "10_lan_waiting",
        "11_ready",
        "12_layout_failure",
        "13_rematch_failure",
        "14_lan_files_mismatch",
        "15_lan_host_silent",
        "16_no_match",
    ]
    .into_iter()
    .enumerate()
    .map(|(case, label)| Shot {
        polish: Some(case),
        ..shot(
            label,
            if case < 6 {
                GameState::Loadout
            } else {
                GameState::Loading
            },
        )
    })
    .collect()
}

pub(super) fn stage(world: &mut World, case: usize) {
    world.resource_mut::<crate::lan::LanRuntime>().leave();
    if case < 6 {
        let mut career = Career::default();
        if case >= 2 {
            for _ in 0..15 {
                career.profile.award_match(Some(1));
            }
        }
        if case == 5 {
            for id in [2, 6, 9] {
                assert!(career.profile.equip(id));
            }
        }
        world.insert_resource(career);
        return;
    }
    if case == 16 {
        world.remove_resource::<HexLaunchRequest>();
        return;
    }
    let context = match case {
        10 | 14 | 15 => LaunchContext::Lan,
        13 => LaunchContext::Rematch,
        _ => LaunchContext::Local,
    };
    let rules = if case == 8 {
        PlayRules::Race
    } else {
        PlayRules::Ascent
    };
    let seat = if case == 7 || case == 10 {
        PlaySeat::Architect
    } else {
        PlaySeat::Observer
    };
    let request = world.resource_mut::<HexLaunchRequestSequence>().issue(
        context,
        observed_core::PlayerId(0),
        case == 9,
        context == LaunchContext::Lan,
        HexLaunchSpec {
            requested_seed: 42,
            config: observed_match::hex_wfc::HexMatchConfig {
                teams: if case == 6 { 1 } else { 2 },
                members_per_team: if case == 6 { 1 } else { 2 },
                ..default()
            },
            seed_policy: if context == LaunchContext::Lan {
                HexSeedPolicy::Exact {
                    expected_content_hash: [0; 32],
                }
            } else {
                HexSeedPolicy::Nearby
            },
        },
        (rules, seat),
    );
    world.insert_resource(request);
    if context == LaunchContext::Lan {
        // A local unserved UDP socket retains a staged launch while the viewer waits.
        // No server is contacted, and no multiplayer acceptance is claimed.
        let mut client = observed_net::lan::LanClient::connect(
            "127.0.0.1:9".parse().unwrap(),
            901,
            None,
            None,
            [0; 32],
        )
        .expect("fixture socket");
        client.launch = Some(observed_net::lan::LanLaunch {
            seed: request.spec.requested_seed,
            match_number: 9,
            config: request.spec.config,
            simulation_content_hash: [0; 32],
            ascent: true,
            architects: if seat == PlaySeat::Architect { 1 } else { 0 },
        });
        world.resource_mut::<crate::lan::LanRuntime>().client = Some(client);
    }
}

pub(super) fn pose(world: &mut World, case: usize) {
    if case < 6 {
        use crate::screens::loadout::LoadoutAction;
        let selected = match case {
            1 => 1,
            2 | 5 => 2,
            3 => 6,
            4 => 9,
            _ => 0,
        };
        let mut actions = world.query::<(Entity, &LoadoutAction)>();
        let entity = actions
            .iter(world)
            .find_map(|(entity, action)| {
                (*action == LoadoutAction::Select(selected)).then_some(entity)
            })
            .expect("cosmetic selector");
        world.trigger(bevy::ui_widgets::Activate { entity });
        world
            .resource_mut::<bevy::input_focus::InputFocus>()
            .set(entity, bevy::input_focus::FocusCause::Navigated);
        return;
    }
    world.remove_resource::<HexLaunchWorker>();
    let mut state = HexLoadingState::default();
    state.request_id = world
        .get_resource::<HexLaunchRequest>()
        .map(|request| request.request_id);
    state.elapsed = std::time::Duration::from_millis(2400);
    state.attempt = if case == 13 { 2 } else { 1 };
    if case == 16 {
        state.elapsed = std::time::Duration::ZERO;
        state.attempt = 0;
    }
    state.phase = match case {
        10 => HexLoadingPhase::WaitingForPlayers,
        11 => HexLoadingPhase::Ready,
        12..=16 => HexLoadingPhase::Failed,
        _ => HexLoadingPhase::Preparing,
    };
    state.lan_match_number = (case == 10).then_some(9);
    state.error = match case {
        12 => Some(HexLoadingError::Preparation(
            crate::hex_wfc::launch::HexLaunchError::NearbySeedsExhausted {
                requested_seed: 42,
                attempts: 64,
                last_error: None,
            },
        )),
        13 => Some(HexLoadingError::PreparationPanicked(
            "staged failure".into(),
        )),
        14 => Some(HexLoadingError::Preparation(
            crate::hex_wfc::launch::HexLaunchError::ContentHashMismatch {
                expected: [0; 32],
                actual: [1; 32],
            },
        )),
        15 => Some(HexLoadingError::LanServerSilent),
        16 => Some(HexLoadingError::MissingRequest),
        _ => None,
    };
    world.insert_resource(state);
}
