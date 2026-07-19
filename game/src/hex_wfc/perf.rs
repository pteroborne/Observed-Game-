//! Opt-in Phase 96 wall-clock instrumentation for the canonical hex adapter.
//!
//! None of these measurements feed simulation, relayout scheduling, replay, or
//! presentation decisions. `OBSERVED2_CAPTURE_HEX_WFC_PHASE96=<directory>` runs a
//! deterministic production-size audit, captures the first visible frame, and writes a
//! JSON report after the first mutation window. Normal play pays only two absent-resource
//! checks around the fixed step and one absent-resource check in the Update schedule.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_facility::hex_wfc::HexWfcWorld;
use observed_match::hex_wfc::{HexMatchEventKind, HexWfcGeometrySnapshot};
use serde::Serialize;

use super::sim::HexWfcRuntime;
use crate::GameState;

pub(super) const CAPTURE_ENV: &str = "OBSERVED2_CAPTURE_HEX_WFC_PHASE96";
pub(super) const ARC_GATE_ENV: &str = "OBSERVED2_CAPTURE_HEX_WFC_PHASE101";
const DEFAULT_SEED: u64 = 0xF011_FAC1_1177;
const REPORT_TICK: u64 = 600;
const ARC_GATE_COMMITS: usize = 10;
const ARC_GATE_WARMUP_TICK: u64 = 120;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ViewTimingKind {
    Startup,
    MutationRebuild,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ViewTiming {
    kind: &'static str,
    tick: u64,
    generation: u32,
    microseconds: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct CommitTiming {
    tick: u64,
    generation: u32,
    fixed_microseconds: u64,
    frame_microseconds: u64,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
struct PipelineTiming {
    solve_microseconds: u64,
    projection_microseconds: u64,
    collider_scene_microseconds: u64,
    cells: usize,
    geometry_pieces: usize,
    colliders: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
struct TimingStats {
    samples: usize,
    minimum_microseconds: u64,
    median_microseconds: u64,
    p95_microseconds: u64,
    maximum_microseconds: u64,
    mean_microseconds: u64,
}

#[derive(Serialize)]
struct Phase96Report<'a> {
    schema_version: u16,
    seed: u64,
    generation: u32,
    grid: [u64; 3],
    pipeline: PipelineTiming,
    view: &'a [ViewTiming],
    fixed: TimingStats,
    frame: TimingStats,
    commits: &'a [CommitTiming],
    gate: PerformanceGate,
    notes: [&'static str; 4],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
struct PerformanceGate {
    required_commits: usize,
    observed_commits: usize,
    p95_frame_microseconds: u64,
    maximum_mutation_frame_microseconds: u64,
    passed: bool,
}

#[derive(Resource)]
pub(super) struct HexPerfMetrics {
    directory: PathBuf,
    pipeline: PipelineTiming,
    view: Vec<ViewTiming>,
    fixed_microseconds: Vec<u64>,
    frame_microseconds: Vec<u64>,
    commits: Vec<CommitTiming>,
    fixed_started: Option<Instant>,
    frame_count: u32,
    startup_shots: u8,
    report_written: bool,
    arc_gate: bool,
    pending_commit_frame: Option<(usize, bool)>,
}

impl HexPerfMetrics {
    fn new(directory: PathBuf, arc_gate: bool) -> Self {
        Self {
            directory,
            pipeline: PipelineTiming::default(),
            view: Vec::new(),
            fixed_microseconds: Vec::new(),
            frame_microseconds: Vec::new(),
            commits: Vec::new(),
            fixed_started: None,
            frame_count: 0,
            startup_shots: 0,
            report_written: false,
            arc_gate,
            pending_commit_frame: None,
        }
    }
}

/// Install the evidence-only systems when the capture environment variable is present.
pub(super) fn configure(app: &mut App) {
    let (directory, arc_gate) = if let Ok(directory) = std::env::var(ARC_GATE_ENV) {
        (directory, true)
    } else if let Ok(directory) = std::env::var(CAPTURE_ENV) {
        (directory, false)
    } else {
        return;
    };
    let directory = PathBuf::from(directory);
    std::fs::create_dir_all(&directory)
        .expect("Phase 96 performance evidence directory must be creatable");
    app.insert_resource(HexPerfMetrics::new(directory, arc_gate))
        .add_systems(Startup, autostart)
        .add_systems(
            OnEnter(GameState::HexWfc),
            profile_pipeline.after(super::view::setup_view),
        )
        .add_systems(
            Update,
            sample_frame_and_capture
                .after(super::sim::finish_runtime)
                .run_if(in_state(GameState::HexWfc)),
        );
}

fn autostart(mut commands: Commands, mut next: ResMut<NextState<GameState>>) {
    let seed = std::env::var(crate::flow::SEED_OVERRIDE_ENV)
        .ok()
        .and_then(|value| crate::flow::parse_seed_override(&value))
        .unwrap_or(DEFAULT_SEED);
    commands.insert_resource(crate::flow::ActiveMatchSeed(seed));
    commands.insert_resource(crate::sim::state::SpectatorBot::for_seed(seed));
    next.set(GameState::HexWfc);
}

/// Re-run the exact public production pipeline only in evidence mode so its three
/// otherwise-opaque construction phases have independent wall-clock measurements.
/// The measured artifacts are discarded; the live match remains the authoritative one.
fn profile_pipeline(mut metrics: ResMut<HexPerfMetrics>, runtime: Res<HexWfcRuntime>) {
    if metrics.arc_gate {
        return;
    }
    let seed = runtime.match_state.seed;
    let config = runtime.match_state.facility.config;
    let prototypes = super::sim::load_prototypes();

    let started = Instant::now();
    let world = HexWfcWorld::generate(seed, config).expect("profile seed must reproduce");
    let solve = started.elapsed();

    let started = Instant::now();
    let geometry = HexWfcGeometrySnapshot::project(&world, &prototypes)
        .expect("profile world must project through the committed tile corpus");
    let projection = started.elapsed();

    let started = Instant::now();
    let _scene = geometry.rapier_scene();
    let collider_scene = started.elapsed();

    metrics.pipeline = PipelineTiming {
        solve_microseconds: micros(solve),
        projection_microseconds: micros(projection),
        collider_scene_microseconds: micros(collider_scene),
        cells: world.placements.len(),
        geometry_pieces: geometry.pieces.len(),
        colliders: geometry.arena.colliders.len(),
    };
}

pub(super) fn begin_fixed(mut metrics: Option<ResMut<HexPerfMetrics>>) {
    if let Some(metrics) = metrics.as_deref_mut() {
        metrics.fixed_started = Some(Instant::now());
    }
}

pub(super) fn end_fixed(
    mut metrics: Option<ResMut<HexPerfMetrics>>,
    runtime: Option<Res<HexWfcRuntime>>,
) {
    let (Some(metrics), Some(runtime)) = (metrics.as_deref_mut(), runtime) else {
        return;
    };
    let Some(started) = metrics.fixed_started.take() else {
        return;
    };
    let elapsed = micros(started.elapsed());
    if !metrics.arc_gate || runtime.match_state.tick >= ARC_GATE_WARMUP_TICK {
        metrics.fixed_microseconds.push(elapsed);
    }
    if runtime
        .match_state
        .recent_events
        .iter()
        .any(|event| event.kind == HexMatchEventKind::MutationCommitted)
    {
        let index = metrics.commits.len();
        metrics.commits.push(CommitTiming {
            tick: runtime.match_state.tick,
            generation: runtime.match_state.facility.generation,
            fixed_microseconds: elapsed,
            frame_microseconds: 0,
        });
        metrics.pending_commit_frame = Some((index, false));
    }
}

pub(super) fn record_view(
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

fn sample_frame_and_capture(
    time: Res<Time>,
    runtime: Res<HexWfcRuntime>,
    mut metrics: ResMut<HexPerfMetrics>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    metrics.frame_count = metrics.frame_count.saturating_add(1);
    let frame_microseconds = micros(time.delta());
    if !metrics.arc_gate || runtime.match_state.tick >= ARC_GATE_WARMUP_TICK {
        metrics.frame_microseconds.push(frame_microseconds);
    }
    if let Some((index, armed)) = metrics.pending_commit_frame {
        if armed {
            if let Some(commit) = metrics.commits.get_mut(index) {
                commit.frame_microseconds = frame_microseconds;
            }
            metrics.pending_commit_frame = None;
        } else {
            metrics.pending_commit_frame = Some((index, true));
        }
    }

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
        if metrics.arc_gate {
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

    let ready = if metrics.arc_gate {
        metrics.commits.len() >= ARC_GATE_COMMITS
            && metrics
                .commits
                .iter()
                .all(|commit| commit.frame_microseconds > 0)
    } else {
        runtime.match_state.tick >= REPORT_TICK
    };
    if !ready || metrics.report_written {
        return;
    }
    write_report(&mut metrics, &runtime);
    exit.write(AppExit::Success);
}

fn write_report(metrics: &mut HexPerfMetrics, runtime: &HexWfcRuntime) {
    let config = runtime.match_state.facility.config;
    let frame = stats(&metrics.frame_microseconds);
    let maximum_mutation_frame_microseconds = metrics
        .commits
        .iter()
        .map(|commit| commit.frame_microseconds)
        .max()
        .unwrap_or(0);
    let required_commits = if metrics.arc_gate {
        ARC_GATE_COMMITS
    } else {
        0
    };
    let gate = PerformanceGate {
        required_commits,
        observed_commits: metrics.commits.len(),
        p95_frame_microseconds: frame.p95_microseconds,
        maximum_mutation_frame_microseconds,
        passed: !metrics.arc_gate
            || (metrics.commits.len() >= ARC_GATE_COMMITS
                && frame.p95_microseconds <= 16_700
                && maximum_mutation_frame_microseconds <= 33_300),
    };
    let report = Phase96Report {
        schema_version: 2,
        seed: runtime.match_state.seed,
        generation: runtime.match_state.facility.generation,
        grid: [
            u64::from(config.cols),
            u64::from(config.rows),
            u64::from(config.levels),
        ],
        pipeline: metrics.pipeline,
        view: &metrics.view,
        fixed: stats(&metrics.fixed_microseconds),
        frame,
        commits: &metrics.commits,
        gate,
        notes: [
            "wall-clock evidence only; never read by simulation",
            "pipeline probe reproduces the live seed/config and discards its artifacts",
            "frame samples are Bevy Time deltas and include presentation/present delay",
            "mutation frame is the next measured frame, which contains prior-frame fixed/update work",
        ],
    };
    let json = serde_json::to_string_pretty(&report).expect("Phase 96 report serializes");
    std::fs::write(metrics.directory.join("timings.json"), json)
        .expect("Phase 96 timing report writes");
    metrics.report_written = true;
}

fn micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

fn stats(samples: &[u64]) -> TimingStats {
    if samples.is_empty() {
        return TimingStats::default();
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let percentile = |percent: usize| {
        let rank = sorted.len().saturating_mul(percent).div_ceil(100);
        sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
    };
    let total = sorted
        .iter()
        .map(|&sample| u128::from(sample))
        .sum::<u128>();
    TimingStats {
        samples: sorted.len(),
        minimum_microseconds: sorted[0],
        median_microseconds: percentile(50),
        p95_microseconds: percentile(95),
        maximum_microseconds: *sorted.last().expect("non-empty samples"),
        mean_microseconds: u64::try_from(total / sorted.len() as u128).unwrap_or(u64::MAX),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn percentile_summary_uses_nearest_rank_and_keeps_spikes() {
        let samples = (1..=100).collect::<Vec<_>>();
        let summary = stats(&samples);
        assert_eq!(summary.samples, 100);
        assert_eq!(summary.minimum_microseconds, 1);
        assert_eq!(summary.median_microseconds, 50);
        assert_eq!(summary.p95_microseconds, 95);
        assert_eq!(summary.maximum_microseconds, 100);
        assert_eq!(summary.mean_microseconds, 50);
    }

    #[test]
    fn empty_summary_is_explicitly_zero_sampled() {
        assert_eq!(stats(&[]), TimingStats::default());
    }

    #[test]
    fn report_paths_are_scoped_under_the_requested_directory() {
        let directory = Path::new("docs/evidence/arc_m/phase_96");
        assert_eq!(
            directory.join("timings.json"),
            Path::new("docs/evidence/arc_m/phase_96/timings.json")
        );
        assert_eq!(
            directory.join("first_visible_frame.png"),
            Path::new("docs/evidence/arc_m/phase_96/first_visible_frame.png")
        );
        assert_eq!(
            directory.join("startup_frame_002.png"),
            Path::new("docs/evidence/arc_m/phase_96/startup_frame_002.png")
        );
    }
}
