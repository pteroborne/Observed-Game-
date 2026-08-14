//! Production preferences screen built from semantic widgets.
//!
//! [`SettingsRow`] remains a pure shared row model for the deprecated Match regression
//! overlay. The canonical screen never reconstructs order from a Bevy query: each row
//! carries a stable widget identity and explicit order.

use bevy::{
    input::gamepad::{GamepadAxis, GamepadButton},
    input_focus::InputFocus,
    prelude::*,
    ui::InteractionDisabled,
    ui_widgets::Activate,
};
use player_input::{RebindCapture, RebindCaptureEvent, RebindCaptureStatus};

use super::widgets::{
    self, FocusScope, FocusScopeId, UiInputCapture, WidgetId, WidgetLabel, WidgetSpec,
    activation_enabled,
};
use crate::GameState;
use crate::settings::{BindingSlot, Settings, binding_conflict_summary, save_settings};
use crate::view::theme::{ACCENT, BORDER, PANEL, TITLE, WARNING, screen_root, text};

const VOLUME_STEP: f32 = 0.1;
const SENSITIVITY_STEP: f32 = 0.02;
const SENSITIVITY_MIN: f32 = 0.02;
const SENSITIVITY_MAX: f32 = 0.6;
const FOV_STEP_DEGREES: f32 = 5.0;
const FOV_MIN_DEGREES: f32 = 50.0;
const FOV_MAX_DEGREES: f32 = 80.0;
const CAPTURE_OWNER: &str = "settings.rebind";
const PREFERENCES_SCOPE: FocusScopeId = FocusScopeId("settings.preferences");
const BINDINGS_SCOPE: FocusScopeId = FocusScopeId("settings.bindings");
const OPEN_BINDINGS: WidgetId = WidgetId::named("settings.open_bindings");
const BINDINGS_BACK: WidgetId = WidgetId::named("settings.bindings.back");
const BINDING_COLUMNS: usize = 2;
const BINDING_GRID_WIDTH: f32 = 1040.0;
const BINDING_COLUMN_GAP: f32 = 12.0;
const BINDING_BUTTON_WIDTH: f32 = (BINDING_GRID_WIDTH
    - BINDING_COLUMN_GAP * (BINDING_COLUMNS as f32 - 1.0))
    / BINDING_COLUMNS as f32;

const PREFERENCE_ROWS: [SettingsRow; 6] = [
    SettingsRow::MasterVolume,
    SettingsRow::SfxVolume,
    SettingsRow::MusicVolume,
    SettingsRow::MouseSensitivity,
    SettingsRow::FieldOfView,
    SettingsRow::HighContrast,
];

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum SettingsRow {
    MasterVolume,
    SfxVolume,
    MusicVolume,
    MouseSensitivity,
    FieldOfView,
    HighContrast,
    Binding(BindingSlot),
    Back,
}

impl SettingsRow {
    pub(crate) fn all() -> Vec<Self> {
        let mut rows = vec![
            Self::MasterVolume,
            Self::SfxVolume,
            Self::MusicVolume,
            Self::MouseSensitivity,
            Self::FieldOfView,
            Self::HighContrast,
        ];
        rows.extend(BindingSlot::ALL.into_iter().map(Self::Binding));
        rows.push(Self::Back);
        rows
    }

    pub(crate) fn label(self, settings: &Settings) -> String {
        match self {
            Self::MasterVolume => {
                format!("Master volume: {:.0}%", settings.master_volume * 100.0)
            }
            Self::SfxVolume => format!("SFX volume: {:.0}%", settings.sfx_volume * 100.0),
            Self::MusicVolume => {
                format!("Music volume: {:.0}%", settings.music_volume * 100.0)
            }
            Self::MouseSensitivity => {
                format!("Mouse sensitivity: {:.2}", settings.mouse_sensitivity)
            }
            Self::FieldOfView => format!("Field of view: {:.0} deg", settings.fov_degrees),
            Self::HighContrast => format!(
                "High-contrast legend: {}",
                if settings.high_contrast { "ON" } else { "off" }
            ),
            Self::Binding(slot) => format!(
                "{}: {}",
                slot.label(),
                crate::settings::key_name(slot.get(&settings.bindings))
            ),
            Self::Back => "Back".to_string(),
        }
    }

    pub(crate) const fn widget_id(self) -> WidgetId {
        let key = match self {
            Self::MasterVolume => 0,
            Self::SfxVolume => 1,
            Self::MusicVolume => 2,
            Self::MouseSensitivity => 3,
            Self::FieldOfView => 4,
            Self::HighContrast => 5,
            Self::Binding(slot) => 10 + binding_key(slot),
            Self::Back => 100,
        };
        WidgetId::keyed("settings.row", key)
    }
}

const fn binding_key(slot: BindingSlot) -> u64 {
    match slot {
        BindingSlot::MoveLeft => 0,
        BindingSlot::MoveRight => 1,
        BindingSlot::MoveBack => 2,
        BindingSlot::MoveForward => 3,
        BindingSlot::LookLeft => 4,
        BindingSlot::LookRight => 5,
        BindingSlot::LookUp => 6,
        BindingSlot::LookDown => 7,
        BindingSlot::Jump => 8,
        BindingSlot::Sprint => 9,
        BindingSlot::Interact => 10,
        BindingSlot::Torch => 11,
        BindingSlot::RecoverLantern => 12,
        BindingSlot::TacMap => 13,
        BindingSlot::Pause => 14,
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SettingsRowText(pub(crate) SettingsRow);

#[derive(Resource, Default)]
pub(crate) struct SettingsRebind(
    pub(crate) RebindCapture<BindingSlot>,
    /// Keeps global UI activation suppressed until the capture key's frame ends.
    bool,
);

#[derive(Component)]
pub(crate) struct SettingsHint;

#[derive(Component)]
pub(crate) struct SettingsConflictWarning;

#[derive(Component, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum SettingsPage {
    #[default]
    Preferences,
    Bindings,
}

#[derive(Component)]
pub(crate) struct SettingsPageBody;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsPageAction {
    OpenBindings,
    BackToPreferences,
}

pub(crate) fn setup(
    mut commands: Commands,
    settings: Res<Settings>,
    mut rebind: ResMut<SettingsRebind>,
    mut capture: ResMut<UiInputCapture>,
) {
    rebind.0.cancel();
    rebind.1 = false;
    capture.release(CAPTURE_OWNER);
    commands
        .spawn((
            screen_root(GameState::Settings),
            widgets::focus_scope(FocusScope::screen(
                PREFERENCES_SCOPE,
                PREFERENCE_ROWS[0].widget_id(),
                SettingsRow::Back.widget_id(),
            )),
            SettingsPage::Preferences,
        ))
        .with_children(|root| {
            root.spawn(text("SETTINGS", 40.0, TITLE));
            spawn_preferences_page(root, &settings);
            root.spawn((
                SettingsConflictWarning,
                text(
                    binding_conflict_summary(&settings.bindings).unwrap_or_default(),
                    14.0,
                    WARNING,
                ),
            ));
            root.spawn((
                SettingsHint,
                text(
                    "Up/Down or Tab select | Left/Right adjust | activate to change | Esc/B back",
                    15.0,
                    ACCENT,
                ),
            ));
        });
}

/// Routes semantic widget activation at the Settings screen boundary. The queries
/// deliberately stay separate so row actions, page actions, disabled state, and the
/// state-scoped page root retain distinct component contracts.
#[allow(clippy::too_many_arguments)]
pub(crate) fn activate(
    activation: On<Activate>,
    rows: Query<&SettingsRowText>,
    page_actions: Query<&SettingsPageAction>,
    disabled: Query<(), With<InteractionDisabled>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<Settings>,
    mut rebind: ResMut<SettingsRebind>,
    mut capture: ResMut<UiInputCapture>,
    mut screens: Query<(Entity, &mut SettingsPage)>,
    bodies: Query<Entity, With<SettingsPageBody>>,
    mut commands: Commands,
    mut next: ResMut<NextState<GameState>>,
) {
    if !activation_enabled(&activation, &disabled) || rebind.0.is_active() || rebind.1 {
        return;
    }
    if let Ok(action) = page_actions.get(activation.entity) {
        let Ok((screen, mut page)) = screens.single_mut() else {
            return;
        };
        for body in &bodies {
            commands.entity(body).despawn();
        }
        match action {
            SettingsPageAction::OpenBindings => {
                *page = SettingsPage::Bindings;
                commands.entity(screen).insert(FocusScope::grid(
                    BINDINGS_SCOPE,
                    SettingsRow::Binding(BindingSlot::ALL[0]).widget_id(),
                    BINDINGS_BACK,
                    BINDING_COLUMNS as u16,
                ));
                commands.entity(screen).with_children(|root| {
                    spawn_bindings_page(root, &settings);
                });
            }
            SettingsPageAction::BackToPreferences => {
                *page = SettingsPage::Preferences;
                commands.entity(screen).insert(FocusScope::screen(
                    PREFERENCES_SCOPE,
                    PREFERENCE_ROWS[0].widget_id(),
                    SettingsRow::Back.widget_id(),
                ));
                commands.entity(screen).with_children(|root| {
                    spawn_preferences_page(root, &settings);
                });
            }
        }
        return;
    }
    let Ok(row) = rows.get(activation.entity) else {
        return;
    };
    match row.0 {
        SettingsRow::Binding(slot) => {
            let activation_key = [KeyCode::Enter, KeyCode::Space]
                .into_iter()
                .find(|key| keyboard.just_pressed(*key));
            if let Some(key) = activation_key {
                rebind.0.begin_waiting_for_release(slot, key);
            } else {
                rebind.0.begin_armed(slot);
            }
            capture.capture(CAPTURE_OWNER);
        }
        SettingsRow::Back => next.set(GameState::MainMenu),
        row => {
            if adjust_row(row, 1.0, &mut settings) {
                save_settings(&settings);
            }
        }
    }
}

pub(crate) fn adjust_focused(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    focus: Res<InputFocus>,
    rows: Query<&SettingsRowText>,
    rebind: Res<SettingsRebind>,
    mut stick_latch: Local<i8>,
    mut settings: ResMut<Settings>,
) {
    if rebind.0.is_active() || rebind.1 {
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
    if adjust_row(row.0, direction, &mut settings) {
        save_settings(&settings);
    }
}

pub(crate) fn capture_rebind(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut rebind: ResMut<SettingsRebind>,
    mut capture: ResMut<UiInputCapture>,
    mut settings: ResMut<Settings>,
) {
    if rebind.1 {
        if keyboard.get_just_pressed().next().is_none() {
            rebind.1 = false;
            capture.release(CAPTURE_OWNER);
        }
        return;
    }
    let Some(event) = rebind.0.update(&keyboard, KeyCode::Escape) else {
        return;
    };
    match event {
        RebindCaptureEvent::Captured { target, key } => {
            target.set(&mut settings.bindings, key);
            save_settings(&settings);
            rebind.1 = true;
        }
        RebindCaptureEvent::Cancelled { .. } => rebind.1 = true,
        RebindCaptureEvent::Armed { .. } => {}
    }
}

pub(crate) fn refresh_labels(
    settings: Res<Settings>,
    rebind: Res<SettingsRebind>,
    mut rows: Query<(&SettingsRowText, &mut WidgetLabel)>,
    mut warning: Query<&mut Text, With<SettingsConflictWarning>>,
    mut hint: Query<&mut Text, (With<SettingsHint>, Without<SettingsConflictWarning>)>,
) {
    if settings.is_changed() || rebind.is_changed() {
        for (row, mut label) in &mut rows {
            label.0 = match rebind.0.status() {
                Some(RebindCaptureStatus::WaitingForActivationRelease {
                    target,
                    activation_key,
                }) if row.0 == SettingsRow::Binding(target) => format!(
                    "{} - release {}, then press a key",
                    row.0.label(&settings),
                    crate::settings::key_name(activation_key)
                ),
                Some(RebindCaptureStatus::Armed { target })
                    if row.0 == SettingsRow::Binding(target) =>
                {
                    format!("{} - press a key (Esc cancels)", row.0.label(&settings))
                }
                _ => row.0.label(&settings),
            };
        }
        if let Ok(mut warning) = warning.single_mut() {
            **warning = binding_conflict_summary(&settings.bindings).unwrap_or_default();
        }
        if let Ok(mut hint) = hint.single_mut() {
            **hint = if rebind.0.is_active() {
                "Rebinding captures the next keyboard key | Esc cancels".to_string()
            } else {
                "Up/Down or Tab select | Left/Right adjust | activate to change | Esc/B back"
                    .to_string()
            };
        }
    }
}

pub(crate) fn cleanup(mut rebind: ResMut<SettingsRebind>, mut capture: ResMut<UiInputCapture>) {
    rebind.0.cancel();
    rebind.1 = false;
    capture.release(CAPTURE_OWNER);
}

fn spawn_preferences_page(root: &mut ChildSpawnerCommands, settings: &Settings) {
    root.spawn((SettingsPageBody, settings_panel(700.0)))
        .with_children(|panel| {
            panel.spawn(text("PREFERENCES", 18.0, ACCENT));
            for (order, row) in PREFERENCE_ROWS.into_iter().enumerate() {
                widgets::spawn_button(
                    panel,
                    WidgetSpec::enabled(
                        row.widget_id(),
                        PREFERENCES_SCOPE,
                        order as u16,
                        row.label(settings),
                    )
                    .with_size(620.0, 44.0),
                    SettingsRowText(row),
                );
            }
            widgets::spawn_button(
                panel,
                WidgetSpec::enabled(
                    OPEN_BINDINGS,
                    PREFERENCES_SCOPE,
                    PREFERENCE_ROWS.len() as u16,
                    "Controls and key bindings",
                )
                .with_size(620.0, 44.0),
                SettingsPageAction::OpenBindings,
            );
            widgets::spawn_button(
                panel,
                WidgetSpec::enabled(
                    SettingsRow::Back.widget_id(),
                    PREFERENCES_SCOPE,
                    PREFERENCE_ROWS.len() as u16 + 1,
                    "Back to main menu",
                )
                .with_size(620.0, 44.0),
                SettingsRowText(SettingsRow::Back),
            );
        });
}

fn spawn_bindings_page(root: &mut ChildSpawnerCommands, settings: &Settings) {
    root.spawn((SettingsPageBody, settings_panel(1100.0)))
        .with_children(|panel| {
            panel.spawn(text("CONTROLS", 18.0, ACCENT));
            panel
                .spawn(Node {
                    width: px(BINDING_GRID_WIDTH),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    column_gap: px(BINDING_COLUMN_GAP),
                    row_gap: px(2),
                    ..default()
                })
                .with_children(|grid| {
                    for (order, slot) in BindingSlot::ALL.into_iter().enumerate() {
                        let row = SettingsRow::Binding(slot);
                        widgets::spawn_button(
                            grid,
                            WidgetSpec::enabled(
                                row.widget_id(),
                                BINDINGS_SCOPE,
                                order as u16,
                                row.label(settings),
                            )
                            .with_size(BINDING_BUTTON_WIDTH, 44.0),
                            SettingsRowText(row),
                        );
                    }
                });
            widgets::spawn_button(
                panel,
                WidgetSpec::enabled(
                    BINDINGS_BACK,
                    BINDINGS_SCOPE,
                    BindingSlot::ALL.len() as u16,
                    "Back to preferences",
                )
                .with_size(500.0, 44.0),
                SettingsPageAction::BackToPreferences,
            );
        });
}

fn settings_panel(width: f32) -> impl Bundle {
    (
        Node {
            width: px(width),
            padding: UiRect::all(px(18)),
            border: UiRect::all(px(1)),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(4),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(BORDER),
    )
}

fn adjust_volume(value: &mut f32, delta: f32) {
    *value = (*value + delta).clamp(0.0, 1.0);
}

/// Apply one bounded preference adjustment.
///
/// The canonical in-match pause overlay calls this same pure helper so its labels,
/// increments, and clamp rules cannot drift from the standalone Settings screen.
pub(crate) fn adjust_row(row: SettingsRow, direction: f32, settings: &mut Settings) -> bool {
    match row {
        SettingsRow::MasterVolume => {
            adjust_volume(&mut settings.master_volume, direction * VOLUME_STEP)
        }
        SettingsRow::SfxVolume => adjust_volume(&mut settings.sfx_volume, direction * VOLUME_STEP),
        SettingsRow::MusicVolume => {
            adjust_volume(&mut settings.music_volume, direction * VOLUME_STEP)
        }
        SettingsRow::MouseSensitivity => {
            settings.mouse_sensitivity = (settings.mouse_sensitivity
                + direction * SENSITIVITY_STEP)
                .clamp(SENSITIVITY_MIN, SENSITIVITY_MAX);
        }
        SettingsRow::FieldOfView => {
            settings.fov_degrees = (settings.fov_degrees + direction * FOV_STEP_DEGREES)
                .clamp(FOV_MIN_DEGREES, FOV_MAX_DEGREES);
        }
        SettingsRow::HighContrast => settings.high_contrast = !settings.high_contrast,
        SettingsRow::Binding(_) | SettingsRow::Back => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_has_a_unique_stable_id_and_label() {
        let settings = Settings::default();
        let rows = SettingsRow::all();
        assert_eq!(rows.len(), 6 + BindingSlot::ALL.len() + 1);
        let ids = rows
            .iter()
            .map(|row| row.widget_id())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), rows.len());
        assert!(rows.into_iter().all(|row| !row.label(&settings).is_empty()));
    }

    #[test]
    fn adjustment_is_clamped_and_inert_for_binding_and_back_rows() {
        let mut settings = Settings::default();
        assert!(adjust_row(SettingsRow::MasterVolume, -20.0, &mut settings));
        assert_eq!(settings.master_volume, 0.0);
        assert!(!adjust_row(
            SettingsRow::Binding(BindingSlot::Jump),
            1.0,
            &mut settings
        ));
        assert!(!adjust_row(SettingsRow::Back, 1.0, &mut settings));
    }

    /// The Arc Q baseline is 1280×800, and the Controls page is the densest thing the
    /// front end draws. Assert the height it actually needs against that budget rather
    /// than a bare row count, so adding a binding fails here instead of silently
    /// clipping. Verified against
    /// `docs/evidence/arc_q/frontend_1280x800/04_settings_controls.png`.
    #[test]
    fn dense_settings_pages_fit_in_bounded_rows() {
        const BASELINE_HEIGHT: f32 = 800.0;
        /// Title, hint line, panel padding, section label, and the Back row.
        const PAGE_CHROME: f32 = 300.0;
        const BINDING_ROW_HEIGHT: f32 = 44.0 + 2.0 * 3.0 + 2.0;

        let preference_targets = PREFERENCE_ROWS.len() + 2;
        assert_eq!(preference_targets, 8);

        let binding_rows = BindingSlot::ALL.len().div_ceil(BINDING_COLUMNS);
        let needed = PAGE_CHROME + binding_rows as f32 * BINDING_ROW_HEIGHT;
        assert!(
            needed <= BASELINE_HEIGHT,
            "the Controls page needs {needed}px of the {BASELINE_HEIGHT}px baseline; \
             add a column or a page before adding more bindings"
        );

        assert!(BINDING_BUTTON_WIDTH * BINDING_COLUMNS as f32 <= 1280.0);
        assert_ne!(PREFERENCES_SCOPE, BINDINGS_SCOPE);
        assert_ne!(SettingsRow::Back.widget_id(), BINDINGS_BACK);
    }
}
