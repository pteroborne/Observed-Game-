//! Headless capture: one stacked overview and one per-level slice for each
//! preset seed, then a `manifest.json` carrying the census numbers so a reader
//! can diff two runs without re-reading the images.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::{LabState, PRESET_SEEDS, ViewMode};

/// Frames are deferred one tick after the state change that produced them:
/// `capture_progress` runs after `rebuild`, so a newly requested view only
/// reaches the framebuffer on the following frame.
#[derive(Resource)]
pub(crate) struct CaptureRun {
    dir: String,
    seed_index: usize,
    slice: u8,
    stage: Stage,
    armed: bool,
    timer: f32,
    manifest: Vec<serde_json::Value>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Stage {
    Overview,
    Slices,
    Advance,
    Finished,
}

impl CaptureRun {
    pub(crate) fn new(dir: String) -> Self {
        Self {
            dir,
            seed_index: 0,
            slice: 0,
            stage: Stage::Overview,
            armed: false,
            timer: 0.0,
            manifest: Vec::new(),
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
    if run.stage == Stage::Finished {
        return;
    }
    run.timer += time.delta_secs();

    match run.stage {
        Stage::Overview => {
            if !run.armed {
                std::fs::create_dir_all(&run.dir).expect("capture dir must be creatable");
                state.mode = ViewMode::Stack;
                state.dirty = true;
                run.armed = true;
                run.timer = 0.0;
                return;
            }
            if run.timer >= 0.5 {
                let path = format!("{}/seed_{}_stack.png", run.dir, run.seed_index);
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path));
                run.stage = Stage::Slices;
                run.slice = 0;
                run.armed = false;
                run.timer = 0.0;
            }
        }
        Stage::Slices => {
            let levels = state.world.config.levels;
            if run.slice >= levels {
                run.stage = Stage::Advance;
                run.armed = false;
                run.timer = 0.0;
                return;
            }
            if !run.armed {
                state.mode = ViewMode::Slice;
                state.focus_level = run.slice;
                state.dirty = true;
                run.armed = true;
                run.timer = 0.0;
                return;
            }
            if run.timer >= 0.4 {
                let path = format!(
                    "{}/seed_{}_level_{}.png",
                    run.dir, run.seed_index, run.slice
                );
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path));
                run.slice += 1;
                run.armed = false;
                run.timer = 0.0;
            }
        }
        Stage::Advance => {
            let census = state.archetype_census();
            let districts = state
                .district_census()
                .into_iter()
                .map(|(register, (cells, regions))| {
                    serde_json::json!({
                        "register": format!("{register:?}"),
                        "cells": cells,
                        "regions": regions,
                    })
                })
                .collect::<Vec<_>>();
            run.manifest.push(serde_json::json!({
                "seed": format!("{:#018x}", state.world.seed),
                "attempts": state.world.last_attempts,
                "rooms": state.world.blueprints.len(),
                "archetypes": census,
                "districts": districts,
            }));

            let next = run.seed_index + 1;
            if next < PRESET_SEEDS.len() {
                run.seed_index = next;
                *state = LabState::new(next);
                run.stage = Stage::Overview;
                run.armed = false;
                run.timer = 0.0;
                return;
            }
            let config = state.world.config;
            let manifest = serde_json::json!({
                "lab": "iso_observer_lab",
                "phase": "arc_o_phase_104_baseline",
                "grid": {
                    "cols": config.cols,
                    "rows": config.rows,
                    "levels": config.levels,
                },
                "legend": {
                    "colour": "architecture register, via observed_style::architecture_surface",
                    "height": "hex archetype; corridors low, rooms mid, shafts tall",
                },
                "seeds": run.manifest,
            });
            std::fs::write(
                format!("{}/manifest.json", run.dir),
                serde_json::to_string_pretty(&manifest).expect("manifest serializes"),
            )
            .expect("manifest must be writable");
            run.stage = Stage::Finished;
            exit.write(AppExit::Success);
        }
        Stage::Finished => {}
    }
}
