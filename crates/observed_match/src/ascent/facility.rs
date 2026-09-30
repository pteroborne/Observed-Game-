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

use std::borrow::Cow;
use std::collections::BTreeMap;

use observed_core::PlayerId;

use super::economy::KINETIC_SHOT_COST;
use super::session::{ASCENT_INPUT_VERSION, AscentSession, InputFrame, Refusal, Role, Seat};
use super::sim::{
    ArchitectLab, Embodiment, GuardianId, GuardianKind, MatchOutcome, ObserverId, TeamId,
};
use crate::hex_wfc::{HexInputFrame, HexMatchEventKind, HexReleasedKind, HexWfcMatch};

mod bots;
mod doors;
mod power;
mod sensors;
pub use doors::AtDoor;
pub use power::{AtFixture, FIXTURE_REACH, Fixture, FixtureKind};
pub use sensors::AtSensor;

/// The rules' id for the first-person match's own Guardian, the Tumbler: reserved, above
/// every Guardian the rules release themselves.
pub const TUMBLER: GuardianId = GuardianId(u16::MAX);

/// The Rogue's seat, always a bot's: the faction the corrupted join, whose seats become
/// Rogue seats of their own as they fall.
pub const ROGUE_SEAT: PlayerId = PlayerId(ARCHITECT_SEATS - 1);

/// Architect seats are numbered past every body: team `t`'s Architect is this plus `t`.
/// Every peer of a LAN match seats them the same way.
pub const ARCHITECT_SEATS: u16 = 200;

/// The seat team `team`'s Architect sits in.
#[must_use]
pub fn architect_seat(team: observed_core::TeamId) -> PlayerId {
    PlayerId(ARCHITECT_SEATS + u16::from(team.0))
}

/// An Architect seat for every team of `physical`: `human`'s held by a player, every
/// other by a bot.
#[must_use]
pub fn architect_seats(
    physical: &HexWfcMatch,
    human: Option<observed_core::TeamId>,
) -> BTreeMap<PlayerId, Seat> {
    architect_seats_where(physical, |team| Some(team) == human)
}

/// The seats every Ascent match has besides its bodies: an Architect for every team of
/// `physical`, held by a player where `human` says so and by a bot everywhere else, and
/// the Rogue, always a bot, whom the corrupted join (`ROGUE_SEAT`).
#[must_use]
pub fn architect_seats_where(
    physical: &HexWfcMatch,
    human: impl Fn(observed_core::TeamId) -> bool,
) -> BTreeMap<PlayerId, Seat> {
    let mut seats: BTreeMap<PlayerId, Seat> = physical
        .teams
        .keys()
        .map(|&team| {
            (
                architect_seat(team),
                Seat {
                    role: Role::Architect(TeamId(team.0)),
                    bot: !human(team),
                },
            )
        })
        .collect();
    seats.insert(
        ROGUE_SEAT,
        Seat {
            role: Role::Rogue,
            bot: true,
        },
    );
    seats
}

/// The rules seat `player`'s seat command belongs to: their team's Architect seat when
/// they sit at its desk (`at_desk`), their own otherwise. Every peer of a LAN match maps a
/// frame's seat commands this way.
#[must_use]
pub fn seat_for(physical: &HexWfcMatch, player: PlayerId, at_desk: bool) -> PlayerId {
    match physical.players.get(&player) {
        Some(state) if at_desk => architect_seat(state.team),
        _ => player,
    }
}

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
    /// Every floor's generator and recharge station, where each stands (`power`).
    fixtures: Vec<Fixture>,
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
        let mut fixtures = Vec::new();
        let sim = ArchitectLab::over_facility(
            physical.facility.clone(),
            seed,
            &embodiments,
            lobby,
            |world, observers, prison_core| {
                let (economy, sited) = power::site(world, observers, prison_core, physical);
                fixtures = sited;
                economy
            },
        );
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
            fixtures,
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
        physical.step(&self.affordable(bodies));
        self.pay_for_shots(physical);
        self.observe(physical);
        self.operate_generators(physical, bodies);
        self.operate_doors(physical, bodies);
        self.operate_sensors(physical, bodies);
        let refusals = self.session.advance(seats)?;
        self.recharge_at_stations(physical);
        let rewrites = self.session.sim.take_rewrites();
        if !rewrites.is_empty() {
            // Legality admits only tiles the authored corpus builds, and never a room or a
            // stair, so a rewrite the physical match cannot build is a broken invariant.
            physical
                .apply_directed_change(rewrites)
                .expect("the rules rewrite only what the facility can build");
        }
        // After the rewrites, which consume the doors of the cells they took.
        self.place_doors(physical);
        // The major Guardians' bodies walk where the Rogue sent them, until the rules say
        // the directive is spent (`ascent::sim::directive`).
        physical.direct_guardians(self.session.sim.directed.map(|directive| directive.cell));
        self.place_sensors(physical);
        // What the rules release - a wave's minors, a requisition's major - is given a
        // body here, under the rules' own id, and the rules follow that body from the next
        // tick (`observe`). A release onto a cell the facility no longer builds is lost.
        for guardian in self.session.sim.take_releases() {
            let kind = match guardian.kind {
                GuardianKind::Major => HexReleasedKind::Major,
                GuardianKind::Minor => HexReleasedKind::Minor,
            };
            physical.release_guardian(guardian.id.0, kind, guardian.cell);
        }
        // A floor that has collapsed takes its minors with it.
        let collapsed = &self.session.sim.collapsed_floors;
        let taken: Vec<u16> = physical
            .released
            .iter()
            .filter(|(_, guardian)| {
                guardian.kind() == HexReleasedKind::Minor
                    && collapsed.contains(&guardian.cell().level)
            })
            .map(|(&id, _)| id)
            .collect();
        for id in taken {
            physical.remove_released(id);
        }
        // The rules decide the match; the physical match stops when they have.
        if self.session.sim.outcome != MatchOutcome::Running {
            physical.status = crate::hex_wfc::HexMatchStatus::Finished;
        }
        Ok(refusals)
    }

    /// `bodies`, less every kinetic shot whose Observer's pool cannot pay for it. The rules
    /// own charge; the physical match only resolves a shot it is handed.
    fn affordable<'a>(&self, bodies: &'a HexInputFrame) -> Cow<'a, HexInputFrame> {
        let broke = |player: &PlayerId| {
            self.bodies
                .get(player)
                .is_some_and(|&id| self.session.sim.economy.charge(id) < KINETIC_SHOT_COST)
        };
        let shoots = |actions: &crate::hex_wfc::HexActionButtons| {
            actions.kinetic_push || actions.kinetic_pull
        };
        if !bodies
            .commands
            .iter()
            .any(|(player, command)| shoots(&command.actions) && broke(player))
        {
            return Cow::Borrowed(bodies);
        }
        let mut frame = bodies.clone();
        for (player, command) in &mut frame.commands {
            if broke(player) {
                command.actions.kinetic_push = false;
                command.actions.kinetic_pull = false;
            }
        }
        Cow::Owned(frame)
    }

    /// Spend [`KINETIC_SHOT_COST`] from the pool of every Observer whose shot landed this
    /// tick. A shot that selected nothing raised no event and costs nothing.
    fn pay_for_shots(&mut self, physical: &HexWfcMatch) {
        for event in &physical.recent_events {
            if matches!(
                event.kind,
                HexMatchEventKind::KineticPush | HexMatchEventKind::KineticPull
            ) && let Some(&id) = event.player.as_ref().and_then(|p| self.bodies.get(p))
            {
                self.session.sim.economy.spend_charge(id, KINETIC_SHOT_COST);
            }
        }
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
        // The match's own Guardian, the Tumbler, is the rules' too: where it stands, in a
        // match it hunts in - frozen by sight or not, it is standing there. Its id is
        // reserved above anything the rules release.
        self.session.sim.embody_guardian(
            TUMBLER,
            GuardianKind::Major,
            physical.guardian.cell,
            physical.guardian_hunts(),
        );
        // Every Guardian released since tick zero, where its body is; one whose body has
        // gone - lost to the void, taken with its floor - is gone from the rules too.
        for (&id, guardian) in &physical.released {
            let kind = match guardian.kind() {
                HexReleasedKind::Major => GuardianKind::Major,
                HexReleasedKind::Minor => GuardianKind::Minor,
            };
            self.session
                .sim
                .embody_guardian(GuardianId(id), kind, guardian.cell(), true);
        }
        self.session
            .sim
            .retire_embodied_guardians(|id| id == TUMBLER || physical.released.contains_key(&id.0));
        // A lantern's anchor is the rules' anchor: no card rewrites or retracts what it holds.
        self.session.sim.anchored = physical.anchored_cells();
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

    /// Bring a card of `kind` into the hand `seat` plays from - a joined Rogue's own, a
    /// team's, or the rules' Rogue hand - and say which, if its deck deals one. For
    /// evidence captures and tests, as `stage_door` is: play draws only by refill.
    pub fn stage_card(
        &mut self,
        seat: PlayerId,
        kind: crate::ascent::sim::CardKind,
    ) -> Option<crate::ascent::sim::CardId> {
        let session = &mut self.session;
        let deck = if let Some(hand) = session.rogue_hands.get_mut(&seat) {
            &mut hand.deck
        } else if let Some(team) = session.team(seat) {
            &mut session.hands.get_mut(&team)?.deck
        } else {
            &mut session.sim.deck
        };
        deck.stage_kind(kind).then_some(())?;
        deck.hand
            .iter()
            .find(|card| card.kind == kind)
            .map(|card| card.id)
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
