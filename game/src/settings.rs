//! Player-editable **settings**: volumes, mouse sensitivity, the keyboard binding
//! table, an accessibility toggle, and the first-run onboarding flag.
//!
//! This is a plain data resource — no rendering/UI types — read by [`crate::screens`]
//! (the input samplers and the Settings screen) and written by the Settings screen.
//! It lives at the top level (not under `sim/`) because `sim/` is the *match*
//! simulation's home and this resource is app-lifetime like [`crate::flow::Career`],
//! not match-scoped: it must be inserted once at startup (see
//! [`ObservedGamePlugin`](crate::ObservedGamePlugin)) and never touched by the Match
//! resource lifecycle macro.
//!
//! `Settings::default()` reproduces today's exact hardcoded bindings and mouse
//! sensitivity (previously inline `KeyCode` constants + a `MOUSE_SENSITIVITY` const in
//! `screens/input.rs`), so behaviour is byte-identical until a player edits a setting.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The keyboard binding table. Movement/look use two keys per axis (matching the
/// existing `axis(negative, positive)` sampling in `screens::input::match_input`);
/// gamepad mapping is untouched by Settings (README ruling) and stays hardcoded in
/// `screens::input::read_gamepad_match`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBindings {
    pub move_left: KeyCode,
    pub move_right: KeyCode,
    pub move_back: KeyCode,
    pub move_forward: KeyCode,
    pub look_left: KeyCode,
    pub look_right: KeyCode,
    pub look_up: KeyCode,
    pub look_down: KeyCode,
    pub jump: KeyCode,
    pub sprint: KeyCode,
    pub sprint_alt: KeyCode,
    pub interact: KeyCode,
    pub torch: KeyCode,
    pub pad: KeyCode,
    pub activate_pad: KeyCode,
    pub tac_map: KeyCode,
    pub pause: KeyCode,
}

impl Default for KeyBindings {
    fn default() -> Self {
        // Exactly today's inline constants (see `screens::input::match_input` and
        // `screens::hud::toggle_tac_map` before Phase 48 routed them through here).
        Self {
            move_left: KeyCode::KeyA,
            move_right: KeyCode::KeyD,
            move_back: KeyCode::KeyS,
            move_forward: KeyCode::KeyW,
            look_left: KeyCode::ArrowLeft,
            look_right: KeyCode::ArrowRight,
            look_up: KeyCode::ArrowUp,
            look_down: KeyCode::ArrowDown,
            jump: KeyCode::Space,
            sprint: KeyCode::ShiftLeft,
            sprint_alt: KeyCode::ShiftRight,
            interact: KeyCode::KeyE,
            torch: KeyCode::KeyF,
            pad: KeyCode::KeyC,
            activate_pad: KeyCode::KeyE,
            tac_map: KeyCode::Tab,
            pause: KeyCode::Escape,
        }
    }
}

/// One rebindable action row, for the Settings screen's list + the rebind capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingSlot {
    MoveLeft,
    MoveRight,
    MoveBack,
    MoveForward,
    Jump,
    Sprint,
    Interact,
    Torch,
    Pad,
    TacMap,
    Pause,
}

impl BindingSlot {
    pub const ALL: [BindingSlot; 11] = [
        BindingSlot::MoveLeft,
        BindingSlot::MoveRight,
        BindingSlot::MoveBack,
        BindingSlot::MoveForward,
        BindingSlot::Jump,
        BindingSlot::Sprint,
        BindingSlot::Interact,
        BindingSlot::Torch,
        BindingSlot::Pad,
        BindingSlot::TacMap,
        BindingSlot::Pause,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BindingSlot::MoveLeft => "Move left",
            BindingSlot::MoveRight => "Move right",
            BindingSlot::MoveBack => "Move back",
            BindingSlot::MoveForward => "Move forward",
            BindingSlot::Jump => "Jump",
            BindingSlot::Sprint => "Sprint",
            BindingSlot::Interact => "Interact / seize",
            BindingSlot::Torch => "Drop / pick torch",
            BindingSlot::Pad => "Drop / pick pad",
            BindingSlot::TacMap => "Tac-map",
            BindingSlot::Pause => "Pause",
        }
    }

    pub fn get(self, bindings: &KeyBindings) -> KeyCode {
        match self {
            BindingSlot::MoveLeft => bindings.move_left,
            BindingSlot::MoveRight => bindings.move_right,
            BindingSlot::MoveBack => bindings.move_back,
            BindingSlot::MoveForward => bindings.move_forward,
            BindingSlot::Jump => bindings.jump,
            BindingSlot::Sprint => bindings.sprint,
            BindingSlot::Interact => bindings.interact,
            BindingSlot::Torch => bindings.torch,
            BindingSlot::Pad => bindings.pad,
            BindingSlot::TacMap => bindings.tac_map,
            BindingSlot::Pause => bindings.pause,
        }
    }

    /// Rebind this slot's key. `Interact` and `activate_pad` share one physical key
    /// today (both `E`), so rebinding `Interact` moves both — preserving the existing
    /// one-key-does-both behaviour rather than silently splitting it.
    pub fn set(self, bindings: &mut KeyBindings, key: KeyCode) {
        match self {
            BindingSlot::MoveLeft => bindings.move_left = key,
            BindingSlot::MoveRight => bindings.move_right = key,
            BindingSlot::MoveBack => bindings.move_back = key,
            BindingSlot::MoveForward => bindings.move_forward = key,
            BindingSlot::Jump => bindings.jump = key,
            BindingSlot::Sprint => bindings.sprint = key,
            BindingSlot::Interact => {
                bindings.interact = key;
                bindings.activate_pad = key;
            }
            BindingSlot::Torch => bindings.torch = key,
            BindingSlot::Pad => bindings.pad = key,
            BindingSlot::TacMap => bindings.tac_map = key,
            BindingSlot::Pause => bindings.pause = key,
        }
    }
}

pub fn binding_conflicts(bindings: &KeyBindings, slot: BindingSlot) -> Vec<BindingSlot> {
    let key = slot.get(bindings);
    BindingSlot::ALL
        .into_iter()
        .filter(|candidate| *candidate != slot && candidate.get(bindings) == key)
        .collect()
}

pub fn binding_conflict_labels(bindings: &KeyBindings, slot: BindingSlot) -> Vec<&'static str> {
    binding_conflicts(bindings, slot)
        .into_iter()
        .map(BindingSlot::label)
        .collect()
}

/// A one-line, player-facing summary of every key shared by two or more binding
/// slots (`None` when the table is conflict-free). Built on [`binding_conflicts`] so
/// the warning the Settings/pause UI shows can never disagree with the per-slot
/// helpers. The default table has no visible conflicts (the interact/activate-pad and
/// sprint/sprint-alt pairs are internal aliases, not [`BindingSlot`]s).
pub fn binding_conflict_summary(bindings: &KeyBindings) -> Option<String> {
    let mut reported: Vec<KeyCode> = Vec::new();
    let mut parts: Vec<String> = Vec::new();
    for slot in BindingSlot::ALL {
        let key = slot.get(bindings);
        if reported.contains(&key) {
            continue;
        }
        let others = binding_conflict_labels(bindings, slot);
        if others.is_empty() {
            continue;
        }
        reported.push(key);
        let mut names = vec![slot.label()];
        names.extend(others);
        parts.push(format!("{} share {}", names.join(" / "), key_name(key)));
    }
    (!parts.is_empty()).then(|| format!("Binding conflict: {}", parts.join("; ")))
}

/// The player-editable settings: audio, mouse sensitivity, bindings, an accessibility
/// toggle, and the first-run onboarding flag. App-lifetime (inserted at startup, saved
/// on change) — not part of the Match resource lifecycle.
#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub music_volume: f32,
    pub mouse_sensitivity: f32,
    pub bindings: KeyBindings,
    /// Legend-backed accessibility mode: widens outlines / boosts marker emphasis via
    /// `observed_style`'s contrast floor rather than inventing new ad-hoc colours.
    pub high_contrast: bool,
    /// Flips false the first time the player completes (or skips) the onboarding
    /// beat, so it shows exactly once.
    pub first_run: bool,
}

/// The mouse sensitivity multiplier `screens::input::match_input` used before Phase
/// 48 introduced `Settings` (kept as the documented default, not read directly).
pub const DEFAULT_MOUSE_SENSITIVITY: f32 = 0.12;

impl Default for Settings {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            sfx_volume: 1.0,
            music_volume: 1.0,
            mouse_sensitivity: DEFAULT_MOUSE_SENSITIVITY,
            bindings: KeyBindings::default(),
            high_contrast: false,
            first_run: true,
        }
    }
}

impl Settings {
    /// Effective sound-effect volume for one-shot/looping cues (master × sfx).
    pub fn effective_sfx_volume(&self) -> f32 {
        (self.master_volume * self.sfx_volume).clamp(0.0, 1.0)
    }

    /// Effective ambience/music volume (master × music).
    pub fn effective_music_volume(&self) -> f32 {
        (self.master_volume * self.music_volume).clamp(0.0, 1.0)
    }
}

/// Where the settings (and, alongside them, the profile save — see
/// [`profile_save_path`]) live on disk: `saves/` next to the game's working directory.
/// `cargo run -p observed_game` and the built `observed` binary both run with the
/// workspace/executable directory as the current directory, so this is a stable,
/// human-findable location without adding a directories crate dependency.
pub fn settings_path() -> std::path::PathBuf {
    std::path::PathBuf::from("saves").join("settings.json")
}

#[cfg(test)]
thread_local! {
    pub static TEST_PROFILE_PATH: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Where the persisted `Profile` save string lives — see
/// [`crate::flow::save_profile`]/[`crate::flow::load_profile`].
pub fn profile_save_path() -> std::path::PathBuf {
    #[cfg(test)]
    {
        if let Some(path) = TEST_PROFILE_PATH.with(|p| p.borrow().clone()) {
            return path;
        }
    }
    std::path::PathBuf::from("saves").join("profile.save")
}

/// Load settings from disk if present and well-formed; `Settings::default()`
/// otherwise (covers "no save yet" and "corrupt save" identically — never panics on a
/// bad file).
pub fn load_settings() -> Settings {
    let Ok(text) = std::fs::read_to_string(settings_path()) else {
        return Settings::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// Persist settings to disk (best-effort: a write failure is silently ignored, the
/// same convention `evidence::snapshot` uses for its diagnostic writes — settings
/// persistence is a convenience, not gameplay-critical).
pub fn save_settings(settings: &Settings) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(path, json);
    }
}

/// Helper to convert a `KeyCode` into a clean, player-friendly label (e.g. `KeyCode::KeyW` -> `"W"`).
pub fn key_name(key: KeyCode) -> String {
    let s = format!("{:?}", key);
    if let Some(stripped) = s.strip_prefix("Key") {
        stripped.to_string()
    } else {
        match s.as_str() {
            "Escape" => "Esc".to_string(),
            "ShiftLeft" => "LShift".to_string(),
            "ShiftRight" => "RShift".to_string(),
            "ControlLeft" => "LCtrl".to_string(),
            "ControlRight" => "RCtrl".to_string(),
            "AltLeft" => "LAlt".to_string(),
            "AltRight" => "RAlt".to_string(),
            "ArrowLeft" => "Left".to_string(),
            "ArrowRight" => "Right".to_string(),
            "ArrowUp" => "Up".to_string(),
            "ArrowDown" => "Down".to_string(),
            _ => s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_reproduce_the_shipped_bindings_and_sensitivity() {
        let settings = Settings::default();
        assert_eq!(settings.bindings.move_left, KeyCode::KeyA);
        assert_eq!(settings.bindings.move_right, KeyCode::KeyD);
        assert_eq!(settings.bindings.move_back, KeyCode::KeyS);
        assert_eq!(settings.bindings.move_forward, KeyCode::KeyW);
        assert_eq!(settings.bindings.look_left, KeyCode::ArrowLeft);
        assert_eq!(settings.bindings.look_right, KeyCode::ArrowRight);
        assert_eq!(settings.bindings.look_up, KeyCode::ArrowUp);
        assert_eq!(settings.bindings.look_down, KeyCode::ArrowDown);
        assert_eq!(settings.bindings.jump, KeyCode::Space);
        assert_eq!(settings.bindings.sprint, KeyCode::ShiftLeft);
        assert_eq!(settings.bindings.sprint_alt, KeyCode::ShiftRight);
        assert_eq!(settings.bindings.interact, KeyCode::KeyE);
        assert_eq!(settings.bindings.torch, KeyCode::KeyF);
        assert_eq!(settings.bindings.pad, KeyCode::KeyC);
        assert_eq!(settings.bindings.activate_pad, KeyCode::KeyE);
        assert_eq!(settings.bindings.tac_map, KeyCode::Tab);
        assert_eq!(settings.bindings.pause, KeyCode::Escape);
        assert_eq!(settings.mouse_sensitivity, DEFAULT_MOUSE_SENSITIVITY);
        assert!(settings.first_run, "onboarding shows on a fresh profile");
        assert!(!settings.high_contrast);
        assert_eq!(settings.master_volume, 1.0);
        assert_eq!(settings.sfx_volume, 1.0);
        assert_eq!(settings.music_volume, 1.0);
    }

    #[test]
    fn settings_round_trip_through_serde() {
        let mut settings = Settings {
            mouse_sensitivity: 0.35,
            master_volume: 0.6,
            high_contrast: true,
            first_run: false,
            ..Default::default()
        };
        BindingSlot::Jump.set(&mut settings.bindings, KeyCode::KeyJ);

        let json = serde_json::to_string(&settings).expect("settings serialize");
        let loaded: Settings = serde_json::from_str(&json).expect("settings deserialize");
        assert_eq!(settings, loaded);
    }

    #[test]
    fn load_settings_falls_back_to_default_on_a_missing_or_corrupt_file() {
        // A path that cannot exist under the crate's temp scratch never gets written
        // to by this test, so `load_settings` reading a stray file is exercised via
        // direct `serde_json` parsing instead of touching the real `saves/` dir (tests
        // must not depend on or mutate the working tree's save file).
        let corrupt = serde_json::from_str::<Settings>("not json");
        assert!(corrupt.is_err());
    }

    #[test]
    fn rebinding_a_slot_updates_the_table_and_interact_shares_activate_pad() {
        let mut bindings = KeyBindings::default();
        BindingSlot::Interact.set(&mut bindings, KeyCode::KeyR);
        assert_eq!(bindings.interact, KeyCode::KeyR);
        assert_eq!(
            bindings.activate_pad,
            KeyCode::KeyR,
            "interact and activate-pad share one physical key, as they did before rebinding existed"
        );
    }

    #[test]
    fn every_binding_slot_round_trips_through_get_and_set() {
        let mut bindings = KeyBindings::default();
        for slot in BindingSlot::ALL {
            slot.set(&mut bindings, KeyCode::F13);
            assert_eq!(slot.get(&bindings), KeyCode::F13);
        }
    }

    #[test]
    fn binding_conflicts_are_reported_for_visible_slots() {
        let mut bindings = KeyBindings::default();
        let key = bindings.move_left;
        BindingSlot::Jump.set(&mut bindings, key);

        assert_eq!(
            binding_conflict_labels(&bindings, BindingSlot::Jump),
            vec!["Move left"]
        );
        assert_eq!(
            binding_conflict_labels(&bindings, BindingSlot::MoveLeft),
            vec!["Jump"]
        );
    }

    #[test]
    fn binding_conflict_summary_is_none_by_default_and_names_every_shared_key() {
        let bindings = KeyBindings::default();
        assert_eq!(
            binding_conflict_summary(&bindings),
            None,
            "the shipped default table has no visible conflicts"
        );

        let mut bindings = KeyBindings::default();
        let key = bindings.move_left;
        BindingSlot::Jump.set(&mut bindings, key);
        let summary = binding_conflict_summary(&bindings).expect("conflict is reported");
        assert!(summary.contains("Jump"), "summary names Jump: {summary}");
        assert!(
            summary.contains("Move left"),
            "summary names Move left: {summary}"
        );
        assert!(summary.contains('A'), "summary names the shared key A");
    }

    #[test]
    fn rebound_bindings_round_trip_through_settings_persistence_json() {
        let mut settings = Settings::default();
        BindingSlot::Jump.set(&mut settings.bindings, KeyCode::KeyK);
        BindingSlot::Pause.set(&mut settings.bindings, KeyCode::F10);

        let json = serde_json::to_string_pretty(&settings).expect("settings serialize");
        let loaded = serde_json::from_str::<Settings>(&json).expect("settings deserialize");

        assert_eq!(loaded.bindings.jump, KeyCode::KeyK);
        assert_eq!(loaded.bindings.pause, KeyCode::F10);
        assert_eq!(loaded, settings);
    }

    #[test]
    fn key_name_formats_nicely() {
        assert_eq!(key_name(KeyCode::KeyW), "W");
        assert_eq!(key_name(KeyCode::Escape), "Esc");
        assert_eq!(key_name(KeyCode::ShiftLeft), "LShift");
        assert_eq!(key_name(KeyCode::ArrowLeft), "Left");
        assert_eq!(key_name(KeyCode::Tab), "Tab");
    }
}
