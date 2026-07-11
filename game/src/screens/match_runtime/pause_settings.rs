//! The pause-menu route to Settings: an overlay panel nested in the Match's
//! [`PausePanel`](crate::view::components::PausePanel), not a `GameState` transition
//! (leaving `GameState::Match` despawns the whole session — see
//! `session::cleanup_match_resources` — so Settings must be reachable *without*
//! exiting Match while paused). Reuses [`super::super::settings::SettingsRow`] for the
//! row list/labels so there is exactly one definition of what a settings row is and
//! how it reads, shared between the standalone screen and this overlay.

use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::*;
use player_input::{RebindCapture, RebindCaptureEvent, RebindCaptureStatus};

use super::super::settings::SettingsRow;
use crate::settings::{Settings, binding_conflict_summary, key_name, save_settings};
use crate::sim::state::MatchPaused;
use crate::view::components::{PauseSettingsElement, PauseSettingsPanel};
use crate::view::theme::{ACCENT, DIM, WARNING, text};

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

/// The in-flight rebind capture for the pause overlay: the shared
/// [`player_input::RebindCapture`] state machine (the exact implementation
/// `control_lab` proved). Starting a rebind with Enter begins in the
/// waiting-for-release stage, so the activation press structurally cannot become the
/// binding; once armed, the next key pressed is captured (Escape cancels; pressing
/// Enter *again* after arming binds Enter deliberately). Same lifecycle rationale as
/// [`PauseSettingsOpen`]: not in `for_each_match_resource!` because it holds no
/// observable cross-match data — it is inert unless the match is paused, and
/// `pause_settings_capture_rebind` cancels it the moment the match unpauses.
#[derive(Resource, Default)]
pub(crate) struct PauseSettingsRebind(pub(crate) RebindCapture<crate::settings::BindingSlot>);

/// `O` toggles the panel while paused; closing it also cancels any in-flight rebind.
/// Suspended while a rebind capture is active so `O` can itself be captured as a
/// binding instead of closing the panel mid-capture.
pub(crate) fn toggle_pause_settings(
    keyboard: Res<ButtonInput<KeyCode>>,
    paused: Res<MatchPaused>,
    mut open: ResMut<PauseSettingsOpen>,
    mut rebind: ResMut<PauseSettingsRebind>,
    mut panel: Query<&mut Visibility, With<PauseSettingsPanel>>,
) {
    if !paused.0 || rebind.0.is_active() || !keyboard.just_pressed(KeyCode::KeyO) {
        return;
    }
    open.0 = !open.0;
    if !open.0 {
        rebind.0.cancel();
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
            let capture_prompt = match rebind.0.status() {
                Some(RebindCaptureStatus::WaitingForActivationRelease {
                    target,
                    activation_key,
                }) if matches!(row, SettingsRow::Binding(slot) if *slot == target) => Some(
                    format!("release {}, then press a key", key_name(activation_key)),
                ),
                Some(RebindCaptureStatus::Armed { target })
                    if matches!(row, SettingsRow::Binding(slot) if *slot == target) =>
                {
                    Some("press a key (Esc cancels)".to_string())
                }
                _ => None,
            };
            let label = match capture_prompt {
                Some(prompt) => format!("{} — {prompt}", row.label(&settings)),
                None => row.label(&settings),
            };
            p.spawn((
                PauseSettingsElement,
                text(label, 15.0, if selected { ACCENT } else { DIM }),
            ));
        }
        if let Some(warning) = binding_conflict_summary(&settings.bindings) {
            p.spawn((PauseSettingsElement, text(warning, 12.0, WARNING)));
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn pause_settings_navigate(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    paused: Res<MatchPaused>,
    open: Res<PauseSettingsOpen>,
    rebind: Res<PauseSettingsRebind>,
    mut cursor: ResMut<PauseSettingsCursor>,
    ui_assets: Res<crate::view::components::UiAssets>,
    settings: Res<Settings>,
    mut audio_director: ResMut<crate::screens::audio::AudioDirector>,
) {
    if !paused.0 || !open.0 || rebind.0.is_active() {
        return;
    }
    let count = SettingsRow::all().len();
    let old_val = cursor.0;
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
    if cursor.0 != old_val {
        crate::screens::audio::play_ui_sound(
            &mut commands,
            Some(&mut *audio_director),
            &ui_assets.hover,
            crate::view::components::MatchAudioCue::UiHover,
            &settings,
        );
    }
}

fn adjust_volume(value: &mut f32, delta: f32) {
    *value = (*value + delta).clamp(0.0, 1.0);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn pause_settings_adjust(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    paused: Res<MatchPaused>,
    open: Res<PauseSettingsOpen>,
    rebind: Res<PauseSettingsRebind>,
    cursor: Res<PauseSettingsCursor>,
    mut settings: ResMut<Settings>,
    ui_assets: Res<crate::view::components::UiAssets>,
    mut audio_director: ResMut<crate::screens::audio::AudioDirector>,
) {
    if !paused.0 || !open.0 || rebind.0.is_active() {
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
        SettingsRow::Binding(_) | SettingsRow::Back => return, // inert rows do not play click
    }
    crate::screens::audio::play_ui_sound(
        &mut commands,
        Some(&mut *audio_director),
        &ui_assets.click,
        crate::view::components::MatchAudioCue::UiClick,
        &settings,
    );
    save_settings(&settings);
}

/// Enter on a binding row begins a rebind capture *waiting for Enter's release* —
/// the press that opened the prompt structurally cannot become the binding (bug
/// backlog #1). Pressing Enter again once the capture is armed binds Enter
/// deliberately.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pause_settings_activate(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    paused: Res<MatchPaused>,
    open: Res<PauseSettingsOpen>,
    cursor: Res<PauseSettingsCursor>,
    mut rebind: ResMut<PauseSettingsRebind>,
    mut settings: ResMut<Settings>,
    ui_assets: Res<crate::view::components::UiAssets>,
    mut audio_director: ResMut<crate::screens::audio::AudioDirector>,
) {
    if !paused.0 || !open.0 || rebind.0.is_active() || !keyboard.just_pressed(KeyCode::Enter) {
        return;
    }
    let rows = SettingsRow::all();
    let Some(row) = rows.get(cursor.0).copied() else {
        return;
    };
    crate::screens::audio::play_ui_sound(
        &mut commands,
        Some(&mut *audio_director),
        &ui_assets.click,
        crate::view::components::MatchAudioCue::UiClick,
        &settings,
    );
    match row {
        SettingsRow::Binding(slot) => rebind.0.begin_waiting_for_release(slot, KeyCode::Enter),
        SettingsRow::HighContrast => {
            settings.high_contrast = !settings.high_contrast;
            save_settings(&settings);
        }
        _ => {}
    }
}

/// Drive the shared [`RebindCapture`] while paused: once armed, the next keyboard key
/// pressed becomes the slot's new binding (persisted immediately); Escape cancels.
/// Unpausing mid-capture cancels the capture so no stale armed state survives into
/// live play or a later pause.
pub(crate) fn pause_settings_capture_rebind(
    keyboard: Res<ButtonInput<KeyCode>>,
    paused: Res<MatchPaused>,
    mut rebind: ResMut<PauseSettingsRebind>,
    mut settings: ResMut<Settings>,
) {
    if !paused.0 {
        rebind.0.cancel();
        return;
    }
    if let Some(RebindCaptureEvent::Captured { target: slot, key }) =
        rebind.0.update(&keyboard, KeyCode::Escape)
    {
        slot.set(&mut settings.bindings, key);
        save_settings(&settings);
    }
}
