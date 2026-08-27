//! Script runner executing JSON view scripts for hex_tile_lab with 2-phase
//! render stabilization (configure -> settle -> screenshot -> exit).

use std::fs;
use std::path::{Path, PathBuf};

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_content::ArchitectureRegister;
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
    /// Runtime variant (rotation-0 layout 0 is 0; layout 1 is 6).
    pub variant: Option<u16>,
    pub composition: Option<String>,
    pub view_mode: Option<String>,
    pub render_mode: Option<String>,
    /// Architecture register 1-10 (1-9 use the matching digit; 10 uses 0).
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
    /// A hand-chosen run, as `"archetype"` or `"archetype:variant"` entries.
    ///
    /// Takes precedence over `tile_id`/`composition`: a script that names a run
    /// is asking about composition, and the single-tile selectors cannot answer
    /// that question.
    pub run: Option<Vec<String>>,
    /// Walk the body forward while the script runs, instead of standing still.
    ///
    /// The flag already existed on `LabState` and nothing could set it. It is
    /// the production character controller being driven, not a camera path, so
    /// what it proves is that the run is *walkable* - a seam that does not mate
    /// stops the body rather than merely looking wrong.
    pub walk: Option<bool>,
    /// Light the scene exactly as the shipped facility lights it.
    ///
    /// The lab's Lit mode is an *inspection* light: it floors ambient so
    /// geometry stays legible, stretches fog to 60..170 m so an orbit camera
    /// can stand back, and carries a headlamp the facility deliberately does
    /// not ship. Every one of those makes a tile easier to look at and makes
    /// the capture a worse answer to "how will this read in the game".
    pub facility_lighting: Option<bool>,
    /// Capture this many frames instead of one, `frame_interval` ticks apart.
    pub frames: Option<u32>,
    pub frame_interval: Option<u32>,
}

/// Parse one `"archetype"` or `"archetype:variant"` run entry.
#[must_use]
fn parse_step(entry: &str) -> (String, u16) {
    match entry.split_once(':') {
        Some((archetype, variant)) => (
            archetype.trim().to_string(),
            variant.trim().parse().unwrap_or(0),
        ),
        None => (entry.trim().to_string(), 0),
    }
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
    /// How many frames of a sequence have been written so far.
    pub shot: u32,
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

fn composition_position(state: &LabState, needle: &str, variant: Option<u16>) -> Option<usize> {
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
        matches!(c, Composition::SingleTile { archetype, variant: candidate }
            if (archetype == needle || archetype.contains(needle))
                && variant.is_none_or(|variant| *candidate == variant))
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
            && (1..=ArchitectureRegister::ALL.len() as u8).contains(&register)
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

        // A named run is appended to the list and selected, so everything
        // downstream - the title, the camera framing, the capture filename -
        // works on it exactly as it does on the built-in compositions.
        if let Some(ref entries) = script.run {
            let steps: Vec<(String, u16)> = entries.iter().map(|e| parse_step(e)).collect();
            state.compositions.push(Composition::Run { steps });
            let position = state.compositions.len() - 1;
            state.switch(position);
        } else {
            let target = script
                .tile_id
                .as_deref()
                .or(script.composition.as_deref())
                .and_then(|needle| composition_position(&state, needle, script.variant));
            if let Some(position) = target {
                state.switch(position);
            }
        }
        state.scripted_walk = script.walk == Some(true);
        state.facility_lighting = script.facility_lighting == Some(true);

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
        if state.view_mode == ViewMode::FirstPerson
            && let Some(position) = script.camera_pos
        {
            let eye = Vec3::from_array(position);
            let target = script
                .camera_target
                .map(Vec3::from_array)
                .unwrap_or(state.center);
            let direction = (target - eye).normalize_or_zero();
            state.body.position =
                eye - Vec3::Y * (state.config.eye_height - state.config.half_height);
            state.body.velocity = Vec3::ZERO;
            state.body.yaw = direction.x.atan2(-direction.z);
            state.body.pitch = direction.y.asin();
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

    // Phase 2: shoot once the scene has settled. Generous, because the tile
    // library is built before the first frame: shooting too early saves a black
    // PNG and still reports success, which reads as a render regression.
    //
    // A sequence numbers its files and keeps walking between them; a single
    // still is that with `frames = 1`, so there is one path rather than two.
    let frames = script.frames.unwrap_or(1).max(1);
    #[allow(clippy::cast_precision_loss)]
    let interval = script.frame_interval.unwrap_or(12) as f32 / 60.0;
    let due = 6.0 + exec.shot as f32 * interval;
    if exec.configured && exec.shot < frames && exec.timer >= due {
        if let Some(ref out_path) = script.output_image {
            let path = if frames == 1 {
                out_path.clone()
            } else {
                let stem = out_path.strip_suffix(".png").unwrap_or(out_path);
                format!("{stem}_{:03}.png", exec.shot)
            };
            if let Some(parent) = Path::new(&path).parent() {
                let _ = fs::create_dir_all(parent);
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
        exec.shot += 1;
        exec.captured = exec.shot >= frames;
    }

    // Phase 3: exit after the last screenshot's GPU writeback completes.
    if exec.captured && exec.timer >= due + 1.0 {
        app_exit.write(AppExit::Success);
    }
}
