//! Tests for the asynchronous hex launch request lifecycle (see `loading.rs`).

use observed_facility::hex_wfc::HexWfcConfig;
use observed_match::hex_wfc::HexMatchConfig;

use super::*;
use crate::hex_wfc::launch::HexSeedPolicy;

fn request(sequence: &mut HexLaunchRequestSequence) -> HexLaunchRequest {
    sequence.issue(
        LaunchContext::Local,
        PlayerId(7),
        true,
        false,
        HexLaunchSpec {
            requested_seed: 41,
            config: HexMatchConfig {
                teams: 2,
                members_per_team: 2,
                guardian: true,
                wfc: HexWfcConfig {
                    cols: 4,
                    rows: 4,
                    levels: 1,
                    min_rooms: 1,
                    max_rooms: 2,
                    retry_budget: 1,
                    min_room_distance: 1,
                },
            },
            seed_policy: HexSeedPolicy::Nearby,
        },
    )
}

#[test]
fn request_ids_are_unique_and_metadata_survives_finalization() {
    let mut sequence = HexLaunchRequestSequence::default();
    let first = request(&mut sequence);
    let second = request(&mut sequence);

    assert_ne!(first.request_id, second.request_id);
    assert_eq!(first.request_id.get(), 1);
    assert_eq!(second.request_id.get(), 2);
    assert_eq!(first.context, LaunchContext::Local);
    assert_eq!(first.local_player, PlayerId(7));
    assert!(first.spectator);
    assert!(!first.networked);
    assert_eq!(first.spec.requested_seed, 41);
}

#[test]
fn stale_and_cancelled_completions_never_become_ready() {
    let mut sequence = HexLaunchRequestSequence::default();
    let first = request(&mut sequence);
    let second = request(&mut sequence);
    let mut state = HexLoadingState::default();
    state.begin(second.request_id, 1);

    assert_eq!(
        state.accept_completion(first.request_id, Ok(())),
        CompletionAcceptance::Stale
    );
    assert_eq!(state.phase, HexLoadingPhase::Preparing);

    state.cancel();
    assert_eq!(
        state.accept_completion(second.request_id, Ok(())),
        CompletionAcceptance::Inactive
    );
    assert_eq!(state.phase, HexLoadingPhase::Cancelled);
}

#[test]
fn failure_then_retry_uses_a_new_identity_and_clears_the_error() {
    let mut sequence = HexLaunchRequestSequence::default();
    let failed_request = request(&mut sequence);
    let mut state = HexLoadingState::default();
    state.begin(failed_request.request_id, 1);
    let error = HexLoadingError::Preparation(HexLaunchError::CatalogLoad(
        "fixture catalog failure".to_string(),
    ));

    assert_eq!(
        state.accept_completion(failed_request.request_id, Err(error.clone())),
        CompletionAcceptance::Failed
    );
    assert_eq!(state.phase, HexLoadingPhase::Failed);
    assert_eq!(state.error, Some(error));

    let retry_request = sequence.reissue(failed_request);
    state.begin(retry_request.request_id, 2);
    assert_ne!(retry_request.request_id, failed_request.request_id);
    assert_eq!(state.request_id, Some(retry_request.request_id));
    assert_eq!(state.attempt, 2);
    assert_eq!(state.phase, HexLoadingPhase::Preparing);
    assert_eq!(state.error, None);

    assert_eq!(
        state.accept_completion(failed_request.request_id, Ok(())),
        CompletionAcceptance::Stale
    );
    assert_eq!(state.phase, HexLoadingPhase::Preparing);
}

#[test]
fn lan_descriptor_matching_rejects_stale_or_conflicting_generations() {
    let mut sequence = HexLaunchRequestSequence::default();
    let mut request = request(&mut sequence);
    request.networked = true;
    request.spec.seed_policy = HexSeedPolicy::Exact {
        expected_content_hash: [4; 32],
    };
    let accepted = observed_net::lan::LanLaunch {
        seed: request.spec.requested_seed,
        match_number: 8,
        config: request.spec.config,
        simulation_content_hash: [4; 32],
    };

    assert!(launch_matches_request(Some(accepted), Some(&request), 8));
    assert!(!launch_matches_request(Some(accepted), Some(&request), 7));
    assert!(!launch_matches_request(
        Some(observed_net::lan::LanLaunch {
            seed: accepted.seed.wrapping_add(1),
            ..accepted
        }),
        Some(&request),
        8
    ));
}

#[test]
fn locally_prepared_lan_launch_waits_for_authoritative_start() {
    let mut sequence = HexLaunchRequestSequence::default();
    let request = request(&mut sequence);
    let mut state = HexLoadingState::default();
    state.begin(request.request_id, 1);
    assert_eq!(
        state.accept_completion(request.request_id, Ok(())),
        CompletionAcceptance::Ready
    );

    state.wait_for_players(9);
    assert_eq!(state.phase, HexLoadingPhase::WaitingForPlayers);
    assert_eq!(state.lan_match_number, Some(9));

    state.ready();
    assert_eq!(state.phase, HexLoadingPhase::Ready);
}
