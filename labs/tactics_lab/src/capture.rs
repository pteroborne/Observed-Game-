//! Headless evidence: play a scripted match and record what each turn did.
//!
//! `OBSERVED2_CAPTURE=<dir>` plays one match per seed with a deterministic
//! policy and writes `manifest.json` — a per-turn record of how many cells the
//! facility actually changed, how many the squad held, and how much of the map
//! the squad knew.
//!
//! Numbers rather than only screenshots, because the question this lab exists to
//! answer is comparative: *did that settings change make the facility's
//! behaviour more readable?* Two runs of images cannot be diffed by a reader, and
//! two manifests can.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::settings::MatchSettings;
use crate::sim::relayout::ShiftOutcome;
use crate::sim::unit::PLAYER_TEAM;
use crate::sim::{MatchStatus, TacticsGame};
use crate::view::setup::SEEDS;
use crate::{AppState, LabSettings, LabState};

/// Turns each captured match plays before it is called.
const CAPTURE_TURNS: u32 = 24;

#[derive(Resource)]
struct CaptureRun {
    dir: String,
    seed_index: usize,
    turn: u32,
    /// Frames are deferred one tick after the state change that produced them:
    /// the board only reaches the framebuffer on the frame after a rebuild.
    armed: bool,
    manifest: Vec<serde_json::Value>,
}

/// Install the capture harness when the environment asks for it.
pub fn configure(app: &mut App) {
    let Ok(dir) = std::env::var("OBSERVED2_CAPTURE") else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        warn!("capture directory {dir} could not be created; capture disabled");
        return;
    }
    app.insert_resource(CaptureRun {
        dir,
        seed_index: 0,
        turn: 0,
        armed: false,
        manifest: Vec::new(),
    })
    .add_systems(Startup, start_capture)
    .add_systems(Update, capture_progress.run_if(in_state(AppState::Play)));
}

fn start_capture(mut settings: ResMut<LabSettings>, mut next: ResMut<NextState<AppState>>) {
    settings.0 = capture_settings(SEEDS[0]);
    next.set(AppState::Play);
}

/// The settings every captured match runs. Pinned rather than taken from the
/// setup screen so two capture runs are comparable by construction.
#[must_use]
pub fn capture_settings(seed: u64) -> MatchSettings {
    MatchSettings {
        seed,
        ..MatchSettings::standard()
    }
}

fn capture_progress(
    mut commands: Commands,
    mut run: ResMut<CaptureRun>,
    mut state: ResMut<LabState>,
    mut settings: ResMut<LabSettings>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !run.armed {
        run.armed = true;
        return;
    }
    run.armed = false;

    let seed = SEEDS[run.seed_index];
    let path = format!(
        "{}/tactics_seed{:02}_turn{:02}.png",
        run.dir, run.seed_index, run.turn
    );
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
    run.manifest.push(turn_record(&state.game, seed));

    // A deterministic policy: walk every unit as far as it can toward the exit,
    // then end the turn. Not clever, and not meant to be — it is a fixed ruler
    // held against two different rule configurations.
    let goal = state.game.world.config.exit();
    let ids: Vec<observed_core::PlayerId> = state
        .game
        .units
        .values()
        .filter(|unit| unit.team == PLAYER_TEAM)
        .map(|unit| unit.id)
        .collect();
    for id in ids {
        state.selected = Some(id);
        state.move_selected_toward(goal);
    }
    state.game.end_turn();
    state.dirty = true;
    run.turn += 1;

    let finished = run.turn >= CAPTURE_TURNS || state.game.status != MatchStatus::Running;
    if !finished {
        return;
    }
    run.seed_index += 1;
    run.turn = 0;
    if run.seed_index < SEEDS.len() {
        settings.0 = capture_settings(SEEDS[run.seed_index]);
        // Re-entering Play rebuilds the match from the new settings.
        next.set(AppState::Setup);
        return;
    }
    let manifest = serde_json::json!({
        "turns_per_match": CAPTURE_TURNS,
        "seeds": SEEDS.iter().map(|seed| format!("{seed:#x}")).collect::<Vec<_>>(),
        "records": run.manifest,
    });
    let path = format!("{}/manifest.json", run.dir);
    if let Ok(text) = serde_json::to_string_pretty(&manifest)
        && std::fs::write(&path, text).is_err()
    {
        warn!("could not write {path}");
    }
    exit.write(AppExit::Success);
}

/// What one turn is worth recording.
#[must_use]
pub fn turn_record(game: &TacticsGame, seed: u64) -> serde_json::Value {
    serde_json::json!({
        "seed": format!("{seed:#x}"),
        "turn": game.turn,
        "generation": game.world.generation,
        "known_cells": game
            .knowledge
            .get(&PLAYER_TEAM)
            .map_or(0, |map| map.cells.len()),
        "observed_cells": game.observation.visible_cells.len(),
        "anchored_cells": game.anchored_cells().len(),
        "telegraphed_cells": game
            .telegraph
            .as_ref()
            .map_or(0, |telegraph| telegraph.cells().len()),
        "last_shift": match game.last_shift {
            Some(ShiftOutcome::Committed) => "committed",
            Some(ShiftOutcome::Held) => "held",
            Some(ShiftOutcome::NothingToShift) => "none",
            None => "none",
        },
        "keystones": game.objectives.team(PLAYER_TEAM).keystones,
        "status": match game.status {
            MatchStatus::Running => "running",
            MatchStatus::Escaped => "escaped",
            MatchStatus::Outrun => "outrun",
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record that omitted the shift outcome would make two capture runs
    /// incomparable on the one axis the lab exists to measure.
    #[test]
    fn a_turn_record_reports_what_the_facility_did() {
        let game = TacticsGame::new(capture_settings(SEEDS[0])).expect("solves");
        let record = turn_record(&game, SEEDS[0]);
        for key in [
            "seed",
            "turn",
            "generation",
            "known_cells",
            "observed_cells",
            "telegraphed_cells",
            "last_shift",
            "status",
        ] {
            assert!(record.get(key).is_some(), "record is missing {key}");
        }
    }

    #[test]
    fn every_offered_seed_produces_a_capturable_match() {
        for seed in SEEDS {
            assert!(
                TacticsGame::new(capture_settings(seed)).is_ok(),
                "seed {seed:#x} does not solve"
            );
        }
    }
}
