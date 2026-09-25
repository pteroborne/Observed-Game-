//! Architect Ascent in the game: the rules riding beside the first-person match.
//!
//! The runtime keeps its `HexWfcMatch` where every presentation system reads it, and
//! holds the Ascent rules ([`AscentRules`]) beside it. A local launch with the Ascent
//! rules builds them; each tick steps both together; the rules' outcome ends the match.
//! Every team's Architect seat is held by the rules' own loyal bot for now.

use std::collections::BTreeMap;

use bevy::prelude::*;
use observed_core::{PlayerId, TeamId};
use observed_match::ascent::facility::AscentRules;
use observed_match::ascent::session::{ASCENT_INPUT_VERSION, InputFrame, Role, Seat};
use observed_match::ascent::sim::{MatchOutcome, TeamId as AscentTeam};
use observed_match::hex_wfc::{HexBodyPlace, HexInputFrame, HexPlayerState, HexWfcMatch};

use super::sim::HexWfcRuntime;
use crate::flow::MatchResult;

/// Architect seats are numbered past every body: team `t`'s Architect is this plus `t`.
const ARCHITECT_SEATS: u16 = 200;

/// How far below the facility the prison dimension lies, and how far apart each team's
/// maze is from the next. Only presentation reads these: in the simulation each maze is a
/// space of its own, and a jailed body's position is in its maze's own frame.
const PRISON_DEPTH: f32 = 1_000.0;
const PRISON_SPACING: f32 = 2_000.0;

/// The rules for a fresh local match, with a bot Architect for every team. `None` when
/// the facility cannot host them, which a solved facility always can.
pub(super) fn rules_for(match_state: &mut HexWfcMatch) -> Option<AscentRules> {
    let seats = match_state
        .teams
        .keys()
        .map(|team| {
            (
                PlayerId(ARCHITECT_SEATS + u16::from(team.0)),
                Seat {
                    role: Role::Architect(AscentTeam(team.0)),
                    bot: true,
                },
            )
        })
        .collect();
    let seed = match_state.seed;
    AscentRules::new(match_state, seed, seats)
        .inspect_err(|refusal| warn!("Architect Ascent could not start: {refusal:?}"))
        .ok()
}

/// Step the match through its rules, if it has them. Returns whether it did.
pub(super) fn step(runtime: &mut HexWfcRuntime, bodies: &HexInputFrame) -> bool {
    let runtime = &mut *runtime;
    let Some(rules) = runtime.ascent.as_mut() else {
        return false;
    };
    let seats = InputFrame {
        version: ASCENT_INPUT_VERSION,
        tick: rules.rules().tick + 1,
        commands: BTreeMap::new(),
    };
    // A refused frame is one the rules would refuse whole: the match has already been
    // decided, and the physical match has stopped with it.
    let _ = rules.step(&mut runtime.match_state, bodies, &seats);
    true
}

/// The local player's result from the rules' outcome.
pub(crate) fn result_for(rules: &AscentRules, game: &HexWfcMatch, local: PlayerId) -> MatchResult {
    let local_team = game
        .players
        .get(&local)
        .map_or(TeamId(0), |player| player.team);
    let winner = match rules.rules().outcome {
        MatchOutcome::LoyalVictory => rules.rules().summit_team.map(|team| TeamId(team.0)),
        MatchOutcome::RogueVictory | MatchOutcome::Running => None,
    };
    MatchResult {
        local_team,
        placement: (winner == Some(local_team)).then_some(1),
        escaped: usize::from(winner.is_some()),
        absorbed: game.teams.len() - usize::from(winner.is_some()),
        winner,
        local_won: winner == Some(local_team),
    }
}

/// Where presentation draws a body: in the facility where it is, and in its team's maze,
/// far below and apart from every other team's, when it is jailed.
#[must_use]
pub(super) fn presented_position(player: &HexPlayerState) -> Vec3 {
    match player.place {
        HexBodyPlace::Prison => player.position + prison_offset(player.team),
        HexBodyPlace::Facility | HexBodyPlace::Void => player.position,
    }
}

/// Where team `team`'s maze is drawn.
#[must_use]
pub(super) fn prison_offset(team: TeamId) -> Vec3 {
    Vec3::new(f32::from(team.0) * PRISON_SPACING, -PRISON_DEPTH, 0.0)
}
