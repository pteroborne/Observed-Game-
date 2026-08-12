//! Headless evidence: let the spectator bot play and record what each turn did.
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

use crate::bot::{self, BotDirective};
use crate::settings::MatchSettings;
use crate::sim::relayout::ShiftOutcome;
use crate::sim::unit::PLAYER_TEAM;
use crate::sim::{MatchStatus, TacticsGame};
use crate::view::ViewMode;
use crate::view::setup::SEEDS;
use crate::{AppState, LabSettings, LabState};

/// Turns each captured match plays before it is called.
const CAPTURE_TURNS: u32 = 24;

#[derive(Clone, Copy)]
enum CapturePreset {
    Standard,
    Guided,
}

impl CapturePreset {
    fn settings(self, seed: u64) -> MatchSettings {
        let mut settings = match self {
            Self::Standard => MatchSettings::standard(),
            Self::Guided => MatchSettings::guided(),
        };
        settings.seed = seed;
        settings
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Guided => "guided",
        }
    }
}

#[derive(Resource)]
struct CaptureRun {
    dir: String,
    seed_index: usize,
    turn: u32,
    /// Frames are deferred one tick after the state change that produced them:
    /// the board only reaches the framebuffer on the frame after a rebuild.
    armed: bool,
    view: ViewMode,
    preset: CapturePreset,
    reveal_full_map: bool,
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
    let view = match std::env::var("OBSERVED2_CAPTURE_VIEW").as_deref() {
        Ok("map") => ViewMode::Overview,
        _ => ViewMode::Deck,
    };
    let preset = match std::env::var("OBSERVED2_CAPTURE_PRESET").as_deref() {
        Ok("guided") => CapturePreset::Guided,
        _ => CapturePreset::Standard,
    };
    let reveal_full_map = std::env::var_os("OBSERVED2_CAPTURE_FULL_MAP").is_some();
    app.insert_resource(CaptureRun {
        dir,
        seed_index: 0,
        turn: 0,
        armed: false,
        view,
        preset,
        reveal_full_map,
        manifest: Vec::new(),
    })
    // Both entry points run the same function. Between seeds the run drops to
    // Setup so the next match is rebuilt from fresh settings, and nothing else
    // would ever take it out again — the only other route into Play is the Start
    // button, and a headless run has nobody to press it. Startup covers the first
    // match without depending on whether the initial state transition has already
    // fired, and arming twice is harmless because it is a pure function of
    // `seed_index`.
    .add_systems(Startup, arm_next_match)
    .add_systems(OnEnter(AppState::Setup), arm_next_match)
    .add_systems(Update, capture_progress.run_if(in_state(AppState::Play)));
}

/// Configure and enter the match for the run's current seed.
///
/// Seeds that do not solve are skipped rather than entered. `enter_play` sends a
/// failed solve back to Setup, which is right for a player at the setup screen
/// and would be an unbreakable loop here — Setup re-arms the same seed, which
/// fails again, forever, with nothing written. Checking before entering is what
/// makes the run terminate whatever the corpus contains.
fn arm_next_match(
    run: Option<ResMut<CaptureRun>>,
    mut settings: ResMut<LabSettings>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut run) = run else {
        return;
    };
    while let Some(&seed) = SEEDS.get(run.seed_index) {
        let mut candidate = run.preset.settings(seed);
        candidate.reveal_full_map |= run.reveal_full_map;
        if TacticsGame::new(candidate).is_ok() {
            settings.0 = candidate;
            next.set(AppState::Play);
            return;
        }
        warn!("seed {seed:#x} does not solve; skipping it");
        run.seed_index += 1;
        run.turn = 0;
    }
    // Every remaining seed was unusable, so there is nothing left to play.
    exit.write(AppExit::Success);
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
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.mode != run.view {
        state.mode = run.view;
        state.level = state
            .selected
            .and_then(|id| state.game.units.get(&id))
            .map_or(state.level, |unit| unit.cell.level);
        state.dirty = true;
        state.overlay_dirty = true;
        run.armed = true;
        return;
    }
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

    // Exercise the exact policy and ordinary action boundary exposed by the
    // interactive spectator controls. A guard turns a bot regression into a
    // finite, diagnosable capture rather than a hung evidence run.
    for _ in 0..512 {
        match bot::choose(&state.game) {
            BotDirective::Act { unit, action } => {
                state.selected = Some(unit);
                if let Err(refusal) = state.game.apply(unit, action) {
                    warn!("capture bot action {action:?} was refused: {refusal:?}");
                    break;
                }
                if let Some(actor) = state.game.units.get(&unit) {
                    state.level = actor.cell.level;
                }
            }
            BotDirective::EndTurn => {
                crate::resolve_turn(&mut state);
                break;
            }
            BotDirective::Finished => break,
        }
    }
    state.dirty = true;
    state.overlay_dirty = true;
    run.turn += 1;

    let finished = run.turn >= CAPTURE_TURNS || state.game.status != MatchStatus::Running;
    if !finished {
        return;
    }
    run.seed_index += 1;
    run.turn = 0;
    if run.seed_index < SEEDS.len() {
        // Bouncing through Setup rebuilds the match; `arm_next_match` picks the
        // new seed up on the way back.
        next.set(AppState::Setup);
        return;
    }
    let manifest = serde_json::json!({
        "turns_per_match": CAPTURE_TURNS,
        "policy": "spectator_bot",
        "preset": run.preset.label(),
        "force_full_map": run.reveal_full_map,
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
