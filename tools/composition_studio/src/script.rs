//! Headless driver: describe a view in JSON, get a deterministic screenshot.
//!
//! Mirrors `labs/hex_tile_lab/src/script_runner.rs` so the evidence workflow is
//! the same one the tile lab already uses: configure at `t >= 0.1`, shoot at
//! `t >= 0.8`, exit at `t >= 1.6`. The gap between shooting and exiting is
//! load-bearing — `save_to_disk` writes on a *later* frame, so quitting sooner
//! loses the PNG.
//!
//! ```json
//! {
//!   "seed_index": 2,
//!   "layer": 0,
//!   "zoom": 0.5,
//!   "compare": true,
//!   "hide_menu": true,
//!   "archetype_bias": { "shaft": 2.5, "junction": 0.5 },
//!   "output_image": "docs/evidence/composition_studio/shaft_heavy.png"
//! }
//! ```
//!
//! Read the path from `--script <path>` or `OBSERVED2_SCRIPT`.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_facility::hex_wfc::HexArchetype;
use serde::Deserialize;

use crate::{LabMenuState, Layer, StudioState, StudioTab};

/// Every field optional: a script says only what it wants to change.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudioScript {
    pub seed_index: Option<usize>,
    /// A level index, or omit for all levels.
    pub layer: Option<u8>,
    pub zoom: Option<f32>,
    /// Colour cells that differ from the same seed at the baseline profile.
    pub compare: Option<bool>,
    pub hide_menu: Option<bool>,
    /// Which panel to show: `solve`, `tuning`, `coverage`, `districts`,
    /// `diagnostics`.
    pub tab: Option<String>,
    /// Run the whole-catalog seam audit before shooting. Slow; off by default.
    pub audit_seams: Option<bool>,
    /// `"focus"`, `"layer"`, or omit for the schematic.
    pub detail: Option<String>,
    /// Cell to select, as `[q, r, level]` — the focus set is built from it.
    pub select: Option<[u16; 3]>,
    /// View azimuth detent, `0..6`. Detent 0 is the historical camera.
    pub detent: Option<usize>,
    /// Cut away the ceiling and near walls. Defaults on in detail mode.
    pub cutaway: Option<bool>,
    /// Pins to paint before solving, as `[q, r, level, brush]` — brush being a
    /// [`crate::brush::Brush`] label such as `"shaft"` or `"junction"`.
    #[serde(default)]
    pub paint: Vec<PaintedPin>,
    /// Archetype bias overrides, keyed by the profile's own field names
    /// (`shaft`, `junction`, `room`, …).
    #[serde(default)]
    pub archetype_bias: std::collections::BTreeMap<String, f64>,
    pub output_image: Option<String>,
}

/// One pin for a scripted capture.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaintedPin {
    pub q: u16,
    pub r: u16,
    #[serde(default)]
    pub level: u8,
    pub brush: String,
}

/// Where the runner is in the configure → shoot → exit sequence.
#[derive(Resource)]
pub struct ScriptRun {
    pub script: StudioScript,
    pub timer: f32,
    pub phase: u8,
}

/// The script path, from the CLI or the environment.
#[must_use]
pub fn script_path() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--script" {
            return args.next();
        }
    }
    std::env::var("OBSERVED2_SCRIPT").ok()
}

/// Parse a script, or explain why it could not be parsed. Never falls back to a
/// default script: a capture that silently photographed the wrong view is worse
/// than one that did not run.
pub fn load_script(path: &str) -> Result<StudioScript, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    // PowerShell 5.1's `Out-File -Encoding utf8` writes a BOM, and every author
    // on Windows will produce one by accident at least once. `serde_json`
    // rejects it with "expected value at line 1 column 1", which names the
    // symptom and hides the cause.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    serde_json::from_str(text).map_err(|error| format!("{path}: {error}"))
}

fn bias_field(name: &str) -> Option<HexArchetype> {
    Some(match name {
        "void" => HexArchetype::Void,
        "room" => HexArchetype::Room,
        "straight" => HexArchetype::Straight,
        "corner" => HexArchetype::Corner,
        "junction" => HexArchetype::Junction,
        "ramp_up" => HexArchetype::RampUp,
        "ramp_head" => HexArchetype::RampHead,
        "shaft" => HexArchetype::Shaft,
        "expanse" => HexArchetype::Expanse,
        _ => return None,
    })
}

pub fn script_system(
    time: Res<Time>,
    mut run: ResMut<ScriptRun>,
    mut state: ResMut<StudioState>,
    mut menu: ResMut<LabMenuState>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    run.timer += time.delta_secs();

    if run.phase == 0 && run.timer >= 0.1 {
        let script = run.script.clone();
        if let Some(index) = script.seed_index {
            state.seed_index = index;
            state.invalidate_baseline();
        }
        state.layer = script.layer.map_or(Layer::All, Layer::Single);
        if let Some(zoom) = script.zoom {
            state.zoom = zoom;
        }
        if let Some(compare) = script.compare {
            state.show_baseline_compare = compare;
        }
        if let Some(hide) = script.hide_menu {
            menu.is_open = !hide;
        }
        if let Some(name) = &script.tab {
            match StudioTab::ALL
                .iter()
                .position(|tab| tab.label().eq_ignore_ascii_case(name))
            {
                Some(index) => menu.active_tab = index,
                None => state.status = format!("ERROR: unknown tab {name:?}"),
            }
        }
        if script.audit_seams == Some(true) {
            crate::input::run_seam_audit(&mut state);
        }
        let config = state.config;
        for painted in &script.paint {
            match crate::brush::Brush::ALL
                .iter()
                .find(|brush| brush.label().eq_ignore_ascii_case(&painted.brush))
            {
                Some(brush) => {
                    crate::brush::paint(
                        &mut state.profile,
                        config,
                        observed_hex::HexCoord {
                            q: painted.q,
                            r: painted.r,
                            level: painted.level,
                        },
                        *brush,
                    );
                }
                None => state.status = format!("ERROR: unknown brush {:?}", painted.brush),
            }
        }
        if !script.paint.is_empty() {
            state.refresh_pin_diagnostics();
        }
        if let Some([q, r, level]) = script.select {
            #[allow(clippy::cast_possible_truncation)]
            let coord = observed_hex::HexCoord {
                q,
                r,
                level: level as u8,
            };
            state.selected = Some(coord);
        }
        if let Some(detent) = script.detent {
            state.detent = detent % crate::viewport::AZIMUTH_DETENTS;
        }
        if let Some(cutaway) = script.cutaway {
            state.cutaway = cutaway;
        }
        if let Some(mode) = &script.detail {
            state.detail_mode = match mode.to_ascii_lowercase().as_str() {
                "focus" => crate::detail::DetailMode::Focus,
                "layer" => crate::detail::DetailMode::Layer,
                "off" => crate::detail::DetailMode::Off,
                other => {
                    state.status = format!("ERROR: unknown detail mode {other:?}");
                    crate::detail::DetailMode::Off
                }
            };
        }
        for (name, value) in &script.archetype_bias {
            match bias_field(name) {
                Some(archetype) => {
                    state.profile.archetype_bias =
                        state.profile.archetype_bias.with(archetype, *value);
                }
                None => {
                    state.status = format!("ERROR: unknown archetype bias {name:?}");
                }
            }
        }
        // Solve immediately rather than waiting out the interactive debounce.
        state.solve_dirty = true;
        state.last_edit = None;
        run.phase = 1;
    }

    if run.phase == 1 && run.timer >= 0.8 {
        if let Some(path) = run.script.output_image.clone() {
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
        run.phase = 2;
    }

    // `save_to_disk` writes on a later frame; exiting at 0.8 would lose it.
    if run.phase == 2 && run.timer >= 1.6 {
        run.phase = 3;
        exit.write(AppExit::Success);
    }
}
