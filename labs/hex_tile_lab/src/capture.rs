//! `OBSERVED2_CAPTURE` evidence pass: one still per curated family
//! representative, then a scripted first-person diagonal-ramp ascent, then a
//! `manifest.json` recording the library size and the measured height gain.
//!
//! The library is ~800 tiles, so this shoots a hand-picked selection (the
//! three straight interiors, diagonal-face pieces, the blueprint room cells,
//! and a cross-register trim comparison) rather than every tile.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_authoring::TilePrototype;

use crate::LabState;

/// Curated capture set: `(register, archetype, variant)`.
const CAPTURE_SET: &[(&str, &str, u16)] = &[
    ("megastructure", "hall_straight", 0),
    ("megastructure", "hall_straight", 1),
    ("megastructure", "hall_straight", 2),
    ("institutional", "hall_straight", 0),
    ("monolith", "hall_straight", 0),
    ("thinning", "hall_straight", 0),
    ("megastructure", "hall_straight", 4), // SE/NW diagonal axis, colonnade
    ("megastructure", "hall_cap", 0),
    ("megastructure", "hall_cap", 1),    // diagonal-face door
    ("megastructure", "hall_corner", 1), // E + SE, 60 degrees
    ("megastructure", "hall_corner", 2), // E + SW, 120 degrees
    ("megastructure", "hall_junction", 0b010101), // three-way E/SW/NW
    ("megastructure", "hall_junction", 0b001111), // four-way E/SE/SW/W
    ("megastructure", "ramp", 0),
    ("megastructure", "ramp", 2), // SouthWest exit — the walked diagonal
    ("megastructure", "shaft", 0),
    ("megastructure", "shaft_top", 0),
    ("megastructure", "shaft_bottom", 0),
    ("megastructure", "shaft_landing", 1), // diagonal-face landing door
    ("megastructure", "room_single", 0),
    ("megastructure", "room_double_west", 0),
    ("megastructure", "room_double_se", 0),
    ("megastructure", "room_tri_b", 0),
    ("megastructure", "room_fork_c", 0),
    ("megastructure", "room_atrium_lower", 0),
    ("megastructure", "room_atrium_upper", 0),
];

/// The ramp the scripted first-person ascent climbs: a diagonal (SouthWest
/// exit) prefab, proving a non-East direction walks clean.
const WALK_RAMP: (&str, &str, u16) = ("megastructure", "ramp", 2);

fn find_tile(tiles: &[TilePrototype], key: (&str, &str, u16)) -> Option<usize> {
    tiles.iter().position(|tile| {
        tile.key.register == key.0 && tile.key.archetype == key.1 && tile.key.variant == key.2
    })
}

#[derive(Resource)]
pub struct CaptureRun {
    dir: String,
    phase: usize,
    timer: f32,
    walk_frame: u32,
    start_feet: f32,
    max_feet: f32,
    finished: bool,
}

impl CaptureRun {
    pub fn new(dir: String) -> Self {
        std::fs::create_dir_all(&dir).expect("capture dir must be creatable");
        Self {
            dir,
            phase: 0,
            timer: 0.0,
            walk_frame: 0,
            start_feet: 0.0,
            max_feet: f32::MIN,
            finished: false,
        }
    }
}

/// Capture script: one still per curated tile, then a scripted diagonal-ramp
/// ascent with periodic frames, then the manifest (including the measured
/// height gain).
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
    let targets: Vec<usize> = CAPTURE_SET
        .iter()
        .filter_map(|&key| find_tile(&state.tiles, key))
        .collect();

    if run.phase == 0 {
        // First tick: jump to the first capture target.
        state.switch(targets[0]);
        run.phase = 1;
        run.timer = 0.0;
        return;
    }

    let still_phases = targets.len() * 2;
    if run.phase <= still_phases {
        // Even slots settle + screenshot the current tile; odd slots wait for
        // the async save before switching, so frames never bleed across.
        let slot = run.phase - 1;
        if slot.is_multiple_of(2) && run.timer >= 0.9 {
            let key = &state.tiles[targets[slot / 2]].key;
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!(
                    "{}/tile_{}_{}_v{}.png",
                    run.dir, key.register, key.archetype, key.variant
                )));
            run.phase += 1;
            run.timer = 0.0;
        } else if !slot.is_multiple_of(2) && run.timer >= 0.5 {
            run.phase += 1;
            run.timer = 0.0;
            if run.phase <= still_phases {
                state.switch(targets[(run.phase - 1) / 2]);
            } else {
                // Set up the scripted ramp ascent (step_body drives it).
                let ramp = find_tile(&state.tiles, WALK_RAMP).expect("walk ramp present");
                state.switch(ramp);
                run.start_feet = state.feet_height();
                state.scripted_walk = true;
                // Near-level gaze while walking: the slope and walls stay in
                // frame instead of the doorway void filling the view.
                state.body.pitch = 0.10;
            }
        }
        return;
    }

    // Scripted ascent runs in step_body; sample height and frames here.
    let feet = state.feet_height();
    run.max_feet = run.max_feet.max(feet);
    // Stop at the summit instead of marching out the level-1 door and off
    // the tile — the remaining frames hold the top-of-ramp view.
    if state.scripted_walk && feet >= observed_hex::TILE_LEVEL_HEIGHT + 0.35 {
        state.scripted_walk = false;
    }
    if run.timer >= 0.6 && run.walk_frame < 8 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(format!(
                "{}/ramp_walk_{:03}.png",
                run.dir, run.walk_frame
            )));
        run.walk_frame += 1;
        run.timer = 0.0;
    }
    if run.walk_frame >= 8 && run.timer >= 1.0 {
        let gain = run.max_feet - run.start_feet;
        let captured: Vec<String> = CAPTURE_SET
            .iter()
            .filter(|&&key| find_tile(&state.tiles, key).is_some())
            .map(|(reg, arch, variant)| format!("{reg}/{arch}/v{variant}"))
            .collect();
        let manifest = serde_json::json!({
            "lab": "hex_tile_lab",
            "library_tiles": state.tiles.len(),
            "captured": captured,
            "walk_ramp": format!("{}/{}/v{}", WALK_RAMP.0, WALK_RAMP.1, WALK_RAMP.2),
            "ramp_start_feet_m": run.start_feet,
            "ramp_max_feet_m": run.max_feet,
            "ramp_height_gain_m": gain,
            "ramp_gate_passed": gain >= 7.4,
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
