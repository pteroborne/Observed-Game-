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

/// The defect this guards is not "the solve panicked" but "the solve panicked and
/// nobody found out". `bevy_tasks::TaskPool::spawn` leaves the future unguarded, the
/// pool thread swallows the payload, and the next poll of the cancelled handle panics
/// again on the *main* thread — killing the process between a finished solve and a
/// playable map, with nothing written down. Catching it on the worker keeps the handle
/// pollable and turns the payload into something the error screen can show.
#[test]
fn a_panicking_solve_becomes_a_reported_failure_rather_than_a_cancelled_task() {
    let outcome = guard_preparation(|| panic!("the collapse ran out of candidates"));

    let Err(HexLoadingError::PreparationPanicked(detail)) = outcome else {
        panic!("a panicking solve must be classified as a panic, got {outcome:?}");
    };
    assert!(
        detail.contains("the collapse ran out of candidates"),
        "the panic message must survive into the report: {detail}"
    );
    assert!(
        detail.contains("loading_tests.rs"),
        "the report must name where the panic came from: {detail}"
    );
}

/// An `unwrap`/`expect` payload is a `String`; a bare `panic!("literal")` is a
/// `&'static str`. Both are common in the solver, and neither may report as empty.
#[test]
fn every_panic_payload_shape_produces_a_readable_report() {
    let borrowed: Box<dyn std::any::Any + Send> = Box::new("a static message");
    let owned: Box<dyn std::any::Any + Send> = Box::new("an owned message".to_string());
    let opaque: Box<dyn std::any::Any + Send> = Box::new(17_u32);

    assert_eq!(describe_panic(borrowed.as_ref(), None), "a static message");
    assert_eq!(describe_panic(owned.as_ref(), None), "an owned message");
    assert_eq!(
        describe_panic(opaque.as_ref(), None),
        "a non-string panic payload"
    );
}

/// An ordinary solve failure must not be relabelled as a panic: the two mean very
/// different things to whoever reads the report.
#[test]
fn an_ordinary_solve_failure_is_not_reported_as_a_panic() {
    let outcome = guard_preparation(|| {
        Err(HexLaunchError::CatalogLoad("fixture catalog failure".into()))
    });

    assert_eq!(
        outcome.expect_err("the fixture always fails"),
        HexLoadingError::Preparation(HexLaunchError::CatalogLoad(
            "fixture catalog failure".into()
        ))
    );
}

/// Every failure must be able to say something to a player. An error variant whose
/// text is empty is the same defect as no error at all.
#[test]
fn every_loading_error_says_something() {
    for error in [
        HexLoadingError::MissingRequest,
        HexLoadingError::Preparation(HexLaunchError::CatalogLoad("disk".into())),
        HexLoadingError::PreparationPanicked("index out of bounds (at solve.rs:9:1)".into()),
        HexLoadingError::WorkerLost("handle cancelled".into()),
        HexLoadingError::LanLaunchUnavailable,
        HexLoadingError::LanLaunchWithdrawn,
        HexLoadingError::LanTransport("socket closed".into()),
    ] {
        assert!(
            !error.to_string().trim().is_empty(),
            "{error:?} renders as nothing"
        );
    }
    assert!(
        HexLoadingError::PreparationPanicked("index out of bounds (at solve.rs:9:1)".into())
            .to_string()
            .contains("solve.rs:9:1"),
        "the panic location must reach the player-facing text"
    );
}

/// `begin_request` advances the phase immediately but inserts the worker through
/// `Commands`, so one flush of grace is legitimate. Anything past that is a load that
/// will never finish, and it must report rather than sit on "Solving..." forever.
#[test]
fn a_lost_worker_is_reported_only_after_the_command_flush_window() {
    let mut sequence = HexLaunchRequestSequence::default();
    let request = request(&mut sequence);
    let mut state = HexLoadingState::default();
    state.begin(request.request_id, 1);

    state.elapsed = Duration::ZERO;
    assert!(worker_watchdog(&state).is_none());
    state.elapsed = WORKER_WATCHDOG_GRACE - Duration::from_millis(1);
    assert!(worker_watchdog(&state).is_none());

    state.elapsed = WORKER_WATCHDOG_GRACE;
    let error = worker_watchdog(&state).expect("a worker missing past the grace is lost");
    assert!(matches!(error, HexLoadingError::WorkerLost(_)));

    // A load that is not preparing has no worker to miss.
    state.ready();
    state.elapsed = WORKER_WATCHDOG_GRACE * 4;
    assert!(worker_watchdog(&state).is_none());
}

/// Reaching `Failed` without recording the reason is the exact shape of this bug, so
/// pin the invariant on the state machine itself.
#[test]
fn no_path_reaches_the_failed_phase_without_an_error_to_show() {
    let mut sequence = HexLaunchRequestSequence::default();
    let request = request(&mut sequence);

    let mut missing = HexLoadingState::default();
    missing.fail_missing_request();
    assert_eq!(missing.phase, HexLoadingPhase::Failed);
    assert!(missing.error.is_some());

    let mut panicked = HexLoadingState::default();
    panicked.begin(request.request_id, 1);
    panicked.fail(HexLoadingError::PreparationPanicked("boom".into()));
    assert_eq!(panicked.phase, HexLoadingPhase::Failed);
    assert!(panicked.error.is_some());

    let mut completed = HexLoadingState::default();
    completed.begin(request.request_id, 1);
    assert_eq!(
        completed.accept_completion(
            request.request_id,
            Err(HexLoadingError::WorkerLost("handle cancelled".into()))
        ),
        CompletionAcceptance::Failed
    );
    assert_eq!(completed.phase, HexLoadingPhase::Failed);
    assert!(completed.error.is_some());
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
