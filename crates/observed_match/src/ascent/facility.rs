//! Architect Ascent on the real facility: one world, walked by bodies and rewritten by
//! Architects.
//!
//! The first-person match ([`HexWfcMatch`]) owns bodies, physics and geometry. The
//! Ascent rules ([`AscentSession`]) own cards, cooldowns, contradictions, retraction,
//! power, sight and outcomes. Each tick the bodies move first; the rules then read where
//! they are and which way they face, apply the seats' commands, and everything they
//! rewrote is committed to the physical facility as one directed change.
//!
//! The rules are the only writer. They keep the logical facility they reason about and
//! the physical match keeps the one it builds, and every rewrite reaches both, so after
//! every step the two hold the same placements. The rules never move a body, and the
//! director's scheduled relayout is switched off: here the facility changes because an
//! Architect played a card or a contradiction retracted, and for no other reason.

use std::collections::BTreeMap;

use observed_core::PlayerId;

use super::session::{ASCENT_INPUT_VERSION, AscentSession, InputFrame, Refusal, Role, Seat};
use super::sim::{ArchitectLab, Embodiment, MatchOutcome, ObserverId, TeamId};
use crate::hex_wfc::{HexInputFrame, HexWfcMatch};

#[derive(Clone, Debug)]
pub struct AscentMatch {
    physical: HexWfcMatch,
    session: AscentSession,
    /// Which body each Observer is.
    bodies: BTreeMap<PlayerId, ObserverId>,
}

impl AscentMatch {
    /// Every player of `physical` is an embodied Observer on the team the physical match
    /// gave them. `seats` names everyone without a body: one Architect for each of those
    /// teams, and any Rogue operators.
    ///
    /// `physical` must not have stepped: a match is directed from tick zero or never.
    pub fn new(
        mut physical: HexWfcMatch,
        seed: u64,
        seats: BTreeMap<PlayerId, Seat>,
    ) -> Result<Self, Refusal> {
        if physical.tick != 0 {
            return Err(Refusal::Tick);
        }
        if seats.iter().any(|(player, seat)| {
            physical.players.contains_key(player) || matches!(seat.role, Role::Observer(_))
        }) {
            return Err(Refusal::Roster);
        }
        physical.hand_mutation_to_architects();
        let mut bodies = BTreeMap::new();
        let mut embodiments = Vec::new();
        for (&player, state) in &physical.players {
            let id = ObserverId(player.0);
            let (cell, facing) = physical
                .body_cell_and_facing(player)
                .expect("every player has a body");
            embodiments.push(Embodiment {
                id,
                team: TeamId(state.team.0),
                cell,
                facing,
            });
            bodies.insert(player, id);
        }
        let sim = ArchitectLab::over_facility(physical.facility.clone(), seed, &embodiments);
        let mut roster = seats;
        for (&player, &id) in &bodies {
            roster.insert(
                player,
                Seat {
                    role: Role::Observer(id),
                    bot: false,
                },
            );
        }
        let mut game = Self {
            physical,
            session: AscentSession::new(sim, seed, roster)?,
            bodies,
        };
        game.observe();
        Ok(game)
    }

    /// Advance one fixed tick: `bodies` moves the Observers, `seats` carries every
    /// Architect's and Rogue's command. Returns the commands the rules refused.
    ///
    /// A frame the rules would refuse whole is refused before any body moves, so the
    /// physical clock and the rules' clock never part.
    pub fn step(
        &mut self,
        bodies: &HexInputFrame,
        seats: &InputFrame,
    ) -> Result<BTreeMap<PlayerId, Refusal>, Refusal> {
        if seats.version != ASCENT_INPUT_VERSION {
            return Err(Refusal::Version);
        }
        if seats.tick != self.session.sim.tick + 1 {
            return Err(Refusal::Tick);
        }
        if self.session.sim.outcome != MatchOutcome::Running {
            return Err(Refusal::MatchFinished);
        }
        self.physical.step(bodies);
        self.observe();
        let refusals = self.session.advance(seats)?;
        let rewrites = self.session.sim.take_rewrites();
        if !rewrites.is_empty() {
            // Legality admits only tiles the authored corpus builds, and never a room or a
            // stair, so a rewrite the physical match cannot build is a broken invariant.
            self.physical
                .apply_directed_change(rewrites)
                .expect("the rules rewrite only what the facility can build");
        }
        Ok(refusals)
    }

    /// Give the rules the bodies' cells and facings. What an Observer sees and wards
    /// is still the rules' cell sight along that facing: the physical match has no
    /// field of view of its own to give them yet.
    fn observe(&mut self) {
        for (&player, &id) in &self.bodies {
            if let Some((cell, facing)) = self.physical.body_cell_and_facing(player) {
                self.session.sim.embody(id, cell, facing);
            }
        }
        self.session.sim.refresh_observation();
    }

    /// The first-person match: bodies, geometry, colliders, the Guardian.
    #[must_use]
    pub const fn physical(&self) -> &HexWfcMatch {
        &self.physical
    }

    /// The seats and their rules.
    #[must_use]
    pub const fn session(&self) -> &AscentSession {
        &self.session
    }

    /// The rules, which a card preview inspects without advancing time.
    #[must_use]
    pub const fn rules(&self) -> &ArchitectLab {
        &self.session.sim
    }

    /// The Observer a body is.
    #[must_use]
    pub fn observer_for(&self, player: PlayerId) -> Option<ObserverId> {
        self.bodies.get(&player).copied()
    }
}

#[cfg(test)]
mod tests;
