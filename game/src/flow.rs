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

/// Choose a fresh launch seed for the one-key results-screen rematch. Even a
/// same-tick clock collision cannot replay the previous facility accidentally.
pub fn rematch_seed(previous: u64) -> u64 {
    let candidate = launch_seed();
    if candidate == previous {
        previous.wrapping_add(1)
    } else {
        candidate
    }
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

/// Resolve the canonical continuous full-WFC match into the existing career/result
/// envelope. Networking can later transmit the match snapshot without changing this
/// presentation-facing outcome.
pub fn resolve_full_wfc(game: &observed_match::full_wfc::FullWfcMatch) -> MatchResult {
    let placement = game
        .escape_order
        .iter()
        .position(|team| *team == LOCAL_TEAM)
        .map(|index| index as u8 + 1);
    let winner = game.escape_order.first().copied();
    MatchResult {
        placement,
        escaped: game.teams.values().filter(|team| team.escaped).count(),
        absorbed: game.teams.values().filter(|team| team.eliminated).count(),
        winner,
        local_won: winner == Some(LOCAL_TEAM),
    }
}

/// Run a whole deterministic match to its end (headless — used by the career loop
/// and tests). This is the *same* [`MatchDirector`] the interactive Match screen
/// steps frame by frame, run to completion in one call, so a headless career match
/// and an on-screen match of the same seed resolve identically (pinned by the
/// `headless_and_interactive_matches_agree_on_the_result` characterization test).
pub fn play_match() -> MatchResult {
    let career = load_career();
    let profile_config = crate::sim::director::BotPopulations {
        rival_teams: career.bot_rival_teams,
        ai_teammates: career.bot_ai_teammates,
        guardian: career.bot_guardian,
    };
    let config = if let Some(env_config) = crate::sim::director::BotPopulations::from_env() {
        env_config
    } else {
        profile_config
    };
    let mut director = MatchDirector::new(
        MATCH_SEED,
        crate::map_catalog::active_map_spec(MATCH_SEED),
        config,
    );
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
    pub bot_rival_teams: bool,
    pub bot_ai_teammates: bool,
    pub bot_guardian: bool,
}

impl Default for Career {
    fn default() -> Self {
        Self {
            profile: Profile::new(),
            matches_completed: 0,
            last_result: None,
            last_unlocks: Vec::new(),
            awarded: false,
            bot_rival_teams: true,
            bot_ai_teammates: true,
            bot_guardian: true,
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
    #[cfg(test)]
    if crate::settings::TEST_PROFILE_PATH.with(|path| path.borrow().is_none()) {
        return Career::default();
    }

    let save_text =
        std::fs::read_to_string(crate::settings::profile_save_path()).unwrap_or_default();
    let first_line = save_text.lines().next().unwrap_or("");
    let profile = Profile::parse(first_line).unwrap_or_default();

    let mut bot_rival_teams = true;
    let mut bot_ai_teammates = true;
    let mut bot_guardian = true;

    for line in save_text.lines().skip(1) {
        if let Some((key, value)) = line.split_once('=') {
            match key {
                "bot_rival_teams" => {
                    if let Ok(b) = value.parse::<bool>() {
                        bot_rival_teams = b;
                    }
                }
                "bot_ai_teammates" => {
                    if let Ok(b) = value.parse::<bool>() {
                        bot_ai_teammates = b;
                    }
                }
                "bot_guardian" => {
                    if let Ok(b) = value.parse::<bool>() {
                        bot_guardian = b;
                    }
                }
                _ => {}
            }
        }
    }

    Career {
        profile,
        bot_rival_teams,
        bot_ai_teammates,
        bot_guardian,
        ..Career::default()
    }
}

/// Persist the career's profile to disk (best-effort — a write failure is silently
/// ignored, matching `settings::save_settings`'s convention).
pub fn save_profile(career: &Career) {
    #[cfg(test)]
    if crate::settings::TEST_PROFILE_PATH.with(|path| path.borrow().is_none()) {
        return;
    }

    let path = crate::settings::profile_save_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let base_serialized = career.profile.serialize();
    let full_serialized = format!(
        "{}\nbot_rival_teams={}\nbot_ai_teammates={}\nbot_guardian={}",
        base_serialized, career.bot_rival_teams, career.bot_ai_teammates, career.bot_guardian
    );
    let _ = std::fs::write(path, full_serialized);
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_progression::progression::catalog;

    #[test]
    fn play_match_is_deterministic_and_the_local_team_wins() {
        let a = play_match();
        let b = play_match();
        let career = load_career();
        let config = crate::sim::director::BotPopulations::from_env().unwrap_or(
            crate::sim::director::BotPopulations {
                rival_teams: career.bot_rival_teams,
                ai_teammates: career.bot_ai_teammates,
                guardian: career.bot_guardian,
            },
        );
        let configured_team_count = if config.rival_teams { 4 } else { 1 };
        assert_eq!(a, b);
        assert_eq!(a.placement, Some(1));
        assert!(a.local_won);
        assert_eq!(a.escaped, 1);
        assert_eq!(a.escaped + a.absorbed, configured_team_count);
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
