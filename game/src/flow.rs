//! The **career/flow model** that ties the proven systems into one player loop:
//! play the competitive match → record the result → award progression (XP, levels,
//! unlocks) into a persistent profile → repeat. This is the pure-logic spine of the
//! assembled game; the Bevy state machine in `screens` is its presentation.
//!
//! The match is the proven [`competitive_facility`] brain and the profile is the
//! proven `observed_progression` one — reused unchanged. Crucially the match takes no
//! profile, so the cohesive whole still preserves the orthogonality each lab proved:
//! cosmetics/progression never change a result. A test re-asserts that at this
//! integrated level.

use bevy::prelude::Resource;
use observed_core::TeamId;
use observed_match::elimination::EliminationSeries;
use observed_match::facility::CompetitiveFacility;
use observed_net::netmatch::NetMatch;
use observed_net::network::NetworkProfile;
use observed_progression::progression::Profile;

use crate::sim::director::MatchDirector;

/// The team the local player owns across the whole game.
pub const LOCAL_TEAM: TeamId = TeamId(0);

/// The seed the assembled game's match runs on.
pub const MATCH_SEED: u64 = 1;
pub const SEED_OVERRIDE_ENV: &str = "OBSERVED2_SEED";

#[derive(Resource, Copy, Clone, Debug, PartialEq, Eq)]
pub struct ActiveMatchSeed(pub u64);

impl Default for ActiveMatchSeed {
    fn default() -> Self {
        Self(MATCH_SEED)
    }
}

pub fn parse_seed_override(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).ok()
    } else {
        trimmed.parse::<u64>().ok()
    }
}

pub fn launch_seed() -> u64 {
    if let Ok(value) = std::env::var(SEED_OVERRIDE_ENV)
        && let Some(seed) = parse_seed_override(&value)
    {
        return seed;
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| {
            let nanos = d.as_nanos() as u64;
            nanos ^ u64::from(std::process::id()).rotate_left(17)
        })
        .unwrap_or(MATCH_SEED)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchResult {
    pub placement: Option<u8>,
    pub escaped: usize,
    pub absorbed: usize,
    pub winner: Option<TeamId>,
    pub local_won: bool,
}

/// Read the local player's outcome from a finished match.
pub fn resolve(facility: &CompetitiveFacility) -> MatchResult {
    MatchResult {
        placement: facility.team(LOCAL_TEAM).and_then(|t| t.placement),
        escaped: facility.escaped_count(),
        absorbed: facility.absorbed_count(),
        winner: facility.winner,
        local_won: facility.winner == Some(LOCAL_TEAM),
    }
}

/// Read the local player's final outcome from a completed elimination series.
pub fn resolve_series(series: &EliminationSeries) -> MatchResult {
    MatchResult {
        placement: series.placement_for(LOCAL_TEAM),
        escaped: series.escaped_total(),
        absorbed: series.absorbed_total(),
        winner: series.winner,
        local_won: series.winner == Some(LOCAL_TEAM),
    }
}

/// Run a whole deterministic match to its end (headless — used by the career loop
/// and tests). This is the *same* [`MatchDirector`] the interactive Match screen
/// steps frame by frame, run to completion in one call, so a headless career match
/// and an on-screen match of the same seed resolve identically (pinned by the
/// `headless_and_interactive_matches_agree_on_the_result` characterization test).
pub fn play_match() -> MatchResult {
    let mut director =
        MatchDirector::new(MATCH_SEED, crate::map_catalog::active_map_spec(MATCH_SEED));
    director.run_to_completion()
}

/// Run the **networked first-person hybrid match** to convergence and resolve the
/// local team's result. This is the match the interactive Match screen runs (it
/// steps the same lockstep transport on screen); the transport replicates, it does
/// not alter the outcome — see the orthogonality test.
pub fn play_networked_match(seed: u64, profile: NetworkProfile) -> MatchResult {
    let mut net = NetMatch::authored(seed, profile);
    net.run_until_synchronized(100_000);
    resolve(&net.peers[0].match_state.competitive)
}

/// The persistent player career: the profile plus the in-flight result awaiting its
/// reward. Survives across matches (it lives for the whole app, not per-state).
#[derive(Resource)]
pub struct Career {
    pub profile: Profile,
    pub matches_completed: u32,
    pub last_result: Option<MatchResult>,
    pub last_unlocks: Vec<u16>,
    awarded: bool,
}

impl Default for Career {
    fn default() -> Self {
        Self {
            profile: Profile::new(),
            matches_completed: 0,
            last_result: None,
            last_unlocks: Vec::new(),
            awarded: false,
        }
    }
}

impl Career {
    /// Begin a fresh match: clear the pending result so its reward can be granted.
    pub fn begin_match(&mut self) {
        self.last_result = None;
        self.last_unlocks.clear();
        self.awarded = false;
    }

    pub fn record(&mut self, result: MatchResult) {
        self.last_result = Some(result);
        self.awarded = false;
    }

    /// Grant the pending result's XP/unlocks to the profile, exactly once.
    pub fn award(&mut self) -> bool {
        if self.awarded {
            return false;
        }
        let Some(result) = self.last_result.clone() else {
            return false;
        };
        self.last_unlocks = self.profile.award_match(result.placement);
        self.matches_completed += 1;
        self.awarded = true;
        true
    }

    pub fn awarded(&self) -> bool {
        self.awarded
    }
}

/// Load the persisted `Profile` (Phase 48: profile disk-persistence, folded in
/// alongside Settings using the profile's existing `serialize`/`parse` — see
/// `crate::settings::profile_save_path`) into a fresh [`Career`]; a missing or
/// malformed save falls back to `Profile::new()` exactly like a first launch, never
/// panicking on a bad file.
pub fn load_career() -> Career {
    let profile = std::fs::read_to_string(crate::settings::profile_save_path())
        .ok()
        .and_then(|text| Profile::parse(&text))
        .unwrap_or_default();
    Career {
        profile,
        ..Career::default()
    }
}

/// Persist the career's profile to disk (best-effort — a write failure is silently
/// ignored, matching `settings::save_settings`'s convention).
pub fn save_profile(profile: &Profile) {
    let path = crate::settings::profile_save_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, profile.serialize());
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_progression::progression::catalog;

    #[test]
    fn play_match_is_deterministic_and_the_local_team_wins() {
        let a = play_match();
        let b = play_match();
        assert_eq!(a, b);
        assert_eq!(a.placement, Some(1));
        assert!(a.local_won);
        assert_eq!(a.escaped, 1);
        assert_eq!(a.escaped + a.absorbed, 4);
    }

    #[test]
    fn a_full_career_loop_grows_the_persistent_profile() {
        let mut career = Career::default();
        assert_eq!(career.profile.level(), 0);
        for n in 1..=5 {
            career.begin_match();
            career.record(play_match());
            assert!(career.award(), "each completed match awards once");
            assert_eq!(career.matches_completed, n);
        }
        assert!(
            career.profile.level() >= 2,
            "the profile leveled across the run"
        );
        assert!(career.profile.xp > 0);
        assert!(career.profile.matches_played == 5);
    }

    #[test]
    fn award_is_granted_exactly_once_per_match() {
        let mut career = Career::default();
        career.record(play_match());
        assert!(career.award());
        let xp = career.profile.xp;
        assert!(
            !career.award(),
            "a second award for the same result is a no-op"
        );
        assert_eq!(career.profile.xp, xp);
        assert_eq!(career.matches_completed, 1);
    }

    #[test]
    fn the_networked_match_resolves_and_the_transport_is_orthogonal() {
        // The same match replicated over a hostile and a clean network lands on the
        // identical local-team result: the network replicates, it does not alter.
        let hostile = play_networked_match(MATCH_SEED, NetworkProfile::Hostile);
        let clean = play_networked_match(MATCH_SEED, NetworkProfile::Clean);
        assert_eq!(hostile, clean, "the transport must not change the outcome");
        assert!(hostile.local_won);
        assert_eq!(hostile.placement, Some(1));
        assert_eq!(hostile.escaped + hostile.absorbed, 4);
    }

    #[test]
    fn seed_override_accepts_decimal_and_hex_values() {
        assert_eq!(parse_seed_override("42"), Some(42));
        assert_eq!(parse_seed_override(" 0x2a "), Some(42));
        assert_eq!(parse_seed_override("not-a-seed"), None);
    }

    #[test]
    fn progression_and_cosmetics_never_change_the_match() {
        // The integrated re-assertion of orthogonality: grind a career and equip
        // every unlocked cosmetic, then the match still resolves identically.
        let baseline = play_match();
        let mut career = Career::default();
        for _ in 0..12 {
            career.record(play_match());
            career.award();
        }
        for cosmetic in catalog() {
            career.profile.equip(cosmetic.id);
        }
        assert_eq!(
            play_match(),
            baseline,
            "the match is independent of the career"
        );
    }
}
