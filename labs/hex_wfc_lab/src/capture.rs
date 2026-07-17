//! Headless capture pipeline: steps a solve replay across a fixed frame budget,
//! screenshotting each frame and writing a `manifest.json`, then exits.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::LabState;

/// The capture runs in two phases: first the animated solve on level 0
/// (`hex_wfc_NNN.png`), then one final-state slice per level
/// (`slice_L.png`) so verticality — shaft columns, upper-level ramps and the
/// multi-hex rooms — is visible across the stack.
#[derive(Resource)]
pub(crate) struct CaptureRun {
    dir: String,
    frame: u32,
    total_frames: u32,
    slice: u8,
    /// Whether the current slice's level has been applied and rebuilt; the
    /// screenshot is deferred one tick because `capture_progress` runs after
    /// `rebuild_visuals`, so the new level only reaches the framebuffer next
    /// frame.
    armed: bool,
    timer: f32,
    finished: bool,
}

impl CaptureRun {
    pub(crate) fn new(dir: String) -> Self {
        Self {
            dir,
            frame: 0,
            total_frames: 28,
            slice: 0,
            armed: false,
            timer: 0.0,
            finished: false,
        }
    }
}

pub(crate) fn capture_progress(
    time: Res<Time>,
    mut run: ResMut<CaptureRun>,
    mut state: ResMut<LabState>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    if run.finished {
        return;
    }
    run.timer += time.delta_secs();
    if run.frame == 0 {
        std::fs::create_dir_all(&run.dir).expect("capture dir must be creatable");
        state.playing = false;
        state.cursor = 0;
        state.current_level = 0;
        state.dirty = true;
    }
    // Phase 1: one animated frame every 0.35 s, stepping an equal share of the
    // trace between frames so the whole solve fits the frame budget.
    if run.timer >= 0.35 && run.frame < run.total_frames {
        let share = state.trace.len().div_ceil(run.total_frames as usize - 1);
        let path = format!("{}/hex_wfc_{:03}.png", run.dir, run.frame);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        state.advance(share);
        run.frame += 1;
        run.timer = 0.0;
        return;
    }
    // Phase 2: freeze on the solved layout and screenshot each level slice.
    // Arm (set level + request rebuild) on one tick, screenshot on a later one.
    let levels = state.world.config.levels;
    if run.frame >= run.total_frames && run.slice < levels {
        if !run.armed {
            state.cursor = state.trace.len();
            state.current_level = run.slice;
            state.dirty = true;
            run.armed = true;
            run.timer = 0.0;
            return;
        }
        if run.timer >= 0.4 {
            let path = format!("{}/slice_{}.png", run.dir, run.slice);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
            run.slice += 1;
            run.armed = false;
            run.timer = 0.0;
        }
        return;
    }
    if run.slice >= levels && run.timer >= 1.0 {
        let manifest = serde_json::json!({
            "lab": "hex_wfc_lab",
            "seed": format!("{:#018x}", state.world.seed),
            "trace_steps": state.trace.len(),
            "attempts": state.world.last_attempts,
            "rooms": state.world.room_count(),
            "levels": levels,
            "frames": (0..run.total_frames)
                .map(|f| format!("hex_wfc_{f:03}.png"))
                .collect::<Vec<_>>(),
            "slices": (0..levels).map(|l| format!("slice_{l}.png")).collect::<Vec<_>>(),
        });
        std::fs::write(
            format!("{}/manifest.json", run.dir),
            serde_json::to_string_pretty(&manifest).expect("manifest serializes"),
        )
        .expect("manifest must be writable");
        run.finished = true;
        exit.write(AppExit::Success);
    }
}
