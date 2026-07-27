//! Headless capture: one stacked overview and one per-level slice for each
//! preset seed, then a `manifest.json` carrying the census numbers so a reader
//! can diff two runs without re-reading the images.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::{LabState, Layer, PRESET_SEEDS};

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
    Inspect,
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
                state.layer = Layer::All;
                state.selected = None;
                // Evidence frames fit the whole layer. The interactive default
                // is zoomed well in, which is right for reading a doorway and
                // wrong for a before/after comparison of a whole floor.
                state.zoom = 1.0;
                state.pan = Vec2::ZERO;
                state.dirty = true;
                run.armed = true;
                run.timer = 0.0;
                return;
            }
            // The stacked view spawns every cell of every level plus every
            // connection — roughly thirteen thousand entities at production
            // scale. Half a second was enough before links existed; it is not
            // now, and a short window screenshots an empty frame.
            if run.timer >= 2.0 {
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
                run.stage = Stage::Inspect;
                run.armed = false;
                run.timer = 0.0;
                return;
            }
            if !run.armed {
                state.layer = Layer::Single(run.slice);
                state.dirty = true;
                run.armed = true;
                run.timer = 0.0;
                return;
            }
            if run.timer >= 0.8 {
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
        // One frame with a cell pinned, so the inspector has evidence of its
        // own rather than only existing when a human is holding the mouse.
        Stage::Inspect => {
            if !run.armed {
                state.layer = Layer::Single(0);
                // The inspector frame shows the interactive view, close in,
                // because that is how the panel is actually used.
                state.zoom = 0.34;
                // A shaft on the ground floor: the most-placed archetype in the
                // facility, and the one whose tile identity matters most.
                state.selected = state
                    .world
                    .placements
                    .iter()
                    .find(|(coord, placement)| {
                        coord.level == 0
                            && placement.archetype
                                == observed_facility::hex_wfc::HexArchetype::Shaft
                    })
                    .or_else(|| state.world.placements.iter().next())
                    .map(|(coord, _)| *coord);
                state.dirty = true;
                run.armed = true;
                run.timer = 0.0;
                return;
            }
            if run.timer >= 1.2 {
                let path = format!("{}/seed_{}_inspect.png", run.dir, run.seed_index);
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path));
                // The selection is cleared when the next seed starts, not here:
                // `save_to_disk` reads the framebuffer a frame or more later, so
                // clearing now photographs an empty panel.
                run.stage = Stage::Advance;
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
            let (composition, lateral, vertical) = state.composition_census();
            run.manifest.push(serde_json::json!({
                "seed": format!("{:#018x}", state.world.seed),
                "attempts": state.world.last_attempts,
                "rooms": state.world.blueprints.len(),
                "archetypes": census,
                "composition": composition,
                "links": { "lateral": lateral, "vertical": vertical },
                "districts": districts,
            }));

            let next = run.seed_index + 1;
            if next < PRESET_SEEDS.len() {
                run.timer = 0.0;
                run.seed_index = next;
                *state = LabState::new(next);
                run.stage = Stage::Overview;
                run.armed = false;
                run.timer = 0.0;
                return;
            }
            // `save_to_disk` writes on a later frame, so the final seed needs a
            // grace window or its screenshots never reach the disk.
            if run.timer < 1.5 {
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
                    "colour": "district accent, via observed_style::architecture",
                    "height": "hex archetype, via observed_style::hex_sketch",
                    "width": "composition: rooms fill their hex and merge, hallways are ribbons",
                    "bars": "one per open port pair; risers join floors",
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
