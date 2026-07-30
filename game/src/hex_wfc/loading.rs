//! Responsive preparation boundary for the canonical hex facility.
//!
//! The menu resolves every launch choice into a [`HexLaunchRequest`] before entering
//! [`GameState::Loading`](crate::GameState::Loading). This module then owns the one
//! expensive operation: running [`prepare`] on Bevy's async compute pool. Completion
//! is identity checked, so a cancelled or superseded solve can never launch a match.

use std::time::{Duration, Instant};

use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use observed_core::PlayerId;

use crate::{GameState, play_setup::LaunchContext};

use super::launch::{HexLaunchError, HexLaunchSpec, PreparedHexLaunch, prepare};

/// Stable identity for one preparation attempt. Retry always receives a fresh value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HexLaunchRequestId(u64);

impl HexLaunchRequestId {
    #[must_use]
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

/// Monotonic request-id allocator shared by local, rematch, and LAN launch paths.
#[derive(Resource, Debug, Default)]
pub(crate) struct HexLaunchRequestSequence {
    last_issued: u64,
}

impl HexLaunchRequestSequence {
    /// Finalize a launch request. Callers must insert the returned resource before
    /// requesting [`GameState::Loading`].
    pub(crate) fn issue(
        &mut self,
        context: LaunchContext,
        local_player: PlayerId,
        spectator: bool,
        networked: bool,
        spec: HexLaunchSpec,
    ) -> HexLaunchRequest {
        HexLaunchRequest {
            request_id: self.next_id(),
            context,
            local_player,
            spectator,
            networked,
            spec,
        }
    }

    fn reissue(&mut self, request: HexLaunchRequest) -> HexLaunchRequest {
        HexLaunchRequest {
            request_id: self.next_id(),
            ..request
        }
    }

    fn next_id(&mut self) -> HexLaunchRequestId {
        self.last_issued = self.last_issued.wrapping_add(1).max(1);
        HexLaunchRequestId(self.last_issued)
    }
}

/// Complete immutable input and presentation metadata for one launch attempt.
///
/// Simulation preparation reads only [`Self::spec`]. The remaining fields let the
/// runtime and loading screen preserve ownership, spectator, network, and back-route
/// behavior without reconstructing those decisions from UI entities.
#[derive(Resource, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HexLaunchRequest {
    pub(crate) request_id: HexLaunchRequestId,
    pub(crate) context: LaunchContext,
    pub(crate) local_player: PlayerId,
    pub(crate) spectator: bool,
    pub(crate) networked: bool,
    pub(crate) spec: HexLaunchSpec,
}

/// One-shot handoff from the loading worker to `HexWfc` runtime setup.
///
/// Keeping the prepared value in an `Option` lets an ordinary Bevy setup system move
/// the large authoritative match out exactly once through [`Self::take`], without an
/// exclusive `World` system or a second solve.
#[derive(Resource, Debug, Default)]
pub(crate) struct PreparedHexLaunchSlot(pub(crate) Option<PreparedHexLaunch>);

impl PreparedHexLaunchSlot {
    #[must_use]
    pub(crate) const fn ready(prepared: PreparedHexLaunch) -> Self {
        Self(Some(prepared))
    }

    pub(crate) fn take(&mut self) -> Option<PreparedHexLaunch> {
        self.0.take()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum HexLoadingPhase {
    #[default]
    AwaitingRequest,
    Preparing,
    WaitingForPlayers,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HexLoadingError {
    MissingRequest,
    Preparation(HexLaunchError),
    LanLaunchUnavailable,
    LanLaunchWithdrawn,
    LanTransport(String),
}

impl std::fmt::Display for HexLoadingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingRequest => formatter.write_str(
                "No finalized launch request was provided. Return to Play and try again.",
            ),
            Self::Preparation(error) => error.fmt(formatter),
            Self::LanLaunchUnavailable => formatter
                .write_str("The server launch changed or disappeared before preparation began."),
            Self::LanLaunchWithdrawn => formatter
                .write_str("The server returned to the lobby while players were preparing."),
            Self::LanTransport(error) => write!(formatter, "send launch readiness: {error}"),
        }
    }
}

/// Honest, player-facing loading state. There is deliberately no invented percentage:
/// the deterministic WFC solve currently exposes no meaningful incremental progress.
#[derive(Resource, Debug)]
pub(crate) struct HexLoadingState {
    pub(crate) request_id: Option<HexLaunchRequestId>,
    pub(crate) attempt: u32,
    pub(crate) phase: HexLoadingPhase,
    pub(crate) elapsed: Duration,
    pub(crate) error: Option<HexLoadingError>,
    pub(crate) lan_match_number: Option<u32>,
    started_at: Option<Instant>,
}

impl Default for HexLoadingState {
    fn default() -> Self {
        Self {
            request_id: None,
            attempt: 0,
            phase: HexLoadingPhase::AwaitingRequest,
            elapsed: Duration::ZERO,
            error: None,
            lan_match_number: None,
            started_at: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompletionAcceptance {
    Ready,
    Failed,
    Stale,
    Inactive,
}

impl HexLoadingState {
    fn begin(&mut self, request_id: HexLaunchRequestId, attempt: u32) {
        self.request_id = Some(request_id);
        self.attempt = attempt.max(1);
        self.phase = HexLoadingPhase::Preparing;
        self.elapsed = Duration::ZERO;
        self.error = None;
        self.lan_match_number = None;
        self.started_at = Some(Instant::now());
    }

    fn fail_missing_request(&mut self) {
        self.request_id = None;
        self.attempt = 0;
        self.phase = HexLoadingPhase::Failed;
        self.elapsed = Duration::ZERO;
        self.error = Some(HexLoadingError::MissingRequest);
        self.lan_match_number = None;
        self.started_at = None;
    }

    fn update_elapsed(&mut self) {
        if matches!(
            self.phase,
            HexLoadingPhase::Preparing | HexLoadingPhase::WaitingForPlayers
        ) && let Some(started_at) = self.started_at
        {
            self.elapsed = started_at.elapsed();
        }
    }

    fn cancel(&mut self) {
        self.update_elapsed();
        self.phase = HexLoadingPhase::Cancelled;
        self.error = None;
        self.lan_match_number = None;
        self.started_at = None;
    }

    fn accept_completion(
        &mut self,
        request_id: HexLaunchRequestId,
        outcome: Result<(), HexLoadingError>,
    ) -> CompletionAcceptance {
        if self.request_id != Some(request_id) {
            return CompletionAcceptance::Stale;
        }
        if self.phase != HexLoadingPhase::Preparing {
            return CompletionAcceptance::Inactive;
        }
        self.update_elapsed();
        self.started_at = None;
        match outcome {
            Ok(()) => {
                self.phase = HexLoadingPhase::Ready;
                self.error = None;
                CompletionAcceptance::Ready
            }
            Err(error) => {
                self.phase = HexLoadingPhase::Failed;
                self.error = Some(error);
                CompletionAcceptance::Failed
            }
        }
    }

    fn wait_for_players(&mut self, match_number: u32) {
        self.phase = HexLoadingPhase::WaitingForPlayers;
        self.lan_match_number = Some(match_number);
        self.error = None;
        self.started_at = Instant::now().checked_sub(self.elapsed);
    }

    fn ready(&mut self) {
        self.update_elapsed();
        self.phase = HexLoadingPhase::Ready;
        self.error = None;
        self.started_at = None;
    }

    fn fail(&mut self, error: HexLoadingError) {
        self.update_elapsed();
        self.phase = HexLoadingPhase::Failed;
        self.error = Some(error);
        self.started_at = None;
    }
}

struct HexLaunchTaskResult {
    request_id: HexLaunchRequestId,
    prepared: Result<PreparedHexLaunch, HexLaunchError>,
}

#[derive(Resource)]
pub(crate) struct HexLaunchWorker {
    task: Task<HexLaunchTaskResult>,
}

/// `OnEnter(GameState::Loading)`: start exactly the finalized request currently in
/// resources. Missing input becomes a visible, recoverable error instead of a panic.
pub(crate) fn start_loading(
    mut commands: Commands,
    request: Option<Res<HexLaunchRequest>>,
    lan: Option<Res<crate::lan::LanRuntime>>,
    mut state: ResMut<HexLoadingState>,
) {
    commands.remove_resource::<PreparedHexLaunchSlot>();
    let Some(request) = request else {
        commands.remove_resource::<HexLaunchWorker>();
        state.fail_missing_request();
        return;
    };
    let lan_match_number = if request.networked {
        let Some(match_number) = matching_lan_generation(&request, lan.as_deref()) else {
            commands.remove_resource::<HexLaunchWorker>();
            state.fail(HexLoadingError::LanLaunchUnavailable);
            return;
        };
        Some(match_number)
    } else {
        None
    };
    begin_request(&mut commands, *request, &mut state, 1);
    state.lan_match_number = lan_match_number;
}

/// `Update` while loading: poll once and return immediately when the worker is still
/// busy. Only the current, active request may publish a prepared match or transition.
pub(crate) fn poll_loading(
    mut commands: Commands,
    mut worker: Option<ResMut<HexLaunchWorker>>,
    request: Option<Res<HexLaunchRequest>>,
    mut lan: Option<ResMut<crate::lan::LanRuntime>>,
    mut state: ResMut<HexLoadingState>,
    mut next: ResMut<NextState<GameState>>,
) {
    state.update_elapsed();

    if state.phase == HexLoadingPhase::WaitingForPlayers {
        poll_lan_barrier(
            &mut commands,
            request.as_deref(),
            lan.as_deref_mut(),
            &mut state,
            &mut next,
        );
        return;
    }
    let Some(worker) = worker.as_deref_mut() else {
        return;
    };
    let Some(completion) = block_on(poll_once(&mut worker.task)) else {
        return;
    };
    commands.remove_resource::<HexLaunchWorker>();

    let outcome = completion
        .prepared
        .as_ref()
        .map(|_| ())
        .map_err(|error| HexLoadingError::Preparation(error.clone()));
    match state.accept_completion(completion.request_id, outcome) {
        CompletionAcceptance::Ready => {
            let prepared = completion
                .prepared
                .expect("accepted ready completion contains a prepared launch");
            commands.insert_resource(PreparedHexLaunchSlot::ready(prepared));
            if request.as_deref().is_some_and(|request| request.networked) {
                let Some(match_number) = state.lan_match_number else {
                    commands.remove_resource::<PreparedHexLaunchSlot>();
                    state.fail(HexLoadingError::LanLaunchUnavailable);
                    return;
                };
                let Some(client) = lan.as_deref_mut().and_then(|lan| lan.client.as_mut()) else {
                    commands.remove_resource::<PreparedHexLaunchSlot>();
                    state.fail(HexLoadingError::LanLaunchUnavailable);
                    return;
                };
                if lan_launch_withdrawn(client, match_number) {
                    commands.remove_resource::<PreparedHexLaunchSlot>();
                    commands.remove_resource::<HexLaunchRequest>();
                    state.fail(HexLoadingError::LanLaunchWithdrawn);
                    next.set(GameState::Lobby);
                    return;
                }
                if !launch_matches_request(client.launch, request.as_deref(), match_number) {
                    commands.remove_resource::<PreparedHexLaunchSlot>();
                    state.fail(HexLoadingError::LanLaunchUnavailable);
                    return;
                }
                if let Err(error) = client.mark_launch_ready(match_number) {
                    commands.remove_resource::<PreparedHexLaunchSlot>();
                    state.fail(HexLoadingError::LanTransport(error.to_string()));
                    return;
                }
                state.wait_for_players(match_number);
                if client.launch_has_started(match_number) {
                    state.ready();
                    next.set(GameState::HexWfc);
                }
            } else {
                next.set(GameState::HexWfc);
            }
        }
        CompletionAcceptance::Failed
        | CompletionAcceptance::Stale
        | CompletionAcceptance::Inactive => {}
    }
}

/// Start a fresh attempt for the same finalized launch. A new request id is the hard
/// boundary that prevents a late completion from the previous task being accepted.
pub(crate) fn retry_loading(
    commands: &mut Commands,
    sequence: &mut HexLaunchRequestSequence,
    request: &mut HexLaunchRequest,
    lan: Option<&crate::lan::LanRuntime>,
    state: &mut HexLoadingState,
) -> Option<HexLaunchRequestId> {
    if state.phase != HexLoadingPhase::Failed {
        return None;
    }
    let attempt = state.attempt.saturating_add(1).max(1);
    *request = sequence.reissue(*request);
    commands.remove_resource::<PreparedHexLaunchSlot>();
    let lan_match_number = if request.networked {
        let Some(match_number) = matching_lan_generation(request, lan) else {
            state.fail(HexLoadingError::LanLaunchUnavailable);
            return None;
        };
        Some(match_number)
    } else {
        None
    };
    begin_request(commands, *request, state, attempt);
    state.lan_match_number = lan_match_number;
    Some(request.request_id)
}

/// Cancel the active attempt. Dropping the task handle asks Bevy's executor to cancel
/// it; identity and phase checks still reject a result that won a completion race.
pub(crate) fn cancel_loading(commands: &mut Commands, state: &mut HexLoadingState) {
    state.cancel();
    commands.remove_resource::<HexLaunchWorker>();
    commands.remove_resource::<PreparedHexLaunchSlot>();
}

/// `OnExit(GameState::Loading)`: defensive cleanup for external state changes.
pub(crate) fn cleanup_loading_worker(mut commands: Commands) {
    commands.remove_resource::<HexLaunchWorker>();
}

fn begin_request(
    commands: &mut Commands,
    request: HexLaunchRequest,
    state: &mut HexLoadingState,
    attempt: u32,
) {
    state.begin(request.request_id, attempt);
    let request_id = request.request_id;
    let spec = request.spec;
    let task = AsyncComputeTaskPool::get().spawn(async move {
        HexLaunchTaskResult {
            request_id,
            prepared: prepare(spec),
        }
    });
    commands.insert_resource(HexLaunchWorker { task });
}

fn matching_lan_generation(
    request: &HexLaunchRequest,
    lan: Option<&crate::lan::LanRuntime>,
) -> Option<u32> {
    let launch = lan?.client.as_ref()?.launch?;
    launch_matches_request(Some(launch), Some(request), launch.match_number)
        .then_some(launch.match_number)
}

fn launch_matches_request(
    launch: Option<observed_net::lan::LanLaunch>,
    request: Option<&HexLaunchRequest>,
    match_number: u32,
) -> bool {
    let (Some(launch), Some(request)) = (launch, request) else {
        return false;
    };
    launch.match_number == match_number
        && request.networked
        && launch.seed == request.spec.requested_seed
        && launch.config == request.spec.config
        && matches!(
            request.spec.seed_policy,
            super::launch::HexSeedPolicy::Exact {
                expected_content_hash
            } if expected_content_hash == launch.simulation_content_hash
        )
}

fn poll_lan_barrier(
    commands: &mut Commands,
    request: Option<&HexLaunchRequest>,
    lan: Option<&mut crate::lan::LanRuntime>,
    state: &mut HexLoadingState,
    next: &mut NextState<GameState>,
) {
    let Some(match_number) = state.lan_match_number else {
        commands.remove_resource::<PreparedHexLaunchSlot>();
        state.fail(HexLoadingError::LanLaunchUnavailable);
        return;
    };
    let Some(lan) = lan else {
        commands.remove_resource::<PreparedHexLaunchSlot>();
        state.fail(HexLoadingError::LanLaunchUnavailable);
        return;
    };
    let Some(client) = lan.client.as_mut() else {
        commands.remove_resource::<PreparedHexLaunchSlot>();
        state.fail(HexLoadingError::LanLaunchUnavailable);
        return;
    };
    if lan_launch_withdrawn(client, match_number) {
        commands.remove_resource::<PreparedHexLaunchSlot>();
        commands.remove_resource::<HexLaunchRequest>();
        state.fail(HexLoadingError::LanLaunchWithdrawn);
        next.set(GameState::Lobby);
        return;
    }
    if !launch_matches_request(client.launch, request, match_number) {
        commands.remove_resource::<PreparedHexLaunchSlot>();
        state.fail(HexLoadingError::LanLaunchUnavailable);
        return;
    }
    if client.launch_has_started(match_number) {
        state.ready();
        next.set(GameState::HexWfc);
    }
}

fn lan_launch_withdrawn(client: &observed_net::lan::LanClient, match_number: u32) -> bool {
    client.lobby.as_ref().is_some_and(|lobby| {
        lobby.match_number >= match_number && lobby.phase != observed_net::lan::WirePhase::InMatch
    })
}

#[cfg(test)]
#[path = "loading_tests.rs"]
mod tests;
