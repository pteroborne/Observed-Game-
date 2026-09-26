//! Team requests from the seats bots hold: what a bot Observer asks for, and how a bot
//! Architect answers.
//!
//! A bot Observer - a bot seat, or a body a bot drives (`voice`) - asks its Architect
//! for help when it is in trouble it can name, through
//! the same `SeatCommand::Request` a human's ask takes, so the request is bounded, team-only
//! and refused on a cell the team has not found, like any other:
//!
//! - **Need rescue**, jailed: at the prison lobby, where a rescuer has to go, which every
//!   team knows whether or not it has found it.
//! - **Restore power**, on a floor without it.
//! - **Build a route**, stuck: on the same cell for [`STALL_BEATS`] beats.
//!
//! It asks once and lets the request stand; when the trouble passes it withdraws the
//! request, and when the trouble changes it asks for the new thing. It judges only what a
//! body standing there would know: where it is, whether the lights are on, and how long it
//! has been going nowhere - never the rules' own map of the facility.
//!
//! A bot Architect acknowledges its team's requests on its beat, and builds toward the
//! routes it was asked for (`ArchitectLab::loyal_intent`).

use observed_core::PlayerId;
use observed_hex::HexCoord;

use super::{AscentSession, RequestKind, Role, SeatCommand};
use crate::ascent::sim::{ACTOR_BEAT_TICKS, ObserverId, ObserverState, TeamId};

/// How many beats on one cell make a bot Observer stuck.
pub const STALL_BEATS: u64 = 8;

impl AscentSession {
    /// Voice the requests of `players`' Observer seats, whose bodies a bot drives outside
    /// the rules: an embodied match's bodies are human seats to the rules, which take their
    /// movement from the physical match, but a bot moving one still needs to ask for help.
    pub fn voice(&mut self, players: impl IntoIterator<Item = PlayerId>) {
        self.voiced.extend(players);
    }

    /// The trouble a bot Observer can name, and where: nothing when it has none.
    pub(super) fn trouble(&self, id: ObserverId) -> Option<(RequestKind, HexCoord)> {
        let observer = self.sim.observers.get(&id)?;
        match observer.state {
            ObserverState::Corrupted => None,
            ObserverState::Jailed => Some((RequestKind::Rescue, observer.cell)),
            ObserverState::Active if !self.sim.economy.is_powered(observer.cell.level) => {
                Some((RequestKind::Power, observer.cell))
            }
            ObserverState::Active => {
                let (cell, since) = self.stalls.get(&id)?;
                let beats = self.sim.tick.saturating_sub(*since) / u64::from(ACTOR_BEAT_TICKS);
                (*cell == observer.cell && beats >= STALL_BEATS)
                    .then_some((RequestKind::Route, observer.cell))
            }
        }
    }

    /// What `player`'s body asks its Architect for when its player asks: the trouble it
    /// is in if it is jailed or in the dark, as a bot would name it; otherwise a route on
    /// from the cell it faces, if the team has found that cell, or from where it stands.
    /// Nothing for a seat that is not a body, or a body lost to the void.
    #[must_use]
    pub fn ask_for_help(&self, player: PlayerId) -> Option<(RequestKind, HexCoord)> {
        let Role::Observer(id) = self.seats.get(&player)?.role else {
            return None;
        };
        let observer = self.sim.observers.get(&id)?;
        match observer.state {
            ObserverState::Corrupted => None,
            ObserverState::Jailed => Some((RequestKind::Rescue, observer.cell)),
            ObserverState::Active if !self.sim.economy.is_powered(observer.cell.level) => {
                Some((RequestKind::Power, observer.cell))
            }
            ObserverState::Active => {
                let known = self.sim.team_knowledge(observer.team).discovered_cells;
                let ahead = self
                    .sim
                    .world
                    .config
                    .grid()
                    .neighbor(observer.cell, observer.facing)
                    .filter(|cell| known.contains(cell));
                Some((RequestKind::Route, ahead.unwrap_or(observer.cell)))
            }
        }
    }

    /// Once a beat: note where every Observer stands, and let every bot Observer ask for
    /// what it needs or withdraw what it no longer does.
    pub(super) fn run_bot_requests(&mut self) {
        let tick = self.sim.tick;
        for (&id, observer) in &self.sim.observers {
            let moved = self
                .stalls
                .get(&id)
                .is_none_or(|(cell, _)| *cell != observer.cell);
            if moved {
                self.stalls.insert(id, (observer.cell, tick));
            }
        }
        if !(tick + 1).is_multiple_of(u64::from(ACTOR_BEAT_TICKS)) {
            return;
        }
        let bots: Vec<(PlayerId, ObserverId)> = self
            .seats
            .iter()
            .filter(|(player, seat)| seat.bot || self.voiced.contains(player))
            .filter_map(|(&player, seat)| match seat.role {
                Role::Observer(id) => Some((player, id)),
                _ => None,
            })
            .collect();
        for (player, id) in bots {
            let asking = self.requests.get(&player).map(|r| (r.kind, r.target));
            match self.trouble(id) {
                Some(trouble) if asking != Some(trouble) => {
                    // A cell the team has not found is refused, as a human's ask would be;
                    // the bot asks again next beat, once it has.
                    let _ = self.accept(
                        player,
                        SeatCommand::Request {
                            kind: trouble.0,
                            target: trouble.1,
                        },
                    );
                }
                Some(_) => {}
                None => {
                    self.requests.remove(&player);
                }
            }
        }
    }

    /// A bot Architect of `team` acknowledges every request its team has not yet had
    /// acknowledged.
    pub(super) fn acknowledge_as_bot(&mut self, team: TeamId) {
        let Some(architect) = self
            .seats
            .iter()
            .find(|(_, seat)| seat.bot && seat.role == Role::Architect(team))
            .map(|(&player, _)| player)
        else {
            return;
        };
        let unanswered: Vec<(PlayerId, u64)> = self
            .requests
            .values()
            .filter(|request| request.team == team && request.acknowledged_by.is_none())
            .map(|request| (request.author, request.created_at))
            .collect();
        for (author, created_at) in unanswered {
            let _ = self.accept(architect, SeatCommand::Acknowledge { author, created_at });
        }
    }

    /// Where `team` has asked its Architect for a route, oldest first.
    #[must_use]
    pub fn asked_routes(&self, team: TeamId) -> Vec<HexCoord> {
        let mut asked: Vec<_> = self
            .requests
            .values()
            .filter(|request| request.team == team && request.kind == RequestKind::Route)
            .map(|request| (request.created_at, request.target))
            .collect();
        asked.sort_unstable();
        asked.into_iter().map(|(_, target)| target).collect()
    }
}
