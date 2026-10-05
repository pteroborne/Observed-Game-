//! Player-facing preparation screen for the canonical hex facility.
//!
//! This module owns its own typed actions; the global menu dispatcher never needs a
//! loading-specific branch. Progress copy reports elapsed time and the actual coarse
//! state only, because the WFC solve does not expose a trustworthy percentage.

use bevy::{
    ecs::system::SystemParam,
    input_focus::{FocusCause, InputFocus, tab_navigation::TabIndex},
    prelude::*,
    ui::InteractionDisabled,
    ui_widgets::Activate,
};

use super::widgets::{self, FocusScope, FocusScopeId, WidgetId, WidgetSpec, activation_enabled};
use crate::{
    GameState,
    hex_wfc::loading::{
        HexLaunchRequest, HexLaunchRequestSequence, HexLoadingError, HexLoadingPhase,
        HexLoadingState, cancel_loading, retry_loading,
    },
    play_setup::{LaunchContext, PlayRules, PlaySeat},
    view::theme::{ACCENT, DIM, TITLE, WARNING, panel, screen_root, summary_panel, text},
};

const LOADING_SCOPE: FocusScopeId = FocusScopeId("hex_loading");
const RETRY: WidgetId = WidgetId::named("hex_loading.retry");
const CANCEL: WidgetId = WidgetId::named("hex_loading.cancel");

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LoadingAction {
    Retry,
    Cancel,
}

#[derive(Component)]
pub(crate) struct LoadingStatusText;

#[derive(Component)]
pub(crate) struct LoadingElapsedText;

#[derive(Component)]
pub(crate) struct LoadingErrorText;

#[derive(Component)]
struct LoadingNextActionText;

#[derive(Component)]
pub(crate) struct RetryAvailability {
    enabled: bool,
}

#[derive(Component)]
pub(crate) struct CancelButton;

#[derive(SystemParam)]
pub(crate) struct LoadingActivationContext<'w, 's> {
    commands: Commands<'w, 's>,
    sequence: ResMut<'w, HexLaunchRequestSequence>,
    request: Option<ResMut<'w, HexLaunchRequest>>,
    lan: Option<ResMut<'w, crate::lan::LanRuntime>>,
    state: ResMut<'w, HexLoadingState>,
    next: ResMut<'w, NextState<GameState>>,
}

type LoadingErrorQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Text, &'static mut Visibility),
    (
        With<LoadingErrorText>,
        Without<LoadingStatusText>,
        Without<LoadingElapsedText>,
    ),
>;

type LoadingNextActionQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<LoadingNextActionText>,
        Without<LoadingStatusText>,
        Without<LoadingElapsedText>,
        Without<LoadingErrorText>,
    ),
>;

#[derive(SystemParam)]
pub(crate) struct LoadingRefreshContext<'w, 's> {
    state: Res<'w, HexLoadingState>,
    request: Option<Res<'w, HexLaunchRequest>>,
    status_text: Query<'w, 's, &'static mut Text, With<LoadingStatusText>>,
    elapsed_text:
        Query<'w, 's, &'static mut Text, (With<LoadingElapsedText>, Without<LoadingStatusText>)>,
    error_text: LoadingErrorQuery<'w, 's>,
    retry: Query<'w, 's, (Entity, &'static mut RetryAvailability)>,
    cancel: Query<'w, 's, Entity, With<CancelButton>>,
    focus: ResMut<'w, InputFocus>,
    commands: Commands<'w, 's>,
    next_action_text: LoadingNextActionQuery<'w, 's>,
}

pub(crate) fn setup(mut commands: Commands, request: Option<Res<HexLaunchRequest>>) {
    let summary = request.as_deref().map_or_else(
        || "No match was selected. Go back to choose a match.".to_string(),
        request_summary,
    );
    let cancel_label = request
        .as_deref()
        .map_or("Back to Play", |request| cancel_label(request.context));

    commands
        .spawn(screen_root(GameState::Loading))
        .with_children(|root| {
            root.spawn(text("GETTING READY", 42.0, TITLE));
            root.spawn(text("Preparing the facility for your match.", 16.0, DIM));
            root.spawn(summary_panel()).with_children(|summary_panel| {
                summary_panel.spawn(text(summary, 16.0, ACCENT));
                summary_panel.spawn((
                    LoadingStatusText,
                    text("Preparing rooms and routes...", 18.0, TITLE),
                ));
                summary_panel.spawn((
                    LoadingElapsedText,
                    text("Elapsed: 0.0 s | attempt 1", 15.0, DIM),
                ));
                summary_panel.spawn((
                    LoadingErrorText,
                    text("", 15.0, WARNING),
                    Visibility::Hidden,
                ));
            });
            root.spawn((
                panel(),
                widgets::focus_scope(FocusScope::screen(LOADING_SCOPE, RETRY, CANCEL)),
            ))
            .with_children(|actions| {
                let retry = widgets::spawn_button(
                    actions,
                    WidgetSpec::disabled(RETRY, LOADING_SCOPE, 0, "Retry preparation"),
                    LoadingAction::Retry,
                );
                actions
                    .commands()
                    .entity(retry)
                    .insert(RetryAvailability { enabled: false });
                let cancel = widgets::spawn_button(
                    actions,
                    WidgetSpec::enabled(CANCEL, LOADING_SCOPE, 1, cancel_label),
                    LoadingAction::Cancel,
                );
                actions.commands().entity(cancel).insert(CancelButton);
            });
            root.spawn(text(
                "You can go back while the facility is being prepared.",
                15.0,
                DIM,
            ))
            .insert(LoadingNextActionText);
        });
}

/// Screen-local activation observer. Register with `App::add_observer`.
pub(crate) fn activate(
    activation: On<Activate>,
    actions: Query<&LoadingAction>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut context: LoadingActivationContext,
) {
    if !activation_enabled(&activation, &disabled) {
        return;
    }
    let Ok(action) = actions.get(activation.entity) else {
        return;
    };
    match action {
        LoadingAction::Retry => {
            let Some(request) = context.request.as_deref_mut() else {
                return;
            };
            retry_loading(
                &mut context.commands,
                &mut context.sequence,
                request,
                context.lan.as_deref(),
                &mut context.state,
            );
        }
        LoadingAction::Cancel => {
            let launch_context = context.request.as_deref().map(|request| request.context);
            let target = launch_context.map_or(GameState::Play, cancel_target);
            cancel_loading(&mut context.commands, &mut context.state);
            context.commands.remove_resource::<HexLaunchRequest>();
            if launch_context == Some(LaunchContext::Lan)
                && let Some(lan) = context.lan.as_deref_mut()
            {
                lan.leave();
            }
            context.next.set(target);
        }
    }
}

/// Keep status, elapsed time, errors, and retry availability synchronized with the
/// core state. Register in `Update` while `GameState::Loading` is active.
pub(crate) fn refresh(mut context: LoadingRefreshContext) {
    for mut text in &mut context.next_action_text {
        **text = next_action_label(context.state.phase, context.request.as_deref()).to_string();
    }
    for mut text in &mut context.status_text {
        **text = status_label(context.state.phase).to_string();
    }
    for mut text in &mut context.elapsed_text {
        **text = if context.state.attempt == 0 {
            format!("Elapsed: {:.1} s", context.state.elapsed.as_secs_f32())
        } else {
            format!(
                "Elapsed: {:.1} s | attempt {}",
                context.state.elapsed.as_secs_f32(),
                context.state.attempt
            )
        };
    }
    for (mut text, mut visibility) in &mut context.error_text {
        if let Some(error) = &context.state.error {
            **text = error_label(error).to_string();
            *visibility = Visibility::Inherited;
        } else {
            **text = String::new();
            *visibility = Visibility::Hidden;
        }
    }

    let retry_enabled = context.state.phase == HexLoadingPhase::Failed && context.request.is_some();
    let cancel_entity = context.cancel.single().ok();
    for (entity, mut availability) in &mut context.retry {
        if availability.enabled == retry_enabled {
            continue;
        }
        availability.enabled = retry_enabled;
        if retry_enabled {
            context
                .commands
                .entity(entity)
                .remove::<InteractionDisabled>()
                .insert(TabIndex(0));
            context.focus.set(entity, FocusCause::Navigated);
        } else {
            context
                .commands
                .entity(entity)
                .insert(InteractionDisabled)
                .remove::<TabIndex>();
            if context.focus.get() == Some(entity) {
                if let Some(cancel_entity) = cancel_entity {
                    context.focus.set(cancel_entity, FocusCause::Navigated);
                } else {
                    context.focus.clear();
                }
            }
        }
    }
}

fn request_summary(request: &HexLaunchRequest) -> String {
    let config = request.spec.config;
    let context = match request.context {
        LaunchContext::Local => "Local run",
        LaunchContext::Rematch => "Rematch",
        LaunchContext::Lan => "LAN match",
    };
    let role = if request.spectator {
        "Spectator"
    } else if request.rules == PlayRules::Ascent {
        match request.seat {
            PlaySeat::Architect => "Architect",
            PlaySeat::Observer => "Observer",
        }
    } else {
        "Explorer"
    };
    let bodies = match (request.rules, config.members_per_team == 1) {
        (PlayRules::Ascent, true) => "Observer",
        (PlayRules::Ascent, false) => "Observers",
        (PlayRules::Race, true) => "explorer",
        (PlayRules::Race, false) => "explorers",
    };
    let architects = if request.rules == PlayRules::Ascent {
        " + 1 Architect per team"
    } else {
        ""
    };
    format!(
        "{context} | {} | {role}\n{} team{} | {} {bodies} per team{architects}\n{} floors",
        request.rules.label(),
        config.teams,
        if config.teams == 1 { "" } else { "s" },
        config.members_per_team,
        config.wfc.levels
    )
}

fn error_label(error: &HexLoadingError) -> &'static str {
    use crate::hex_wfc::launch::HexLaunchError;
    match error {
        HexLoadingError::MissingRequest => {
            "No match setup is available. Go back to choose a match."
        }
        HexLoadingError::Preparation(HexLaunchError::CatalogLoad(_)) => {
            "The facility files could not be loaded. Check your game installation, then retry."
        }
        HexLoadingError::Preparation(HexLaunchError::ContentHashMismatch { .. }) => {
            "Your facility files differ from the host's. Use the same game version and files, then join again."
        }
        HexLoadingError::Preparation(HexLaunchError::ExactSeedRejected { .. }) => {
            "The host's facility could not be prepared here. Leave and ask the host to start a new match."
        }
        HexLoadingError::Preparation(HexLaunchError::NearbySeedsExhausted { .. }) => {
            "This facility could not be prepared. Go back and start a new match to try another layout."
        }
        HexLoadingError::PreparationPanicked(_) | HexLoadingError::WorkerLost(_) => {
            "Facility preparation stopped unexpectedly. Retry, or go back to choose another match."
        }
        HexLoadingError::LanLaunchUnavailable | HexLoadingError::LanLaunchWithdrawn => {
            "The host changed or cancelled this match. Leave and join the host again."
        }
        HexLoadingError::LanTransport(_) => {
            "Could not contact the host. Check your connection, then retry or leave."
        }
        HexLoadingError::LanServerSilent => {
            "The host stopped responding. Leave and find the host again."
        }
    }
}

fn next_action_label(phase: HexLoadingPhase, request: Option<&HexLaunchRequest>) -> &'static str {
    let lan = request.is_some_and(|request| request.context == LaunchContext::Lan);
    match phase {
        HexLoadingPhase::Failed if request.is_none() => "Go back and choose a match to start.",
        HexLoadingPhase::Failed if lan => "Retry this launch, or leave to find a host again.",
        HexLoadingPhase::Failed => "Retry this match, or go back to change your setup.",
        HexLoadingPhase::WaitingForPlayers => {
            "The match starts when everyone is ready. You can leave while waiting."
        }
        HexLoadingPhase::Ready => "Your match is ready. Entering now...",
        _ if lan => "You can leave while the host's facility is being prepared.",
        _ => "You can go back while the facility is being prepared.",
    }
}

const fn cancel_target(context: LaunchContext) -> GameState {
    match context {
        LaunchContext::Local => GameState::Play,
        LaunchContext::Rematch => GameState::Results,
        LaunchContext::Lan => GameState::LanBrowser,
    }
}

const fn cancel_label(context: LaunchContext) -> &'static str {
    match context {
        LaunchContext::Local => "Back to Play",
        LaunchContext::Rematch => "Back to Results",
        LaunchContext::Lan => "Leave LAN match",
    }
}

const fn status_label(phase: HexLoadingPhase) -> &'static str {
    match phase {
        HexLoadingPhase::AwaitingRequest => "Waiting for your match setup...",
        HexLoadingPhase::Preparing => "Preparing rooms and routes...",
        HexLoadingPhase::WaitingForPlayers => {
            "Facility ready. Waiting for other players and server start..."
        }
        HexLoadingPhase::Ready => "Facility ready. Entering the match...",
        HexLoadingPhase::Failed => "Facility preparation could not complete.",
        HexLoadingPhase::Cancelled => "Facility preparation cancelled.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_returns_to_the_screen_that_owns_the_launch() {
        assert_eq!(cancel_target(LaunchContext::Local), GameState::Play);
        assert_eq!(cancel_target(LaunchContext::Rematch), GameState::Results);
        assert_eq!(cancel_target(LaunchContext::Lan), GameState::LanBrowser);
    }

    #[test]
    fn loading_copy_never_claims_a_fake_percentage() {
        for phase in [
            HexLoadingPhase::AwaitingRequest,
            HexLoadingPhase::Preparing,
            HexLoadingPhase::WaitingForPlayers,
            HexLoadingPhase::Ready,
            HexLoadingPhase::Failed,
            HexLoadingPhase::Cancelled,
        ] {
            assert!(!status_label(phase).contains('%'));
        }
    }

    #[test]
    fn loading_summary_uses_finalized_roles_and_actual_body_counts() {
        use crate::hex_wfc::launch::{HexLaunchSpec, HexSeedPolicy};
        let mut sequence = HexLaunchRequestSequence::default();
        let mut request = sequence.issue(
            LaunchContext::Rematch,
            observed_core::PlayerId(0),
            false,
            false,
            HexLaunchSpec {
                requested_seed: 42,
                config: observed_match::hex_wfc::HexMatchConfig {
                    teams: 1,
                    members_per_team: 1,
                    ..default()
                },
                seed_policy: HexSeedPolicy::Nearby,
            },
            (PlayRules::Ascent, PlaySeat::Architect),
        );
        let summary = request_summary(&request);
        assert!(summary.contains("Rematch | Architect Ascent | Architect"));
        assert!(summary.contains("1 team | 1 Observer per team + 1 Architect per team"));
        request.spectator = true;
        assert!(request_summary(&request).contains("Spectator"));
        request.spectator = false;
        request.rules = PlayRules::Race;
        assert!(request_summary(&request).contains("Facility race | Explorer"));
        assert!(!request_summary(&request).contains("Architect"));
    }

    #[test]
    fn a_missing_request_offers_back_and_lan_errors_explain_reconnection() {
        assert_eq!(
            next_action_label(HexLoadingPhase::Failed, None),
            "Go back and choose a match to start."
        );
        assert!(error_label(&HexLoadingError::LanServerSilent).contains("Leave"));
        assert!(
            error_label(&HexLoadingError::Preparation(
                crate::hex_wfc::launch::HexLaunchError::ContentHashMismatch {
                    expected: [0; 32],
                    actual: [1; 32]
                }
            ))
            .contains("same game version and files")
        );
    }
}
