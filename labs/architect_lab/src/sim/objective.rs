//! What the Rogue Architecture Intelligence wants, and the primitive it is built from.
//!
//! The objectives catalogue holds fourteen candidates. They reduce to three questions the
//! simulation has to answer every tick — match a shape, evaluate a topology property, hold
//! a state for N consecutive evaluations. This module implements the third, which is the
//! cheapest of them and needs no machinery that does not already exist.
//!
//! See `docs/objective_primitives.md` for the reasoning and for why the catalogue's
//! wording of Darkness could never have fired.

/// Has a condition held for N consecutive evaluations?
///
/// Reports progress rather than a boolean, because an objective that never completes and
/// one that nearly completes every match are indistinguishable from a boolean, and telling
/// those two apart is the whole reason the primitive exists.
///
/// The primitive is unit-agnostic: it counts consecutive calls to [`StateHold::observe`],
/// and the caller decides the cadence. Darkness evaluates once per actor beat, because
/// observation is only refreshed on a beat.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateHold {
    /// Consecutive evaluations required to complete.
    pub required: u64,
    /// Consecutive evaluations the condition has held for, right now.
    pub streak: u64,
    /// The longest streak seen this match, whether or not it completed.
    pub longest: u64,
    /// Every evaluation the condition held, consecutive or not.
    pub total_held: u64,
    /// The tick the hold completed, if it has.
    pub completed_at: Option<u64>,
}

impl StateHold {
    #[must_use]
    pub const fn new(required: u64) -> Self {
        Self {
            required,
            streak: 0,
            longest: 0,
            total_held: 0,
            completed_at: None,
        }
    }

    /// Record one evaluation. Returns true on the evaluation that completes the hold, and
    /// only that one, so a caller can act on the edge rather than the level.
    ///
    /// A condition going false resets the streak to zero: consecutive means consecutive.
    /// Completion is sticky — once the hold has completed, later evaluations still update
    /// the progress counters but cannot complete it a second time.
    pub fn observe(&mut self, held: bool, tick: u64) -> bool {
        if !held {
            self.streak = 0;
            return false;
        }
        self.streak += 1;
        self.total_held += 1;
        self.longest = self.longest.max(self.streak);
        if self.completed_at.is_none() && self.streak >= self.required {
            self.completed_at = Some(tick);
            return true;
        }
        false
    }

    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.completed_at.is_some()
    }

    /// How close the condition has come, as a fraction of what it needs. Saturates at 1.0.
    #[must_use]
    pub fn best_progress(&self) -> f32 {
        if self.required == 0 {
            return 1.0;
        }
        (self.longest as f32 / self.required as f32).min(1.0)
    }
}

/// The condition under which the Rogue wins this match.
///
/// `Purge` is the behaviour every match has had until now and stays the default, so an
/// objective that is not selected changes nothing. Selecting another does not disable it:
/// a Rogue always wins by emptying the facility of Active Observers, and an objective is
/// an *additional* route.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub enum RogueObjective {
    /// Eliminate every Observer. Jailed or corrupted, it does not care which.
    #[default]
    Purge,
    /// Go unwitnessed: hold the facility dark for [`DARKNESS_BEATS`] consecutive beats.
    Darkness,
}

impl RogueObjective {
    pub const ALL: [Self; 2] = [Self::Purge, Self::Darkness];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Purge => "Purge",
            Self::Darkness => "Darkness",
        }
    }

    /// One line an Observer could infer the objective from, for the legend.
    #[must_use]
    pub const fn tell(self) -> &'static str {
        match self {
            Self::Purge => "Guardian density rising, routes bending toward you",
            Self::Darkness => "The floors going quiet, and staying quiet",
        }
    }
}

/// Consecutive dark beats the Rogue needs to win by Darkness.
///
/// A beat is `ACTOR_BEAT_TICKS` ticks, and observation only changes on a beat, so beats are
/// the honest unit.
///
/// The number was chosen against the measured streak distribution, and the measurement
/// says the dial has only two settings. Bot matches produce a cluster of short streaks
/// from ordinary movement (1-10 beats) and then exactly one enormous streak per match
/// (52, 149, 560 beats in Quick Climb, Full Ascent and Deep Stack), with nothing in
/// between. Any threshold from 11 to 50 therefore behaves identically: it ignores the
/// noise and fires on the blackout. Twelve sits just past the noise.
///
/// That is not a well-tuned objective, and the comment should not pretend otherwise. The
/// facility is dark for 82-92% of beats in every mode, because floors lose power and
/// nothing restores it — see `why_the_facility_is_dark` in the playtest instrument, and
/// bug #44. Until power can come back on, Darkness fires early in every non-Pocket match
/// regardless of this constant. The measurement is the deliverable; the dial is waiting
/// on the economy.
pub const DARKNESS_BEATS: u64 = 12;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broken_streak_starts_over() {
        let mut hold = StateHold::new(3);
        assert!(!hold.observe(true, 1));
        assert!(!hold.observe(true, 2));
        assert!(!hold.observe(false, 3));
        assert_eq!(hold.streak, 0);
        // Two more would have completed it had the streak survived.
        assert!(!hold.observe(true, 4));
        assert!(!hold.observe(true, 5));
        assert!(!hold.is_complete());
        assert!(hold.observe(true, 6));
        assert_eq!(hold.completed_at, Some(6));
    }

    #[test]
    fn progress_survives_a_streak_that_never_completed() {
        let mut hold = StateHold::new(10);
        for tick in 1..=7 {
            hold.observe(true, tick);
        }
        hold.observe(false, 8);
        assert!(!hold.is_complete());
        assert_eq!(hold.longest, 7, "a near miss has to remain visible");
        assert_eq!(hold.total_held, 7);
        assert!((hold.best_progress() - 0.7).abs() < 1e-6);
    }

    #[test]
    fn completion_fires_once_and_keeps_its_tick() {
        let mut hold = StateHold::new(2);
        assert!(!hold.observe(true, 10));
        assert!(hold.observe(true, 20));
        assert!(
            !hold.observe(true, 30),
            "completion is an edge, not a level"
        );
        assert_eq!(hold.completed_at, Some(20));
        assert_eq!(hold.total_held, 3, "progress keeps accruing after the win");
    }

    #[test]
    fn a_hold_of_one_completes_immediately() {
        let mut hold = StateHold::new(1);
        assert!(hold.observe(true, 5));
        assert_eq!(hold.longest, 1);
    }
}
