//! The Settings screen (`GameState::Settings`, reachable from the Main Menu) and the
//! shared row-drawing/navigation it lends to the in-Match pause overlay
//! ([`super::match_runtime::pause_settings`]).
//!
//! Rows are a single flat list: three volume sliders, mouse sensitivity, the
//! high-contrast accessibility toggle, then one row per rebindable action
//! ([`crate::settings::BindingSlot`]). Left/Right (or the gamepad stick/D-pad)
//! adjusts a slider/toggle in place; Enter/A on a binding row begins a rebind capture
//! through [`player_input::RebindCapture`] — the *same* state machine `control_lab`
//! proved, not a copy: it arms only once the activation press is released, then the
//! next key pressed becomes the binding (Escape cancels) — so one row list serves
//! both mouse/keyboard and gamepad navigation without a separate widget system.

use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;
use player_input::{RebindCapture, RebindCaptureEvent};

use super::input::{gamepad_back_pressed, gamepad_confirm_pressed, gamepad_menu_axis};
use crate::GameState;
use crate::settings::{BindingSlot, Settings, binding_conflict_summary, save_settings};
use crate::view::theme::{ACCENT, DIM, TITLE, WARNING, panel, screen_root, text};

const VOLUME_STEP: f32 = 0.1;
const SENSITIVITY_STEP: f32 = 0.02;
const SENSITIVITY_MIN: f32 = 0.02;
const SENSITIVITY_MAX: f32 = 0.6;

/// One editable row in the Settings screen, in a fixed order shared by cursor
/// navigation and rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsRow {
    MasterVolume,
    SfxVolume,
    MusicVolume,
    MouseSensitivity,
    HighContrast,
    Binding(BindingSlot),
    Back,
}

impl SettingsRow {
    pub(crate) fn all() -> Vec<SettingsRow> {
        let mut rows = vec![
            SettingsRow::MasterVolume,
            SettingsRow::SfxVolume,
            SettingsRow::MusicVolume,
            SettingsRow::MouseSensitivity,
            SettingsRow::HighContrast,
        ];
        rows.extend(BindingSlot::ALL.into_iter().map(SettingsRow::Binding));
        rows.push(SettingsRow::Back);
        rows
    }

    pub(crate) fn label(self, settings: &Settings) -> String {
        match self {
            SettingsRow::MasterVolume => {
                format!("Master volume: {:.0}%", settings.master_volume * 100.0)
            }
            SettingsRow::SfxVolume => format!("SFX volume: {:.0}%", settings.sfx_volume * 100.0),
            SettingsRow::MusicVolume => {
                format!("Music volume: {:.0}%", settings.music_volume * 100.0)
            }
            SettingsRow::MouseSensitivity => {
                format!("Mouse sensitivity: {:.2}", settings.mouse_sensitivity)
            }
            SettingsRow::HighContrast => format!(
                "High-contrast legend: {}",
                if settings.high_contrast { "ON" } else { "off" }
            ),
            SettingsRow::Binding(slot) => format!(
                "{}: {}",
                slot.label(),
                crate::settings::key_name(slot.get(&settings.bindings))
            ),
            SettingsRow::Back => "Back".to_string(),
        }
    }
}

#[derive(Component)]
pub(crate) struct SettingsRowText(pub(crate) SettingsRow);

#[derive(Resource, Default)]
pub(crate) struct SettingsCursor(pub(crate) usize);

/// While active, the shared [`player_input::RebindCapture`] state machine (the exact
/// implementation `control_lab` proved) owns the keyboard: the capture arms only once
/// the activation key (Enter/Space) is *released*, so the press that started the
/// rebind can never be captured; the next key pressed after arming becomes the
/// binding (Escape cancels; pressing the activation key again after arming binds it
/// deliberately).
#[derive(Resource, Default)]
pub(crate) struct SettingsRebind(pub(crate) RebindCapture<BindingSlot>);

#[derive(Component)]
pub(crate) struct SettingsHint;

/// The one-line binding-conflict warning under the row list (empty text while the
/// binding table is conflict-free).
#[derive(Component)]
pub(crate) struct SettingsConflictWarning;

pub(crate) fn setup_settings(
    mut commands: Commands,
    settings: Res<Settings>,
    mut cursor: ResMut<SettingsCursor>,
    mut rebind: ResMut<SettingsRebind>,
) {
    cursor.0 = 0;
    rebind.0.cancel();
    commands
        .spawn(screen_root(GameState::Settings))
        .with_children(|root| {
            root.spawn(text("SETTINGS", 40.0, TITLE));
            root.spawn(panel()).with_children(|p| {
                for row in SettingsRow::all() {
                    p.spawn((SettingsRowText(row), text(row.label(&settings), 18.0, DIM)));
                }
            });
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
                    "Up/Down select | Left/Right adjust | Enter rebinds a key | Esc back",
                    15.0,
                    ACCENT,
                ),
            ));
        });
}

/// Shared navigation: Up/Down (or gamepad) moves [`SettingsCursor`] across every row,
/// used by both the standalone screen and the pause overlay. Suspended while a rebind
/// capture is in progress (arrow keys during capture would otherwise both navigate and
/// almost-certainly not be the intended new binding).
#[allow(clippy::too_many_arguments)]
pub(crate) fn settings_navigate(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    rebind: Res<SettingsRebind>,
    mut cursor: ResMut<SettingsCursor>,
    rows: Query<&SettingsRowText>,
    ui_assets: Res<crate::view::components::UiAssets>,
    settings: Res<crate::settings::Settings>,
) {
    if rebind.0.is_active() {
        return;
    }
    let count = rows.iter().count();
    if count == 0 {
        return;
    }
    let old_val = cursor.0;
    if keyboard.just_pressed(KeyCode::ArrowDown) || keyboard.just_pressed(KeyCode::KeyS) {
        cursor.0 = (cursor.0 + 1) % count;
    }
    if keyboard.just_pressed(KeyCode::ArrowUp) || keyboard.just_pressed(KeyCode::KeyW) {
        cursor.0 = (cursor.0 + count - 1) % count;
    }
    let direction = gamepad_menu_axis(&gamepads);
    if direction < 0 {
        cursor.0 = (cursor.0 + 1) % count;
    } else if direction > 0 {
        cursor.0 = (cursor.0 + count - 1) % count;
    }

    if cursor.0 != old_val {
        crate::screens::audio::play_ui_sound(
            &mut commands,
            None,
            &ui_assets.hover,
            crate::view::components::MatchAudioCue::UiHover,
            &settings,
        );
    }
}

pub(crate) fn settings_highlight(
    cursor: Res<SettingsCursor>,
    rows: Query<(Entity, &SettingsRowText)>,
    order: Query<&SettingsRowText>,
    mut colors: Query<&mut TextColor>,
) {
    let ordered: Vec<SettingsRow> = order.iter().map(|r| r.0).collect();
    let Some(selected) = ordered.get(cursor.0).copied() else {
        return;
    };
    for (entity, row) in &rows {
        if let Ok(mut color) = colors.get_mut(entity) {
            color.0 = if row.0 == selected { ACCENT } else { DIM };
        }
    }
}

/// Refresh every row's label text from the live [`Settings`] each frame (cheap: a
/// couple dozen short strings), so a slider nudge or a completed rebind is visible
/// immediately.
pub(crate) fn settings_refresh_labels(
    settings: Res<Settings>,
    mut rows: Query<(&SettingsRowText, &mut Text)>,
    mut warning: Query<&mut Text, (With<SettingsConflictWarning>, Without<SettingsRowText>)>,
) {
    if !settings.is_changed() {
        return;
    }
    for (row, mut text) in &mut rows {
        **text = row.0.label(&settings);
    }
    if let Ok(mut text) = warning.single_mut() {
        **text = binding_conflict_summary(&settings.bindings).unwrap_or_default();
    }
}

fn adjust_volume(value: &mut f32, delta: f32) {
    *value = (*value + delta).clamp(0.0, 1.0);
}

/// Left/Right (or gamepad stick/D-pad) adjusts the row under the cursor: nudges a
/// slider, flips the accessibility toggle, or (for a binding row) is a no-op — bindings
/// only change via a rebind capture (Enter), never an axis nudge.
#[allow(clippy::too_many_arguments)]
pub(crate) fn settings_adjust(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    cursor: Res<SettingsCursor>,
    rebind: Res<SettingsRebind>,
    order: Query<&SettingsRowText>,
    mut settings: ResMut<Settings>,
    ui_assets: Res<crate::view::components::UiAssets>,
) {
    if rebind.0.is_active() {
        return;
    }
    let left = keyboard.just_pressed(KeyCode::ArrowLeft) || keyboard.just_pressed(KeyCode::KeyA);
    let right = keyboard.just_pressed(KeyCode::ArrowRight) || keyboard.just_pressed(KeyCode::KeyD);
    let mut gamepad_left = false;
    let mut gamepad_right = false;
    for gamepad in &gamepads {
        gamepad_left |= gamepad.just_pressed(bevy::input::gamepad::GamepadButton::DPadLeft);
        gamepad_right |= gamepad.just_pressed(bevy::input::gamepad::GamepadButton::DPadRight);
    }
    let (dec, inc) = (left || gamepad_left, right || gamepad_right);
    if !dec && !inc {
        return;
    }
    let ordered: Vec<SettingsRow> = order.iter().map(|r| r.0).collect();
    let Some(row) = ordered.get(cursor.0).copied() else {
        return;
    };
    let sign = if inc { 1.0 } else { -1.0 };
    match row {
        SettingsRow::MasterVolume => adjust_volume(&mut settings.master_volume, sign * VOLUME_STEP),
        SettingsRow::SfxVolume => adjust_volume(&mut settings.sfx_volume, sign * VOLUME_STEP),
        SettingsRow::MusicVolume => adjust_volume(&mut settings.music_volume, sign * VOLUME_STEP),
        SettingsRow::MouseSensitivity => {
            settings.mouse_sensitivity = (settings.mouse_sensitivity + sign * SENSITIVITY_STEP)
                .clamp(SENSITIVITY_MIN, SENSITIVITY_MAX);
        }
        SettingsRow::HighContrast => settings.high_contrast = !settings.high_contrast,
        SettingsRow::Binding(_) | SettingsRow::Back => return, // inert rows do not play click
    }
    crate::screens::audio::play_ui_sound(
        &mut commands,
        None,
        &ui_assets.click,
        crate::view::components::MatchAudioCue::UiClick,
        &settings,
    );
    save_settings(&settings);
}

/// Enter/A on the row under the cursor: a binding row begins a rebind capture; the
/// toggle row flips (mirrors `settings_adjust`'s toggle path, since a toggle has no
/// natural "left vs right" distinction worth requiring); every other row is inert
/// (sliders are adjusted with Left/Right, not activated).
///
/// A keyboard activation (Enter/Space) starts the capture *waiting for that key's
/// release*, so the press that opened the prompt structurally cannot become the
/// binding. A gamepad confirm has no keyboard key to swallow, so it arms immediately.
#[allow(clippy::too_many_arguments)]
pub(crate) fn settings_activate(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    cursor: Res<SettingsCursor>,
    order: Query<&SettingsRowText>,
    mut rebind: ResMut<SettingsRebind>,
    mut settings: ResMut<Settings>,
    ui_assets: Res<crate::view::components::UiAssets>,
) {
    if rebind.0.is_active() {
        return;
    }
    let activation_key = [KeyCode::Enter, KeyCode::Space]
        .into_iter()
        .find(|key| keyboard.just_pressed(*key));
    if activation_key.is_none() && !gamepad_confirm_pressed(&gamepads) {
        return;
    }
    let ordered: Vec<SettingsRow> = order.iter().map(|r| r.0).collect();
    let Some(row) = ordered.get(cursor.0).copied() else {
        return;
    };
    crate::screens::audio::play_ui_sound(
        &mut commands,
        None,
        &ui_assets.click,
        crate::view::components::MatchAudioCue::UiClick,
        &settings,
    );
    match row {
        SettingsRow::Binding(slot) => match activation_key {
            Some(key) => rebind.0.begin_waiting_for_release(slot, key),
            None => rebind.0.begin_armed(slot),
        },
        SettingsRow::HighContrast => {
            settings.high_contrast = !settings.high_contrast;
            save_settings(&settings);
        }
        _ => {}
    }
}

/// Drive the shared [`RebindCapture`]: once armed (activation key released), the next
/// keyboard key pressed becomes the slot's new binding; Escape cancels without
/// changing anything. Gamepad input is untouched by rebinding (README ruling), so this
/// only reads the keyboard.
pub(crate) fn settings_capture_rebind(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut rebind: ResMut<SettingsRebind>,
    mut settings: ResMut<Settings>,
) {
    if let Some(RebindCaptureEvent::Captured { target: slot, key }) =
        rebind.0.update(&keyboard, KeyCode::Escape)
    {
        slot.set(&mut settings.bindings, key);
        save_settings(&settings);
    }
}

/// Escape leaves the Settings screen back to the Main Menu — only when no rebind
/// capture is in flight. This runs *before* `settings_capture_rebind` in the
/// `.chain()`, so on the frame Escape cancels a capture the capture is still active
/// here and the screen stays put (the cancel and the back-out never share one press).
pub(crate) fn settings_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    rebind: Res<SettingsRebind>,
    mut next: ResMut<NextState<GameState>>,
) {
    if rebind.0.is_active() {
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) || gamepad_back_pressed(&gamepads) {
        next.set(GameState::MainMenu);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_has_a_stable_label_and_the_row_count_matches_bindings_plus_extras() {
        let settings = Settings::default();
        let rows = SettingsRow::all();
        // 5 non-binding rows (3 volumes + sensitivity + high-contrast) + one row per
        // binding slot + Back.
        assert_eq!(rows.len(), 5 + BindingSlot::ALL.len() + 1);
        for row in rows {
            assert!(!row.label(&settings).is_empty());
        }
    }
}
