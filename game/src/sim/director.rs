//! The **match director**: the single owner of the running match's brains and the one
//! authority on how a match ends.
//!
//! A match involves three cooperating models — the **live networked hybrid match**
//! (`LiveNetMatch`, what the player physically walks), the **elimination series**
//! (`EliminationSeries`, the multi-round "don't come in last" result the HUD reports),
//! and, in spectator mode, the **teamplay brain** (`TeamplayMatch`) that feeds the
//! series instead of the wall-clock autoplay. Before Arc G3 these lived in separate
//! resources advanced by separate systems, and the match could end through either
//! model's finish line with the result read from whichever happened to be done. The
//! director encapsulates that coordination behind one API:
//!
//! - [`MatchDirector::tick`] — the per-frame advance (series autoplay cadence, live
//!   transport pumping, forced wait rounds, live-finish → series fast-forward), which
//!   yields the [`MatchResult`] exactly once, the frame the match completes.
//! - [`MatchDirector::run_to_completion`] — the deterministic headless path (careers,
//!   tests) that applies the same rule, so a headless run and an on-screen run of the
//!   same seed resolve identically (pinned by a characterization test).
//! - [`MatchDirector::outcome`] — the single resolution rule: a finished series is the
//!   authority; the live competitive standings only speak when the series never
//!   finished (spectator mode abandoned mid-series).
//!
//! The `live` and `series` fields stay public: presentation and the crossing
//! controller read them freely (`director.live.host_match()`), and scripted
//! capture/diagnostic drivers force rounds directly. The director owns *coordination*
//! and *completion*, not every access.

use bevy::prelude::*;
use std::time::Duration;

use observed_core::RoomId;
use observed_facility::map_spec::MapSpec;
use observed_match::elimination::{EliminationSeries, MAX_AUTOPLAY_TICKS};
use observed_match::hybrid::LocalAction;
use observed_match::teamplay::TeamplayMatch;
use observed_net::netmatch::LiveNetMatch;
use observed_net::network::NetworkProfile;

use crate::flow::{self, LOCAL_TEAM, MatchResult};
use crate::sim::state::SpectatorBot;

/// Wall-clock cadence of one elimination-series autoplay tick during play.
const SERIES_AUTOPLAY_SECS: f32 = 0.45;
/// Wall-clock cadence of forced `Wait` rounds once the local team is no longer an
/// active runner, so the remaining teams can finish.
const WAIT_ROUND_SECS: f32 = 0.45;
/// Pump budget for settling the lockstep transport after the match completes.
const SYNC_PUMP_BUDGET: usize = 64;
/// Pump budget for running the live match to completion headlessly.
const HEADLESS_PUMP_BUDGET: u32 = 100_000;
/// Pump budget for settling the transport after each scripted (evidence-driver)
/// round.
const SCRIPTED_ROUND_PUMP_BUDGET: usize = 400;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BotPopulations {
    pub rival_teams: bool,
    pub ai_teammates: bool,
    pub guardian: bool,
}

impl Default for BotPopulations {
    fn default() -> Self {
        Self {
            rival_teams: true,
            ai_teammates: true,
            guardian: true,
        }
    }
}

impl BotPopulations {
    pub fn from_env() -> Option<Self> {
        let value = std::env::var("OBSERVED2_BOTS").ok()?;
        Self::from_str(&value)
    }

    pub fn from_str(value: &str) -> Option<Self> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }
        let mut config = Self::default(); // default: all true

        // Split on common separators: '|', ',', '+', ';', or whitespace
        let parts: Vec<&str> = trimmed
            .split(['|', ',', '+', ';', ' '])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        for part in parts {
            match part.to_lowercase().as_str() {
                "all" | "default" => {
                    config.rival_teams = true;
                    config.ai_teammates = true;
                    config.guardian = true;
                }
                "none" => {
                    config.rival_teams = false;
                    config.ai_teammates = false;
                    config.guardian = false;
                }
                "no_guardian" | "no-guardian" | "noguardian" => {
                    config.guardian = false;
                }
                "no_rivals" | "no-rivals" | "norivals" => {
                    config.rival_teams = false;
                }
                "no_teammates" | "no-teammates" | "noteammates" => {
                    config.ai_teammates = false;
                }
                _ => {
                    panic!(
                        "unknown OBSERVED2_BOTS value `{part}`; valid flags: all, none, no_guardian, no_rivals, no_teammates"
                    );
                }
            }
        }
        Some(config)
    }
}

#[derive(Resource)]
pub struct MatchDirector {
    /// The live, host-authoritative networked first-person match: the host is the
    /// locally-played match, replicated over the lockstep transport to a remote.
    pub live: LiveNetMatch,
    /// The elimination-series outcome model the HUD reports and the career records.
    pub series: EliminationSeries,
    pub config: BotPopulations,
    autoplay_timer: Timer,
    wait_timer: Timer,
    /// Latched once the completion rule has fired (the result has been yielded).
    pub done: bool,
}

impl MatchDirector {
    pub fn new(seed: u64, map_spec: MapSpec, config: BotPopulations) -> Self {
        let mut director = Self {
            live: LiveNetMatch::new_for_map_spec(seed, NetworkProfile::Hostile, map_spec),
            series: EliminationSeries::new(seed),
            config,
            autoplay_timer: Timer::from_seconds(SERIES_AUTOPLAY_SECS, TimerMode::Repeating),
            wait_timer: Timer::from_seconds(WAIT_ROUND_SECS, TimerMode::Repeating),
            done: false,
        };
        director.apply_population_config(config);
        director
    }

    fn apply_population_config(&mut self, config: BotPopulations) {
        let start_room = self
            .live
            .host
            .match_state
            .competitive
            .structure
            .graph
            .players[0];

        // 1. Configure Host Team & Players list
        let t0_members = if config.ai_teammates { 2 } else { 1 };
        let mut host_teams = Vec::new();
        let mut host_players = Vec::new();

        host_teams.push(observed_match::facility::TeamRun {
            id: observed_core::TeamId(0),
            member_base: 0,
            members: t0_members,
            speed: self.live.host.match_state.competitive.teams[0].speed,
            pace: 0.0,
            role: observed_match::director::Role::Runner,
            placement: None,
            objective_index: 0,
        });
        for _ in 0..t0_members {
            host_players.push(start_room);
        }

        for i in 1..4 {
            let members = if config.rival_teams { 2 } else { 0 };
            let member_base = if config.rival_teams {
                host_players.len()
            } else {
                0
            };

            host_teams.push(observed_match::facility::TeamRun {
                id: observed_core::TeamId(i as u8),
                member_base,
                members,
                speed: self.live.host.match_state.competitive.teams[i].speed,
                pace: 0.0,
                role: observed_match::director::Role::Runner,
                placement: None,
                objective_index: 0,
            });

            if config.rival_teams {
                for _ in 0..2 {
                    host_players.push(start_room);
                }
            }
        }

        self.live.host.match_state.competitive.teams = host_teams;
        self.live
            .host
            .match_state
            .competitive
            .structure
            .graph
            .players = host_players;
        self.live
            .host
            .match_state
            .competitive
            .structure
            .recompute_connectivity();

        // 2. Configure Remote Team & Players list
        let mut remote_teams = Vec::new();
        let mut remote_players = Vec::new();

        remote_teams.push(observed_match::facility::TeamRun {
            id: observed_core::TeamId(0),
            member_base: 0,
            members: t0_members,
            speed: self.live.remote.match_state.competitive.teams[0].speed,
            pace: 0.0,
            role: observed_match::director::Role::Runner,
            placement: None,
            objective_index: 0,
        });
        for _ in 0..t0_members {
            remote_players.push(start_room);
        }

        for i in 1..4 {
            let members = if config.rival_teams { 2 } else { 0 };
            let member_base = if config.rival_teams {
                remote_players.len()
            } else {
                0
            };

            remote_teams.push(observed_match::facility::TeamRun {
                id: observed_core::TeamId(i as u8),
                member_base,
                members,
                speed: self.live.remote.match_state.competitive.teams[i].speed,
                pace: 0.0,
                role: observed_match::director::Role::Runner,
                placement: None,
                objective_index: 0,
            });

            if config.rival_teams {
                for _ in 0..2 {
                    remote_players.push(start_room);
                }
            }
        }

        self.live.remote.match_state.competitive.teams = remote_teams;
        self.live
            .remote
            .match_state
            .competitive
            .structure
            .graph
            .players = remote_players;
        self.live
            .remote
            .match_state
            .competitive
            .structure
            .recompute_connectivity();

        // 3. Configure Series
        if !config.rival_teams {
            self.series.alive_teams = vec![observed_core::TeamId(0)];
            self.series.current = observed_match::elimination::SeriesRound::new(
                1,
                vec![observed_core::TeamId(0)],
                self.series.keystone_rooms.clone(),
            );
        }
    }

    /// Raw model completion: either the live run or the elimination series has crossed
    /// its finish line. `tick` applies the presentation-facing policy on top of this
    /// predicate; Spectate mode may hold a ready series result until the visible
    /// first-person body finishes or stops.
    pub fn finished(&self) -> bool {
        self.live.finished() || self.series.finished()
    }

    /// The single outcome authority: a finished series resolves the match; the live
    /// standings only speak when the series never finished.
    pub fn outcome(&self) -> MatchResult {
        if self.series.finished() {
            flow::resolve_series(&self.series)
        } else {
            flow::resolve(&self.live.host_match().competitive)
        }
    }

    /// One presentation-frame advance. `spectator_driven_series` suppresses the
    /// wall-clock series autoplay (spectator mode feeds the series through
    /// [`Self::pump_spectator`] instead). Returns the [`MatchResult`] exactly once —
    /// the frame the match completes.
    pub fn tick(
        &mut self,
        dt: Duration,
        spectator_driven_series: bool,
        spectator_visible_finished: bool,
    ) -> Option<MatchResult> {
        if self.done {
            return None;
        }
        if !spectator_driven_series
            && !self.series.finished()
            && self.autoplay_timer.tick(dt).just_finished()
        {
            self.series.advance_autoplay_tick();
        }
        for _ in 0..3 {
            self.live.pump();
        }
        if !self.live.finished()
            && !self.live.local_active()
            && self.wait_timer.tick(dt).just_finished()
        {
            self.live.force_round(LocalAction::Wait);
        }
        if self.live.finished() && !self.series.finished() {
            self.series.run_to_winner(MAX_AUTOPLAY_TICKS);
        }
        // Match-end is gated on the *visible* run, never on the independently-paced
        // series alone: the series autoplays on a wall-clock timer (normal play) or a
        // teamplay pump (spectator), which on a large generated map resolves the whole
        // elimination long before the player/bot has physically traversed the facility.
        // Ending on the series there made the match feel like it "ended too quickly".
        // - Normal play: end when the first-person run finishes (`self.live`); the
        //   series is resolved above for the placement `outcome()` reads.
        // - Spectator: end when the live run finishes, or when the teamplay series is
        //   done *and* the visible bot has reached the exit or given up
        //   (`spectator_visible_finished`) — so the camera body is never abandoned
        //   mid-facility.
        let screen_finished = if spectator_driven_series {
            self.live.finished() || (self.series.finished() && spectator_visible_finished)
        } else {
            self.live.finished()
        };
        if screen_finished {
            self.settle_transport();
            self.done = true;
            return Some(self.outcome());
        }
        None
    }

    /// Run the whole match deterministically without a frame loop (headless careers
    /// and tests). Applies the identical completion rule as [`Self::tick`], so a
    /// headless run and an interactive run of the same seed resolve identically.
    pub fn run_to_completion(&mut self) -> MatchResult {
        self.live.run_to_completion_headless(HEADLESS_PUMP_BUDGET);
        if !self.series.finished() {
            self.series.run_to_winner(MAX_AUTOPLAY_TICKS);
        }
        self.settle_transport();
        self.done = true;
        self.outcome()
    }

    /// Advance the spectator's teamplay brain one bot tick and feed any completed
    /// round into the series; on a series round boundary, reseat the teamplay brain
    /// for the next round and refocus the spectator on a surviving team.
    pub fn pump_spectator(&mut self, spectator: &mut SpectatorBot) {
        if self.series.finished() {
            spectator.last_teamplay_event = self.series.last_event.clone();
            return;
        }

        if !spectator.teamplay.finished {
            let events = spectator.teamplay.advance_bot_tick();
            if let Some(event) = events.last() {
                spectator.last_teamplay_event = event.summary();
            }
        }

        let Some(outcome) = spectator.teamplay.round_outcome() else {
            return;
        };

        self.series.apply_teamplay_round(outcome);
        if self.series.finished() {
            spectator.last_teamplay_event = self.series.last_event.clone();
            return;
        }

        let next_seed = spectator
            .seed
            .wrapping_add(u64::from(self.series.current.index).wrapping_mul(0xA11_C0D3));
        spectator.teamplay = TeamplayMatch::for_round(
            next_seed,
            spectator.seed,
            self.series.current.index,
            self.series.alive_teams.clone(),
            self.series.adversary_strength(),
        );
        spectator.teamplay_frame_accum = 0;
        spectator.focused_team = self
            .series
            .alive_teams
            .iter()
            .copied()
            .find(|team| *team == LOCAL_TEAM)
            .unwrap_or_else(|| self.series.alive_teams[0]);
        spectator.focused_member = 0;
        spectator.last_teamplay_event = self.series.last_event.clone();
    }

    /// Credit a keystone the local player picked up to the local team's current
    /// series-round objective.
    pub fn record_local_keystone(&mut self, room: RoomId) {
        if let Some(team) = self.series.current.team_mut(LOCAL_TEAM)
            && !team.collected_keystones.contains(&room)
        {
            team.collected_keystones.push(room);
            team.collected_keystones.sort_by_key(|r| r.0);
        }
    }

    /// Pump the lockstep transport until the peers converge (bounded).
    fn settle_transport(&mut self) {
        for _ in 0..SYNC_PUMP_BUDGET {
            if self.live.in_sync() {
                break;
            }
            self.live.pump();
        }
    }

    /// Drive up to `rounds` scripted rounds (Advance while the local team is an
    /// active runner, Wait otherwise), pumping the transport to sync after each.
    /// The staging helper the scripted evidence drivers (captures, visual audit,
    /// tour) use to fast-forward the match into a photographable state.
    pub fn force_scripted_rounds(&mut self, rounds: usize) {
        for _ in 0..rounds {
            if self.live.finished() {
                break;
            }
            let action = if self.live.local_active() {
                LocalAction::Advance
            } else {
                LocalAction::Wait
            };
            self.live.force_round(action);
            for _ in 0..SCRIPTED_ROUND_PUMP_BUDGET {
                if self.live.in_sync() {
                    break;
                }
                self.live.pump();
            }
        }
    }

    /// Clear the brain's pending reroute-feedback window so scripted evidence shots
    /// aren't taken mid light-flicker/door-slam.
    pub fn suppress_reroute_feedback(&mut self) {
        self.live.host.match_state.reroute_feedback_ticks = 0;
    }
}
