//! `OBSERVED2_CAPTURE` evidence pass: one orbit still per composition in the
//! deduplicated browse list, then a `manifest.json` recording what was shot.
//!
//! The composition list is one entry per authored module plus the curated
//! stacks and room blueprints (~20 entries), so this shoots everything —
//! no hand-picked capture set to go stale when the corpus changes.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::LabState;

#[derive(Resource)]
pub struct CaptureRun {
    dir: String,
    phase: usize,
    timer: f32,
    finished: bool,
}

impl CaptureRun {
    pub fn new(dir: String) -> Self {
        std::fs::create_dir_all(&dir).expect("capture dir must be creatable");
        Self {
            dir,
            phase: 0,
            timer: 0.0,
            finished: false,
        }
    }
}

/// One settled orbit-cutaway still per composition, then the manifest.
pub fn capture_progress(
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
    let total = state.compositions.len();
    if total == 0 {
        return;
    }

    if run.phase == 0 {
        // Evidence defaults: orbit framing with the roof cut away so the
        // interior is actually visible in the still.
        state.overlay = false;
        state.view_mode = crate::ViewMode::Orbit;
        state.cross_section = true;
        state.orbit_yaw = 0.8;
        state.orbit_pitch = 0.9;
        state.switch(0);
        run.phase = 1;
        run.timer = 0.0;
        return;
    }

    // Even slots settle + screenshot; odd slots wait for the async save
    // before switching, so frames never bleed across.
    let still_phases = total * 2;
    if run.phase <= still_phases {
        let slot = run.phase - 1;
        if slot.is_multiple_of(2) && run.timer >= 0.9 {
            let index = slot / 2;
            let slug = state.compositions[index].slug();
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!(
                    "{}/comp_{index:02}_{slug}.png",
                    run.dir
                )));
            run.phase += 1;
            run.timer = 0.0;
        } else if !slot.is_multiple_of(2) && run.timer >= 0.5 {
            run.phase += 1;
            run.timer = 0.0;
            if run.phase <= still_phases {
                state.switch((run.phase - 1) / 2);
            }
        }
        return;
    }

    if run.timer >= 1.0 {
        let register = state.register();
        let captured: Vec<String> = state
            .compositions
            .iter()
            .map(|c| c.title(&state.tiles, register.slug()))
            .collect();
        let manifest = serde_json::json!({
            "lab": "hex_tile_lab",
            "library_tiles": state.tiles.len(),
            "compositions": captured,
            "register": register.slug(),
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
