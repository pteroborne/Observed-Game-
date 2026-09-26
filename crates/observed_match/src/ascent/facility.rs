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

/// The Ascent rules for one physical match: its seats, and which body each Observer is.
///
/// Kept beside the [`HexWfcMatch`] it rules rather than around it, so a host that already
/// holds a first-person match (the game) keeps holding it, and every reader of that match
/// keeps working. [`AscentMatch`] bundles the two for callers that hold neither.
#[derive(Clone, Debug)]
pub struct AscentRules {
    session: AscentSession,
    /// Which body each Observer is.
    bodies: BTreeMap<PlayerId, ObserverId>,
}

impl AscentRules {
    /// Every player of `physical` is an embodied Observer on the team the physical match
    /// gave them. `seats` names everyone without a body: one Architect for each of those
    /// teams, and any Rogue operators.
    ///
    /// Directs `physical` and gives it a prison, so it must not have stepped: a match is
    /// directed from tick zero or never.
    pub fn new(
        physical: &mut HexWfcMatch,
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
        physical.send_catches_to_prison();
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
        let prison = physical
            .prison
            .as_ref()
            .expect("just sent catches to prison");
        let lobby = (prison.lobby.clone(), prison.lobby_anchor);
        let sim = ArchitectLab::over_facility(physical.facility.clone(), seed, &embodiments, lobby);
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
        let mut rules = Self {
            session: AscentSession::new(sim, seed, roster)?,
            bodies,
        };
        rules.observe(physical);
        Ok(rules)
    }

    /// Advance `physical` and the rules one fixed tick together: `bodies` moves the
    /// Observers, `seats` carries every Architect's and Rogue's command. Returns the
    /// commands the rules refused.
    ///
    /// A frame the rules would refuse whole is refused before any body moves, so the
    /// physical clock and the rules' clock never part.
    pub fn step(
        &mut self,
        physical: &mut HexWfcMatch,
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
        physical.step(bodies);
        self.observe(physical);
        let refusals = self.session.advance(seats)?;
        let rewrites = self.session.sim.take_rewrites();
        if !rewrites.is_empty() {
            // Legality admits only tiles the authored corpus builds, and never a room or a
            // stair, so a rewrite the physical match cannot build is a broken invariant.
            physical
                .apply_directed_change(rewrites)
                .expect("the rules rewrite only what the facility can build");
        }
        // The rules decide the match; the physical match stops when they have.
        if self.session.sim.outcome != MatchOutcome::Running {
            physical.status = crate::hex_wfc::HexMatchStatus::Finished;
        }
        Ok(refusals)
    }

    /// Give the rules the bodies' cells, facings and places: in the facility, jailed, or
    /// lost to the void. What an Observer sees and wards is still the rules' cell sight
    /// along that facing: the physical match has no field of view of its own to give them
    /// yet.
    fn observe(&mut self, physical: &HexWfcMatch) {
        for (&player, &id) in &self.bodies {
            if let Some((cell, facing)) = physical.body_cell_and_facing(player)
                && let Some(place) = physical.body_place(player)
            {
                self.session.sim.embody(id, cell, facing, place);
            }
        }
        self.session.sim.refresh_observation();
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

    /// Voice the requests of the bodies `players`, which a bot drives: their seats are
    /// human to the rules, which move them from the physical match, but a bot still asks
    /// its Architect for help (`AscentSession::voice`).
    pub fn voice(&mut self, players: impl IntoIterator<Item = PlayerId>) {
        self.session.voice(
            players
                .into_iter()
                .filter(|player| self.bodies.contains_key(player)),
        );
    }
}

/// A first-person match and its Ascent rules, held together.
#[derive(Clone, Debug)]
pub struct AscentMatch {
    physical: HexWfcMatch,
    ascent: AscentRules,
}

impl AscentMatch {
    /// See [`AscentRules::new`].
    pub fn new(
        mut physical: HexWfcMatch,
        seed: u64,
        seats: BTreeMap<PlayerId, Seat>,
    ) -> Result<Self, Refusal> {
        let ascent = AscentRules::new(&mut physical, seed, seats)?;
        Ok(Self { physical, ascent })
    }

    /// See [`AscentRules::step`].
    pub fn step(
        &mut self,
        bodies: &HexInputFrame,
        seats: &InputFrame,
    ) -> Result<BTreeMap<PlayerId, Refusal>, Refusal> {
        self.ascent.step(&mut self.physical, bodies, seats)
    }

    /// The first-person match: bodies, geometry, colliders, the Guardian.
    #[must_use]
    pub const fn physical(&self) -> &HexWfcMatch {
        &self.physical
    }

    /// The seats and their rules.
    #[must_use]
    pub const fn session(&self) -> &AscentSession {
        self.ascent.session()
    }

    /// The rules, which a card preview inspects without advancing time.
    #[must_use]
    pub const fn rules(&self) -> &ArchitectLab {
        self.ascent.rules()
    }

    /// The Observer a body is.
    #[must_use]
    pub fn observer_for(&self, player: PlayerId) -> Option<ObserverId> {
        self.ascent.observer_for(player)
    }

    /// See [`AscentRules::voice`].
    pub fn voice(&mut self, players: impl IntoIterator<Item = PlayerId>) {
        self.ascent.voice(players);
    }
}

#[cfg(test)]
mod tests;
