//! Two threat models, and the three ways observation may bear on one.

use observed_hex::coords::{HexCoord, lateral_distance};

use crate::sim::rules::{LockSet, Setback, Threat};
use crate::sim::state::{MatchState, PawnId};

/// What a guardian walks towards. Camping guardians play nothing like hunting
/// ones, which is what makes this a strategy rather than a number.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GuardianTarget {
    /// The closest free pawn.
    #[default]
    NearestPawn,
    /// The nearest flag nobody has planted yet — a camper, not a hunter.
    CampNearestUnplantedFlag,
    /// The prison, so rescues are contested rather than free.
    GuardPrison,
    /// Around the flags in order, indifferent to where the pawns are.
    FixedPatrol,
}

/// Whether being observed bears on a guardian at all.
///
/// Reasoning did not settle this, so it is a setting rather than a decision:
/// the three readings produce visibly different games and the lab exists to
/// tell them apart.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConeInteraction {
    /// Observation holds structure and nothing else. A pawn is never safe
    /// merely by facing the right way.
    Ignores,
    /// A guardian may not enter a cell somebody is *looking at*. Facing is a
    /// shield — and only a shield in the direction you point it, so turning
    /// away from a guardian is a real risk and facing it is a real defence.
    ///
    /// **The shipped rule.** Without it a walled board is a guardian trap:
    /// corridors remove the room to evade, and the squad was being wiped in six
    /// turns with no counterplay. Making the cone a shield gives facing
    /// something to do besides hold structure, and turns "where is everyone
    /// looking" into the squad's actual defence.
    #[default]
    Blocked,
    /// A guardian may enter an observed cell but takes nobody that turn.
    /// Facing buys time, not safety.
    Slowed,
}

#[derive(Clone, Copy, Debug)]
pub struct Guardians {
    pub target: GuardianTarget,
    pub cone: ConeInteraction,
    /// Guardians move on turns divisible by this. **Not a difficulty knob.**
    ///
    /// At cadence 1 a guardian moves one hex per turn and so does a pawn, which
    /// on a 37-cell board makes it strictly inescapable: it closes distance it
    /// never gives back and corners you against the rim. The first draft ran at
    /// cadence 1 and jailed every pawn by turn seven, every time. A slower
    /// threat is one a player can be clever about, which is the only kind worth
    /// having.
    pub cadence: u16,
}

impl Default for Guardians {
    fn default() -> Self {
        Self {
            target: GuardianTarget::NearestPawn,
            cone: ConeInteraction::Blocked,
            cadence: 2,
        }
    }
}

impl Guardians {
    fn goal(&self, state: &MatchState, guardian: HexCoord, index: usize) -> Option<HexCoord> {
        match self.target {
            GuardianTarget::NearestPawn => state
                .free_pawns()
                .min_by_key(|pawn| (lateral_distance(guardian, pawn.at), pawn.id))
                .map(|pawn| pawn.at),
            GuardianTarget::CampNearestUnplantedFlag => state
                .unplanted_flags()
                .min_by_key(|flag| lateral_distance(guardian, flag.at))
                .map(|flag| flag.at),
            GuardianTarget::GuardPrison => Some(state.prisons[index % state.prisons.len()]),
            GuardianTarget::FixedPatrol => {
                // Each guardian owns one flag hex and circles back to it. The
                // stride keeps two guardians from stacking on the same post.
                (!state.flags.is_empty()).then(|| {
                    let post = (index + state.turn as usize / 4) % state.flags.len();
                    state.flags[post].at
                })
            }
        }
    }
}

impl Threat for Guardians {
    fn name(&self) -> &'static str {
        "Guardians"
    }

    fn act(&self, state: &mut MatchState, locks: &LockSet, setback: &dyn Setback) {
        let before: Vec<HexCoord> = state.guardians.iter().map(|g| g.at).collect();
        // A guardian standing still still takes whoever walks into it, so only
        // movement is gated on the cadence.
        let moves = self.cadence <= 1 || state.turn.is_multiple_of(self.cadence);

        for index in 0..state.guardians.len() {
            state.guardians[index].prev_at = state.guardians[index].at;
            if !moves {
                state.guardians[index].stalled = false;
                continue;
            }
            state.guardians[index].stalled = false;
            let at = state.guardians[index].at;
            let Some(goal) = self.goal(state, at, index) else {
                continue;
            };
            let Some(step) = state.board.step_towards(at, goal) else {
                continue;
            };
            match self.cone {
                ConeInteraction::Ignores => state.guardians[index].at = step,
                ConeInteraction::Blocked => {
                    // `is_covered`, not `is_held`: you are shielded where you
                    // are *looking*, not where you happen to stand. Holding the
                    // cell you occupy made every pawn permanently unreachable
                    // and left the guardians parked next to the squad for a
                    // whole match doing nothing at all.
                    if !locks.is_covered(step) {
                        state.guardians[index].at = step;
                    } else {
                        // Route around rather than simply stalling: the nearest
                        // unobserved neighbour that still closes the distance.
                        let detour = state
                            .board
                            .open_neighbours(at)
                            .filter(|&(_, cell)| !locks.is_covered(cell))
                            .min_by_key(|&(_, cell)| lateral_distance(cell, goal));
                        if let Some((_, cell)) = detour
                            && lateral_distance(cell, goal) < lateral_distance(at, goal)
                        {
                            state.guardians[index].at = cell;
                        }
                    }
                }
                ConeInteraction::Slowed => {
                    state.guardians[index].at = step;
                    if locks.is_covered(step) {
                        state.guardians[index].stalled = true;
                    }
                }
            }
        }

        // A pawn is taken when a guardian ends on it, or when the two ran
        // straight through each other. `prev_at` is recorded at the top of the
        // turn, before any pawn moved, which is what makes the swap detectable.
        let mut taken: Vec<PawnId> = Vec::new();
        for pawn in &state.pawns {
            if pawn.jailed || pawn.immune {
                continue;
            }
            let caught = state.guardians.iter().enumerate().any(|(i, guardian)| {
                if guardian.stalled {
                    return false;
                }
                let shared = guardian.at == pawn.at;
                let swapped = guardian.at == pawn.prev_at && before[i] == pawn.at;
                shared || swapped
            });
            if caught {
                taken.push(pawn.id);
            }
        }
        for id in taken {
            setback.take(state, id);
        }
    }
}

/// No threat at all — the control, for asking whether guardians are what makes
/// a mode tense or merely what makes it slow.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoThreat;

impl Threat for NoThreat {
    fn name(&self) -> &'static str {
        "None"
    }

    fn act(&self, _state: &mut MatchState, _locks: &LockSet, _setback: &dyn Setback) {}
}
