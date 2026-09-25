//! Stable seats and team-scoped commands over the single Ascent simulation.
//!
//! A transport authenticates a PlayerId; it never accepts a client-supplied role,
//! team, ObserverId, or authoritative position. Both local and remote adapters
//! feed the same versioned frames into this boundary.

use std::collections::BTreeMap;

use observed_core::PlayerId;
use observed_hex::HexCoord;

use super::sim::{
    ArchitectCommand, ArchitectLab, CommandRefusal, Deck, MatchOutcome, ObserverCommand,
    ObserverId, ObserverRefusal, ObserverState, TeamId,
};

mod snapshot;
pub use snapshot::{CellRead, GuardianRead, ObserverRead, SeatSnapshot};

pub const ASCENT_INPUT_VERSION: u16 = 1;
pub const REQUEST_LIFETIME_TICKS: u64 = 900;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Architect(TeamId),
    Observer(ObserverId),
    Rogue,
    Spectator(TeamId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Seat {
    pub role: Role,
    pub bot: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestKind {
    Route,
    Power,
    Recharge,
    Rescue,
    HoldObservation,
    ReleaseObservation,
}

impl RequestKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Route => "Build a route",
            Self::Power => "Restore power",
            Self::Recharge => "Need recharge",
            Self::Rescue => "Need rescue",
            Self::HoldObservation => "Hold this in view",
            Self::ReleaseObservation => "Look away for construction",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TeamRequest {
    pub author: PlayerId,
    pub team: TeamId,
    pub kind: RequestKind,
    pub target: HexCoord,
    pub created_at: u64,
    pub acknowledged_by: Option<PlayerId>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SeatCommand {
    #[default]
    None,
    Architect(ArchitectCommand),
    Observer(ObserverCommand),
    Request {
        kind: RequestKind,
        target: HexCoord,
    },
    Acknowledge {
        author: PlayerId,
        created_at: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputFrame {
    pub version: u16,
    pub tick: u64,
    pub commands: BTreeMap<PlayerId, SeatCommand>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    Version,
    Tick,
    Roster,
    UnknownSeat,
    WrongRole,
    UnknownTarget,
    RequestExpired,
    MatchFinished,
    Architect(CommandRefusal),
    Observer(ObserverRefusal),
}

#[derive(Clone, Debug)]
pub struct LoyalHand {
    pub deck: Deck,
    pub cooldown: u32,
}

#[derive(Clone, Debug)]
pub struct AscentSession {
    pub sim: ArchitectLab,
    seats: BTreeMap<PlayerId, Seat>,
    pub hands: BTreeMap<TeamId, LoyalHand>,
    pub requests: BTreeMap<PlayerId, TeamRequest>,
}

impl AscentSession {
    /// Bind each Observer exactly once and each loyal Architect at most once.
    /// Every participating team has one Architect; bots occupy ordinary seats.
    pub fn new(
        mut sim: ArchitectLab,
        seed: u64,
        seats: BTreeMap<PlayerId, Seat>,
    ) -> Result<Self, Refusal> {
        if seats.is_empty() || seats.len() > 16 {
            return Err(Refusal::Roster);
        }
        let mut observers = std::collections::BTreeSet::new();
        let mut architects = std::collections::BTreeSet::new();
        for seat in seats.values() {
            match seat.role {
                Role::Observer(id) if !sim.observers.contains_key(&id) || !observers.insert(id) => {
                    return Err(Refusal::Roster);
                }
                Role::Architect(team) if !architects.insert(team) => return Err(Refusal::Roster),
                _ => {}
            }
        }
        if observers.len() != sim.observers.len()
            || seats.values().any(
                |seat| matches!(seat.role, Role::Spectator(team) if !architects.contains(&team)),
            )
            || sim
                .observers
                .values()
                .any(|o| !architects.contains(&o.team))
            || architects
                .iter()
                .any(|team| !sim.observers.values().any(|o| o.team == *team))
        {
            return Err(Refusal::Roster);
        }
        sim.bot_architect = false;
        sim.planning = false;
        sim.setup_placements_left = 0;
        sim.refresh_observation();
        let hands = architects
            .into_iter()
            .map(|team| {
                (
                    team,
                    LoyalHand {
                        deck: sim
                            .new_deck(seed ^ (u64::from(team.0) + 1).wrapping_mul(0x9E37_79B9)),
                        cooldown: 0,
                    },
                )
            })
            .collect();
        Ok(Self {
            sim,
            seats,
            hands,
            requests: BTreeMap::new(),
        })
    }

    pub fn seats(&self) -> &BTreeMap<PlayerId, Seat> {
        &self.seats
    }

    pub fn team(&self, player: PlayerId) -> Option<TeamId> {
        match self.seats.get(&player)?.role {
            Role::Architect(team) | Role::Spectator(team) => Some(team),
            Role::Observer(id) => self.sim.observers.get(&id).map(|o| o.team),
            Role::Rogue => None,
        }
    }

    /// Inspect a card preview without advancing time, consuming a card, or
    /// copying the world. Commit rechecks these same rules against current state.
    pub fn architect_refusal(
        &self,
        player: PlayerId,
        command: ArchitectCommand,
    ) -> Option<Refusal> {
        let Some(seat) = self.seats.get(&player) else {
            return Some(Refusal::UnknownSeat);
        };
        match seat.role {
            Role::Rogue => self.sim.refusal(command).map(Refusal::Architect),
            Role::Architect(team) => {
                let hand = &self.hands[&team];
                let known = &self.sim.team_knowledge[&team].discovered_cells;
                self.sim
                    .refusal_in_context(command, &hand.deck, hand.cooldown, known)
                    .map(Refusal::Architect)
            }
            _ => Some(Refusal::WrongRole),
        }
    }

    /// An entire mismatched or repeated frame is refused before any command mutates
    /// state. Individual refusals do not stop other seats from acting this tick.
    pub fn advance(&mut self, frame: &InputFrame) -> Result<BTreeMap<PlayerId, Refusal>, Refusal> {
        if frame.version != ASCENT_INPUT_VERSION {
            return Err(Refusal::Version);
        }
        if frame.tick != self.sim.tick + 1 {
            return Err(Refusal::Tick);
        }
        if self.sim.outcome != MatchOutcome::Running {
            return Err(Refusal::MatchFinished);
        }
        let mut refusals = BTreeMap::new();
        let mut observers = BTreeMap::new();
        for seat in self.seats.values() {
            if let Role::Observer(id) = seat.role
                && !seat.bot
            {
                observers.insert(id, ObserverCommand::default());
            }
        }
        for (&player, &command) in &frame.commands {
            match self.accept(player, command) {
                Ok(Some((id, command))) => {
                    observers.insert(id, command);
                }
                Ok(None) => {}
                Err(reason) => {
                    refusals.insert(player, reason);
                }
            }
        }
        for hand in self.hands.values_mut() {
            hand.cooldown = hand.cooldown.saturating_sub(1);
        }
        self.sim.tick_with_observers(&observers);
        self.requests.retain(|_, request| {
            self.sim.tick.saturating_sub(request.created_at) < REQUEST_LIFETIME_TICKS
        });
        for seat in self.seats.values_mut() {
            match seat.role {
                Role::Observer(id) if self.sim.observers[&id].state == ObserverState::Corrupted => {
                    seat.role = Role::Rogue
                }
                Role::Architect(team)
                    if !self
                        .sim
                        .observers
                        .values()
                        .any(|o| o.team == team && o.state != ObserverState::Corrupted) =>
                {
                    seat.role = Role::Spectator(team)
                }
                _ => {}
            }
        }
        Ok(refusals)
    }

    fn accept(
        &mut self,
        player: PlayerId,
        command: SeatCommand,
    ) -> Result<Option<(ObserverId, ObserverCommand)>, Refusal> {
        let seat = *self.seats.get(&player).ok_or(Refusal::UnknownSeat)?;
        match command {
            SeatCommand::None => Ok(None),
            SeatCommand::Observer(command) => {
                let Role::Observer(id) = seat.role else {
                    return Err(Refusal::WrongRole);
                };
                self.sim
                    .submit_observer(id, command)
                    .map_err(Refusal::Observer)?;
                Ok(Some((id, ObserverCommand::default())))
            }
            SeatCommand::Architect(command) => {
                match seat.role {
                    Role::Rogue => self.sim.submit(command).map_err(Refusal::Architect)?,
                    Role::Architect(team) => self.submit_loyal(team, command)?,
                    _ => return Err(Refusal::WrongRole),
                }
                Ok(None)
            }
            SeatCommand::Request { kind, target } => {
                if matches!(seat.role, Role::Spectator(_)) {
                    return Err(Refusal::WrongRole);
                }
                let team = self.team(player).ok_or(Refusal::WrongRole)?;
                if !self
                    .sim
                    .team_knowledge(team)
                    .discovered_cells
                    .contains(&target)
                {
                    return Err(Refusal::UnknownTarget);
                }
                self.requests.insert(
                    player,
                    TeamRequest {
                        author: player,
                        team,
                        kind,
                        target,
                        created_at: self.sim.tick,
                        acknowledged_by: None,
                    },
                );
                Ok(None)
            }
            SeatCommand::Acknowledge { author, created_at } => {
                if matches!(seat.role, Role::Spectator(_)) {
                    return Err(Refusal::WrongRole);
                }
                let team = self.team(player).ok_or(Refusal::WrongRole)?;
                let request = self
                    .requests
                    .get_mut(&author)
                    .ok_or(Refusal::RequestExpired)?;
                if request.team != team {
                    return Err(Refusal::WrongRole);
                }
                if request.created_at != created_at
                    || self.sim.tick.saturating_sub(created_at) >= REQUEST_LIFETIME_TICKS
                {
                    return Err(Refusal::RequestExpired);
                }
                request.acknowledged_by = Some(player);
                Ok(None)
            }
        }
    }

    fn submit_loyal(&mut self, team: TeamId, command: ArchitectCommand) -> Result<(), Refusal> {
        let hand = self.hands.get_mut(&team).ok_or(Refusal::WrongRole)?;
        // Swap only the acting faction's command context; there is still exactly one
        // facility, economy, Guardian population, and authoritative mutation path.
        std::mem::swap(&mut self.sim.deck, &mut hand.deck);
        std::mem::swap(&mut self.sim.cooldown, &mut hand.cooldown);
        let known = self.sim.team_knowledge(team).discovered_cells;
        let rogue_known = std::mem::replace(&mut self.sim.known, known);
        let result = self.sim.submit_for_faction(command, Some(team));
        self.sim.known = rogue_known;
        std::mem::swap(&mut self.sim.deck, &mut hand.deck);
        std::mem::swap(&mut self.sim.cooldown, &mut hand.cooldown);
        result.map_err(Refusal::Architect)
    }
}

#[cfg(test)]
mod tests;
