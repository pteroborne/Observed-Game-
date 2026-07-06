//! The pause-menu route to Settings: an overlay panel nested in the Match's
//! [`PausePanel`](crate::view::components::PausePanel), not a `GameState` transition
//! (leaving `GameState::Match` despawns the whole session — see
//! `session::cleanup_match_resources` — so Settings must be reachable *without*
//! exiting Match while paused). Reuses [`super::super::settings::SettingsRow`] for the
//! row list/labels so there is exactly one definition of what a settings row is and
//! how it reads, shared between the standalone screen and this overlay.

use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::*;

use super::super::settings::SettingsRow;
use crate::settings::{Settings, save_settings};
use crate::sim::state::MatchPaused;
use crate::view::components::{PauseSettingsElement, PauseSettingsPanel};
use crate::view::theme::{ACCENT, DIM, text};

const VOLUME_STEP: f32 = 0.1;
const SENSITIVITY_STEP: f32 = 0.02;
const SENSITIVITY_MIN: f32 = 0.02;
const SENSITIVITY_MAX: f32 = 0.6;

/// Whether the in-match pause overlay's settings panel is expanded (toggled with `O`
/// while paused). Match-scoped in spirit (only meaningful while paused) but kept a
/// plain default-resource rather than added to `for_each_match_resource!`, since it
/// holds no data that would leak observably — it always starts collapsed on a fresh
/// Match and this is asserted by `pause_settings_panel_is_hidden_on_a_fresh_match`.
#[derive(Resource, Default)]
pub(crate) struct PauseSettingsOpen(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct PauseSettingsCursor(pub(crate) usize);

#[derive(Resource, Default)]
pub(crate) struct PauseSettingsRebind(pub(crate) Option<crate::settings::BindingSlot>);

/// `O` toggles the panel while paused; closing it also cancels any in-flight rebind.
pub(crate) fn toggle_pause_settings(
    keyboard: Res<ButtonInput<KeyCode>>,
    paused: Res<MatchPaused>,
    mut open: ResMut<PauseSettingsOpen>,
    mut rebind: ResMut<PauseSettingsRebind>,
    mut panel: Query<&mut Visibility, With<PauseSettingsPanel>>,
) {
    if !paused.0 || !keyboard.just_pressed(KeyCode::KeyO) {
        return;
    }
    open.0 = !open.0;
    if !open.0 {
        rebind.0 = None;
    }
    if let Ok(mut visibility) = panel.single_mut() {
        *visibility = if open.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Rebuild the panel's rows each frame while paused and open (same convention as
/// `hud::draw_tac_map`: despawn last frame's dynamic children, rebuild if shown).
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_pause_settings(
    paused: Res<MatchPaused>,
    open: Res<PauseSettingsOpen>,
    settings: Res<Settings>,
    cursor: Res<PauseSettingsCursor>,
    rebind: Res<PauseSettingsRebind>,
    panel: Query<Entity, With<PauseSettingsPanel>>,
    existing: Query<Entity, With<PauseSettingsElement>>,
    mut commands: Commands,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    if !paused.0 || !open.0 {
        return;
    }
    let Ok(panel) = panel.single() else {
        return;
    };
    let rows = SettingsRow::all();
    commands.entity(panel).with_children(|p| {
        for (index, row) in rows.iter().enumerate() {
            let selected = index == cursor.0;
            let is_capturing_this_row =
                selected && matches!(row, SettingsRow::Binding(slot) if rebind.0 == Some(*slot));
            let label = if is_capturing_this_row {
                format!("{} — press a key (Esc cancels)", row.label(&settings))
            } else {
                row.label(&settings)
            };
            p.spawn((
                PauseSettingsElement,
                text(label, 15.0, if selected { ACCENT } else { DIM }),
            ));
        }
        p.spawn((
            PauseSettingsElement,
            text(
                "Up/Down select | Left/Right adjust | Enter rebind | O close",
                12.0,
                ACCENT,
            ),
        ));
    });
}

pub(crate) fn pause_settings_navigate(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    paused: Res<MatchPaused>,
    open: Res<PauseSettingsOpen>,
    rebind: Res<PauseSettingsRebind>,
    mut cursor: ResMut<PauseSettingsCursor>,
) {
    if !paused.0 || !open.0 || rebind.0.is_some() {
        return;
    }
    let count = SettingsRow::all().len();
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        cursor.0 = (cursor.0 + 1) % count;
    }
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        cursor.0 = (cursor.0 + count - 1) % count;
    }
    for gamepad in &gamepads {
        if gamepad.just_pressed(GamepadButton::DPadDown) {
            cursor.0 = (cursor.0 + 1) % count;
        }
        if gamepad.just_pressed(GamepadButton::DPadUp) {
            cursor.0 = (cursor.0 + count - 1) % count;
        }
    }
}

fn adjust_volume(value: &mut f32, delta: f32) {
    *value = (*value + delta).clamp(0.0, 1.0);
}

pub(crate) fn pause_settings_adjust(
    keyboard: Res<ButtonInput<KeyCode>>,
    paused: Res<MatchPaused>,
    open: Res<PauseSettingsOpen>,
    rebind: Res<PauseSettingsRebind>,
    cursor: Res<PauseSettingsCursor>,
    mut settings: ResMut<Settings>,
) {
    if !paused.0 || !open.0 || rebind.0.is_some() {
        return;
    }
    let left = keyboard.just_pressed(KeyCode::ArrowLeft);
    let right = keyboard.just_pressed(KeyCode::ArrowRight);
    if !left && !right {
        return;
    }
    let rows = SettingsRow::all();
    let Some(row) = rows.get(cursor.0).copied() else {
        return;
    };
    let sign = if right { 1.0 } else { -1.0 };
    match row {
        SettingsRow::MasterVolume => adjust_volume(&mut settings.master_volume, sign * VOLUME_STEP),
        SettingsRow::SfxVolume => adjust_volume(&mut settings.sfx_volume, sign * VOLUME_STEP),
        SettingsRow::MusicVolume => adjust_volume(&mut settings.music_volume, sign * VOLUME_STEP),
        SettingsRow::MouseSensitivity => {
            settings.mouse_sensitivity = (settings.mouse_sensitivity + sign * SENSITIVITY_STEP)
                .clamp(SENSITIVITY_MIN, SENSITIVITY_MAX);
        }
        SettingsRow::HighContrast => settings.high_contrast = !settings.high_contrast,
        SettingsRow::Binding(_) | SettingsRow::Back => {}
    }
    save_settings(&settings);
}

pub(crate) fn pause_settings_activate(
    keyboard: Res<ButtonInput<KeyCode>>,
    paused: Res<MatchPaused>,
    open: Res<PauseSettingsOpen>,
    cursor: Res<PauseSettingsCursor>,
    mut rebind: ResMut<PauseSettingsRebind>,
    mut settings: ResMut<Settings>,
) {
    if !paused.0 || !open.0 || rebind.0.is_some() || !keyboard.just_pressed(KeyCode::Enter) {
        return;
    }
    let rows = SettingsRow::all();
    let Some(row) = rows.get(cursor.0).copied() else {
        return;
    };
    match row {
        SettingsRow::Binding(slot) => rebind.0 = Some(slot),
        SettingsRow::HighContrast => {
            settings.high_contrast = !settings.high_contrast;
            save_settings(&settings);
        }
        _ => {}
    }
}

pub(crate) fn pause_settings_capture_rebind(
    keyboard: Res<ButtonInput<KeyCode>>,
    paused: Res<MatchPaused>,
    mut rebind: ResMut<PauseSettingsRebind>,
    mut settings: ResMut<Settings>,
) {
    if !paused.0 {
        return;
    }
    let Some(slot) = rebind.0 else {
        return;
    };
    if keyboard.just_pressed(KeyCode::Escape) {
        rebind.0 = None;
        return;
    }
    let Some(key) = keyboard.get_just_pressed().next().copied() else {
        return;
    };
    slot.set(&mut settings.bindings, key);
    save_settings(&settings);
    rebind.0 = None;
}
