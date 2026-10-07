//! Opt-in wall-clock diagnostics; never read by rules, snapshots, or replay decisions.
use super::HexWfcMatch;
use std::time::Instant;

#[derive(Clone, Debug, Default)]
pub(super) struct MutationProfile {
    enabled: bool,
    mark: Option<Instant>,
    phases: Vec<(&'static str, u64)>,
}

impl HexWfcMatch {
    /// Enable evidence-only mutation phase measurements. Cache and timing state are
    /// excluded from authoritative snapshots and cannot change the commit tick.
    pub fn enable_mutation_profiling(&mut self) {
        self.mutation_profile.enabled = true;
    }

    #[must_use]
    pub fn collider_cache_counts(&self) -> [u64; 2] {
        self.physics.shape_cache_counts()
    }

    #[must_use]
    pub fn mutation_phases(&self) -> &[(&'static str, u64)] {
        &self.mutation_profile.phases
    }

    pub(crate) fn begin_mutation_profile(&mut self) {
        if self.mutation_profile.enabled {
            self.mutation_profile.phases.clear();
            self.mutation_profile.mark = Some(Instant::now());
        }
    }

    pub fn mark_mutation_phase(&mut self, phase: &'static str) {
        if self.mutation_profile.enabled
            && let Some(mark) = self.mutation_profile.mark.replace(Instant::now())
        {
            let elapsed = u64::try_from(mark.elapsed().as_micros()).unwrap_or(u64::MAX);
            self.mutation_profile.phases.push((phase, elapsed));
        }
    }
}
