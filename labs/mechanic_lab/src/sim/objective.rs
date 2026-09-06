//! Two definitions of winning.

use crate::sim::rules::{Objective, Vision};
use crate::sim::state::{LossReason, MatchState, Outcome, PawnId, TeamId};

/// What a pawn must be doing to plant.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlantRule {
    /// Stand on the flag hex.
    StandOnly,
    /// Stand on the flag hex while a *different* free pawn holds it in vision.
    ///
    /// The obvious reading — "the planter must observe the hex" — is vacuous,
    /// because a pawn holds the cell it stands in by default and would always
    /// satisfy it. Requiring a second pair of eyes is the version where the
    /// gate bites, and it is the co-op beat the north star asks for: the
    /// two-operator station, at squad scale.
    #[default]
    Overwatch,
}

/// How many flags a team needs.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlantWin {
    /// Every flag. The single-team reading.
    #[default]
    All,
    /// More than half. The contested reading: with three flags and two teams,
    /// each side's near flag is nearly free and the middle one decides it.
    Majority,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PlantFlags {
    pub rule: PlantRule,
    pub win: PlantWin,
    pub turn_limit: u16,
}

impl Objective for PlantFlags {
    fn name(&self) -> &'static str {
        "PlantFlags"
    }

    fn may_claim(&self, state: &MatchState, pawn: PawnId, vision: &dyn Vision) -> bool {
        let actor = state.pawn(pawn);
        if actor.jailed {
            return false;
        }
        let Some(flag) = state
            .flags
            .iter()
            .find(|flag| flag.at == actor.at && flag.planted_by.is_none())
        else {
            return false;
        };
        match self.rule {
            PlantRule::StandOnly => true,
            PlantRule::Overwatch => state
                .free_pawns()
                .filter(|other| other.id != pawn)
                .any(|other| vision.covers(state, other.at, other.facing, flag.at)),
        }
    }

    fn claim(&self, state: &mut MatchState, pawn: PawnId) {
        let (at, team) = {
            let actor = state.pawn(pawn);
            (actor.at, actor.team)
        };
        if let Some(flag) = state
            .flags
            .iter_mut()
            .find(|flag| flag.at == at && flag.planted_by.is_none())
        {
            flag.planted_by = Some(team);
            state.report.planted.push(at);
        }
    }

    fn evaluate(&self, state: &MatchState) -> Option<Outcome> {
        let teams = state.teams();
        let total = state.flags.len();

        for &team in &teams {
            let held = state.flags_held_by(team);
            let won = match self.win {
                PlantWin::All => held == total && total > 0,
                PlantWin::Majority => held * 2 > total,
            };
            if won {
                return Some(Outcome::Won(team));
            }
        }

        // A team whose pawns are all held can no longer rescue itself.
        let standing: Vec<TeamId> = teams
            .iter()
            .copied()
            .filter(|&team| {
                state
                    .pawns
                    .iter()
                    .any(|pawn| pawn.team == team && !pawn.jailed)
            })
            .collect();
        match standing.len() {
            0 => return Some(Outcome::Lost(LossReason::AllPawnsHeld)),
            1 if teams.len() > 1 => return Some(Outcome::Won(standing[0])),
            _ => {}
        }

        // A readable loss rather than a silent stall: the reachability overlay
        // shows this coming a turn before it lands.
        let any_route = state.free_pawns().any(|pawn| {
            state
                .unplanted_flags()
                .any(|flag| state.board.connected(pawn.at, flag.at))
        });
        if !any_route {
            return Some(Outcome::Lost(LossReason::NoRouteToObjective));
        }

        if state.turn >= self.turn_limit {
            if teams.len() > 1 {
                let best = teams
                    .iter()
                    .map(|&team| (state.flags_held_by(team), team))
                    .max_by_key(|&(held, _)| held);
                return Some(match best {
                    Some((held, team))
                        if held > 0
                            && teams
                                .iter()
                                .filter(|&&other| state.flags_held_by(other) == held)
                                .count()
                                == 1 =>
                    {
                        Outcome::Won(team)
                    }
                    _ => Outcome::Draw,
                });
            }
            return Some(Outcome::Lost(LossReason::TurnLimit));
        }
        None
    }
}

/// Get one pawn to a single goal hex. The second implementation the seam needs,
/// and the shape the first sketch's Race and Collapse modes want.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReachExit {
    pub turn_limit: u16,
}

impl Objective for ReachExit {
    fn name(&self) -> &'static str {
        "ReachExit"
    }

    fn may_claim(&self, _state: &MatchState, _pawn: PawnId, _vision: &dyn Vision) -> bool {
        false
    }

    fn claim(&self, _state: &mut MatchState, _pawn: PawnId) {}

    fn evaluate(&self, state: &MatchState) -> Option<Outcome> {
        // The first flag hex doubles as the exit; standing on it wins.
        let exit = state.flags.first().map(|flag| flag.at)?;
        if let Some(pawn) = state.free_pawns().find(|pawn| pawn.at == exit) {
            return Some(Outcome::Won(pawn.team));
        }
        if state.pawns.iter().all(|pawn| pawn.jailed) {
            return Some(Outcome::Lost(LossReason::AllPawnsHeld));
        }
        if !state
            .free_pawns()
            .any(|pawn| state.board.connected(pawn.at, exit))
        {
            return Some(Outcome::Lost(LossReason::NoRouteToObjective));
        }
        if state.turn >= self.turn_limit {
            return Some(Outcome::Lost(LossReason::TurnLimit));
        }
        None
    }
}
