//! Versioned first-run help for the canonical continuous hex facility.
//!
//! The overlay is deliberately a small presentation adapter: control copy comes from
//! persisted bindings, semantic widgets own keyboard/pointer/controller activation,
//! and completion is persisted in [`crate::settings::UserPreferences`]. It never
//! reads physical input and it refuses to spawn outside [`GameState::HexWfc`].

use bevy::{ecs::system::SystemParam, prelude::*, ui::InteractionDisabled, ui_widgets::Activate};

use super::widgets::{
    self, FocusScope, FocusScopeId, UiInputCapture, WidgetId, WidgetLabel, WidgetSpec,
    activation_enabled,
};
use crate::GameState;
use crate::hex_wfc::HexOnboardingGate;
use crate::play_setup::{PlayPreset, PlayRules, PlaySeat, PlaySetupDraft};
use crate::settings::{OnboardingKind, Settings, key_name, save_settings};
use crate::view::theme::{ACCENT, BORDER, DIM, PANEL, TITLE, WARNING, text};

const SCOPE: FocusScopeId = FocusScopeId("hex_onboarding");
const NEXT: WidgetId = WidgetId::named("hex_onboarding.next");
const SKIP: WidgetId = WidgetId::named("hex_onboarding.skip");
const OVERLAY_PRIORITY: i16 = 300;
const CAPTURE_OWNER: &str = "hex_onboarding";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OnboardingBeat {
    pub(crate) title: &'static str,
    pub(crate) body: String,
}

/// Build the exact first-run copy from the active keyboard bindings. Controller
/// labels describe the fixed mapping in `screens::input::read_gamepad_match`.
pub(crate) fn onboarding_beats(settings: &Settings, kind: OnboardingKind) -> Vec<OnboardingBeat> {
    let keys = &settings.bindings;
    if kind == OnboardingKind::Spectator {
        return vec![
            OnboardingBeat { title: "WATCH THE BOT RUN", body: "You are following bots. The HUD names the followed Observer, their team, their floor and their current objective. F or D-pad left follows the next Observer in a stable order.".into() },
            OnboardingBeat { title: "CHOOSE YOUR VIEW", body: "O or controller north switches the overview. V or controller west looks through the Observer's eyes. R or D-pad right rotates the overview; [ and ] or D-pad down and up change its zoom.".into() },
            OnboardingBeat { title: "READ THEIR MAP", body: format!("{} or RB opens the followed team's discovered map. PgUp/PgDn or D-pad up/down browses known floors. H or controller west shows the reading guide. The spectator overview can show ground truth; the team map still respects discovery.", key_name(keys.tac_map)) },
            OnboardingBeat { title: "PAUSE AND RETURN", body: format!("{} or Start opens the match menu. Camera and follow controls wait while menus or help are open. Close help to return to the bot run.", key_name(keys.pause)) },
        ];
    }
    if kind == OnboardingKind::Rogue {
        return vec![
            OnboardingBeat { title: "YOUR SIDE HAS CHANGED", body: "Falling into true void corrupts an Observer. You now play for the Rogue AI from a Rogue desk. Your goal is to jail every remaining loyal Observer.".into() },
            OnboardingBeat { title: "PLAY FROM THE ROGUE HAND", body: "Pick a card with 1-5 or click it. Click the board to aim, then confirm with another click, Space, Enter, or PLAY. Q/E rotates. The board shows whether the rules will accept your play.".into() },
            OnboardingBeat { title: "DISRUPT THE CLIMB", body: "Use your hand's routes, sensors and Guardian directives against the remaining loyal Observers. The Rogue desk shows its own hand and view. You no longer answer your former team's help requests.".into() },
            OnboardingBeat { title: "KEEP PLAYING", body: format!("Use [ and ] or the floor buttons to switch floors. Keyboard, pointer and controller controls stay visible on the desk. {} or Start opens the match menu.", key_name(keys.pause)) },
        ];
    }
    if kind == OnboardingKind::Architect {
        return vec![
            OnboardingBeat { title: "YOUR TEAM NEEDS A ROUTE", body: "You have no body. Build and repair a route from the desk so every Observer on your team can reach the summit. Your local Observers are bots; on LAN your teammates drive their own bodies.".into() },
            OnboardingBeat { title: "PLAY A CARD", body: "Pick a card with 1-5 or click it. Click the board to aim; click again, Space, Enter, or PLAY confirms. Q/E rotates. Use [ and ] or the floor buttons to change floors. The desk shows why a play is refused.".into() },
            OnboardingBeat { title: "ANSWER YOUR OBSERVERS", body: "Watch your Observers and their requests. F, controller north, or ANSWER acknowledges the oldest unanswered request. Existing observation and protection can block your edits. A catch means prison; a fall adds corruption. Repair the route and help your team recover.".into() },
            OnboardingBeat { title: "THE SUMMIT", body: format!("Bring the whole team to the summit. R requests another card when your team's resources allow it. {} or Start opens the match menu. Keyboard, pointer, and controller desk controls remain visible on screen.", key_name(keys.pause)) },
        ];
    }
    if kind == OnboardingKind::Observer {
        return vec![
            OnboardingBeat {
                title: "CLIMB TOGETHER",
                body: format!(
                    "Bring every Observer to the summit; your Architect builds the route. {}/{}/{}/{} move; mouse or {}/{}/{}/{} looks. {} jumps, {} sprints. Controller: sticks move/look, south jumps, left trigger or stick click sprints.",
                    key_name(keys.move_forward),
                    key_name(keys.move_left),
                    key_name(keys.move_back),
                    key_name(keys.move_right),
                    key_name(keys.look_up),
                    key_name(keys.look_left),
                    key_name(keys.look_down),
                    key_name(keys.look_right),
                    key_name(keys.jump),
                    key_name(keys.sprint)
                ),
            },
            OnboardingBeat {
                title: "ASK FOR A ROUTE",
                body: format!(
                    "{} or D-pad left asks your Architect for help. The request describes your current need, such as a route or rescue. Your Architect is a bot in local Observer play; LAN teammates can claim the desk.",
                    key_name(keys.ask)
                ),
            },
            OnboardingBeat {
                title: "PROTECT YOUR CROSSING",
                body: format!(
                    "{} uses mechanisms. {} deploys an anchor lantern at the threshold you are looking at; {} recovers it. Controller: west uses, left bumper anchors, east recovers. Observation and protection constrain the Architect's edits.",
                    key_name(keys.interact),
                    key_name(keys.torch),
                    key_name(keys.recover_lantern)
                ),
            },
            OnboardingBeat {
                title: "RECOVER AND REGROUP",
                body: format!(
                    "A Guardian catch puts you in prison. Falls add corruption; recover and help your teammates continue. {} opens the survivor map; right trigger or Select does too. {} or Start opens the match menu. The goal is the summit, together.",
                    key_name(keys.tac_map),
                    key_name(keys.pause)
                ),
            },
        ];
    }
    vec![
        OnboardingBeat {
            title: "READ THE FACILITY",
            body: format!(
                "{}/{}/{}/{} move; mouse or {}/{}/{}/{} looks. On controller, use the left and right sticks. Open thresholds are physical crossings: rooms let you decide, while corridors commit you to traversal risk.",
                key_name(keys.move_forward),
                key_name(keys.move_left),
                key_name(keys.move_back),
                key_name(keys.move_right),
                key_name(keys.look_up),
                key_name(keys.look_left),
                key_name(keys.look_down),
                key_name(keys.look_right),
            ),
        },
        OnboardingBeat {
            title: "TRAVERSE",
            body: format!(
                "{} jumps and {} sprints. On controller, the south face button jumps; left trigger or left-stick click sprints. A fall reroutes you instead of ending the run.",
                key_name(keys.jump),
                key_name(keys.sprint),
            ),
        },
        OnboardingBeat {
            title: "COOPERATE AND ANCHOR",
            body: format!(
                "{} uses mechanisms and collects objectives; the west face button does the same on controller. {} or left bumper deploys an anchor lantern at the threshold you are looking at. {} or the east face button recovers it.",
                key_name(keys.interact),
                key_name(keys.torch),
                key_name(keys.recover_lantern),
            ),
        },
        OnboardingBeat {
            title: "FIND A WAY OUT",
            body: format!(
                "Collect the keystones shown on the HUD, synchronize a team station, then regroup at the exit. {} opens the survivor map; controller right trigger or Select does too. {} or Start opens the match menu.",
                key_name(keys.tac_map),
                key_name(keys.pause),
            ),
        },
    ]
}

#[derive(Component)]
pub(crate) struct OnboardingPanel;

#[derive(Component)]
pub(crate) struct OnboardingProgress;

#[derive(Component)]
pub(crate) struct OnboardingTitle;

#[derive(Component)]
pub(crate) struct OnboardingBody;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OnboardingAction {
    Next,
    Skip,
}

/// Presence is the canonical input adapter's modal seam: while this resource exists,
/// gameplay intent is neutral and the cursor belongs to the semantic overlay.
#[derive(Resource, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct OnboardingState {
    pub(crate) step: usize,
    pub(crate) kind: OnboardingKind,
}

/// A semantic pause-menu request to review the current rules and role again.
#[derive(Resource)]
pub(crate) struct RoleHelpRequest;

pub(crate) fn request_help(
    commands: &mut Commands,
    gate: &mut HexOnboardingGate,
    capture: &mut UiInputCapture,
) {
    gate.active = true;
    capture.capture_for_scope(CAPTURE_OWNER, SCOPE);
    commands.insert_resource(RoleHelpRequest);
}

type OnboardingBodyQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<OnboardingBody>,
        Without<OnboardingProgress>,
        Without<OnboardingTitle>,
    ),
>;

/// Spawn only for a fresh/revised onboarding in the canonical production match.
/// Keeping the state check here as well as in plugin scheduling prevents an accidental
/// future registration from reviving onboarding in the deprecated Match fixture.
#[derive(SystemParam)]
pub(crate) struct OnboardingSpawnContext<'w> {
    current: Res<'w, State<GameState>>,
    settings: Res<'w, Settings>,
    play_setup: Res<'w, PlaySetupDraft>,
    lan: Res<'w, crate::lan::LanRuntime>,
    gate: ResMut<'w, HexOnboardingGate>,
    capture: ResMut<'w, UiInputCapture>,
    requested: Option<Res<'w, RoleHelpRequest>>,
    existing: Option<Res<'w, OnboardingState>>,
    request: Option<Res<'w, crate::hex_wfc::loading::HexLaunchRequest>>,
    runtime: Option<Res<'w, crate::hex_wfc::sim::HexWfcRuntime>>,
    live_spectator: Option<Res<'w, crate::sim::state::SpectatorBot>>,
}

pub(crate) fn spawn(mut commands: Commands, context: OnboardingSpawnContext) {
    let OnboardingSpawnContext {
        current,
        settings,
        play_setup,
        lan,
        mut gate,
        mut capture,
        requested,
        existing,
        request,
        runtime,
        live_spectator,
    } = context;
    let replay = requested.is_some();
    commands.remove_resource::<RoleHelpRequest>();
    if existing.is_some() {
        return;
    }
    let networked = request.as_ref().map_or_else(
        || runtime.as_ref().is_some_and(|runtime| runtime.networked),
        |request| request.networked,
    );
    let spectator = request.as_ref().map_or_else(
        || {
            if runtime.is_some() {
                live_spectator.is_some()
            } else {
                play_setup.preset == PlayPreset::Spectate
            }
        },
        |request| request.spectator,
    );
    let corrupted = runtime.as_ref().is_some_and(|runtime| {
        runtime.ascent.as_ref().is_some_and(|ascent| {
            ascent
                .observer_for(runtime.local_player)
                .and_then(|id| ascent.rules().observers.get(&id))
                .is_some_and(|observer| {
                    observer.state == observed_match::ascent::sim::ObserverState::Corrupted
                })
        })
    });
    let kind = review_kind(
        help_kind(&play_setup, &lan, networked),
        spectator,
        corrupted,
    );
    if *current.get() != GameState::HexWfc
        || (!replay && !settings.needs_help(kind))
        || (!replay && spectator)
        || evidence_capture_active()
    {
        return;
    }

    let beats = onboarding_beats(&settings, kind);
    let first = &beats[0];
    gate.active = true;
    capture.capture_for_scope(CAPTURE_OWNER, SCOPE);
    commands.insert_resource(OnboardingState { step: 0, kind });
    commands
        .spawn((
            OnboardingPanel,
            DespawnOnExit(GameState::HexWfc),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(24)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.005, 0.008, 0.015, 0.82)),
            GlobalZIndex(200),
            Name::new("Canonical first-run onboarding"),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: px(720),
                    max_width: percent(94),
                    padding: UiRect::axes(px(42), px(34)),
                    border: UiRect::all(px(2)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(14),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(BORDER),
                widgets::focus_scope(FocusScope::overlay(
                    SCOPE,
                    NEXT,
                    SKIP,
                    OVERLAY_PRIORITY,
                )),
            ))
            .with_children(|panel| {
                panel.spawn((
                    OnboardingProgress,
                    text(progress_label(0, beats.len(), kind), 14.0, ACCENT),
                ));
                panel.spawn((OnboardingTitle, text(first.title, 34.0, TITLE)));
                panel.spawn((
                    OnboardingBody,
                    text(first.body.clone(), 18.0, TITLE),
                    TextLayout::justify(Justify::Center),
                    Node {
                        max_width: px(620),
                        ..default()
                    },
                ));
                panel.spawn(text(
                    "Unwatched, unprotected connections can change. Frame lights show anchor protection; observation is a separate rule.",
                    14.0,
                    DIM,
                ));
                if networked {
                    panel.spawn(text(
                        "LAN MATCH CONTINUES - the runner sends neutral input while help is open",
                        14.0,
                        WARNING,
                    ));
                }
                widgets::spawn_button(
                    panel,
                    WidgetSpec::enabled(NEXT, SCOPE, 0, next_label(0, beats.len(), kind))
                        .with_size(520.0, 50.0),
                    OnboardingAction::Next,
                );
                widgets::spawn_button(
                    panel,
                    WidgetSpec::enabled(SKIP, SCOPE, 1, if replay { "Close help" } else { "Skip help" })
                        .with_size(520.0, 46.0),
                    OnboardingAction::Skip,
                );
                panel.spawn(text(
                    "Enter / A continues | Esc / B skips | Review role help from the match menu",
                    13.0,
                    ACCENT,
                ));
            });
        });
}

/// Screen-local semantic activation observer. It advances one explicit step or records
/// the current revision as complete when the player finishes or skips.
#[allow(clippy::too_many_arguments)]
pub(crate) fn activate(
    activation: On<Activate>,
    actions: Query<&OnboardingAction>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut state: Option<ResMut<OnboardingState>>,
    mut gate: ResMut<HexOnboardingGate>,
    mut settings: ResMut<Settings>,
    panels: Query<Entity, With<OnboardingPanel>>,
    mut progress: Query<&mut Text, With<OnboardingProgress>>,
    mut titles: Query<&mut Text, (With<OnboardingTitle>, Without<OnboardingProgress>)>,
    mut bodies: OnboardingBodyQuery,
    mut next_labels: Query<(&OnboardingAction, &mut WidgetLabel)>,
    mut commands: Commands,
) {
    if !activation_enabled(&activation, &disabled) {
        return;
    }
    let Ok(action) = actions.get(activation.entity) else {
        return;
    };
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    let beats = onboarding_beats(&settings, state.kind);

    if *action == OnboardingAction::Skip || state.step + 1 >= beats.len() {
        finish(&mut commands, &mut gate, &mut settings, &panels, state.kind);
        return;
    }

    state.step += 1;
    let beat = &beats[state.step];
    if let Ok(mut label) = progress.single_mut() {
        **label = progress_label(state.step, beats.len(), state.kind);
    }
    if let Ok(mut label) = titles.single_mut() {
        **label = beat.title.to_string();
    }
    if let Ok(mut label) = bodies.single_mut() {
        **label = beat.body.clone();
    }
    for (action, mut label) in &mut next_labels {
        if *action == OnboardingAction::Next {
            label.0 = next_label(state.step, beats.len(), state.kind).to_string();
        }
    }
}

/// Remove the modal resource even when the match exits before the player responds.
/// An interrupted first run remains incomplete and is offered again next time.
pub(crate) fn cleanup(
    mut commands: Commands,
    mut gate: ResMut<HexOnboardingGate>,
    mut capture: ResMut<UiInputCapture>,
) {
    commands.remove_resource::<OnboardingState>();
    commands.remove_resource::<RoleHelpRequest>();
    gate.active = false;
    capture.release(CAPTURE_OWNER);
}

/// Keep the modal input capture through the exact key/button edge that dismissed
/// onboarding. Without this one-frame latch, Escape/East can also open Pause after
/// the observer removes `OnboardingState`, depending on cross-plugin system order.
pub(crate) fn release_capture_after_dismissal(
    onboarding: Option<Res<OnboardingState>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut capture: ResMut<UiInputCapture>,
) {
    if onboarding.is_some() {
        return;
    }
    let dismissal_edge =
        keyboard.any_just_pressed([KeyCode::Escape, KeyCode::Enter, KeyCode::Space])
            || gamepads.iter().any(|gamepad| {
                gamepad.just_pressed(GamepadButton::East)
                    || gamepad.just_pressed(GamepadButton::South)
            });
    if !dismissal_edge {
        capture.release(CAPTURE_OWNER);
    }
}

fn finish(
    commands: &mut Commands,
    gate: &mut HexOnboardingGate,
    settings: &mut Settings,
    panels: &Query<Entity, With<OnboardingPanel>>,
    kind: OnboardingKind,
) {
    settings.complete_help(kind);
    save_settings(settings);
    gate.active = false;
    commands.remove_resource::<OnboardingState>();
    for panel in panels {
        commands.entity(panel).despawn();
    }
}

fn help_kind(
    setup: &PlaySetupDraft,
    lan: &crate::lan::LanRuntime,
    networked: bool,
) -> OnboardingKind {
    let (ascent, architect) = if let Some(launch) = networked
        .then(|| lan.client.as_ref().and_then(|client| client.launch))
        .flatten()
    {
        (
            launch.ascent,
            lan.client
                .as_ref()
                .is_some_and(|client| client.is_architect()),
        )
    } else {
        (
            setup.rules == PlayRules::Ascent,
            setup.seat == PlaySeat::Architect,
        )
    };
    if !ascent {
        OnboardingKind::Race
    } else if architect {
        OnboardingKind::Architect
    } else {
        OnboardingKind::Observer
    }
}

fn evidence_capture_active() -> bool {
    std::env::vars_os().any(|(key, _)| {
        key.to_string_lossy()
            .starts_with("OBSERVED2_CAPTURE_HEX_WFC")
    })
}

fn review_kind(kind: OnboardingKind, spectator: bool, corrupted: bool) -> OnboardingKind {
    if spectator {
        OnboardingKind::Spectator
    } else if corrupted {
        OnboardingKind::Rogue
    } else {
        kind
    }
}

fn progress_label(step: usize, total: usize, kind: OnboardingKind) -> String {
    let role = match kind {
        OnboardingKind::Race => "FACILITY RACE HELP",
        OnboardingKind::Observer => "OBSERVER HELP",
        OnboardingKind::Architect => "ARCHITECT HELP",
        OnboardingKind::Spectator => "SPECTATOR HELP",
        OnboardingKind::Rogue => "ROGUE HELP",
    };
    format!("{role}  -  {} / {total}", step + 1)
}

fn next_label(step: usize, total: usize, kind: OnboardingKind) -> &'static str {
    if step + 1 < total {
        "Next"
    } else {
        match kind {
            OnboardingKind::Race => "Start exploring",
            OnboardingKind::Observer => "Start climbing",
            OnboardingKind::Architect => "Take the desk",
            OnboardingKind::Spectator => "Return to viewing",
            OnboardingKind::Rogue => "Return to the Rogue desk",
        }
    }
}

#[cfg(test)]
#[path = "onboarding_tests.rs"]
mod tests;
