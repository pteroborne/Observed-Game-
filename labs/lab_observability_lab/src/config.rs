//! The configuration half of Phase A7, built on `bevy_mod_config`.
//!
//! [`ObservabilityConfig`] is a single typed config root holding every lab knob.
//! The crate gives us three things for free: a typed schema with defaults, JSON
//! persistence (via the [`ConfigManager`] alias), and change tracking. The egui
//! editor feature is intentionally left off — the lab renders its own neon-noir
//! overlay instead of adopting an egui UI stack.
//!
//! Knobs split into two roles, and keeping that split honest is the whole point:
//!
//! - [`ObservabilityConfig::seed`] is a *candidate* launch seed. It only enters
//!   deterministic simulation when explicitly committed into a
//!   [`crate::model::LaunchManifest`]; until then it is pending.
//! - Every other field is a pure debug/presentation knob that is applied live
//!   and can never change simulation state.

use bevy::prelude::*;
use bevy_mod_config::{Config, ConfigNode, ScalarData};

/// The persistence manager: compact JSON through `bevy_mod_config`'s Serde
/// manager. `Json` has no `Default` (its formatter does not), so the app
/// constructs it with [`Json::new`](bevy_mod_config::manager::serde::Json::new)
/// via `init_config_with`.
pub type ConfigManager = bevy_mod_config::manager::serde::Json;

/// Every lab knob, as a single `bevy_mod_config` root.
#[derive(Config)]
pub struct ObservabilityConfig {
    /// Candidate launch seed. Part of the deterministic manifest only once
    /// committed (the lab's `Enter` action); editing it leaves the running
    /// simulation untouched and merely marks a pending relaunch.
    #[config(default = 1975)]
    pub seed: u32,
    /// Whether the diagnostics overlay is shown.
    #[config(default = true)]
    pub overlay: bool,
    /// Atmosphere fog strength, `0.0..=1.0` (presentation only).
    #[config(default = 0.45)]
    pub fog: f32,
    /// Bloom/glow strength, `0.0..=1.0` (presentation only).
    #[config(default = 0.6)]
    pub bloom: f32,
    /// Color-vision preview mode index into `ColorVisionMode::ALL` (0..=4).
    #[config(default = 0)]
    pub color_vision: u32,
    /// Event-trace verbosity (0 off, 1 key events, 2 all). Logging only.
    #[config(default = 2)]
    pub trace_verbosity: u32,
    /// Capture-mode warm-up tick count (how many ticks to pre-run before the
    /// evidence screenshot). Not part of deterministic match state.
    #[config(default = 16)]
    pub capture_frame: u32,
}

/// The lab's config field names, used to address scalars by path when editing
/// them at runtime. Keeping them in one place avoids stringly-typed drift.
pub mod field {
    /// `seed` field key.
    pub const SEED: &str = "seed";
    /// `overlay` field key.
    pub const OVERLAY: &str = "overlay";
    /// `fog` field key.
    pub const FOG: &str = "fog";
    /// `bloom` field key.
    pub const BLOOM: &str = "bloom";
    /// `color_vision` field key.
    pub const COLOR_VISION: &str = "color_vision";
    /// `trace_verbosity` field key.
    pub const TRACE_VERBOSITY: &str = "trace_verbosity";
}

/// True when this config node is the leaf for `name` (e.g. `"fog"`).
pub fn is_field(node: &ConfigNode, name: &str) -> bool {
    node.path.last().map(String::as_str) == Some(name)
}

/// Bumps a node's change generation so `ReadConfigChange` consumers notice a
/// value written directly through its [`ScalarData`] component.
pub fn bump(node: &mut ConfigNode) {
    node.generation = node.generation.next();
}

/// Sets a `u32` scalar and bumps its generation if the value actually changed.
pub fn set_u32(node: &mut ConfigNode, data: &mut ScalarData<u32>, value: u32) {
    if data.0 != value {
        data.0 = value;
        bump(node);
    }
}

/// Sets an `f32` scalar (clamped to `0.0..=1.0`) and bumps generation on change.
pub fn set_unit_f32(node: &mut ConfigNode, data: &mut ScalarData<f32>, value: f32) {
    let clamped = value.clamp(0.0, 1.0);
    if data.0 != clamped {
        data.0 = clamped;
        bump(node);
    }
}

/// Sets a `bool` scalar and bumps generation on change.
pub fn set_bool(node: &mut ConfigNode, data: &mut ScalarData<bool>, value: bool) {
    if data.0 != value {
        data.0 = value;
        bump(node);
    }
}
