//! Canonical match overlays for the production hex facility.
//!
//! One typed resource owns mutually-exclusive gameplay chrome: ordinary play, the
//! survivor map, or one page of the pause flow.  The UI is rebuilt from that state
//! and uses the same semantic widget/focus layer as the frontend.  Simulation reads
//! only the pure policy exposed here; presentation never becomes authoritative.

use bevy::{
    ecs::system::SystemParam,
    input::gamepad::{GamepadAxis, GamepadButton},
    input_focus::InputFocus,
    prelude::*,
    ui::InteractionDisabled,
    ui_widgets::Activate,
};

use crate::GameState;
use crate::screens::settings::{self, SettingsRow};
use crate::screens::widgets::{
    self, FocusScope, FocusScopeId, WidgetId, WidgetLabel, WidgetSpec, activation_enabled,
};
use crate::settings::{Settings, save_settings};
use crate::view::theme::{ACCENT, DIM, TITLE, WARNING, panel, text};

use super::sim::HexWfcRuntime;

const ROOT_SCOPE: FocusScopeId = FocusScopeId("hex.pause.root");
const SETTINGS_SCOPE: FocusScopeId = FocusScopeId("hex.pause.settings");
const CONTROLS_SCOPE: FocusScopeId = FocusScopeId("hex.pause.controls");
const CONFIRM_SCOPE: FocusScopeId = FocusScopeId("hex.pause.confirm_leave");

const RESUME: WidgetId = WidgetId::named("hex.pause.resume");
const OPEN_SETTINGS: WidgetId = WidgetId::named("hex.pause.open_settings");
const OPEN_CONTROLS: WidgetId = WidgetId::named("hex.pause.open_controls");
const REQUEST_LEAVE: WidgetId = WidgetId::named("hex.pause.request_leave");
const SETTINGS_BACK: WidgetId = WidgetId::named("hex.pause.settings_back");
const CONTROLS_BACK: WidgetId = WidgetId::named("hex.pause.controls_back");
const CANCEL_LEAVE: WidgetId = WidgetId::named("hex.pause.cancel_leave");
const CONFIRM_LEAVE: WidgetId = WidgetId::named("hex.pause.confirm_leave");

/// A nested pause page.  Confirmation is a page, rather than a boolean attached to a
/// cursor, so Back has one unambiguous destination and leaving can only happen from
/// the explicit confirmation action.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum PausePage {
    #[default]
    Root,
    Settings,
    Controls,
    ConfirmLeave,
}

/// The complete modal state of the canonical match.
#[derive(Resource, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum MatchOverlayState {
    #[default]
    Playing,
    SurvivorMap,
    Pause(PausePage),
}

impl MatchOverlayState {
    pub(super) const fn captures_local_input(self) -> bool {
        !matches!(self, Self::Playing)
    }

    const fn semantic_back(self) -> Option<Self> {
        match self {
            Self::Playing => None,
            Self::SurvivorMap | Self::Pause(PausePage::Root) => Some(Self::Playing),
            Self::Pause(PausePage::Settings | PausePage::Controls | PausePage::ConfirmLeave) => {
                Some(Self::Pause(PausePage::Root))
            }
        }
    }
}

/// What the authoritative adapter may do for this overlay/network combination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SimulationPolicy {
    /// Offline pause is a real pause: no authoritative tick is produced.
    Stop,
    /// The match advances, but the local seat contributes an entirely neutral command.
    ContinueNeutral,
    /// The match advances with the sampled local command.
    ContinueLocal,
}

impl SimulationPolicy {
    pub(super) const fn sends_neutral_input(self) -> bool {
        matches!(self, Self::ContinueNeutral)
    }
}

pub(super) const fn simulation_policy(
    overlay: MatchOverlayState,
    networked: bool,
    onboarding_active: bool,
) -> SimulationPolicy {
    if onboarding_active {
        return if networked {
            SimulationPolicy::ContinueNeutral
        } else {
            SimulationPolicy::Stop
        };
    }
    match (overlay, networked) {
        (MatchOverlayState::Pause(_), false) => SimulationPolicy::Stop,
        (MatchOverlayState::Playing, _) => SimulationPolicy::ContinueLocal,
        (MatchOverlayState::SurvivorMap | MatchOverlayState::Pause(_), _) => {
            SimulationPolicy::ContinueNeutral
        }
    }
}

/// A sampled set of overlay requests.  Reduction is deliberately pure so key overlap
/// (for example Pause rebound to the map key) has a documented, tested priority.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct OverlayHotkeys {
    pub pause: bool,
    pub map: bool,
    pub back: bool,
}

pub(super) fn reduce_hotkeys(
    current: MatchOverlayState,
    hotkeys: OverlayHotkeys,
) -> MatchOverlayState {
    // Back owns nested pages.  When there is no back destination (ordinary play), a
    // default Escape binding must still be allowed to open Pause.
    if hotkeys.back
        && let Some(back) = current.semantic_back()
    {
        return back;
    }
    // Pause outranks the map when bindings overlap.
    if hotkeys.pause {
        return match current {
            MatchOverlayState::Pause(_) => MatchOverlayState::Playing,
            MatchOverlayState::Playing | MatchOverlayState::SurvivorMap => {
                MatchOverlayState::Pause(PausePage::Root)
            }
        };
    }
    if hotkeys.map {
        return match current {
            MatchOverlayState::Playing => MatchOverlayState::SurvivorMap,
            MatchOverlayState::SurvivorMap => MatchOverlayState::Playing,
            MatchOverlayState::Pause(_) => current,
        };
    }
    current
}

#[derive(Component)]
pub(super) struct OverlayRoot;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OverlayAction {
    Resume,
    OpenSettings,
    OpenControls,
    RequestLeave,
    ConfirmLeave,
    Back,
    AdjustSetting(SettingsRow),
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OverlaySettingRow(SettingsRow);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OverlayEffect {
    None,
    LeaveMatch,
}

fn reduce_action(state: &mut MatchOverlayState, action: OverlayAction) -> OverlayEffect {
    match action {
        OverlayAction::Resume => *state = MatchOverlayState::Playing,
        OverlayAction::OpenSettings if *state == MatchOverlayState::Pause(PausePage::Root) => {
            *state = MatchOverlayState::Pause(PausePage::Settings);
        }
        OverlayAction::OpenControls if *state == MatchOverlayState::Pause(PausePage::Root) => {
            *state = MatchOverlayState::Pause(PausePage::Controls);
        }
        OverlayAction::RequestLeave if *state == MatchOverlayState::Pause(PausePage::Root) => {
            *state = MatchOverlayState::Pause(PausePage::ConfirmLeave);
        }
        OverlayAction::ConfirmLeave
            if *state == MatchOverlayState::Pause(PausePage::ConfirmLeave) =>
        {
            return OverlayEffect::LeaveMatch;
        }
        OverlayAction::Back => {
            if let Some(back) = state.semantic_back() {
                *state = back;
            }
        }
        OverlayAction::AdjustSetting(_)
        | OverlayAction::OpenSettings
        | OverlayAction::OpenControls
        | OverlayAction::RequestLeave
        | OverlayAction::ConfirmLeave => {}
    }
    OverlayEffect::None
}

/// Reset app-lifetime overlay state whenever a new canonical match becomes active.
pub(super) fn reset(mut overlay: ResMut<MatchOverlayState>) {
    *overlay = MatchOverlayState::Playing;
}

/// Keep the simulation-owned map projection bit in lockstep with typed overlay state.
/// This only writes when the resource changed, preserving direct evidence-capture map
/// control on later frames.
pub(super) fn sync_runtime_map(
    overlay: Res<MatchOverlayState>,
    mut runtime: Option<ResMut<HexWfcRuntime>>,
) {
    if !overlay.is_changed() {
        return;
    }
    let Some(runtime) = runtime.as_deref_mut() else {
        return;
    };
    runtime.map_open = matches!(*overlay, MatchOverlayState::SurvivorMap);
    if runtime.map_open {
        runtime.map_level = runtime.local().cell.level;
    }
}

/// Rebuild only the active overlay page.  Focus order lives on typed widgets in the
/// shared frontend foundation; there is no page-local numeric cursor.
pub(super) fn rebuild(
    mut commands: Commands,
    overlay: Res<MatchOverlayState>,
    runtime: Option<Res<HexWfcRuntime>>,
    settings: Res<Settings>,
    roots: Query<Entity, With<OverlayRoot>>,
) {
    if !overlay.is_changed() {
        return;
    }
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    let MatchOverlayState::Pause(page) = *overlay else {
        return;
    };
    let networked = runtime.as_deref().is_some_and(|runtime| runtime.networked);
    commands
        .spawn((
            OverlayRoot,
            DespawnOnExit(GameState::HexWfc),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.005, 0.008, 0.015, 0.78)),
            GlobalZIndex(100),
            Name::new("Canonical match overlay"),
        ))
        .with_children(|root| match page {
            PausePage::Root => spawn_root_page(root, networked),
            PausePage::Settings => spawn_settings_page(root, &settings, networked),
            PausePage::Controls => spawn_controls_page(root, &settings, networked),
            PausePage::ConfirmLeave => spawn_confirmation_page(root, networked),
        });
}

fn spawn_root_page(root: &mut ChildSpawnerCommands, networked: bool) {
    root.spawn((
        panel(),
        widgets::focus_scope(FocusScope::overlay(ROOT_SCOPE, RESUME, RESUME, 100)),
    ))
    .with_children(|panel| {
        panel.spawn(text("PAUSED", 40.0, TITLE));
        spawn_network_notice(panel, networked);
        widgets::spawn_button(
            panel,
            WidgetSpec::enabled(RESUME, ROOT_SCOPE, 0, "Resume"),
            OverlayAction::Resume,
        );
        widgets::spawn_button(
            panel,
            WidgetSpec::enabled(OPEN_SETTINGS, ROOT_SCOPE, 1, "Settings"),
            OverlayAction::OpenSettings,
        );
        widgets::spawn_button(
            panel,
            WidgetSpec::enabled(OPEN_CONTROLS, ROOT_SCOPE, 2, "Controls"),
            OverlayAction::OpenControls,
        );
        widgets::spawn_button(
            panel,
            WidgetSpec::enabled(REQUEST_LEAVE, ROOT_SCOPE, 3, "Leave match..."),
            OverlayAction::RequestLeave,
        );
        panel.spawn(text(
            "Up/Down or Tab select | activate | Esc/B resumes",
            14.0,
            ACCENT,
        ));
    });
}

fn pause_setting_rows() -> Vec<SettingsRow> {
    SettingsRow::all()
        .into_iter()
        .filter(|row| !matches!(row, SettingsRow::Binding(_) | SettingsRow::Back))
        .collect()
}

fn spawn_settings_page(root: &mut ChildSpawnerCommands, settings: &Settings, networked: bool) {
    let rows = pause_setting_rows();
    let first = rows[0].widget_id();
    root.spawn((
        panel(),
        widgets::focus_scope(FocusScope::overlay(
            SETTINGS_SCOPE,
            first,
            SETTINGS_BACK,
            100,
        )),
    ))
    .with_children(|panel| {
        panel.spawn(text("PAUSE SETTINGS", 36.0, TITLE));
        spawn_network_notice(panel, networked);
        for (order, row) in rows.into_iter().enumerate() {
            widgets::spawn_button(
                panel,
                WidgetSpec::enabled(
                    row.widget_id(),
                    SETTINGS_SCOPE,
                    order as u16,
                    row.label(settings),
                )
                .with_size(600.0, 44.0),
                (OverlayAction::AdjustSetting(row), OverlaySettingRow(row)),
            );
        }
        widgets::spawn_button(
            panel,
            WidgetSpec::enabled(SETTINGS_BACK, SETTINGS_SCOPE, 100, "Back"),
            OverlayAction::Back,
        );
        panel.spawn(text(
            "Left/Right adjust | activate advances | Esc/B back",
            14.0,
            ACCENT,
        ));
    });
}

fn spawn_controls_page(root: &mut ChildSpawnerCommands, settings: &Settings, networked: bool) {
    root.spawn((
        panel(),
        widgets::focus_scope(FocusScope::overlay(
            CONTROLS_SCOPE,
            CONTROLS_BACK,
            CONTROLS_BACK,
            100,
        )),
    ))
    .with_children(|panel| {
        panel.spawn(text("CONTROLS", 36.0, TITLE));
        spawn_network_notice(panel, networked);
        for row in SettingsRow::all()
            .into_iter()
            .filter(|row| matches!(row, SettingsRow::Binding(_)))
        {
            panel.spawn(text(row.label(settings), 16.0, TITLE));
        }
        panel.spawn(text(
            "Bindings can be changed from Settings on the main menu.",
            14.0,
            DIM,
        ));
        widgets::spawn_button(
            panel,
            WidgetSpec::enabled(CONTROLS_BACK, CONTROLS_SCOPE, 0, "Back"),
            OverlayAction::Back,
        );
    });
}

fn spawn_confirmation_page(root: &mut ChildSpawnerCommands, networked: bool) {
    root.spawn((
        panel(),
        widgets::focus_scope(FocusScope::overlay(
            CONFIRM_SCOPE,
            CANCEL_LEAVE,
            CANCEL_LEAVE,
            200,
        )),
    ))
    .with_children(|panel| {
        panel.spawn(text("LEAVE MATCH?", 36.0, WARNING));
        panel.spawn(text(
            if networked {
                "You will disconnect from the LAN match. The other players continue."
            } else {
                "Current match progress will be abandoned."
            },
            17.0,
            TITLE,
        ));
        widgets::spawn_button(
            panel,
            WidgetSpec::enabled(CANCEL_LEAVE, CONFIRM_SCOPE, 0, "Cancel"),
            OverlayAction::Back,
        );
        widgets::spawn_button(
            panel,
            WidgetSpec::enabled(CONFIRM_LEAVE, CONFIRM_SCOPE, 1, "Leave match"),
            OverlayAction::ConfirmLeave,
        );
        panel.spawn(text("Esc/B cancels", 14.0, ACCENT));
    });
}

fn spawn_network_notice(parent: &mut ChildSpawnerCommands, networked: bool) {
    if networked {
        parent.spawn(text(
            "ONLINE MATCH CONTINUES - your runner sends neutral input",
            16.0,
            WARNING,
        ));
    }
}

/// Handle semantic widget activation.  The transport and state transition are only
/// touched after the confirm-page action passes through the pure reducer.
#[derive(SystemParam)]
pub(super) struct OverlayActivationContext<'w> {
    overlay: ResMut<'w, MatchOverlayState>,
    settings: ResMut<'w, Settings>,
    runtime: Option<Res<'w, HexWfcRuntime>>,
    lan: ResMut<'w, crate::lan::LanRuntime>,
    next: ResMut<'w, NextState<GameState>>,
}

pub(super) fn activate(
    activation: On<Activate>,
    actions: Query<&OverlayAction>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut context: OverlayActivationContext,
) {
    if !activation_enabled(&activation, &disabled) {
        return;
    }
    let Ok(action) = actions.get(activation.entity) else {
        return;
    };
    if let OverlayAction::AdjustSetting(row) = *action {
        if *context.overlay == MatchOverlayState::Pause(PausePage::Settings)
            && settings::adjust_row(row, 1.0, &mut context.settings)
        {
            save_settings(&context.settings);
        }
        return;
    }
    if reduce_action(&mut context.overlay, *action) == OverlayEffect::LeaveMatch {
        if context
            .runtime
            .as_deref()
            .is_some_and(|runtime| runtime.networked)
        {
            context.lan.leave();
        }
        *context.overlay = MatchOverlayState::Playing;
        context.next.set(GameState::MainMenu);
    }
}

/// Left/right adjustments share the standalone screen's pure row operation and use
/// the shared `InputFocus`; no second cursor or row index is introduced.
pub(super) fn adjust_focused(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    overlay: Res<MatchOverlayState>,
    focus: Res<InputFocus>,
    rows: Query<&OverlaySettingRow>,
    mut stick_latch: Local<i8>,
    mut settings: ResMut<Settings>,
) {
    if *overlay != MatchOverlayState::Pause(PausePage::Settings) {
        *stick_latch = 0;
        return;
    }
    let mut decrease = keyboard.just_pressed(KeyCode::ArrowLeft);
    let mut increase = keyboard.just_pressed(KeyCode::ArrowRight);
    let mut stick = 0;
    for gamepad in &gamepads {
        decrease |= gamepad.just_pressed(GamepadButton::DPadLeft);
        increase |= gamepad.just_pressed(GamepadButton::DPadRight);
        let horizontal = gamepad.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
        if horizontal > 0.55 {
            stick = 1;
        } else if horizontal < -0.55 {
            stick = -1;
        }
    }
    if stick == 0 {
        *stick_latch = 0;
    } else if stick != *stick_latch {
        increase |= stick > 0;
        decrease |= stick < 0;
        *stick_latch = stick;
    }
    let direction = match (decrease, increase) {
        (true, false) => -1.0,
        (false, true) => 1.0,
        _ => return,
    };
    let Some(entity) = focus.get() else {
        return;
    };
    let Ok(row) = rows.get(entity) else {
        return;
    };
    if settings::adjust_row(row.0, direction, &mut settings) {
        save_settings(&settings);
    }
}

pub(super) fn refresh_setting_labels(
    settings: Res<Settings>,
    mut rows: Query<(&OverlaySettingRow, &mut WidgetLabel)>,
) {
    if !settings.is_changed() {
        return;
    }
    for (row, mut label) in &mut rows {
        label.0 = row.0.label(&settings);
    }
}

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
