//! Opt-in benchmark telemetry. No effect on ordinary play.
use super::{ARC_GATE_ENV, DEFAULT_SEED, HexPerfMetrics, SOLO_ROUTE_ENV};
use crate::{GameState, hex_wfc::sim::HexWfcRuntime};
use bevy::prelude::*;

pub(super) fn write_progress(
    metrics: Res<HexPerfMetrics>,
    runtime: Option<Res<HexWfcRuntime>>,
    state: Res<State<GameState>>,
    mut frames: Local<u16>,
) {
    *frames = frames.wrapping_add(1);
    if !frames.is_multiple_of(240) {
        return;
    }
    let report = serde_json::json!({"state":format!("{:?}",state.get()),"tick":runtime.as_ref().map(|r|r.match_state.tick),"status":runtime.as_ref().map(|r|format!("{:?}",r.match_state.status)),"commits":metrics.commits.len(),"frames":metrics.frame_count,"players":runtime.as_ref().map(|r|r.match_state.players.len()),"rules":runtime.as_ref().map(|r|if r.ascent.is_some(){"ascent"}else{"race"}),"route_ticks":metrics.route_ticks,"runs":metrics.route_runs});
    let _ = std::fs::write(metrics.directory.join("progress.json"), report.to_string());
}

pub(super) fn autostart(
    mut commands: Commands,
    mut next: ResMut<NextState<GameState>>,
    mut setup: ResMut<crate::play_setup::PlaySetupDraft>,
    mut metrics: ResMut<HexPerfMetrics>,
    mut overview: ResMut<crate::hex_wfc::view::spectate::SpectatorOverview>,
) {
    // The ten-commit gate is the established WFC mutation workload. Persisted
    // Ascent settings must not silently replace it with a finite card match.
    if std::env::var(ARC_GATE_ENV).is_ok() {
        *setup =
            crate::play_setup::PlaySetupDraft::for_preset(crate::play_setup::PlayPreset::TeamRace);
        setup.members_per_team = 4;
    }
    if std::env::var_os(SOLO_ROUTE_ENV).is_some() {
        *setup = crate::play_setup::PlaySetupDraft::default();
        overview.eyes = true;
        overview.active = false;
    }
    metrics.route_runs += 1;
    let seed = std::env::var(crate::flow::SEED_OVERRIDE_ENV)
        .ok()
        .and_then(|value| crate::flow::parse_seed_override(&value))
        .unwrap_or(DEFAULT_SEED);
    commands.insert_resource(crate::flow::ActiveMatchSeed(seed));
    commands.insert_resource(crate::sim::state::SpectatorBot::for_seed(seed));
    next.set(GameState::HexWfc);
}

use super::{ViewTiming, ViewTimingKind, micros};
use std::time::Duration;
/// Record what the streaming window did this frame. Called from
/// [`crate::hex_wfc::view::sync_streamed_cells`]; a no-op outside evidence mode.
pub(in crate::hex_wfc) fn record_streaming(
    metrics: &mut Option<ResMut<HexPerfMetrics>>,
    visible: usize,
    flipped: usize,
    visible_pieces: usize,
) {
    let Some(metrics) = metrics.as_deref_mut() else {
        return;
    };
    metrics.last_streaming_flips = flipped;
    metrics.last_visible_pieces = visible_pieces;
    metrics.peak_visible_cells = metrics.peak_visible_cells.max(visible);
}

pub(in crate::hex_wfc) fn record_view(
    metrics: &mut Option<ResMut<HexPerfMetrics>>,
    kind: ViewTimingKind,
    runtime: &HexWfcRuntime,
    elapsed: Duration,
) {
    let Some(metrics) = metrics.as_deref_mut() else {
        return;
    };
    metrics.view.push(ViewTiming {
        kind: match kind {
            ViewTimingKind::Startup => "startup",
            ViewTimingKind::MutationRebuild => "mutation_delta",
        },
        tick: runtime.match_state.tick,
        generation: runtime.match_state.facility.generation,
        microseconds: micros(elapsed),
    });
}

#[derive(serde::Serialize)]
pub(super) struct Hitch {
    tick: u64,
    frame_us: u64,
    generation: u32,
    cell: String,
    streaming_flips: usize,
    cache_hits_misses: [u64; 2],
    events: Vec<String>,
}
pub(super) fn record_hitch(metrics: &mut HexPerfMetrics, runtime: &HexWfcRuntime, frame_us: u64) {
    metrics.hitches.push(Hitch {
        tick: runtime.match_state.tick,
        frame_us,
        generation: runtime.match_state.geometry.generation,
        cell: format!("{:?}", runtime.viewed().cell),
        streaming_flips: metrics.last_streaming_flips,
        cache_hits_misses: metrics.mesh_cache,
        events: runtime
            .match_state
            .recent_events
            .iter()
            .map(|e| format!("{:?}", e.kind))
            .collect(),
    });
}

use bevy::render::view::screenshot::{Screenshot, save_to_disk};
pub(super) fn startup_shots(metrics: &mut HexPerfMetrics, commands: &mut Commands) {
    // Screenshot extraction lags main-world setup, so keep a geometric sequence of the
    // first render opportunities. The sequence makes the first non-black facility frame
    // falsifiable instead of assuming which extraction frame becomes render-ready.
    const STARTUP_SHOTS: [(u32, u8, &str); 5] = [
        (2, 0b00001, "startup_frame_002.png"),
        (4, 0b00010, "startup_frame_004.png"),
        (8, 0b00100, "startup_frame_008.png"),
        (16, 0b01000, "startup_frame_016.png"),
        (32, 0b10000, "first_visible_frame.png"),
    ];
    for (frame, bit, name) in STARTUP_SHOTS {
        if metrics.arc_gate || std::env::var_os(super::SOLO_ROUTE_ENV).is_some() {
            break;
        }
        if metrics.frame_count >= frame && metrics.startup_shots & bit == 0 {
            metrics.startup_shots |= bit;
            let path = metrics.directory.join(name);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
            break;
        }
    }
}
