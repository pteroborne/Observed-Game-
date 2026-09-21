//! How the Rogue Architecture Intelligence spends its turn.
//!
//! The loyal Architect is a person with hands, and a five-second cooldown between tiles is
//! an honest model of one. The RAI is not a person; it is the building. Giving both the
//! same economy made the City *twitch* — one tile, react, one tile, react — when what it
//! should do is **reconfigure**.
//!
//! Under [`RogueEconomy::Waves`] the Rogue composes over a window and everything lands at
//! once. The facility changes between glances rather than while you watch, which is the
//! register this game has been aiming at, and it turns the Rogue's play from reaction into
//! planning: you cannot lay a trap one tile at a time on a five-second leash.

use super::{ArchitectCommand, CommandRefusal};

/// Ticks between wave commits — thirty seconds at `FIXED_HZ`.
///
/// Chosen against match length rather than taste: Full Ascent runs a little over 300
/// beats, so a thirty-second window gives roughly ten waves. Few enough that each one is
/// a deliberate act, many enough to answer what the Observers did about the last.
pub const WAVE_INTERVAL_TICKS: u32 = 1800;

/// What the Rogue's turn costs.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RogueEconomy {
    /// One card, then five seconds. The loyal Architect's economy, which the Rogue
    /// inherited for no better reason than that it was there.
    #[default]
    Cooldown,
    /// Queue freely over a window; the whole plan lands together on the boundary.
    Waves,
}

impl RogueEconomy {
    pub const ALL: [Self; 2] = [Self::Cooldown, Self::Waves];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cooldown => "Cooldown",
            Self::Waves => "Waves",
        }
    }
}

/// How much warning the facility gives before a wave lands.
///
/// A dial rather than a decision, because the right answer is a play question. The
/// building already telegraphs retraction over three seconds, so a wave arriving in total
/// silence would be inconsistent as well as possibly unfair — but a City that announces
/// itself is less frightening, and somewhere between those is the game.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WaveTelegraph {
    /// The building simply changes.
    Hidden,
    /// A tell proportional to what is coming: a big wave casts a longer shadow.
    #[default]
    Scaled,
    /// The plan itself is public — every queued placement visible before it lands.
    Public,
}

impl WaveTelegraph {
    pub const ALL: [Self; 3] = [Self::Hidden, Self::Scaled, Self::Public];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Hidden => "Hidden",
            Self::Scaled => "Scaled",
            Self::Public => "Public",
        }
    }
}

/// A queued plan and the record of what became of the last one.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WaveState {
    /// Commands awaiting the next boundary, in submission order.
    ///
    /// Order is the whole resolution rule. Two placements can each be legal alone and
    /// contradictory together, so legality is re-checked at commit and the later one loses.
    /// That is deterministic, replayable, and reads as the City's plan partly failing —
    /// which is better drama than silently dropping it.
    pub pending: Vec<ArchitectCommand>,
    /// What the last commit refused, and why. Worth showing: a plan that half-lands is
    /// information the Rogue should get back.
    pub last_drops: Vec<(ArchitectCommand, CommandRefusal)>,
    /// Placements the last commit actually made.
    pub last_committed: usize,
    /// Waves committed this match.
    pub waves: u64,
    /// Placements that landed across every wave this match.
    pub total_committed: u64,
    /// Placements dropped at commit across every wave, with their reasons tallied.
    pub total_dropped: u64,
    /// Why plans failed, counted. A plan that half-lands is the interesting case, and the
    /// reason it half-landed is the design signal.
    pub drop_reasons: std::collections::BTreeMap<&'static str, u64>,
}

impl WaveState {
    /// What an Observer is allowed to know about the coming wave.
    ///
    /// `Hidden` says nothing. `Scaled` gives the size only, so the tell grows with the
    /// weight of what is coming. `Public` hands over the plan.
    #[must_use]
    pub fn telegraph(&self, policy: WaveTelegraph) -> WaveWarning {
        match policy {
            WaveTelegraph::Hidden => WaveWarning::Silent,
            WaveTelegraph::Scaled => WaveWarning::Weight(self.pending.len()),
            WaveTelegraph::Public => WaveWarning::Plan(self.pending.clone()),
        }
    }
}

/// The visible shadow of a coming wave.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WaveWarning {
    /// The building says nothing.
    Silent,
    /// Something of this size is coming.
    Weight(usize),
    /// Exactly this is coming.
    Plan(Vec<ArchitectCommand>),
}
