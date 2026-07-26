//! The serialised shape of a Phase 96 timing report, and the summarisation that builds it.
//!
//! Split out of [`super`] so the sampling systems and the evidence format can each stay
//! reviewable. Nothing here observes the world: it takes collected samples and produces
//! JSON, so it is unit-testable without a Bevy app.

use std::collections::BTreeMap;
use std::time::Duration;

use observed_content::ArchitectureRegister;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct ViewTiming {
    pub(super) kind: &'static str,
    pub(super) tick: u64,
    pub(super) generation: u32,
    pub(super) microseconds: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct CommitTiming {
    pub(super) tick: u64,
    pub(super) generation: u32,
    pub(super) fixed_microseconds: u64,
    pub(super) frame_microseconds: u64,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub(super) struct PipelineTiming {
    pub(super) solve_microseconds: u64,
    pub(super) projection_microseconds: u64,
    pub(super) collider_scene_microseconds: u64,
    pub(super) cells: usize,
    pub(super) geometry_pieces: usize,
    pub(super) colliders: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub(super) struct TimingStats {
    pub(super) samples: usize,
    pub(super) minimum_microseconds: u64,
    pub(super) median_microseconds: u64,
    pub(super) p95_microseconds: u64,
    pub(super) maximum_microseconds: u64,
    pub(super) mean_microseconds: u64,
}

/// One architecture register's cost, paired with the `observed_style` key-light settings
/// that are the only per-register GPU cost in the hex rig. Reported together so the
/// question "does the drop track the key shadow frustum?" is answerable from the report
/// alone, without cross-referencing the palette by hand.
#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct RegisterTiming {
    pub(super) register: &'static str,
    pub(super) key_shadows_enabled: bool,
    pub(super) key_range: f32,
    pub(super) key_outer_angle: f32,
    pub(super) frame: TimingStats,
    /// The simulation slice of the same frames. A register whose `frame` is slow while
    /// `fixed` stays flat is a render cost; one where both rise is a simulation cost.
    pub(super) fixed: TimingStats,
    /// Median resident hull meshes while the runner stood here (draw-call proxy).
    pub(super) median_visible_pieces: u64,
    /// Wall time of the main-app schedule. `frame - main_schedule` is the render sub-app
    /// and presentation; `main_schedule - fixed` is our own Update/PostUpdate work.
    pub(super) main_schedule: TimingStats,
    /// Median fixed steps run per rendered frame. Above 1 the frame is behind the fixed
    /// timestep and `RunFixedMainLoop` is running catch-up steps; multiply by `fixed` to
    /// see how much of the frame that accounts for.
    pub(super) median_fixed_steps_per_frame: u64,
    /// Cost of ONE `route_between_cells` A* from here to the exit. `bot_command` runs two
    /// of these per bot per tick, so multiply accordingly before comparing with `fixed`.
    pub(super) median_route_microseconds: u64,
    /// Cells in the route that call found, as a proxy for how far A* had to search.
    pub(super) median_route_cells: u64,
}

/// One render pass's GPU cost inside one register. The unit is microseconds with two
/// decimals so cheap passes stay legible next to a 90 ms one.
#[derive(Clone, Debug, Serialize)]
pub(super) struct GpuPassTiming {
    pub(super) register: &'static str,
    pub(super) pass: String,
    pub(super) median_microseconds: f64,
    pub(super) p95_microseconds: f64,
    pub(super) samples: usize,
}

/// One `Update` system's CPU cost inside one register.
#[derive(Clone, Debug, Serialize)]
pub(super) struct SystemTiming {
    pub(super) register: &'static str,
    pub(super) system: &'static str,
    pub(super) median_microseconds: u64,
    pub(super) p95_microseconds: u64,
    pub(super) samples: usize,
}

/// Summarise per-system CPU samples, heaviest median first. Unlike the GPU table this
/// keeps every entry, because a system reading zero is itself the useful answer when the
/// question is "which one of these is the 90 ms?".
pub(super) fn system_timings(
    by_register: &BTreeMap<(ArchitectureRegister, &'static str), Vec<u64>>,
) -> Vec<SystemTiming> {
    let mut rows: Vec<SystemTiming> = by_register
        .iter()
        .map(|((register, system), samples)| {
            let summary = stats(samples);
            SystemTiming {
                register: register.slug(),
                system,
                median_microseconds: summary.median_microseconds,
                p95_microseconds: summary.p95_microseconds,
                samples: summary.samples,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.median_microseconds
            .cmp(&a.median_microseconds)
            .then_with(|| a.register.cmp(b.register))
    });
    rows
}

/// Summarise per-pass GPU samples, heaviest median first, keeping only passes that
/// actually cost something so the shadow-pass-per-light fan-out does not bury the table.
pub(super) fn gpu_pass_timings(
    by_register: &BTreeMap<(ArchitectureRegister, String), Vec<u64>>,
) -> Vec<GpuPassTiming> {
    let mut rows: Vec<GpuPassTiming> = by_register
        .iter()
        .map(|((register, pass), samples)| {
            let summary = stats(samples);
            GpuPassTiming {
                register: register.slug(),
                pass: pass.clone(),
                median_microseconds: summary.median_microseconds as f64 / 1_000.0,
                p95_microseconds: summary.p95_microseconds as f64 / 1_000.0,
                samples: summary.samples,
            }
        })
        .filter(|row| row.median_microseconds >= 1.0)
        .collect();
    rows.sort_by(|a, b| {
        b.median_microseconds
            .total_cmp(&a.median_microseconds)
            .then_with(|| a.register.cmp(b.register))
    });
    rows
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub(super) struct PerformanceGate {
    pub(super) required_commits: usize,
    pub(super) observed_commits: usize,
    pub(super) p95_frame_microseconds: u64,
    pub(super) maximum_mutation_frame_microseconds: u64,
    pub(super) passed: bool,
}

#[derive(Serialize)]
pub(super) struct Phase96Report<'a> {
    pub(super) schema_version: u16,
    pub(super) seed: u64,
    pub(super) generation: u32,
    pub(super) grid: [u64; 3],
    /// True when the run released the vsync cap. Medians from a capped run are pinned to
    /// the refresh interval and are NOT comparable between registers.
    pub(super) vsync_uncapped: bool,
    pub(super) pipeline: PipelineTiming,
    pub(super) view: &'a [ViewTiming],
    pub(super) fixed: TimingStats,
    pub(super) frame: TimingStats,
    pub(super) by_register: &'a [RegisterTiming],
    /// Per-render-pass GPU cost, present only when the run enabled GPU profiling.
    pub(super) gpu_passes: &'a [GpuPassTiming],
    /// Per-`Update`-system CPU cost, bucketed by register.
    pub(super) systems: &'a [SystemTiming],
    /// Frames on which the streaming window flipped at least one cell's visibility,
    /// versus frames on which it did not. Separates the cost of *being* somewhere from
    /// the cost of *arriving* there.
    pub(super) streaming_churn_frames: TimingStats,
    pub(super) streaming_steady_frames: TimingStats,
    pub(super) peak_visible_cells: usize,
    pub(super) commits: &'a [CommitTiming],
    pub(super) gate: PerformanceGate,
    pub(super) notes: [&'static str; 6],
}

pub(super) const NOTES: [&str; 6] = [
    "wall-clock evidence only; never read by simulation",
    "pipeline probe reproduces the live seed/config and discards its artifacts",
    "frame samples are Bevy Time deltas and include presentation/present delay",
    "mutation frame is the next measured frame, which contains prior-frame fixed/update work",
    "by_register buckets each frame by the register the runner stood in; a Time delta describes the previous frame, so single samples astride a cell crossing land in the neighbouring bucket",
    "by_register sample counts are whatever the bot's route happened to visit, not a balanced design; compare medians, and treat thin buckets as unmeasured",
];

pub(super) fn micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

pub(super) fn stats(samples: &[u64]) -> TimingStats {
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

/// The per-register sample sets [`register_timings`] folds into one row each. Bundled
/// rather than passed positionally — seven same-typed maps in a row is an argument-order
/// bug waiting to happen.
pub(super) struct RegisterSamples<'a> {
    pub(super) frame: &'a Samples,
    pub(super) fixed: &'a Samples,
    pub(super) pieces: &'a Samples,
    pub(super) main_schedule: &'a Samples,
    pub(super) fixed_steps: &'a Samples,
    pub(super) route_cost: &'a Samples,
    pub(super) route_cells: &'a Samples,
}

/// Samples collected per architecture register.
pub(super) type Samples = BTreeMap<ArchitectureRegister, Vec<u64>>;

/// Summarise each register's samples alongside the key-light settings that drive its
/// shadow pass. Ordered slowest median first so the report opens on the worst offender.
pub(super) fn register_timings(samples: RegisterSamples<'_>) -> Vec<RegisterTiming> {
    let RegisterSamples {
        frame: by_register,
        fixed: fixed_by_register,
        pieces: pieces_by_register,
        main_schedule: main_by_register,
        fixed_steps: steps_by_register,
        route_cost: route_by_register,
        route_cells: route_cells_by_register,
    } = samples;
    let mut rows: Vec<RegisterTiming> = by_register
        .iter()
        .map(|(&register, samples)| {
            let palette = observed_style::architecture(register);
            RegisterTiming {
                register: register.slug(),
                key_shadows_enabled: palette.key_shadows_enabled,
                key_range: palette.key_range,
                key_outer_angle: palette.key_outer_angle,
                frame: stats(samples),
                fixed: fixed_by_register
                    .get(&register)
                    .map(|samples| stats(samples))
                    .unwrap_or_default(),
                median_visible_pieces: pieces_by_register
                    .get(&register)
                    .map(|samples| stats(samples).median_microseconds)
                    .unwrap_or(0),
                main_schedule: main_by_register
                    .get(&register)
                    .map(|samples| stats(samples))
                    .unwrap_or_default(),
                median_fixed_steps_per_frame: steps_by_register
                    .get(&register)
                    .map(|samples| stats(samples).median_microseconds)
                    .unwrap_or(0),
                median_route_microseconds: route_by_register
                    .get(&register)
                    .map(|samples| stats(samples).median_microseconds)
                    .unwrap_or(0),
                median_route_cells: route_cells_by_register
                    .get(&register)
                    .map(|samples| stats(samples).median_microseconds)
                    .unwrap_or(0),
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.frame
            .median_microseconds
            .cmp(&a.frame.median_microseconds)
            .then_with(|| a.register.cmp(b.register))
    });
    rows
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
    fn register_timings_lead_with_the_slowest_and_carry_their_key_settings() {
        let by_register = BTreeMap::from([
            (ArchitectureRegister::OverlitGrid, vec![8_000, 8_200]),
            (ArchitectureRegister::Megastructure, vec![30_000, 31_000]),
            (ArchitectureRegister::InfiniteGallery, vec![12_000]),
        ]);
        let fixed_by_register =
            BTreeMap::from([(ArchitectureRegister::Megastructure, vec![4_000])]);
        let pieces_by_register =
            BTreeMap::from([(ArchitectureRegister::Megastructure, vec![4_200, 4_200])]);

        let main_by_register = BTreeMap::from([(ArchitectureRegister::Megastructure, vec![9_000])]);
        let steps_by_register = BTreeMap::from([(ArchitectureRegister::Megastructure, vec![6])]);

        let route_by_register =
            BTreeMap::from([(ArchitectureRegister::Megastructure, vec![5_500])]);
        let route_cells_by_register =
            BTreeMap::from([(ArchitectureRegister::Megastructure, vec![41])]);

        let rows = register_timings(RegisterSamples {
            frame: &by_register,
            fixed: &fixed_by_register,
            pieces: &pieces_by_register,
            main_schedule: &main_by_register,
            fixed_steps: &steps_by_register,
            route_cost: &route_by_register,
            route_cells: &route_cells_by_register,
        });

        assert_eq!(
            rows.iter().map(|row| row.register).collect::<Vec<_>>(),
            ["megastructure", "infinite_gallery", "overlit_grid"],
            "slowest median must sort first"
        );
        // Each row must carry the palette settings for its own register, so the report
        // can be read against the key-shadow hypothesis without consulting the style crate.
        for row in &rows {
            let register = ArchitectureRegister::ALL
                .into_iter()
                .find(|candidate| candidate.slug() == row.register)
                .expect("row names a real register");
            let palette = observed_style::architecture(register);
            assert_eq!(row.key_shadows_enabled, palette.key_shadows_enabled);
            assert_eq!(row.key_range, palette.key_range);
        }
        assert_eq!(rows[0].frame.samples, 2);
        assert_eq!(rows[1].frame.samples, 1);
        // The simulation slice is carried per register, and a register with no fixed-step
        // samples reports an explicit zero rather than borrowing another register's.
        assert_eq!(rows[0].fixed.median_microseconds, 4_000);
        assert_eq!(rows[1].fixed, TimingStats::default());
        assert_eq!(rows[0].median_visible_pieces, 4_200);
        assert_eq!(rows[1].median_visible_pieces, 0);
        assert_eq!(rows[0].main_schedule.median_microseconds, 9_000);
        assert_eq!(rows[1].main_schedule, TimingStats::default());
        assert_eq!(rows[0].median_fixed_steps_per_frame, 6);
        assert_eq!(rows[1].median_fixed_steps_per_frame, 0);
        assert_eq!(rows[0].median_route_microseconds, 5_500);
        assert_eq!(rows[0].median_route_cells, 41);
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
