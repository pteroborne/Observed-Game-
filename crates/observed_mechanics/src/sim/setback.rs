//! Two consequences for being caught.

use crate::sim::rules::Setback;
use crate::sim::state::{MatchState, PawnId};

/// Taken pawns are held on the prison hex until a free pawn stands there.
///
/// Release grants immunity for the remainder of the resolution. Without it a
/// guardian camping the prison re-takes every rescue on the same turn it
/// happens, and the jailbreak is dead on arrival.
#[derive(Clone, Copy, Debug, Default)]
pub struct Prison;

impl Setback for Prison {
    fn name(&self) -> &'static str {
        "Prison"
    }

    fn take(&self, state: &mut MatchState, pawn: PawnId) {
        let prison = state.prison_for(state.pawn(pawn).team);
        let held = state.pawn_mut(pawn);
        held.jailed = true;
        held.at = prison;
        held.immune = false;
        state.report.taken.push(pawn);
    }

    fn release(&self, state: &mut MatchState) {
        let turn = state.turn;
        for team in state.teams() {
            // A team rescues its own, and its own are held in the prison that
            // `prison_for` sends them to — the rival's, not its home one.
            let prison = state.prison_for(team);
            let rescuer_present = state
                .pawns
                .iter()
                .any(|pawn| !pawn.jailed && pawn.team == team && pawn.at == prison);
            if !rescuer_present {
                continue;
            }
            let freed: Vec<PawnId> = state
                .pawns
                .iter()
                .filter(|pawn| pawn.jailed && pawn.team == team)
                .map(|pawn| pawn.id)
                .collect();
            for id in freed {
                let pawn = state.pawn_mut(id);
                pawn.jailed = false;
                pawn.at = prison;
                pawn.immune = true;
                // A freed pawn counts as having just left base: a rescue that
                // handed back a stale pawn would be worth very little.
                pawn.left_base_at = turn;
                state.report.released.push(id);
            }
            // Going in after your own is worth something.
            state.earn(team, 2);
        }
    }
}

/// Taken pawns return to their team's spawn immediately and keep playing.
#[derive(Clone, Copy, Debug, Default)]
pub struct RespawnAtStart;

impl Setback for RespawnAtStart {
    fn name(&self) -> &'static str {
        "RespawnAtStart"
    }

    fn take(&self, state: &mut MatchState, pawn: PawnId) {
        let team = state.pawn(pawn).team.0 as usize;
        let spawn = state.spawns[team.min(state.spawns.len() - 1)];
        let turn = state.turn;
        let sent = state.pawn_mut(pawn);
        sent.at = spawn;
        sent.jailed = false;
        sent.immune = true;
        sent.left_base_at = turn;
        state.report.taken.push(pawn);
    }

    fn release(&self, _state: &mut MatchState) {}
}
