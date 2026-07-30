//! Player-editable **preferences**: volumes, mouse sensitivity, field of view,
//! keyboard bindings, accessibility, and onboarding state. Match roster/rule choices
//! deliberately live in [`crate::play_setup`], not here.
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
#[serde(default)]
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
    pub recover_lantern: KeyCode,
    /// Retained for the demoted square-WFC and isolated-Place regression fixtures;
    /// canonical Controls intentionally does not advertise it.
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
            recover_lantern: KeyCode::KeyR,
            pad: KeyCode::KeyC,
            activate_pad: KeyCode::KeyE,
            tac_map: KeyCode::Tab,
            pause: KeyCode::Escape,
        }
    }
}

/// One rebindable action row, for the Settings screen's list + the rebind capture.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingSlot {
    MoveLeft,
    MoveRight,
    MoveBack,
    MoveForward,
    LookLeft,
    LookRight,
    LookUp,
    LookDown,
    Jump,
    Sprint,
    Interact,
    Torch,
    RecoverLantern,
    TacMap,
    Pause,
}

impl BindingSlot {
    pub const ALL: [BindingSlot; 15] = [
        BindingSlot::MoveLeft,
        BindingSlot::MoveRight,
        BindingSlot::MoveBack,
        BindingSlot::MoveForward,
        BindingSlot::LookLeft,
        BindingSlot::LookRight,
        BindingSlot::LookUp,
        BindingSlot::LookDown,
        BindingSlot::Jump,
        BindingSlot::Sprint,
        BindingSlot::Interact,
        BindingSlot::Torch,
        BindingSlot::RecoverLantern,
        BindingSlot::TacMap,
        BindingSlot::Pause,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BindingSlot::MoveLeft => "Move left",
            BindingSlot::MoveRight => "Move right",
            BindingSlot::MoveBack => "Move back",
            BindingSlot::MoveForward => "Move forward",
            BindingSlot::LookLeft => "Look left",
            BindingSlot::LookRight => "Look right",
            BindingSlot::LookUp => "Look up",
            BindingSlot::LookDown => "Look down",
            BindingSlot::Jump => "Jump",
            BindingSlot::Sprint => "Sprint",
            BindingSlot::Interact => "Interact / operate",
            BindingSlot::Torch => "Deploy anchor lantern",
            BindingSlot::RecoverLantern => "Recover anchor lantern",
            BindingSlot::TacMap => "Survivor map",
            BindingSlot::Pause => "Pause",
        }
    }

    pub fn get(self, bindings: &KeyBindings) -> KeyCode {
        match self {
            BindingSlot::MoveLeft => bindings.move_left,
            BindingSlot::MoveRight => bindings.move_right,
            BindingSlot::MoveBack => bindings.move_back,
            BindingSlot::MoveForward => bindings.move_forward,
            BindingSlot::LookLeft => bindings.look_left,
            BindingSlot::LookRight => bindings.look_right,
            BindingSlot::LookUp => bindings.look_up,
            BindingSlot::LookDown => bindings.look_down,
            BindingSlot::Jump => bindings.jump,
            BindingSlot::Sprint => bindings.sprint,
            BindingSlot::Interact => bindings.interact,
            BindingSlot::Torch => bindings.torch,
            BindingSlot::RecoverLantern => bindings.recover_lantern,
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
            BindingSlot::LookLeft => bindings.look_left = key,
            BindingSlot::LookRight => bindings.look_right = key,
            BindingSlot::LookUp => bindings.look_up = key,
            BindingSlot::LookDown => bindings.look_down = key,
            BindingSlot::Jump => bindings.jump = key,
            BindingSlot::Sprint => bindings.sprint = key,
            BindingSlot::Interact => {
                bindings.interact = key;
                bindings.activate_pad = key;
            }
            BindingSlot::Torch => bindings.torch = key,
            BindingSlot::RecoverLantern => bindings.recover_lantern = key,
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

/// Onboarding copy and interaction change independently from the rest of the save
/// format. Incrementing this version offers revised help once to players who completed
/// an older version without treating their whole preferences file as new.
pub const CURRENT_ONBOARDING_VERSION: u32 = 1;

/// The player-editable settings: audio, mouse sensitivity, bindings, accessibility,
/// and versioned onboarding completion. App-lifetime (inserted at startup, saved on
/// change) — not part of the match resource lifecycle.
#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserPreferences {
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub music_volume: f32,
    pub mouse_sensitivity: f32,
    /// Vertical field of view used by the canonical first-person Hex camera.
    /// A moderately wide default lets large rooms read as volumes without the
    /// distortion of an extreme action-game lens.
    pub fov_degrees: f32,
    pub bindings: KeyBindings,
    /// Legend-backed accessibility mode: widens outlines / boosts marker emphasis via
    /// `observed_style`'s contrast floor rather than inventing new ad-hoc colours.
    pub high_contrast: bool,
    /// Latest onboarding revision the player completed or skipped. Missing in older
    /// saves by design; serde supplies zero and [`Self::normalized`] migrates the
    /// legacy `first_run` value without surprising established players.
    pub completed_onboarding_version: u32,
    /// Legacy Phase 48 migration signal. New code uses
    /// [`Self::needs_onboarding`] and [`Self::complete_onboarding`]; retaining this
    /// field lets an older `first_run: false` save migrate as already complete.
    pub first_run: bool,
}

/// Compatibility name retained while call sites adopt the clearer domain term.
/// This is intentionally an alias, not a second Bevy resource.
pub type Settings = UserPreferences;

/// The mouse sensitivity multiplier `screens::input::match_input` used before Phase
/// 48 introduced `Settings` (kept as the documented default, not read directly).
pub const DEFAULT_MOUSE_SENSITIVITY: f32 = 0.12;

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            sfx_volume: 1.0,
            music_volume: 1.0,
            mouse_sensitivity: DEFAULT_MOUSE_SENSITIVITY,
            fov_degrees: 60.0,
            bindings: KeyBindings::default(),
            high_contrast: false,
            completed_onboarding_version: 0,
            first_run: true,
        }
    }
}

impl UserPreferences {
    /// Effective sound-effect volume for one-shot/looping cues (master × sfx).
    pub fn effective_sfx_volume(&self) -> f32 {
        (self.master_volume * self.sfx_volume).clamp(0.0, 1.0)
    }

    /// Effective ambience/music volume (master × music).
    pub fn effective_music_volume(&self) -> f32 {
        (self.master_volume * self.music_volume).clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn normalized(mut self) -> Self {
        let defaults = Self::default();
        self.master_volume = finite_clamp(self.master_volume, 0.0, 1.0, defaults.master_volume);
        self.sfx_volume = finite_clamp(self.sfx_volume, 0.0, 1.0, defaults.sfx_volume);
        self.music_volume = finite_clamp(self.music_volume, 0.0, 1.0, defaults.music_volume);
        self.mouse_sensitivity = finite_clamp(
            self.mouse_sensitivity,
            0.02,
            0.6,
            defaults.mouse_sensitivity,
        );
        self.fov_degrees = finite_clamp(self.fov_degrees, 50.0, 80.0, defaults.fov_degrees);
        if self.completed_onboarding_version == 0 && !self.first_run {
            self.completed_onboarding_version = CURRENT_ONBOARDING_VERSION;
        }
        if self.completed_onboarding_version >= CURRENT_ONBOARDING_VERSION {
            self.first_run = false;
        }
        self
    }

    /// Whether the current canonical-match onboarding revision should be offered.
    /// Version zero deliberately consults the old flag so pre-versioned saves that
    /// already opted out do not see the tutorial again.
    pub fn needs_onboarding(&self) -> bool {
        self.needs_onboarding_version(CURRENT_ONBOARDING_VERSION)
    }

    fn needs_onboarding_version(&self, version: u32) -> bool {
        if self.completed_onboarding_version == 0 {
            self.first_run
        } else {
            self.completed_onboarding_version < version
        }
    }

    /// Record completion (including an explicit skip) of the current revision.
    pub fn complete_onboarding(&mut self) {
        self.completed_onboarding_version = CURRENT_ONBOARDING_VERSION;
        self.first_run = false;
    }
}

fn finite_clamp(value: f32, minimum: f32, maximum: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(minimum, maximum)
    } else {
        fallback
    }
}

/// Resolve the platform's writable per-user configuration directory. Settings,
/// match-setup intent, and the profile live together here. Portable builds, evidence
/// drivers, and tests may opt into an explicit `OBSERVED2_CONFIG_DIR`; otherwise the
/// conventional platform location is used, with `saves/` only as a last-resort
/// fallback when no user directory is available.
pub fn config_dir() -> std::path::PathBuf {
    #[cfg(test)]
    if let Some(path) = TEST_CONFIG_DIR.with(|path| path.borrow().clone()) {
        return path;
    }
    if let Some(path) = std::env::var_os("OBSERVED2_CONFIG_DIR") {
        return path.into();
    }
    #[cfg(target_os = "windows")]
    if let Some(path) = std::env::var_os("APPDATA") {
        return std::path::PathBuf::from(path).join("Observed 2");
    }
    #[cfg(target_os = "macos")]
    if let Some(path) = std::env::var_os("HOME") {
        return std::path::PathBuf::from(path)
            .join("Library")
            .join("Application Support")
            .join("Observed 2");
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
            return std::path::PathBuf::from(path).join("observed2");
        }
        if let Some(path) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(path)
                .join(".config")
                .join("observed2");
        }
    }
    std::path::PathBuf::from("saves")
}

pub fn settings_path() -> std::path::PathBuf {
    config_dir().join("settings.json")
}

pub(crate) fn legacy_settings_path() -> std::path::PathBuf {
    std::path::PathBuf::from("saves").join("settings.json")
}

pub fn play_setup_path() -> std::path::PathBuf {
    config_dir().join("play_setup.json")
}

#[cfg(test)]
thread_local! {
    pub static TEST_CONFIG_DIR: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
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
    config_dir().join("profile.save")
}

#[cfg(not(test))]
pub(crate) fn legacy_profile_save_path() -> std::path::PathBuf {
    std::path::PathBuf::from("saves").join("profile.save")
}

/// Load settings from disk if present and well-formed; `Settings::default()`
/// otherwise (covers "no save yet" and "corrupt save" identically — never panics on a
/// bad file).
pub fn load_settings() -> Settings {
    #[cfg(test)]
    if TEST_CONFIG_DIR.with(|path| path.borrow().is_none()) {
        return Settings::default();
    }

    load_settings_from(&settings_path(), &legacy_settings_path())
}

fn load_settings_from(primary: &std::path::Path, legacy: &std::path::Path) -> UserPreferences {
    let parse = |path: &std::path::Path| {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<UserPreferences>(&text).ok())
            .map(UserPreferences::normalized)
    };
    if let Some(settings) = parse(primary) {
        return settings;
    }
    if let Some(settings) = parse(legacy) {
        let _ = save_settings_to(primary, &settings);
        return settings;
    }
    UserPreferences::default()
}

/// Persist settings to disk (best-effort: a write failure is silently ignored, the
/// same convention `evidence::snapshot` uses for its diagnostic writes — settings
/// persistence is a convenience, not gameplay-critical).
pub fn save_settings(settings: &Settings) {
    #[cfg(test)]
    if TEST_CONFIG_DIR.with(|path| path.borrow().is_none()) {
        return;
    }

    if let Err(error) = save_settings_to(&settings_path(), settings) {
        warn!("could not save preferences: {error}");
    }
}

fn save_settings_to(path: &std::path::Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    write_replace(path, json.as_bytes())
}

fn write_replace(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes)?;
    if path.exists() {
        std::fs::copy(&temporary, path)?;
        std::fs::remove_file(temporary)
    } else {
        std::fs::rename(temporary, path)
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
        assert_eq!(settings.fov_degrees, 60.0);
        assert!(settings.first_run, "onboarding shows on a fresh profile");
        assert_eq!(settings.completed_onboarding_version, 0);
        assert!(settings.needs_onboarding());
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
    fn older_settings_gain_the_default_hex_fov() {
        let json = serde_json::to_string(&Settings::default()).expect("settings serialize");
        let mut value: serde_json::Value = serde_json::from_str(&json).expect("settings json");
        value
            .as_object_mut()
            .expect("settings object")
            .remove("fov_degrees");
        let loaded: Settings = serde_json::from_value(value).expect("old settings deserialize");
        assert_eq!(loaded.fov_degrees, 60.0);
    }

    #[test]
    fn pre_versioned_completion_migrates_without_replaying_onboarding() {
        let mut value = serde_json::to_value(Settings::default()).expect("settings serialize");
        let object = value.as_object_mut().expect("settings object");
        object.remove("completed_onboarding_version");
        object.insert("first_run".to_string(), serde_json::Value::Bool(false));

        let legacy: Settings = serde_json::from_value(value).expect("legacy settings parse");
        assert!(
            !legacy.needs_onboarding(),
            "the legacy flag remains an immediate migration signal"
        );
        let migrated = legacy.normalized();
        assert_eq!(
            migrated.completed_onboarding_version,
            CURRENT_ONBOARDING_VERSION
        );
        assert!(!migrated.needs_onboarding());
    }

    #[test]
    fn an_older_completed_revision_is_eligible_for_future_revised_help() {
        let settings = Settings {
            completed_onboarding_version: 1,
            first_run: false,
            ..Default::default()
        };
        assert!(settings.needs_onboarding_version(2));
        assert!(!settings.needs_onboarding_version(1));
    }

    #[test]
    fn loaded_preferences_are_finite_and_clamped_to_supported_ranges() {
        let settings = UserPreferences {
            master_volume: 8.0,
            sfx_volume: -2.0,
            music_volume: f32::NAN,
            mouse_sensitivity: f32::INFINITY,
            fov_degrees: 140.0,
            ..Default::default()
        }
        .normalized();
        assert_eq!(settings.master_volume, 1.0);
        assert_eq!(settings.sfx_volume, 0.0);
        assert_eq!(settings.music_volume, 1.0);
        assert_eq!(settings.mouse_sensitivity, DEFAULT_MOUSE_SENSITIVITY);
        assert_eq!(settings.fov_degrees, 80.0);
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
    fn corrupt_platform_settings_fall_back_to_and_migrate_a_valid_legacy_file() {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let scratch = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target/test-scratch")
            .join(format!(
                "settings-migration-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
        let primary = scratch.join("platform/settings.json");
        let legacy = scratch.join("legacy/settings.json");
        std::fs::create_dir_all(primary.parent().expect("primary parent")).expect("primary dir");
        std::fs::create_dir_all(legacy.parent().expect("legacy parent")).expect("legacy dir");
        std::fs::write(&primary, "not json").expect("corrupt primary");
        let expected = UserPreferences {
            master_volume: 0.4,
            high_contrast: true,
            ..Default::default()
        };
        std::fs::write(
            &legacy,
            serde_json::to_string_pretty(&expected).expect("serialize legacy"),
        )
        .expect("legacy write");

        assert_eq!(load_settings_from(&primary, &legacy), expected);
        let migrated: UserPreferences = serde_json::from_str(
            &std::fs::read_to_string(&primary).expect("migrated primary exists"),
        )
        .expect("migrated primary parses");
        assert_eq!(migrated, expected);
        std::fs::remove_dir_all(scratch).expect("scratch cleanup");
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
