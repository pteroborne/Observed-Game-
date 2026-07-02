//! Session structures and lobby actions.

use super::{
    AccountId, ConnectionState, LaunchManifest, Matchmaker, Participant, ROSTER_SIZE, Region,
    SessionId, SessionPhase, TEAM_COUNT, TEAM_SIZE,
};
use observed_core::{PlayerId, TeamId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub id: SessionId,
    pub region: Region,
    pub build: u32,
    pub participants: Vec<Participant>,
    pub host: Option<AccountId>,
    pub phase: SessionPhase,
    pub match_number: u32,
    pub launch: Option<LaunchManifest>,
    pub host_migrations: u32,
    pub reconnects: u32,
    pub lifecycle_events: u32,
    pub recent_events: Vec<String>,
}

impl Session {
    pub fn formed(id: SessionId, tickets: Vec<super::matchmaking::QueueTicket>) -> Self {
        let region = tickets[0].region;
        let build = tickets[0].build;
        let participants = assign_roster(&tickets);
        let host = participants.iter().map(|player| player.account).min();
        let mut session = Self {
            id,
            region,
            build,
            participants,
            host,
            phase: SessionPhase::Lobby,
            match_number: 0,
            launch: None,
            host_migrations: 0,
            reconnects: 0,
            lifecycle_events: 0,
            recent_events: Vec::new(),
        };
        session.push_event(format!(
            "{} entered lobby: {} players, balanced teams.",
            session.id.label(),
            ROSTER_SIZE
        ));
        session
    }

    pub fn participant(&self, account: AccountId) -> Option<&Participant> {
        self.participants
            .iter()
            .find(|participant| participant.account == account)
    }

    pub fn all_connected(&self) -> bool {
        self.participants.len() == ROSTER_SIZE
            && self
                .participants
                .iter()
                .all(|participant| participant.connection == ConnectionState::Connected)
    }

    pub fn all_ready(&self) -> bool {
        self.all_connected()
            && self
                .participants
                .iter()
                .all(|participant| participant.ready)
    }

    pub fn team_rating(&self, team: TeamId) -> u32 {
        self.participants
            .iter()
            .filter(|participant| participant.team == team)
            .map(|participant| participant.rating as u32)
            .sum()
    }

    pub fn push_event(&mut self, event: String) {
        self.lifecycle_events += 1;
        self.recent_events.push(event);
        if self.recent_events.len() > 8 {
            self.recent_events.remove(0);
        }
    }
}

fn assign_roster(tickets: &[super::matchmaking::QueueTicket]) -> Vec<Participant> {
    let mut by_account = tickets.to_vec();
    by_account.sort_by_key(|ticket| ticket.account);
    let mut participants: Vec<Participant> = by_account
        .iter()
        .enumerate()
        .map(|(index, ticket)| Participant {
            account: ticket.account,
            player: PlayerId(index as u16),
            team: TeamId(0),
            rating: ticket.rating,
            connection: ConnectionState::Connected,
            ready: false,
        })
        .collect();

    let mut rating_order: Vec<usize> = (0..participants.len()).collect();
    rating_order.sort_by_key(|index| {
        (
            std::cmp::Reverse(participants[*index].rating),
            participants[*index].account,
        )
    });
    let mut totals = [0u32; TEAM_COUNT];
    let mut counts = [0usize; TEAM_COUNT];
    for index in rating_order {
        let team = (0..TEAM_COUNT)
            .filter(|team| counts[*team] < TEAM_SIZE)
            .min_by_key(|team| (totals[*team], counts[*team], *team))
            .expect("a team slot is available");
        participants[index].team = TeamId(team as u8);
        totals[team] += participants[index].rating as u32;
        counts[team] += 1;
    }
    participants
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionLabWorld {
    pub matchmaker: Matchmaker,
    pub session: Option<Session>,
    pub demo_step: u8,
    pub reset_count: u32,
}

impl SessionLabWorld {
    pub fn authored() -> Self {
        let mut matchmaker = Matchmaker::new();
        for ticket in authored_tickets() {
            matchmaker
                .enqueue(ticket)
                .expect("unique authored accounts");
        }
        Self {
            matchmaker,
            session: None,
            demo_step: 0,
            reset_count: 0,
        }
    }

    pub fn reset(&mut self) {
        let reset_count = self.reset_count + 1;
        *self = Self {
            reset_count,
            ..Self::authored()
        };
    }

    pub fn advance_demo(&mut self) {
        match self.demo_step {
            0 => {
                self.session = self.matchmaker.form_next();
            }
            1 => {
                if let Some(session) = &mut self.session {
                    let accounts: Vec<AccountId> = session
                        .participants
                        .iter()
                        .map(|participant| participant.account)
                        .collect();
                    for account in accounts {
                        session.set_ready(account, true);
                    }
                }
            }
            2..=4 => {
                if let Some(session) = &mut self.session {
                    session.tick();
                }
            }
            5 => {
                if let Some(session) = &mut self.session {
                    for _ in 0..8 {
                        session.tick();
                    }
                }
            }
            6 => {
                if let Some(session) = &mut self.session
                    && let Some(host) = session.host
                {
                    session.disconnect(host);
                }
            }
            7 => {
                if let Some(session) = &mut self.session {
                    session.tick();
                }
            }
            8 => {
                if let Some(session) = &mut self.session
                    && let Some(account) = session
                        .participants
                        .iter()
                        .find(|participant| participant.connection == ConnectionState::Disconnected)
                        .map(|participant| participant.account)
                {
                    session.reconnect(account);
                }
            }
            9 => {
                if let Some(session) = &mut self.session {
                    session.finish_match();
                }
            }
            _ => {
                if let Some(session) = &mut self.session {
                    session.tick();
                }
            }
        }
        self.demo_step = self.demo_step.saturating_add(1);
    }
}

pub(crate) fn authored_tickets() -> Vec<super::matchmaking::QueueTicket> {
    vec![
        super::matchmaking::QueueTicket {
            account: AccountId(0),
            rating: 1600,
            region: Region::West,
            build: super::CURRENT_BUILD,
            queued_at: 0,
        },
        super::matchmaking::QueueTicket {
            account: AccountId(1),
            rating: 1510,
            region: Region::West,
            build: super::CURRENT_BUILD,
            queued_at: 1,
        },
        super::matchmaking::QueueTicket {
            account: AccountId(2),
            rating: 1430,
            region: Region::West,
            build: super::CURRENT_BUILD,
            queued_at: 2,
        },
        super::matchmaking::QueueTicket {
            account: AccountId(3),
            rating: 1320,
            region: Region::West,
            build: super::CURRENT_BUILD,
            queued_at: 3,
        },
        super::matchmaking::QueueTicket {
            account: AccountId(4),
            rating: 1490,
            region: Region::East,
            build: super::CURRENT_BUILD,
            queued_at: 4,
        },
        super::matchmaking::QueueTicket {
            account: AccountId(5),
            rating: 1500,
            region: Region::West,
            build: super::CURRENT_BUILD + 1,
            queued_at: 5,
        },
    ]
}
