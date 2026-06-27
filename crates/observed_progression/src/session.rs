//! Phase 17 feasibility model: deterministic matchmaking and session formation.
//!
//! The matchmaker selects a compatible four-player roster from a queue, assigns
//! stable `PlayerId`s and balanced `TeamId`s, then hands ownership to a lobby
//! state machine. Readiness, countdown cancellation, host migration, reconnect
//! grace, launch manifests, post-match rematch, and clean closure all live here.
//! Gameplay and lockstep networking consume the launch manifest; they do not own
//! account, queue, or lobby policy.

use std::collections::BTreeSet;

use observed_core::{PlayerId, TeamId};

pub const ROSTER_SIZE: usize = 4;
pub const TEAM_COUNT: usize = 2;
pub const TEAM_SIZE: usize = ROSTER_SIZE / TEAM_COUNT;
pub const COUNTDOWN_TICKS: u8 = 3;
pub const RECONNECT_GRACE_TICKS: u8 = 4;
pub const POST_MATCH_TICKS: u8 = 3;
pub const SKILL_WINDOW: u16 = 400;
pub const PROTOCOL_VERSION: u16 = 1;
pub const CURRENT_BUILD: u32 = 0x2026_0619;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AccountId(pub u16);

impl AccountId {
    pub fn label(self) -> String {
        format!("U{}", self.0 + 1)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(pub u32);

impl SessionId {
    pub fn label(self) -> String {
        format!("S{:04}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Region {
    East,
    West,
}

impl Region {
    pub fn label(self) -> &'static str {
        match self {
            Self::East => "EAST",
            Self::West => "WEST",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueueTicket {
    pub account: AccountId,
    pub rating: u16,
    pub region: Region,
    pub build: u32,
    pub queued_at: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueError {
    DuplicateAccount,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Matchmaker {
    pub queue: Vec<QueueTicket>,
    pub next_session: u32,
    pub formed_sessions: u32,
    pub last_event: String,
}

impl Default for Matchmaker {
    fn default() -> Self {
        Self::new()
    }
}

impl Matchmaker {
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            next_session: 1,
            formed_sessions: 0,
            last_event: "Queue open.".to_string(),
        }
    }

    pub fn enqueue(&mut self, ticket: QueueTicket) -> Result<(), QueueError> {
        if self
            .queue
            .iter()
            .any(|queued| queued.account == ticket.account)
        {
            return Err(QueueError::DuplicateAccount);
        }
        self.queue.push(ticket);
        self.sort_queue();
        self.last_event = format!(
            "{} queued in {} at rating {}.",
            ticket.account.label(),
            ticket.region.label(),
            ticket.rating
        );
        Ok(())
    }

    pub fn form_next(&mut self) -> Option<Session> {
        self.sort_queue();
        let selected = find_compatible_group(&self.queue)?;
        let selected_accounts: BTreeSet<AccountId> =
            selected.iter().map(|ticket| ticket.account).collect();
        self.queue
            .retain(|ticket| !selected_accounts.contains(&ticket.account));

        let session_id = SessionId(self.next_session);
        self.next_session += 1;
        self.formed_sessions += 1;
        self.last_event = format!(
            "{} formed from {} compatible tickets.",
            session_id.label(),
            selected.len()
        );
        Some(Session::formed(session_id, selected))
    }

    fn sort_queue(&mut self) {
        self.queue
            .sort_by_key(|ticket| (ticket.queued_at, ticket.account));
    }
}

fn find_compatible_group(queue: &[QueueTicket]) -> Option<Vec<QueueTicket>> {
    for (anchor_index, anchor) in queue.iter().enumerate() {
        let mut group = vec![*anchor];
        let mut min_rating = anchor.rating;
        let mut max_rating = anchor.rating;
        for candidate in queue.iter().skip(anchor_index + 1) {
            if candidate.region != anchor.region || candidate.build != anchor.build {
                continue;
            }
            let next_min = min_rating.min(candidate.rating);
            let next_max = max_rating.max(candidate.rating);
            if next_max - next_min > SKILL_WINDOW {
                continue;
            }
            group.push(*candidate);
            min_rating = next_min;
            max_rating = next_max;
            if group.len() == ROSTER_SIZE {
                return Some(group);
            }
        }
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Participant {
    pub account: AccountId,
    pub player: PlayerId,
    pub team: TeamId,
    pub rating: u16,
    pub connection: ConnectionState,
    pub ready: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionPhase {
    Lobby,
    Countdown {
        remaining: u8,
    },
    InMatch {
        frame: u32,
    },
    ReconnectGrace {
        missing: Vec<AccountId>,
        remaining: u8,
        resume_frame: u32,
    },
    PostMatch {
        remaining: u8,
    },
    Closed {
        reason: &'static str,
    },
}

impl SessionPhase {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Lobby => "LOBBY",
            Self::Countdown { .. } => "COUNTDOWN",
            Self::InMatch { .. } => "IN MATCH",
            Self::ReconnectGrace { .. } => "RECONNECT",
            Self::PostMatch { .. } => "POST MATCH",
            Self::Closed { .. } => "CLOSED",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchSeat {
    pub account: AccountId,
    pub player: PlayerId,
    pub team: TeamId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchManifest {
    pub session: SessionId,
    pub match_number: u32,
    pub lockstep_session: u32,
    pub seed: u64,
    pub protocol_version: u16,
    pub build: u32,
    pub host: AccountId,
    pub roster: Vec<LaunchSeat>,
}

impl LaunchManifest {
    pub fn valid(&self) -> bool {
        if self.roster.len() != ROSTER_SIZE
            || self.protocol_version != PROTOCOL_VERSION
            || self.build != CURRENT_BUILD
        {
            return false;
        }
        let accounts: BTreeSet<AccountId> = self.roster.iter().map(|seat| seat.account).collect();
        let players: BTreeSet<PlayerId> = self.roster.iter().map(|seat| seat.player).collect();
        if accounts.len() != ROSTER_SIZE || players.len() != ROSTER_SIZE {
            return false;
        }
        if !accounts.contains(&self.host) || self.lockstep_session == 0 {
            return false;
        }
        (0..TEAM_COUNT).all(|team| {
            self.roster
                .iter()
                .filter(|seat| seat.team == TeamId(team as u8))
                .count()
                == TEAM_SIZE
        })
    }
}

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
    fn formed(id: SessionId, tickets: Vec<QueueTicket>) -> Self {
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

    pub fn set_ready(&mut self, account: AccountId, ready: bool) -> bool {
        if !matches!(
            self.phase,
            SessionPhase::Lobby | SessionPhase::Countdown { .. }
        ) {
            return false;
        }
        let Some(participant) = self
            .participants
            .iter_mut()
            .find(|participant| participant.account == account)
        else {
            return false;
        };
        if participant.connection != ConnectionState::Connected {
            return false;
        }
        participant.ready = ready;
        self.push_event(format!(
            "{} is {}.",
            account.label(),
            if ready { "READY" } else { "not ready" }
        ));

        if !ready && matches!(self.phase, SessionPhase::Countdown { .. }) {
            self.phase = SessionPhase::Lobby;
            self.push_event("Countdown cancelled: roster changed readiness.".to_string());
        } else if self.all_ready() && matches!(self.phase, SessionPhase::Lobby) {
            self.phase = SessionPhase::Countdown {
                remaining: COUNTDOWN_TICKS,
            };
            self.push_event(format!("All ready. Launch in {COUNTDOWN_TICKS}."));
        }
        true
    }

    pub fn disconnect(&mut self, account: AccountId) -> bool {
        if matches!(self.phase, SessionPhase::Closed { .. }) {
            return false;
        }
        let Some(participant) = self
            .participants
            .iter_mut()
            .find(|participant| participant.account == account)
        else {
            return false;
        };
        if participant.connection == ConnectionState::Disconnected {
            return false;
        }
        participant.connection = ConnectionState::Disconnected;
        participant.ready = false;
        self.push_event(format!("{} disconnected.", account.label()));
        self.migrate_host_if_needed();

        match self.phase.clone() {
            SessionPhase::Countdown { .. } => {
                self.phase = SessionPhase::Lobby;
                self.push_event("Countdown cancelled: player disconnected.".to_string());
            }
            SessionPhase::InMatch { frame } => {
                self.phase = SessionPhase::ReconnectGrace {
                    missing: self.disconnected_accounts(),
                    remaining: RECONNECT_GRACE_TICKS,
                    resume_frame: frame,
                };
                self.push_event(format!(
                    "Match paused at frame {frame}; reconnect grace {RECONNECT_GRACE_TICKS}."
                ));
            }
            SessionPhase::ReconnectGrace {
                remaining,
                resume_frame,
                ..
            } => {
                self.phase = SessionPhase::ReconnectGrace {
                    missing: self.disconnected_accounts(),
                    remaining,
                    resume_frame,
                };
            }
            SessionPhase::Lobby | SessionPhase::PostMatch { .. } | SessionPhase::Closed { .. } => {}
        }
        true
    }

    pub fn reconnect(&mut self, account: AccountId) -> bool {
        if matches!(self.phase, SessionPhase::Closed { .. }) {
            return false;
        }
        let Some(participant) = self
            .participants
            .iter_mut()
            .find(|participant| participant.account == account)
        else {
            return false;
        };
        if participant.connection == ConnectionState::Connected {
            return false;
        }
        participant.connection = ConnectionState::Connected;
        let player = participant.player;
        let team = participant.team;
        self.migrate_host_if_needed();
        self.reconnects += 1;
        self.push_event(format!(
            "{} reconnected to P{} / {}.",
            account.label(),
            player.0 + 1,
            team.label()
        ));

        if let SessionPhase::ReconnectGrace { resume_frame, .. } = self.phase.clone() {
            let missing = self.disconnected_accounts();
            if missing.is_empty() {
                self.phase = SessionPhase::InMatch {
                    frame: resume_frame,
                };
                self.push_event(format!("Roster restored; resumed frame {resume_frame}."));
            } else {
                let remaining = match self.phase {
                    SessionPhase::ReconnectGrace { remaining, .. } => remaining,
                    _ => unreachable!(),
                };
                self.phase = SessionPhase::ReconnectGrace {
                    missing,
                    remaining,
                    resume_frame,
                };
            }
        }
        true
    }

    pub fn tick(&mut self) {
        match self.phase.clone() {
            SessionPhase::Lobby | SessionPhase::Closed { .. } => {}
            SessionPhase::Countdown { remaining: _ } if !self.all_ready() => {
                self.phase = SessionPhase::Lobby;
                self.push_event("Countdown cancelled: roster invalid.".to_string());
            }
            SessionPhase::Countdown { remaining } if remaining > 1 => {
                self.phase = SessionPhase::Countdown {
                    remaining: remaining - 1,
                };
            }
            SessionPhase::Countdown { .. } => self.launch_match(),
            SessionPhase::InMatch { frame } => {
                self.phase = SessionPhase::InMatch { frame: frame + 1 };
            }
            SessionPhase::ReconnectGrace {
                missing: _,
                remaining,
                resume_frame,
            } if remaining > 1 => {
                self.phase = SessionPhase::ReconnectGrace {
                    missing: self.disconnected_accounts(),
                    remaining: remaining - 1,
                    resume_frame,
                };
            }
            SessionPhase::ReconnectGrace { .. } => {
                self.launch = None;
                for participant in &mut self.participants {
                    participant.ready = false;
                }
                self.phase = SessionPhase::Closed {
                    reason: "reconnect timeout",
                };
                self.push_event("Session closed: reconnect grace expired.".to_string());
            }
            SessionPhase::PostMatch { remaining } if remaining > 1 => {
                self.phase = SessionPhase::PostMatch {
                    remaining: remaining - 1,
                };
            }
            SessionPhase::PostMatch { .. } => {
                for participant in &mut self.participants {
                    participant.ready = false;
                }
                self.launch = None;
                self.phase = SessionPhase::Lobby;
                self.push_event("Returned to lobby for rematch.".to_string());
            }
        }
    }

    pub fn finish_match(&mut self) -> bool {
        if !matches!(self.phase, SessionPhase::InMatch { .. }) {
            return false;
        }
        self.phase = SessionPhase::PostMatch {
            remaining: POST_MATCH_TICKS,
        };
        self.push_event(format!("Match {} complete.", self.match_number));
        true
    }

    fn launch_match(&mut self) {
        if !self.all_ready() {
            self.phase = SessionPhase::Lobby;
            return;
        }
        self.match_number += 1;
        let host = self.host.expect("a connected full roster has a host");
        let manifest = LaunchManifest {
            session: self.id,
            match_number: self.match_number,
            lockstep_session: self.id.0.rotate_left(7) ^ self.match_number ^ 0x1700_2026,
            seed: (u64::from(self.id.0) << 32) | u64::from(self.match_number),
            protocol_version: PROTOCOL_VERSION,
            build: self.build,
            host,
            roster: self
                .participants
                .iter()
                .map(|participant| LaunchSeat {
                    account: participant.account,
                    player: participant.player,
                    team: participant.team,
                })
                .collect(),
        };
        assert!(
            manifest.valid(),
            "session must emit a valid launch manifest"
        );
        self.launch = Some(manifest);
        self.phase = SessionPhase::InMatch { frame: 0 };
        self.push_event(format!(
            "Match {} launched; lockstep handoff ready.",
            self.match_number
        ));
    }

    fn disconnected_accounts(&self) -> Vec<AccountId> {
        self.participants
            .iter()
            .filter(|participant| participant.connection == ConnectionState::Disconnected)
            .map(|participant| participant.account)
            .collect()
    }

    fn migrate_host_if_needed(&mut self) {
        let host_connected = self.host.is_some_and(|host| {
            self.participant(host)
                .is_some_and(|participant| participant.connection == ConnectionState::Connected)
        });
        if host_connected {
            return;
        }
        let old = self.host;
        self.host = self
            .participants
            .iter()
            .filter(|participant| participant.connection == ConnectionState::Connected)
            .map(|participant| participant.account)
            .min();
        if self.host != old {
            self.host_migrations += 1;
            if let (Some(launch), Some(host)) = (&mut self.launch, self.host) {
                launch.host = host;
            }
            self.push_event(format!(
                "Host migrated {} -> {}.",
                old.map(AccountId::label)
                    .unwrap_or_else(|| "none".to_string()),
                self.host
                    .map(AccountId::label)
                    .unwrap_or_else(|| "none".to_string())
            ));
        }
    }

    fn push_event(&mut self, event: String) {
        self.lifecycle_events += 1;
        self.recent_events.push(event);
        if self.recent_events.len() > 8 {
            self.recent_events.remove(0);
        }
    }
}

fn assign_roster(tickets: &[QueueTicket]) -> Vec<Participant> {
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

fn authored_tickets() -> Vec<QueueTicket> {
    vec![
        QueueTicket {
            account: AccountId(0),
            rating: 1600,
            region: Region::West,
            build: CURRENT_BUILD,
            queued_at: 0,
        },
        QueueTicket {
            account: AccountId(1),
            rating: 1510,
            region: Region::West,
            build: CURRENT_BUILD,
            queued_at: 1,
        },
        QueueTicket {
            account: AccountId(2),
            rating: 1430,
            region: Region::West,
            build: CURRENT_BUILD,
            queued_at: 2,
        },
        QueueTicket {
            account: AccountId(3),
            rating: 1320,
            region: Region::West,
            build: CURRENT_BUILD,
            queued_at: 3,
        },
        QueueTicket {
            account: AccountId(4),
            rating: 1490,
            region: Region::East,
            build: CURRENT_BUILD,
            queued_at: 4,
        },
        QueueTicket {
            account: AccountId(5),
            rating: 1500,
            region: Region::West,
            build: CURRENT_BUILD + 1,
            queued_at: 5,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compatible_tickets() -> Vec<QueueTicket> {
        authored_tickets().into_iter().take(4).collect()
    }

    fn formed_session() -> Session {
        let mut matchmaker = Matchmaker::new();
        for ticket in compatible_tickets() {
            matchmaker.enqueue(ticket).unwrap();
        }
        matchmaker.form_next().unwrap()
    }

    fn ready_all(session: &mut Session) {
        let accounts: Vec<AccountId> = session
            .participants
            .iter()
            .map(|participant| participant.account)
            .collect();
        for account in accounts {
            assert!(session.set_ready(account, true));
        }
    }

    fn launch(session: &mut Session) {
        ready_all(session);
        for _ in 0..COUNTDOWN_TICKS {
            session.tick();
        }
        assert!(matches!(session.phase, SessionPhase::InMatch { frame: 0 }));
    }

    #[test]
    fn matchmaking_is_deterministic_independent_of_enqueue_order() {
        let tickets = authored_tickets();
        let mut reversed = tickets.clone();
        reversed.reverse();
        let mut a = Matchmaker::new();
        let mut b = Matchmaker::new();
        for ticket in tickets {
            a.enqueue(ticket).unwrap();
        }
        for ticket in reversed {
            b.enqueue(ticket).unwrap();
        }
        assert_eq!(a.form_next(), b.form_next());
        assert_eq!(a.queue, b.queue);
    }

    #[test]
    fn incompatible_region_and_build_tickets_remain_queued() {
        let mut world = SessionLabWorld::authored();
        let session = world.matchmaker.form_next().unwrap();
        assert_eq!(session.participants.len(), ROSTER_SIZE);
        assert_eq!(world.matchmaker.queue.len(), 2);
        assert!(
            world
                .matchmaker
                .queue
                .iter()
                .any(|ticket| ticket.region == Region::East)
        );
        assert!(
            world
                .matchmaker
                .queue
                .iter()
                .any(|ticket| ticket.build != CURRENT_BUILD)
        );
    }

    #[test]
    fn duplicate_accounts_cannot_queue_twice() {
        let ticket = compatible_tickets()[0];
        let mut matchmaker = Matchmaker::new();
        assert_eq!(matchmaker.enqueue(ticket), Ok(()));
        assert_eq!(
            matchmaker.enqueue(ticket),
            Err(QueueError::DuplicateAccount)
        );
    }

    #[test]
    fn roster_has_stable_player_ids_and_balanced_teams() {
        let session = formed_session();
        for (index, participant) in session.participants.iter().enumerate() {
            assert_eq!(participant.player, PlayerId(index as u16));
        }
        for team in 0..TEAM_COUNT {
            assert_eq!(
                session
                    .participants
                    .iter()
                    .filter(|participant| participant.team == TeamId(team as u8))
                    .count(),
                TEAM_SIZE
            );
        }
        let difference = session
            .team_rating(TeamId(0))
            .abs_diff(session.team_rating(TeamId(1)));
        assert!(difference <= 100, "teams should be rating-balanced");
    }

    #[test]
    fn countdown_requires_a_full_connected_ready_roster() {
        let mut session = formed_session();
        let accounts: Vec<AccountId> = session
            .participants
            .iter()
            .map(|participant| participant.account)
            .collect();
        for account in accounts.iter().take(3) {
            session.set_ready(*account, true);
        }
        assert_eq!(session.phase, SessionPhase::Lobby);
        session.disconnect(accounts[3]);
        assert!(!session.set_ready(accounts[3], true));
        assert_eq!(session.phase, SessionPhase::Lobby);
    }

    #[test]
    fn unready_or_disconnect_cancels_countdown() {
        let mut session = formed_session();
        ready_all(&mut session);
        assert!(matches!(session.phase, SessionPhase::Countdown { .. }));
        let account = session.participants[0].account;
        session.set_ready(account, false);
        assert_eq!(session.phase, SessionPhase::Lobby);

        session.set_ready(account, true);
        assert!(matches!(session.phase, SessionPhase::Countdown { .. }));
        session.disconnect(session.participants[1].account);
        assert_eq!(session.phase, SessionPhase::Lobby);
    }

    #[test]
    fn countdown_emits_a_valid_network_launch_manifest() {
        let mut session = formed_session();
        launch(&mut session);
        let manifest = session.launch.as_ref().expect("launch manifest");
        assert!(manifest.valid());
        assert_eq!(manifest.session, session.id);
        assert_eq!(manifest.host, session.host.unwrap());
        assert_eq!(manifest.match_number, 1);
        assert_ne!(manifest.lockstep_session, 0);
    }

    #[test]
    fn host_migration_is_deterministic() {
        let mut session = formed_session();
        let original = session.host.unwrap();
        assert_eq!(original, AccountId(0));
        session.disconnect(original);
        assert_eq!(session.host, Some(AccountId(1)));
        assert_eq!(session.host_migrations, 1);
    }

    #[test]
    fn in_match_host_migration_updates_the_launch_handoff() {
        let mut session = formed_session();
        launch(&mut session);
        let original = session.host.unwrap();
        session.disconnect(original);
        assert_eq!(session.host, Some(AccountId(1)));
        assert_eq!(session.launch.as_ref().unwrap().host, AccountId(1));
        assert!(session.launch.as_ref().unwrap().valid());
    }

    #[test]
    fn reconnect_re_elects_a_host_after_everyone_was_offline() {
        let mut session = formed_session();
        launch(&mut session);
        let accounts: Vec<AccountId> = session
            .participants
            .iter()
            .map(|participant| participant.account)
            .collect();
        for account in &accounts {
            session.disconnect(*account);
        }
        assert_eq!(session.host, None);

        session.reconnect(AccountId(3));
        assert_eq!(session.host, Some(AccountId(3)));
        assert_eq!(session.launch.as_ref().unwrap().host, AccountId(3));
        assert!(session.launch.as_ref().unwrap().valid());
    }

    #[test]
    fn reconnect_preserves_identity_team_and_match_frame() {
        let mut session = formed_session();
        launch(&mut session);
        for _ in 0..12 {
            session.tick();
        }
        let account = session.host.unwrap();
        let before = *session.participant(account).unwrap();
        session.disconnect(account);
        assert!(matches!(
            session.phase,
            SessionPhase::ReconnectGrace {
                resume_frame: 12,
                ..
            }
        ));
        session.tick();
        session.reconnect(account);
        let after = session.participant(account).unwrap();
        assert_eq!(after.player, before.player);
        assert_eq!(after.team, before.team);
        assert_eq!(session.phase, SessionPhase::InMatch { frame: 12 });
        assert_eq!(session.reconnects, 1);
        assert_eq!(session.launch.as_ref().unwrap().host, session.host.unwrap());
    }

    #[test]
    fn reconnect_timeout_closes_the_session_cleanly() {
        let mut session = formed_session();
        launch(&mut session);
        let account = session.participants[2].account;
        session.disconnect(account);
        for _ in 0..RECONNECT_GRACE_TICKS {
            session.tick();
        }
        assert_eq!(
            session.phase,
            SessionPhase::Closed {
                reason: "reconnect timeout"
            }
        );
        assert!(session.launch.is_none());
        assert!(
            session
                .participants
                .iter()
                .all(|participant| !participant.ready)
        );
        assert!(!session.reconnect(account));
        assert_eq!(
            session.phase,
            SessionPhase::Closed {
                reason: "reconnect timeout"
            }
        );
    }

    #[test]
    fn completed_match_returns_to_lobby_and_can_rematch() {
        let mut session = formed_session();
        launch(&mut session);
        assert!(session.finish_match());
        for _ in 0..POST_MATCH_TICKS {
            session.tick();
        }
        assert_eq!(session.phase, SessionPhase::Lobby);
        assert!(session.launch.is_none());
        assert!(
            session
                .participants
                .iter()
                .all(|participant| !participant.ready)
        );

        launch(&mut session);
        assert_eq!(session.match_number, 2);
        assert!(session.launch.as_ref().unwrap().valid());
    }

    #[test]
    fn the_full_lifecycle_is_deterministic() {
        let mut a = SessionLabWorld::authored();
        let mut b = SessionLabWorld::authored();
        for _ in 0..14 {
            a.advance_demo();
            b.advance_demo();
        }
        assert_eq!(a, b);
    }

    #[test]
    fn reset_restores_the_authored_queue() {
        let mut world = SessionLabWorld::authored();
        for _ in 0..8 {
            world.advance_demo();
        }
        world.reset();
        assert_eq!(world.reset_count, 1);
        assert!(world.session.is_none());
        assert_eq!(world.matchmaker.queue.len(), 6);
        assert_eq!(world.demo_step, 0);
    }
}
