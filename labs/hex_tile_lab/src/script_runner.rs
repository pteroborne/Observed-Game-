//! Script runner executing JSON view scripts for hex_tile_lab with 2-phase
//! render stabilization (configure -> settle -> screenshot -> exit).

use std::fs;
use std::path::{Path, PathBuf};

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use serde::Deserialize;

use crate::{Composition, LabState, RenderMode, ViewMode};

/// JSON view script: composition/tile selection, camera framing, render mode.
///
/// `dev_mode` and `strong_wireframe` are legacy aliases kept for older
/// scripts: they map to the Clay and X-ray render modes. Prefer
/// `render_mode: "lit" | "clay" | "xray" | "colliders"`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ViewScript {
    pub tile_id: Option<String>,
    pub composition: Option<String>,
    pub view_mode: Option<String>,
    pub render_mode: Option<String>,
    /// District register 1-9 (same order as the digit hotkeys).
    pub register: Option<u8>,
    pub camera_pos: Option<[f32; 3]>,
    pub camera_target: Option<[f32; 3]>,
    pub orbit_yaw: Option<f32>,
    pub orbit_pitch: Option<f32>,
    pub radius: Option<f32>,
    pub height: Option<f32>,
    pub strong_wireframe: Option<bool>,
    pub dev_mode: Option<bool>,
    pub cross_section: Option<bool>,
    pub volumetrics: Option<bool>,
    pub hide_menu: Option<bool>,
    pub output_image: Option<String>,
}

impl ViewScript {
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
        serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
    }
}

#[derive(Resource, Default)]
pub struct ScriptExecution {
    pub script: Option<ViewScript>,
    pub script_path: Option<PathBuf>,
    pub configured: bool,
    pub captured: bool,
    pub timer: f32,
}

impl ScriptExecution {
    /// Detect script path from CLI args (`--script <path>`) or `OBSERVED2_SCRIPT`.
    pub fn detect_script() -> Option<PathBuf> {
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            if arg == "--script"
                && let Some(path) = args.next()
            {
                return Some(PathBuf::from(path));
            }
        }
        if let Ok(env_path) = std::env::var("OBSERVED2_SCRIPT")
            && !env_path.is_empty()
        {
            return Some(PathBuf::from(env_path));
        }
        None
    }
}

fn composition_position(state: &LabState, needle: &str) -> Option<usize> {
    if needle == "silo" || needle == "wellshaft" || needle == "silo_wellshaft" {
        return state
            .compositions
            .iter()
            .position(|c| *c == Composition::SiloWellshaft);
    }
    if needle == "shaft_stack" || needle == "tower_7hex" || needle == "tower_7hex_3level" {
        return state
            .compositions
            .iter()
            .position(|c| *c == Composition::SiloWellshaft);
    }
    state.compositions.iter().position(|c| {
        matches!(c, Composition::SingleTile { archetype, .. }
            if archetype == needle || archetype.contains(needle))
    })
}

pub fn run_script_system(
    time: Res<Time>,
    mut state: ResMut<LabState>,
    mut menu_state: ResMut<crate::LabMenuState>,
    mut exec: ResMut<ScriptExecution>,
    mut commands: Commands,
    mut app_exit: MessageWriter<AppExit>,
) {
    let Some(ref script) = exec.script.clone() else {
        return;
    };

    exec.timer += time.delta_secs();

    // Phase 1: apply configuration (tick 0.1 s).
    if exec.timer >= 0.1 && !exec.configured {
        state.auto_orbit = false;

        if script.hide_menu.unwrap_or(true) {
            state.overlay = false;
            menu_state.is_open = false;
        }

        if let Some(register) = script.register
            && (1..=9).contains(&register)
        {
            state.register_index = usize::from(register - 1);
        }

        // Explicit render_mode wins; legacy flags fall back to X-ray / Clay.
        if let Some(ref mode) = script.render_mode {
            state.render_mode = match mode.to_lowercase().as_str() {
                "clay" => RenderMode::Clay,
                "xray" | "x-ray" => RenderMode::Xray,
                "colliders" => RenderMode::Colliders,
                _ => RenderMode::Lit,
            };
        } else if script.strong_wireframe == Some(true) {
            state.render_mode = RenderMode::Xray;
        } else if script.dev_mode == Some(true) {
            state.render_mode = RenderMode::Clay;
        }

        let target = script
            .tile_id
            .as_deref()
            .or(script.composition.as_deref())
            .and_then(|needle| composition_position(&state, needle));
        if let Some(position) = target {
            state.switch(position);
        }

        if let Some(ref mode_str) = script.view_mode {
            match mode_str.to_lowercase().as_str() {
                "orbit" => state.view_mode = ViewMode::Orbit,
                "firstperson" | "first_person" => state.view_mode = ViewMode::FirstPerson,
                "freelook" | "free_look" => state.view_mode = ViewMode::FreeLook,
                _ => {}
            }
        }

        if let Some(target) = script.camera_target {
            state.center = Vec3::from_array(target);
        }
        if let Some(pos) = script.camera_pos {
            state.free_fly_pos = Vec3::from_array(pos);
            let dir = (state.center - state.free_fly_pos).normalize_or_zero();
            state.free_fly_yaw = dir.x.atan2(-dir.z);
            state.free_fly_pitch = dir.y.asin();
        }
        if let Some(yaw) = script.orbit_yaw {
            state.orbit_yaw = yaw;
        }
        if let Some(pitch) = script.orbit_pitch {
            state.orbit_pitch = pitch;
        }
        if let Some(r) = script.radius {
            state.radius = r;
        }
        if let Some(h) = script.height {
            state.height = h;
        }
        if let Some(cross) = script.cross_section {
            state.cross_section = cross;
        }
        if let Some(vol) = script.volumetrics {
            state.volumetrics = vol;
        }

        state.dirty = true;
        exec.configured = true;
    }

    // Phase 2: screenshot after the scene settles (tick 0.8 s).
    if exec.configured && !exec.captured && exec.timer >= 0.8 {
        if let Some(ref out_path) = script.output_image {
            let out_path = out_path.clone();
            if let Some(parent) = Path::new(&out_path).parent() {
                let _ = fs::create_dir_all(parent);
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(out_path));
        }
        exec.captured = true;
    }

    // Phase 3: exit after the screenshot GPU writeback completes (tick 1.6 s).
    if exec.captured && exec.timer >= 1.6 {
        app_exit.write(AppExit::Success);
    }
}
