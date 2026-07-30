//! Player-facing preparation screen for the canonical hex facility.
//!
//! This module owns its own typed actions; the global menu dispatcher never needs a
//! loading-specific branch. Progress copy reports elapsed time and the actual coarse
//! state only, because the WFC solve does not expose a trustworthy percentage.

use bevy::{
    ecs::system::SystemParam,
    input_focus::{InputFocus, tab_navigation::TabIndex},
    prelude::*,
    ui::InteractionDisabled,
    ui_widgets::Activate,
};

use super::widgets::{self, FocusScope, FocusScopeId, WidgetId, WidgetSpec, activation_enabled};
use crate::{
    GameState,
    hex_wfc::loading::{
        HexLaunchRequest, HexLaunchRequestSequence, HexLoadingPhase, HexLoadingState,
        cancel_loading, retry_loading,
    },
    play_setup::LaunchContext,
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
}

pub(crate) fn setup(mut commands: Commands, request: Option<Res<HexLaunchRequest>>) {
    let summary = request.as_deref().map_or_else(
        || "No finalized launch request is available.".to_string(),
        request_summary,
    );
    let cancel_label = request
        .as_deref()
        .map_or("Back to Play", |request| cancel_label(request.context));

    commands
        .spawn(screen_root(GameState::Loading))
        .with_children(|root| {
            root.spawn(text("ASSEMBLING FACILITY", 42.0, TITLE));
            root.spawn(text(
                "The deterministic layout is being prepared off the main thread.",
                16.0,
                DIM,
            ));
            root.spawn(summary_panel()).with_children(|summary_panel| {
                summary_panel.spawn(text(summary, 16.0, ACCENT));
                summary_panel.spawn((
                    LoadingStatusText,
                    text(
                        "Solving connected rooms and traversal routes...",
                        18.0,
                        TITLE,
                    ),
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
                "No progress percentage is shown: solve time varies with the selected seed.",
                13.0,
                DIM,
            ));
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
            **text = format!("Error: {error}");
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
            context.focus.set(entity);
        } else {
            context
                .commands
                .entity(entity)
                .insert(InteractionDisabled)
                .remove::<TabIndex>();
            if context.focus.0 == Some(entity) {
                if let Some(cancel_entity) = cancel_entity {
                    context.focus.set(cancel_entity);
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
    format!(
        "{context} | {} teams x {} | {}x{}x{} cells | seed {} | request {}",
        config.teams,
        config.members_per_team,
        config.wfc.cols,
        config.wfc.rows,
        config.wfc.levels,
        request.spec.requested_seed,
        request.request_id.get(),
    )
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
        HexLoadingPhase::AwaitingRequest => "Waiting for a finalized launch request...",
        HexLoadingPhase::Preparing => "Solving connected rooms and traversal routes...",
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
}
