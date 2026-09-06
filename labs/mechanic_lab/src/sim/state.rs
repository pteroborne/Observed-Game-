//! Everything a match is: the board plus the actors on it.
//!
//! `MatchState` is the only mutable thing the pipeline touches, and it holds no
//! presentation data at all — no entities, no colours, no camera. That is what
//! lets the whole turn pipeline be tested headlessly.

use observed_hex::coords::HexCoord;
use observed_hex::faces::HexFace;

use crate::sim::board::{Board, Edge};
use crate::sim::prng::Prng;
use observed_hex::ports::PortClass;

/// Index into [`MatchState::pawns`]. Also the deterministic tie-break order for
/// every conflict the resolution strategies cannot otherwise settle.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PawnId(pub u8);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TeamId(pub u8);

#[derive(Clone, Copy, Debug)]
pub struct Pawn {
    pub id: PawnId,
    pub team: TeamId,
    pub at: HexCoord,
    /// Where this pawn stood at the top of the turn, before anything moved.
    /// Threats read it to tell a swap from a chase.
    pub prev_at: HexCoord,
    pub facing: HexFace,
    pub jailed: bool,
    /// The turn this pawn last stepped off its own base.
    ///
    /// Stink base / Prisoner's Base recency: higher is fresher, and a fresher
    /// pawn takes a staler one on contact. A pawn standing in its own base is
    /// maximally fresh and cannot be taken at all, which is what makes the trip
    /// home a real cost rather than a formality.
    pub left_base_at: u16,
    /// Set when a pawn is released, cleared at the top of the next turn. Without
    /// it a guardian camping the prison re-takes every rescue on the same
    /// resolution and the jailbreak mechanic is dead on arrival.
    pub immune: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Guardian {
    pub at: HexCoord,
    /// Set when `ConeInteraction::Slowed` let it enter an observed cell. A
    /// stalled guardian moved but takes nobody this turn.
    pub stalled: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Flag {
    pub at: HexCoord,
    pub planted_by: Option<TeamId>,
}

/// What a pawn does with its single action. Facing travels alongside in
/// [`Intent`] and costs nothing, but is still declared before resolution — free
/// means free of action cost, not chosen after seeing the outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Hold,
    Step(HexFace),
    Plant,
}

#[derive(Clone, Copy, Debug)]
pub struct Intent {
    pub pawn: PawnId,
    pub facing: HexFace,
    pub action: Action,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum LossReason {
    TurnLimit,
    /// No free pawn can still reach an unplanted flag.
    NoRouteToObjective,
    AllPawnsHeld,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Won(TeamId),
    Lost(LossReason),
    /// Contested modes can end level; single-team modes never do.
    Draw,
}

/// One boundary the facility intends to change, and what it will become.
///
/// The target class is carried in the simulation whether or not the view is
/// allowed to draw it; `MutationPreview` decides what a player is shown, and is
/// presentation-only for the same reason `tactics_lab`'s whole-map view is —
/// a display setting that quietly changed the rules would be measuring a
/// different game.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PendingChange {
    pub edge: Edge,
    pub to: PortClass,
}

/// Per-turn record of what the rules actually did, for the HUD and the metrics.
/// Never read by the simulation itself.
#[derive(Clone, Debug, Default)]
pub struct TurnReport {
    pub refused_moves: Vec<PawnId>,
    pub rewired: Vec<PendingChange>,
    pub refused_rewires: Vec<PendingChange>,
    pub taken: Vec<PawnId>,
    pub released: Vec<PawnId>,
    pub planted: Vec<HexCoord>,
}

#[derive(Clone, Debug)]
pub struct MatchState {
    pub board: Board,
    pub pawns: Vec<Pawn>,
    pub guardians: Vec<Guardian>,
    pub flags: Vec<Flag>,
    /// One prison per team — `prisons[t]` is team `t`'s, and it holds the
    /// *other* team's pawns. With a single team it degenerates to one neutral
    /// cell, which is what mode 1 uses.
    pub prisons: Vec<HexCoord>,
    pub spawns: Vec<HexCoord>,
    pub turn: u16,
    /// What the facility will do to itself next turn unless held.
    pub telegraph: Vec<PendingChange>,
    pub outcome: Option<Outcome>,
    pub report: TurnReport,
    pub rng: Prng,
}

impl MatchState {
    #[must_use]
    pub fn pawn(&self, id: PawnId) -> &Pawn {
        &self.pawns[id.0 as usize]
    }

    pub fn pawn_mut(&mut self, id: PawnId) -> &mut Pawn {
        &mut self.pawns[id.0 as usize]
    }

    /// Pawns that can act: on the board and not held.
    pub fn free_pawns(&self) -> impl Iterator<Item = &Pawn> + '_ {
        self.pawns.iter().filter(|pawn| !pawn.jailed)
    }

    /// Every team with at least one pawn, in id order.
    #[must_use]
    pub fn teams(&self) -> Vec<TeamId> {
        let mut teams: Vec<TeamId> = self.pawns.iter().map(|pawn| pawn.team).collect();
        teams.sort_unstable();
        teams.dedup();
        teams
    }

    #[must_use]
    pub fn base_of(&self, team: TeamId) -> HexCoord {
        self.spawns[team.0 as usize % self.spawns.len()]
    }

    /// Where this team's pawns are held when taken: the next team's prison.
    #[must_use]
    pub fn prison_for(&self, team: TeamId) -> HexCoord {
        self.prisons[(team.0 as usize + 1) % self.prisons.len()]
    }

    /// Recency, higher being fresher. Standing in your own base is untouchable.
    #[must_use]
    pub fn freshness(&self, id: PawnId) -> u16 {
        let pawn = self.pawn(id);
        if pawn.at == self.base_of(pawn.team) {
            u16::MAX
        } else {
            pawn.left_base_at
        }
    }

    #[must_use]
    pub fn flags_held_by(&self, team: TeamId) -> usize {
        self.flags
            .iter()
            .filter(|flag| flag.planted_by == Some(team))
            .count()
    }

    #[must_use]
    pub fn occupant(&self, coord: HexCoord) -> Option<PawnId> {
        self.pawns
            .iter()
            .find(|pawn| !pawn.jailed && pawn.at == coord)
            .map(|pawn| pawn.id)
    }

    pub fn unplanted_flags(&self) -> impl Iterator<Item = &Flag> + '_ {
        self.flags.iter().filter(|flag| flag.planted_by.is_none())
    }

    /// A stable fingerprint of everything the rules may read. Two runs of the
    /// same `ModeSpec` and intent log must agree on this, which is what the
    /// determinism test asserts.
    #[must_use]
    pub fn digest(&self) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |value: u64| {
            hash ^= value;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        };
        eat(u64::from(self.turn));
        for pawn in &self.pawns {
            eat(u64::from(pawn.left_base_at));
        }
        for edge in self.board.interior_edges() {
            eat(self.board.port(edge) as u64);
        }
        for pawn in &self.pawns {
            eat(u64::from(pawn.at.q) << 32 | u64::from(pawn.at.r));
            eat(pawn.facing.index() as u64);
            eat(u64::from(pawn.jailed));
        }
        for guardian in &self.guardians {
            eat(u64::from(guardian.at.q) << 32 | u64::from(guardian.at.r));
        }
        for flag in &self.flags {
            eat(flag.planted_by.map_or(u64::MAX, |team| u64::from(team.0)));
        }
        // The verdict is part of what a match *was*. Without it two runs that
        // disagreed about who won could fingerprint identically, which makes
        // the digest useless for exactly the comparison it exists to serve.
        eat(match self.outcome {
            None => 0,
            Some(Outcome::Won(team)) => 1 + u64::from(team.0),
            Some(Outcome::Lost(reason)) => 100 + reason as u64,
            Some(Outcome::Draw) => 200,
        });
        for change in &self.telegraph {
            eat(u64::from(change.edge.cell.q) << 32 | u64::from(change.edge.cell.r));
            eat(change.edge.face.index() as u64);
            eat(change.to as u64);
        }
        hash
    }
}
