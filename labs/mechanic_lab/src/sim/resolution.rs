//! Two ways for intents to become positions.
//!
//! The conflict table `docs/mechanic_lab_plan.md` fixes is entirely
//! [`Simultaneous`]'s business; [`Sequential`] has no conflicts to settle
//! because each pawn sees the board the previous one left behind. That is the
//! difference the seam exists to expose.

use observed_hex::coords::HexCoord;

use crate::sim::rules::Resolution;
use crate::sim::state::{MatchState, PawnId};

/// Everyone commits blind and the board settles once.
#[derive(Clone, Copy, Debug, Default)]
pub struct Simultaneous;

impl Resolution for Simultaneous {
    fn name(&self) -> &'static str {
        "Simultaneous"
    }

    fn settle(
        &self,
        state: &MatchState,
        desired: &[(PawnId, HexCoord)],
    ) -> Vec<(PawnId, HexCoord)> {
        let origin: Vec<(PawnId, HexCoord)> = desired
            .iter()
            .map(|&(id, _)| (id, state.pawn(id).at))
            .collect();
        // Only teammates contend for space. Cross-team pawns may share a hex
        // and swap through each other, because that contact is what the threat
        // stage adjudicates by recency — refusing it here would mean rival
        // pawns could never meet.
        let team: Vec<_> = desired.iter().map(|&(id, _)| state.pawn(id).team).collect();
        let mut accepted: Vec<HexCoord> = desired.iter().map(|&(_, to)| to).collect();

        // Every pass compares against a snapshot and applies its refusals
        // afterwards. Refusing in place would let the first pawn's retreat
        // dissolve the very conflict the second pawn is part of, and one of the
        // two would sail through — which is a priority, and an invisible one.
        loop {
            let snapshot = accepted.clone();
            let mut refuse = vec![false; accepted.len()];

            for i in 0..snapshot.len() {
                if snapshot[i] == origin[i].1 {
                    continue;
                }
                // Two pawns wanting the same empty hex are *both* refused.
                // Symmetric, and it declines to invent a priority nobody can see.
                if snapshot
                    .iter()
                    .enumerate()
                    .any(|(j, &to)| j != i && team[j] == team[i] && to == snapshot[i])
                {
                    refuse[i] = true;
                    continue;
                }
                // Bodies do not pass through each other, so a straight swap is
                // refused. A longer cycle is consistent and stands.
                if snapshot.iter().enumerate().any(|(j, &to)| {
                    j != i && team[j] == team[i] && to == origin[i].1 && snapshot[i] == origin[j].1
                }) {
                    refuse[i] = true;
                    continue;
                }
                // A pawn may follow one that is leaving, so a destination is
                // blocked only by a pawn that stays put.
                if (0..snapshot.len()).any(|j| {
                    j != i
                        && team[j] == team[i]
                        && snapshot[j] == origin[j].1
                        && origin[j].1 == snapshot[i]
                }) {
                    refuse[i] = true;
                }
            }

            if !refuse.iter().any(|&r| r) {
                break;
            }
            // Refusing one pawn can strand its follower, so this runs to a
            // fixpoint rather than settling in a single pass.
            for (i, refused) in refuse.into_iter().enumerate() {
                if refused {
                    accepted[i] = origin[i].1;
                }
            }
        }

        desired
            .iter()
            .enumerate()
            .map(|(i, &(id, _))| (id, accepted[i]))
            .collect()
    }
}

/// Each pawn moves fully before the next is considered, in pawn id order.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sequential;

impl Resolution for Sequential {
    fn name(&self) -> &'static str {
        "Sequential"
    }

    fn settle(
        &self,
        state: &MatchState,
        desired: &[(PawnId, HexCoord)],
    ) -> Vec<(PawnId, HexCoord)> {
        let mut order: Vec<usize> = (0..desired.len()).collect();
        order.sort_by_key(|&i| desired[i].0);

        // Teammates only, for the same reason `Simultaneous` checks teams.
        let mut occupied: Vec<(crate::sim::state::TeamId, HexCoord)> = state
            .free_pawns()
            .map(|pawn| (pawn.team, pawn.at))
            .collect();
        let mut accepted: Vec<HexCoord> =
            desired.iter().map(|&(id, _)| state.pawn(id).at).collect();

        for i in order {
            let (id, want) = desired[i];
            let from = state.pawn(id).at;
            let side = state.pawn(id).team;
            if want == from {
                continue;
            }
            if occupied
                .iter()
                .any(|&(who, cell)| who == side && cell == want)
            {
                continue;
            }
            if let Some(slot) = occupied
                .iter()
                .position(|&(who, cell)| who == side && cell == from)
            {
                occupied[slot] = (side, want);
            }
            accepted[i] = want;
        }

        desired
            .iter()
            .enumerate()
            .map(|(i, &(id, _))| (id, accepted[i]))
            .collect()
    }
}
