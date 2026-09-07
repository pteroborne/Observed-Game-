//! An architect: a [`Mutation`] driven by played tiles instead of a generator.
//!
//! That it fits the existing seam at all is the point. The facility's changes
//! were always a swappable strategy; making one of the players *be* that
//! strategy is a substitution, not a new subsystem. `Mutation` was built to
//! answer "what does the facility do when nobody is looking", and the answer
//! here is "whatever somebody decided".
//!
//! The rogue churn runs alongside rather than being replaced, because the
//! question this slice exists to answer is not "can a player lay tiles" but
//! **"can the other players tell a deliberate play from background noise"**. An
//! architect with the board to itself would never test that.

use observed_hex::coords::HexCoord;
use observed_hex::ports::PortClass;

use crate::sim::board::{Board, Edge};
use crate::sim::mutation::Scope;
use crate::sim::rules::{LockSet, Mutation};
use crate::sim::state::{ChangeSource, MatchState, PendingChange, TeamId};
use crate::sim::tiles::{Refusal, TilePlay};

/// Which team's architect is playing, how much churn runs beside it, and how
/// many tiles it may lay per turn.
#[derive(Clone, Copy, Debug)]
pub struct Architect {
    pub team: TeamId,
    /// The rogue AI's share of the facility, running independently.
    pub rogue: Scope,
}

impl Default for Architect {
    fn default() -> Self {
        Self {
            team: TeamId(0),
            rogue: Scope::Capped { base: 2, cap: 4 },
        }
    }
}

/// The boundaries a play would change, and nothing else.
///
/// A tile that already matches what is there produces no changes, which matters
/// for legibility: a play is only telegraphed where it actually does something,
/// so the marks on screen are the play's real footprint rather than its outline.
#[must_use]
pub fn footprint(board: &Board, play: TilePlay) -> Vec<(Edge, PortClass)> {
    let size = board.size();
    observed_hex::faces::HexFace::LATERAL
        .into_iter()
        .filter_map(|face| {
            let neighbour = size.neighbor(play.cell, face)?;
            if !board.on_board(neighbour) {
                return None;
            }
            let edge = Edge {
                cell: play.cell,
                face,
            };
            let wanted = play.shape.port(play.rotation, face);
            (board.port(edge) != wanted).then_some((edge.canonical(size), wanted))
        })
        .collect()
}

/// Whether a play may be made at all, and why not when it may not.
///
/// Held ground refuses architecture. That is observe-to-freeze seen from the
/// other side of the table: an architect may not rebuild what the operatives
/// are holding — **including their own team's**. Their attention is the price
/// of their safety, and it is paid to their own architect as much as to the
/// enemy's.
pub fn vet(state: &MatchState, locks: &LockSet, play: TilePlay) -> Result<(), Refusal> {
    if !state.board.on_board(play.cell) {
        return Err(Refusal::OffBoard);
    }
    if state.flags.iter().any(|flag| flag.at == play.cell)
        || state.prisons.contains(&play.cell)
        || state.spawns.contains(&play.cell)
    {
        return Err(Refusal::Protected);
    }
    if locks.is_held(play.cell) {
        return Err(Refusal::Held);
    }

    let size = state.board.size();
    let changes = footprint(&state.board, play);
    for (edge, _) in &changes {
        let other = size.neighbor(edge.cell, edge.face);
        if locks.is_held(edge.cell) || other.is_some_and(|other| locks.is_held(other)) {
            return Err(Refusal::Held);
        }
    }

    // Solvability: try it on a copy. A lab that can deal an unwinnable match
    // wastes the tester's time, and an architect who can wall the objective off
    // by accident is worse than one who cannot play at all.
    let mut trial = state.board.clone();
    for (edge, port) in changes {
        trial.set_port(edge, port);
    }
    let anchors: Vec<HexCoord> = state
        .flags
        .iter()
        .map(|flag| flag.at)
        .chain(state.pawns.iter().map(|pawn| pawn.at))
        .collect();
    let Some(&first) = anchors.first() else {
        return Ok(());
    };
    if anchors.iter().any(|&cell| !trial.connected(first, cell)) {
        return Err(Refusal::WouldDisconnect);
    }
    Ok(())
}

impl Mutation for Architect {
    fn name(&self) -> &'static str {
        "Architect+Rogue"
    }

    fn telegraph(&self, state: &mut MatchState, locks: &LockSet) {
        state.refusals.clear();
        let mut pending: Vec<PendingChange> = Vec::new();

        // The architect's declared plays first, so a tile beats the churn where
        // the two want the same boundary. A player's decision outranking a
        // generator is the whole reason to have a player.
        let queued = std::mem::take(&mut state.architect_queue);
        let allowance = state
            .hand_of(self.team)
            .map_or(0, |hand| hand.plays_per_turn);
        let mut spent = 0_u8;

        for play in queued {
            if spent >= allowance {
                state.refusals.push((play, Refusal::NoPlaysLeft));
                continue;
            }
            if !state
                .hand_of(self.team)
                .is_some_and(|hand| hand.holds(play.shape))
            {
                state.refusals.push((play, Refusal::NotInHand));
                continue;
            }
            if let Err(refusal) = vet(state, locks, play) {
                state.refusals.push((play, refusal));
                continue;
            }
            for (edge, to) in footprint(&state.board, play) {
                pending.push(PendingChange {
                    edge,
                    to,
                    source: ChangeSource::Architect(self.team),
                });
            }
            if let Some(hand) = state.hands.get_mut(self.team.0 as usize) {
                hand.spend(play.shape);
            }
            state.report.tiles_played.push(play);
            spent += 1;
        }

        // Then the rogue AI, over whatever the architect did not claim.
        let claimed: Vec<Edge> = pending.iter().map(|change| change.edge).collect();
        let protected: Vec<HexCoord> = state
            .flags
            .iter()
            .map(|flag| flag.at)
            .chain(state.prisons.iter().copied())
            .collect();
        let size = state.board.size();
        let mut churn: Vec<Edge> = state
            .board
            .interior_edges()
            .into_iter()
            .filter(|edge| !claimed.contains(edge))
            .filter(|edge| {
                let other = size.neighbor(edge.cell, edge.face);
                !protected.contains(&edge.cell)
                    && other.is_some_and(|other| !protected.contains(&other))
                    && !locks.is_held(edge.cell)
                    && !other.is_some_and(|other| locks.is_held(other))
            })
            .collect();
        if let Scope::Capped { base, cap } = self.rogue {
            let wanted = (base + state.turn).min(cap) as usize;
            state.rng.shuffle(&mut churn);
            churn.truncate(wanted);
        }
        for edge in churn {
            pending.push(PendingChange {
                edge,
                to: match state.board.port(edge) {
                    PortClass::Door => PortClass::Sealed,
                    _ => PortClass::Door,
                },
                source: ChangeSource::Rogue,
            });
        }

        pending.sort_by_key(|change| (change.edge, change.to as u8));
        pending.dedup_by_key(|change| change.edge);
        state.telegraph = pending;

        // Draw back up for next turn, and reset the allowance.
        let mut rng = state.rng;
        if let Some(hand) = state.hands.get_mut(self.team.0 as usize) {
            hand.played_this_turn = 0;
            hand.draw_owed(&mut rng);
        }
        state.rng = rng;
    }

    fn apply(&self, state: &mut MatchState, locks: &LockSet) {
        let size = state.board.size();
        let pending = std::mem::take(&mut state.telegraph);
        for change in pending {
            let other = size.neighbor(change.edge.cell, change.edge.face);
            let held =
                locks.is_held(change.edge.cell) || other.is_some_and(|other| locks.is_held(other));
            if held {
                // Operatives refusing the facility's change is the loop's
                // heart: attention spent holding ground becomes material their
                // architect can build with. Observation earns architecture.
                if let ChangeSource::Rogue = change.source {
                    let owner = state.pawns.first().map(|pawn| pawn.team);
                    if let Some(team) = owner {
                        state.earn(team, 1);
                    }
                }
                state.report.refused_rewires.push(change);
            } else {
                state.board.set_port(change.edge, change.to);
                state.report.rewired.push(change);
            }
        }
    }
}
