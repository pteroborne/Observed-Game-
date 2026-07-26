//! Opt-in Phase 96 wall-clock instrumentation for the canonical hex adapter.
//!
//! None of these measurements feed simulation, relayout scheduling, replay, or
//! presentation decisions. `OBSERVED2_CAPTURE_HEX_WFC_PHASE96=<directory>` runs a
//! deterministic production-size audit, captures the first visible frame, and writes a
//! JSON report after the first mutation window. Normal play pays only two absent-resource
//! checks around the fixed step and one absent-resource check in the Update schedule.

mod report;
mod systems;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::HexWfcWorld;
use observed_match::hex_wfc::{HexMatchEventKind, HexWfcGeometrySnapshot};

use self::report::{
    CommitTiming, NOTES, PerformanceGate, Phase96Report, PipelineTiming, ViewTiming,
    gpu_pass_timings, micros, register_timings, stats, system_timings,
};
use super::sim::HexWfcRuntime;
use crate::GameState;

pub(super) const CAPTURE_ENV: &str = "OBSERVED2_CAPTURE_HEX_WFC_PHASE96";
pub(super) const ARC_GATE_ENV: &str = "OBSERVED2_CAPTURE_HEX_WFC_PHASE101";
const DEFAULT_SEED: u64 = 0xF011_FAC1_1177;
const REPORT_TICK: u64 = 600;
/// Overrides [`REPORT_TICK`] so a run can be given a long enough route for the bot to
/// cross several architecture registers. The default 600 ticks is ~10 s of simulation,
/// during which the runner may never leave the register it spawned in — enough to time
/// a facility, not enough to *compare* registers to each other.
pub(super) const REPORT_TICK_ENV: &str = "OBSERVED2_CAPTURE_HEX_WFC_TICKS";
/// Opt in to an uncapped present mode for the duration of a capture.
///
/// The shipping window is `AutoVsync`, which pins every frame the GPU finishes early to
/// the refresh interval. That is correct for play and correct for the Phase 101 gate
/// (whose 16_700 µs p95 threshold is calibrated against it), but it makes *comparative*
/// measurement impossible: any register the GPU can render inside the refresh interval
/// reports the same median as every other, and only the frames that miss vsync differ.
/// Set this to measure what a register actually costs rather than when it misses.
pub(super) const UNCAPPED_ENV: &str = "OBSERVED2_CAPTURE_HEX_WFC_UNCAPPED";
/// Opt in to per-render-pass GPU timing (`RenderDiagnosticsPlugin` plus the wgpu timestamp
/// features it needs). Read in [`crate::run`], because timestamp queries have to be
/// requested when the device is created. Answers "which pass?" rather than "how slow?".
pub(crate) const GPU_PROFILE_ENV: &str = "OBSERVED2_CAPTURE_HEX_WFC_GPU";
const ARC_GATE_COMMITS: usize = 10;
const ARC_GATE_WARMUP_TICK: u64 = 120;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ViewTimingKind {
    Startup,
    MutationRebuild,
}

#[derive(Resource)]
pub(super) struct HexPerfMetrics {
    directory: PathBuf,
    pipeline: PipelineTiming,
    view: Vec<ViewTiming>,
    fixed_microseconds: Vec<u64>,
    frame_microseconds: Vec<u64>,
    /// Frame samples bucketed by the architecture register the runner stood in.
    frame_by_register: BTreeMap<ArchitectureRegister, Vec<u64>>,
    /// Fixed-step (simulation) samples bucketed the same way, so a register's cost can be
    /// attributed to the render path or the simulation rather than guessed at.
    fixed_by_register: BTreeMap<ArchitectureRegister, Vec<u64>>,
    /// Frame samples split by whether the streaming window changed that frame, which
    /// separates steady-state render cost from the cost of cells popping in and out.
    frame_streaming_steady: Vec<u64>,
    frame_streaming_churn: Vec<u64>,
    /// Cells the streaming window flipped on the most recent [`super::view`] pass.
    last_streaming_flips: usize,
    last_visible_pieces: usize,
    peak_visible_cells: usize,
    /// Resident hull-mesh counts bucketed by register — the draw-call load the renderer
    /// was actually carrying in each one.
    pieces_by_register: BTreeMap<ArchitectureRegister, Vec<u64>>,
    /// GPU nanoseconds per render pass, keyed by `(register, pass name)`. Populated only
    /// under [`GPU_PROFILE_ENV`]; this is what names the pass responsible for a drop
    /// instead of leaving it to inference.
    gpu_pass_by_register: BTreeMap<(ArchitectureRegister, String), Vec<u64>>,
    /// Wall time of the main-app schedule (`First` through `Last`), bucketed by register.
    /// Against the whole-frame figure this splits our own systems from the render
    /// sub-app's extract/prepare/queue work, which no pass timestamp covers.
    main_schedule_by_register: BTreeMap<ArchitectureRegister, Vec<u64>>,
    main_schedule_started: Option<Instant>,
    /// Wall time of each main-schedule phase, keyed by `(register, phase)`. Bevy runs
    /// `First → PreUpdate → RunFixedMainLoop → Update → PostUpdate → Last`; stamping the
    /// head of each gives the span of the one before it. This is what says whether a cost
    /// is our game systems (`Update`) or engine propagation (`PostUpdate`).
    phase_by_register: BTreeMap<(ArchitectureRegister, &'static str), Vec<u64>>,
    phase_mark: Option<(&'static str, Instant)>,
    /// Fixed steps executed since the last rendered frame, and the per-frame history of
    /// that count by register. `RunFixedMainLoop` drains an accumulator, so a frame that
    /// overruns the fixed timestep runs several steps to catch up — and if each step is
    /// itself slower than the timestep, the two compound.
    fixed_steps_this_frame: u32,
    fixed_steps_by_register: BTreeMap<ArchitectureRegister, Vec<u64>>,
    /// Cost of a single `route_between_cells` A* from the runner to the exit, and the
    /// length of the route it found. See [`systems::probe_route_cost`].
    route_probe_by_register: BTreeMap<ArchitectureRegister, Vec<u64>>,
    route_length_by_register: BTreeMap<ArchitectureRegister, Vec<u64>>,
    commits: Vec<CommitTiming>,
    fixed_started: Option<Instant>,
    frame_count: u32,
    startup_shots: u8,
    report_written: bool,
    arc_gate: bool,
    report_tick: u64,
    pending_commit_frame: Option<(usize, bool)>,
}

impl HexPerfMetrics {
    fn new(directory: PathBuf, arc_gate: bool) -> Self {
        Self {
            report_tick: std::env::var(REPORT_TICK_ENV)
                .ok()
                .and_then(|value| value.trim().parse::<u64>().ok())
                .filter(|ticks| *ticks > 0)
                .unwrap_or(REPORT_TICK),
            directory,
            pipeline: PipelineTiming::default(),
            view: Vec::new(),
            fixed_microseconds: Vec::new(),
            frame_microseconds: Vec::new(),
            frame_by_register: BTreeMap::new(),
            fixed_by_register: BTreeMap::new(),
            frame_streaming_steady: Vec::new(),
            frame_streaming_churn: Vec::new(),
            last_streaming_flips: 0,
            last_visible_pieces: 0,
            peak_visible_cells: 0,
            pieces_by_register: BTreeMap::new(),
            gpu_pass_by_register: BTreeMap::new(),
            main_schedule_by_register: BTreeMap::new(),
            main_schedule_started: None,
            phase_by_register: BTreeMap::new(),
            phase_mark: None,
            fixed_steps_this_frame: 0,
            fixed_steps_by_register: BTreeMap::new(),
            route_probe_by_register: BTreeMap::new(),
            route_length_by_register: BTreeMap::new(),
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
            Startup,
            uncap_present_mode.run_if(|| std::env::var(UNCAPPED_ENV).is_ok()),
        )
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
    app.add_systems(
        First,
        begin_main_schedule.run_if(in_state(GameState::HexWfc)),
    )
    .add_systems(Last, end_main_schedule.run_if(in_state(GameState::HexWfc)));
    systems::configure(app);
    if std::env::var(GPU_PROFILE_ENV).is_ok() {
        app.add_systems(
            Update,
            sample_gpu_passes
                .after(super::sim::finish_runtime)
                .before(sample_frame_and_capture)
                .run_if(in_state(GameState::HexWfc)),
        );
    }
}

/// Release the vsync cap so frame samples report GPU cost rather than refresh interval.
/// See [`UNCAPPED_ENV`]. Never runs unless that variable is set, so play and the Phase 101
/// gate keep the shipping `AutoVsync`.
fn uncap_present_mode(mut window: Query<&mut Window, With<bevy::window::PrimaryWindow>>) {
    if let Ok(mut window) = window.single_mut() {
        window.present_mode = bevy::window::PresentMode::AutoNoVsync;
    }
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
    metrics.fixed_steps_this_frame = metrics.fixed_steps_this_frame.saturating_add(1);
    if !metrics.arc_gate || runtime.match_state.tick >= ARC_GATE_WARMUP_TICK {
        metrics.fixed_microseconds.push(elapsed);
        if let Some(register) = current_register(&runtime) {
            metrics
                .fixed_by_register
                .entry(register)
                .or_default()
                .push(elapsed);
        }
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

/// Record what the streaming window did this frame. Called from
/// [`crate::hex_wfc::view::sync_streamed_cells`]; a no-op outside evidence mode.
pub(super) fn record_streaming(
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

/// Bucket every render pass's GPU time under the register the runner is standing in.
///
/// Paths look like `render/<pass>/elapsed_gpu` (`main_opaque_pass_3d`, `prepass`, `ssao`,
/// and one `shadow_spot_light_N` / `shadow_point_light_N_M` per shadow-casting light), so
/// the resulting table names the pass responsible for a register's cost. A no-op unless
/// [`GPU_PROFILE_ENV`] put `RenderDiagnosticsPlugin` in the app.
fn sample_gpu_passes(
    runtime: Res<HexWfcRuntime>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    mut metrics: ResMut<HexPerfMetrics>,
) {
    let Some(register) = current_register(&runtime) else {
        return;
    };
    if metrics.arc_gate && runtime.match_state.tick < ARC_GATE_WARMUP_TICK {
        return;
    }
    for diagnostic in diagnostics.iter() {
        let path = diagnostic.path().as_str();
        let Some(rest) = path.strip_prefix("render/") else {
            continue;
        };
        let Some(pass) = rest.strip_suffix("/elapsed_gpu") else {
            continue;
        };
        let Some(value) = diagnostic.value() else {
            continue;
        };
        // Reported in milliseconds; kept as nanoseconds so the sub-microsecond passes
        // stay distinguishable from the ones that are genuinely zero.
        let nanoseconds = (value * 1_000_000.0).round().max(0.0) as u64;
        metrics
            .gpu_pass_by_register
            .entry((register, pass.to_string()))
            .or_default()
            .push(nanoseconds);
    }
}

/// Stamp the start of the main-app schedule. Paired with [`end_main_schedule`] in `Last`.
fn begin_main_schedule(mut metrics: ResMut<HexPerfMetrics>) {
    metrics.main_schedule_started = Some(Instant::now());
}

/// Close the main-schedule bracket. Whatever the frame costs beyond this is the render
/// sub-app plus presentation — the part no `elapsed_gpu` pass span accounts for.
fn end_main_schedule(runtime: Res<HexWfcRuntime>, mut metrics: ResMut<HexPerfMetrics>) {
    let now = Instant::now();
    systems::close_span(&runtime, &mut metrics, now);
    let Some(started) = metrics.main_schedule_started.take() else {
        return;
    };
    let elapsed = micros(now.duration_since(started));
    if let Some(register) = current_register(&runtime) {
        metrics
            .main_schedule_by_register
            .entry(register)
            .or_default()
            .push(elapsed);
    }
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
        if let Some(register) = current_register(&runtime) {
            metrics
                .frame_by_register
                .entry(register)
                .or_default()
                .push(frame_microseconds);
            let pieces = metrics.last_visible_pieces as u64;
            metrics
                .pieces_by_register
                .entry(register)
                .or_default()
                .push(pieces);
            let steps = u64::from(metrics.fixed_steps_this_frame);
            metrics
                .fixed_steps_by_register
                .entry(register)
                .or_default()
                .push(steps);
        }
        metrics.fixed_steps_this_frame = 0;
        if metrics.last_streaming_flips > 0 {
            metrics.frame_streaming_churn.push(frame_microseconds);
        } else {
            metrics.frame_streaming_steady.push(frame_microseconds);
        }
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
        runtime.match_state.tick >= metrics.report_tick
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
    let by_register = register_timings(report::RegisterSamples {
        frame: &metrics.frame_by_register,
        fixed: &metrics.fixed_by_register,
        pieces: &metrics.pieces_by_register,
        main_schedule: &metrics.main_schedule_by_register,
        fixed_steps: &metrics.fixed_steps_by_register,
        route_cost: &metrics.route_probe_by_register,
        route_cells: &metrics.route_length_by_register,
    });
    let gpu_passes = gpu_pass_timings(&metrics.gpu_pass_by_register);
    let systems = system_timings(&metrics.phase_by_register);
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
        // 3: adds `by_register`, so a report can be read against the per-district
        // key-shadow cost without re-deriving the palette.
        schema_version: 3,
        seed: runtime.match_state.seed,
        generation: runtime.match_state.facility.generation,
        grid: [
            u64::from(config.cols),
            u64::from(config.rows),
            u64::from(config.levels),
        ],
        vsync_uncapped: std::env::var(UNCAPPED_ENV).is_ok(),
        pipeline: metrics.pipeline,
        view: &metrics.view,
        fixed: stats(&metrics.fixed_microseconds),
        frame,
        by_register: &by_register,
        gpu_passes: &gpu_passes,
        systems: &systems,
        streaming_churn_frames: stats(&metrics.frame_streaming_churn),
        streaming_steady_frames: stats(&metrics.frame_streaming_steady),
        peak_visible_cells: metrics.peak_visible_cells,
        commits: &metrics.commits,
        gate,
        notes: NOTES,
    };
    let json = serde_json::to_string_pretty(&report).expect("Phase 96 report serializes");
    std::fs::write(metrics.directory.join("timings.json"), json)
        .expect("Phase 96 timing report writes");
    metrics.report_written = true;
}

/// The architecture register the runner currently stands in — the same lookup the view's
/// [`crate::hex_wfc::view::sync_lighting_and_atmosphere`] uses to pick the palette, so a
/// frame is attributed to exactly the register whose key light was staged for it.
fn current_register(runtime: &HexWfcRuntime) -> Option<ArchitectureRegister> {
    runtime
        .match_state
        .facility
        .architecture
        .get(&runtime.local().cell)
        .copied()
}
