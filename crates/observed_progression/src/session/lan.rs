//! Pure dedicated/listen-server lobby state with stable, host-configured seats.

use observed_core::{PlayerId, TeamId, lan::LanSeatId};

use super::{AccountId, SessionId};

/// The roster the host asks for. It was three constants pinned to 2v2, which is
/// what made every layer above assume four seats and two teams.
///
/// One team is co-op: the whole roster shares a map, and the escape condition is
/// the team's rather than a race between teams.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanRoster {
    pub teams: u8,
    pub members_per_team: u8,
}

/// The most seats any match may have, on the wire and in the simulation alike.
/// See `observed_net::lan::MAX_SEATS`, which must agree with it.
pub const LAN_MAX_SEATS: usize = 16;

#[cfg(test)]
mod seat_cap {
    /// Pinned to the literal because no crate can see all three copies at once:
    /// `observed_net` depends on `observed_match` and neither depends on this
    /// one. `observed_net`'s `the_wire_cap_and_the_roster_guard_agree` covers
    /// the other two against each other.
    #[test]
    fn the_lobby_cap_matches_the_wire_and_the_simulation() {
        assert_eq!(super::LAN_MAX_SEATS, 16);
    }
}

/// What a host gets without saying otherwise: the 2v2 race the arc inherited.
pub const LAN_DEFAULT_ROSTER: LanRoster = LanRoster {
    teams: 2,
    members_per_team: 2,
};

impl LanRoster {
    /// A single team of `members`, which is the co-op configuration.
    #[must_use]
    pub const fn co_op(members: u8) -> Self {
        Self {
            teams: 1,
            members_per_team: members,
        }
    }

    #[must_use]
    pub fn seats(self) -> usize {
        usize::from(self.teams) * usize::from(self.members_per_team)
    }

    /// Whether this roster can actually be seated and sent.
    #[must_use]
    pub fn is_valid(self) -> bool {
        self.teams >= 1 && self.members_per_team >= 1 && (1..=LAN_MAX_SEATS).contains(&self.seats())
    }

    /// The nearest roster that is valid, so a bad host argument clamps rather
    /// than refusing to start a server.
    #[must_use]
    pub fn clamped(self) -> Self {
        let teams = self.teams.max(1);
        let members = self
            .members_per_team
            .max(1)
            .min((LAN_MAX_SEATS / usize::from(teams)).max(1) as u8);
        Self {
            teams,
            members_per_team: members,
        }
    }
}
pub const LAN_COUNTDOWN_TICKS: u16 = 180;
pub const LAN_POST_MATCH_TICKS: u16 = 600;
pub const LAN_RECONNECT_GRACE_TICKS: u64 = 1_800;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LanSeatOccupant {
    Bot,
    Human {
        account: AccountId,
        connected: bool,
        reserved_until: Option<u64>,
    },
}

impl LanSeatOccupant {
    pub const fn connected_human(self) -> Option<AccountId> {
        match self {
            Self::Human {
                account,
                connected: true,
                ..
            } => Some(account),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanArchitectSeat {
    pub team: TeamId,
    pub occupant: LanSeatOccupant,
    pub ready: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanSeat {
    pub player: PlayerId,
    pub team: TeamId,
    pub occupant: LanSeatOccupant,
    pub ready: bool,
}

impl LanSeat {
    #[must_use]
    pub fn connected_human(self) -> Option<AccountId> {
        match self.occupant {
            LanSeatOccupant::Human {
                account,
                connected: true,
                ..
            } => Some(account),
            LanSeatOccupant::Bot | LanSeatOccupant::Human { .. } => None,
        }
    }

    #[must_use]
    pub fn is_bot_controlled(self) -> bool {
        !matches!(
            self.occupant,
            LanSeatOccupant::Human {
                connected: true,
                ..
            }
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LanPhase {
    Lobby,
    Countdown { remaining: u16 },
    InMatch,
    PostMatch { remaining: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LanJoinError {
    AlreadyJoined,
    NoBotSeat,
    NoArchitectSeat,
    PostMatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanLaunchSeat {
    pub player: PlayerId,
    pub team: TeamId,
    pub human: Option<AccountId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanLaunchManifest {
    pub session: SessionId,
    pub match_number: u32,
    pub seed: u64,
    pub seats: Vec<LanLaunchSeat>,
    pub architects: Vec<TeamId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanSession {
    pub id: SessionId,
    pub roster: LanRoster,
    pub seats: Vec<LanSeat>,
    pub architects: Vec<LanArchitectSeat>,
    pub phase: LanPhase,
    pub min_humans: u8,
    pub match_number: u32,
}

impl LanSession {
    /// A lobby with the inherited 2v2 roster.
    #[must_use]
    pub fn new(id: SessionId, min_humans: u8) -> Self {
        Self::with_roster(id, min_humans, LAN_DEFAULT_ROSTER)
    }

    #[must_use]
    pub fn with_roster(id: SessionId, min_humans: u8, roster: LanRoster) -> Self {
        let roster = roster.clamped();
        let seats = (0..roster.seats() as u16)
            .map(|raw| LanSeat {
                player: PlayerId(raw),
                team: TeamId((raw / u16::from(roster.members_per_team)) as u8),
                occupant: LanSeatOccupant::Bot,
                ready: false,
            })
            .collect();
        Self {
            id,
            roster,
            seats,
            architects: Vec::new(),
            phase: LanPhase::Lobby,
            min_humans: min_humans.clamp(1, roster.seats() as u8),
            match_number: 0,
        }
    }

    pub fn with_architects(id: SessionId, min_humans: u8, roster: LanRoster) -> Self {
        let mut session = Self::with_roster(id, min_humans, roster);
        session.architects = (0..session.roster.teams)
            .map(|team| LanArchitectSeat {
                team: TeamId(team),
                occupant: LanSeatOccupant::Bot,
                ready: false,
            })
            .collect();
        session.min_humans = min_humans.clamp(1, session.capacity() as u8);
        session
    }
    pub fn capacity(&self) -> usize {
        self.seats.len() + self.architects.len()
    }
    pub fn participants(&self) -> impl Iterator<Item = (LanSeatId, LanSeatOccupant, bool)> + '_ {
        self.seats
            .iter()
            .map(|s| (LanSeatId::Observer(s.player), s.occupant, s.ready))
            .chain(
                self.architects
                    .iter()
                    .map(|s| (LanSeatId::Architect(s.team), s.occupant, s.ready)),
            )
    }
    pub fn assignment(&self, account: AccountId) -> Option<LanSeatId> {
        self.participants().find_map(|(seat, occupant, _)| {
            matches!(occupant, LanSeatOccupant::Human { account: found, .. } if found == account).then_some(seat)
        })
    }
    pub fn team_for(&self, seat: LanSeatId) -> Option<TeamId> {
        match seat {
            LanSeatId::Observer(player) => self.seats.get(player.index()).map(|s| s.team),
            LanSeatId::Architect(team) => self.architects.get(usize::from(team.0)).map(|s| s.team),
        }
    }
    pub fn occupant(&self, seat: LanSeatId) -> Option<LanSeatOccupant> {
        self.participants()
            .find(|(id, _, _)| *id == seat)
            .map(|(_, occupant, _)| occupant)
    }
    fn assign(&mut self, seat: LanSeatId, occupant: LanSeatOccupant, ready: bool) {
        match seat {
            LanSeatId::Observer(player) => {
                let s = &mut self.seats[player.index()];
                s.occupant = occupant;
                s.ready = ready;
            }
            LanSeatId::Architect(team) => {
                let s = &mut self.architects[usize::from(team.0)];
                s.occupant = occupant;
                s.ready = ready;
            }
        }
    }
    pub fn join_architect(
        &mut self,
        account: AccountId,
        requested_team: Option<TeamId>,
    ) -> Result<LanSeatId, LanJoinError> {
        if matches!(self.phase, LanPhase::PostMatch { .. }) {
            return Err(LanJoinError::PostMatch);
        }
        if self.assignment(account).is_some() {
            return Err(LanJoinError::AlreadyJoined);
        }
        let team = match requested_team {
            Some(team) => (self.occupant(LanSeatId::Architect(team)) == Some(LanSeatOccupant::Bot))
                .then_some(team),
            None => self
                .architects
                .iter()
                .find(|s| s.occupant == LanSeatOccupant::Bot)
                .map(|s| s.team),
        }
        .ok_or(LanJoinError::NoArchitectSeat)?;
        let seat = LanSeatId::Architect(team);
        self.assign(
            seat,
            LanSeatOccupant::Human {
                account,
                connected: true,
                reserved_until: None,
            },
            matches!(self.phase, LanPhase::InMatch),
        );
        self.cancel_countdown();
        Ok(seat)
    }
    pub fn request_assignment_team(
        &mut self,
        account: AccountId,
        team: TeamId,
    ) -> Option<LanSeatId> {
        let from = self.assignment(account)?;
        if from.observer().is_some() {
            return self.request_team(account, team).map(LanSeatId::Observer);
        }
        if !matches!(self.phase, LanPhase::Lobby | LanPhase::Countdown { .. }) {
            return None;
        }
        let to = LanSeatId::Architect(team);
        if from == to {
            return Some(from);
        }
        if self.occupant(to) != Some(LanSeatOccupant::Bot) {
            return None;
        }
        let occupant = self.occupant(from)?;
        self.assign(from, LanSeatOccupant::Bot, false);
        self.assign(to, occupant, false);
        self.cancel_countdown();
        Some(to)
    }

    #[must_use]
    pub fn human_count(&self) -> usize {
        self.participants()
            .filter(|(_, occupant, _)| occupant.connected_human().is_some())
            .count()
    }

    #[must_use]
    pub fn joinable(&self) -> bool {
        !matches!(self.phase, LanPhase::PostMatch { .. })
            && self
                .seats
                .iter()
                .any(|seat| seat.occupant == LanSeatOccupant::Bot)
            || (!matches!(self.phase, LanPhase::PostMatch { .. })
                && self
                    .architects
                    .iter()
                    .any(|seat| seat.occupant == LanSeatOccupant::Bot))
    }

    pub fn join(
        &mut self,
        account: AccountId,
        requested_team: Option<TeamId>,
    ) -> Result<PlayerId, LanJoinError> {
        if matches!(self.phase, LanPhase::PostMatch { .. }) {
            return Err(LanJoinError::PostMatch);
        }
        if self.assignment(account).is_some() {
            return Err(LanJoinError::AlreadyJoined);
        }
        let team = requested_team
            .filter(|team| team.0 < self.roster.teams && self.open_bot_on_team(*team).is_some())
            .unwrap_or_else(|| self.balanced_open_team());
        let index = self
            .open_bot_on_team(team)
            .or_else(|| {
                self.seats
                    .iter()
                    .position(|seat| seat.occupant == LanSeatOccupant::Bot)
            })
            .ok_or(LanJoinError::NoBotSeat)?;
        let player = {
            let seat = &mut self.seats[index];
            seat.occupant = LanSeatOccupant::Human {
                account,
                connected: true,
                reserved_until: None,
            };
            seat.ready = matches!(self.phase, LanPhase::InMatch);
            seat.player
        };
        self.cancel_countdown();
        Ok(player)
    }

    pub fn request_team(&mut self, account: AccountId, team: TeamId) -> Option<PlayerId> {
        if !matches!(self.phase, LanPhase::Lobby | LanPhase::Countdown { .. })
            || team.0 >= self.roster.teams
        {
            return None;
        }
        let from = self.account_seat(account)?;
        if self.seats[from].team == team {
            return Some(self.seats[from].player);
        }
        let to = self.open_bot_on_team(team)?;
        let occupant = self.seats[from].occupant;
        let ready = self.seats[from].ready;
        self.seats[from].occupant = LanSeatOccupant::Bot;
        self.seats[from].ready = false;
        self.seats[to].occupant = occupant;
        self.seats[to].ready = ready;
        self.cancel_countdown();
        Some(self.seats[to].player)
    }

    /// Move between the body's connection and the separate team desk atomically.
    /// A full target role leaves the existing assignment untouched.
    pub fn claim_architect(&mut self, account: AccountId, claim: bool) -> bool {
        if !matches!(self.phase, LanPhase::Lobby | LanPhase::Countdown { .. }) {
            return false;
        }
        let Some(from) = self.assignment(account) else {
            return false;
        };
        if from.architect().is_some() == claim {
            return true;
        }
        let team = self.team_for(from).expect("assigned team");
        let to = if claim {
            LanSeatId::Architect(team)
        } else {
            let Some(index) = self.open_bot_on_team(team) else {
                return false;
            };
            LanSeatId::Observer(self.seats[index].player)
        };
        if self
            .occupant(from)
            .and_then(LanSeatOccupant::connected_human)
            .is_none()
            || self.occupant(to) != Some(LanSeatOccupant::Bot)
        {
            return false;
        }
        let occupant = self.occupant(from).unwrap();
        self.assign(from, LanSeatOccupant::Bot, false);
        self.assign(to, occupant, false);
        self.cancel_countdown();
        true
    }

    pub fn set_ready(&mut self, account: AccountId, ready: bool) -> bool {
        if !matches!(self.phase, LanPhase::Lobby | LanPhase::Countdown { .. }) {
            return false;
        }
        let Some(seat) = self.assignment(account) else {
            return false;
        };
        let occupant = self.occupant(seat).unwrap();
        if occupant.connected_human().is_none() {
            return false;
        }
        self.assign(seat, occupant, ready);
        if !ready {
            self.cancel_countdown();
        }
        true
    }

    pub fn disconnect(&mut self, account: AccountId, now_tick: u64) -> Option<LanSeatId> {
        let seat = self.assignment(account)?;
        self.assign(
            seat,
            LanSeatOccupant::Human {
                account,
                connected: false,
                reserved_until: Some(now_tick.saturating_add(LAN_RECONNECT_GRACE_TICKS)),
            },
            false,
        );
        self.cancel_countdown();
        Some(seat)
    }

    pub fn reconnect(&mut self, account: AccountId, now_tick: u64) -> Option<LanSeatId> {
        let seat = self.assignment(account)?;
        let LanSeatOccupant::Human {
            reserved_until: Some(until),
            connected: false,
            ..
        } = self.occupant(seat)?
        else {
            return None;
        };
        if now_tick > until {
            return None;
        }
        self.assign(
            seat,
            LanSeatOccupant::Human {
                account,
                connected: true,
                reserved_until: None,
            },
            matches!(self.phase, LanPhase::InMatch),
        );
        Some(seat)
    }

    pub fn expire_reservations(&mut self, now_tick: u64) {
        let expired: Vec<_> = self.participants().filter_map(|(seat, occupant, _)| {
            matches!(occupant, LanSeatOccupant::Human { connected: false, reserved_until: Some(until), .. } if now_tick > until).then_some(seat)
        }).collect();
        for seat in expired {
            self.assign(seat, LanSeatOccupant::Bot, false);
        }
    }

    pub fn release(&mut self, account: AccountId) {
        if let Some(seat) = self.assignment(account) {
            self.assign(seat, LanSeatOccupant::Bot, false);
        }
    }
    pub fn clear_ready(&mut self) {
        for seat in &mut self.seats {
            seat.ready = false;
        }
        for seat in &mut self.architects {
            seat.ready = false;
        }
    }
    /// Advance the server-owned lobby/post-match clock by one 60 Hz tick.
    /// Returns a launch manifest exactly once when the countdown completes.
    pub fn tick(&mut self, seed: u64) -> Option<LanLaunchManifest> {
        match self.phase {
            LanPhase::Lobby if self.can_count_down() => {
                self.phase = LanPhase::Countdown {
                    remaining: LAN_COUNTDOWN_TICKS,
                };
            }
            LanPhase::Countdown { .. } if !self.can_count_down() => {
                self.phase = LanPhase::Lobby;
            }
            LanPhase::Countdown { remaining } if remaining > 1 => {
                self.phase = LanPhase::Countdown {
                    remaining: remaining - 1,
                };
            }
            LanPhase::Countdown { .. } => {
                self.phase = LanPhase::InMatch;
                self.match_number = self.match_number.wrapping_add(1);
                return Some(self.launch_manifest(seed));
            }
            LanPhase::PostMatch { remaining } if remaining > 1 => {
                self.phase = LanPhase::PostMatch {
                    remaining: remaining - 1,
                };
            }
            LanPhase::PostMatch { .. } => {
                self.phase = LanPhase::Lobby;
                self.clear_ready();
            }
            LanPhase::Lobby | LanPhase::InMatch => {}
        }
        None
    }

    pub fn finish_match(&mut self) {
        if matches!(self.phase, LanPhase::InMatch) {
            self.phase = LanPhase::PostMatch {
                remaining: LAN_POST_MATCH_TICKS,
            };
        }
    }

    fn launch_manifest(&self, seed: u64) -> LanLaunchManifest {
        LanLaunchManifest {
            session: self.id,
            match_number: self.match_number,
            seed,
            seats: self
                .seats
                .iter()
                .map(|seat| LanLaunchSeat {
                    player: seat.player,
                    team: seat.team,
                    human: match seat.occupant {
                        LanSeatOccupant::Human { account, .. } => Some(account),
                        LanSeatOccupant::Bot => None,
                    },
                })
                .collect(),
            architects: self
                .architects
                .iter()
                .filter(|s| s.occupant.connected_human().is_some())
                .map(|s| s.team)
                .collect(),
        }
    }

    fn can_count_down(&self) -> bool {
        self.human_count() >= usize::from(self.min_humans)
            && self
                .participants()
                .filter(|(_, occupant, _)| occupant.connected_human().is_some())
                .all(|(_, _, ready)| ready)
    }

    fn account_seat(&self, account: AccountId) -> Option<usize> {
        self.seats.iter().position(|seat| {
            matches!(seat.occupant, LanSeatOccupant::Human { account: found, .. } if found == account)
        })
    }

    fn open_bot_on_team(&self, team: TeamId) -> Option<usize> {
        self.seats
            .iter()
            .position(|seat| seat.team == team && seat.occupant == LanSeatOccupant::Bot)
    }

    fn balanced_open_team(&self) -> TeamId {
        (0..self.roster.teams)
            .map(TeamId)
            .filter(|team| self.open_bot_on_team(*team).is_some())
            .min_by_key(|team| {
                (
                    self.seats
                        .iter()
                        .filter(|seat| {
                            seat.team == *team
                                && matches!(seat.occupant, LanSeatOccupant::Human { .. })
                        })
                        .count(),
                    team.0,
                )
            })
            .unwrap_or(TeamId(0))
    }

    fn cancel_countdown(&mut self) {
        if matches!(self.phase, LanPhase::Countdown { .. }) {
            self.phase = LanPhase::Lobby;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_desk_and_three_human_observers_have_four_connections_and_three_bodies() {
        let mut session = LanSession::with_architects(SessionId(10), 4, LanRoster::co_op(3));
        for raw in 0..3 {
            assert_eq!(session.join(AccountId(raw), None), Ok(PlayerId(raw)));
        }
        let desk = LanSeatId::Architect(TeamId(0));
        assert_eq!(session.join_architect(AccountId(3), None), Ok(desk));
        assert_eq!(
            (
                session.capacity(),
                session.human_count(),
                session.seats.len()
            ),
            (4, 4, 3)
        );
        assert_eq!(
            session.join(AccountId(4), None),
            Err(LanJoinError::NoBotSeat)
        );
        assert_eq!(
            session.join_architect(AccountId(4), None),
            Err(LanJoinError::NoArchitectSeat)
        );
        assert_eq!(
            session.join(AccountId(3), None),
            Err(LanJoinError::AlreadyJoined)
        );
        assert!(
            !session.claim_architect(AccountId(3), false),
            "full bodies cannot displace an Observer"
        );
        assert_eq!(session.assignment(AccountId(3)), Some(desk));
        session.disconnect(AccountId(2), 1);
        assert!(
            !session.claim_architect(AccountId(3), false),
            "a reserved body remains owned"
        );
        session.expire_reservations(2 + LAN_RECONNECT_GRACE_TICKS);
        assert!(session.claim_architect(AccountId(3), false));
        assert_eq!(
            session.assignment(AccountId(3)),
            Some(LanSeatId::Observer(PlayerId(2)))
        );
        assert_eq!(session.occupant(desk), Some(LanSeatOccupant::Bot));
        assert!(session.claim_architect(AccountId(0), true));
        assert_eq!(
            session.occupant(LanSeatId::Observer(PlayerId(0))),
            Some(LanSeatOccupant::Bot)
        );
        assert!(!session.claim_architect(AccountId(1), true));
        assert_eq!(
            session.assignment(AccountId(1)),
            Some(LanSeatId::Observer(PlayerId(1)))
        );
        assert_eq!(session.seats.len(), 3);
    }

    #[test]
    fn desk_readiness_reservations_and_reconnect_participate_in_the_lobby_clock() {
        let mut session = LanSession::with_architects(SessionId(11), 2, LanRoster::co_op(1));
        session.join(AccountId(0), None).unwrap();
        let desk = session.join_architect(AccountId(1), None).unwrap();
        session.set_ready(AccountId(0), true);
        session.tick(42);
        assert_eq!(session.phase, LanPhase::Lobby);
        session.set_ready(AccountId(1), true);
        session.tick(42);
        assert!(matches!(session.phase, LanPhase::Countdown { .. }));
        assert_eq!(session.disconnect(AccountId(1), 10), Some(desk));
        assert_eq!(session.phase, LanPhase::Lobby);
        assert_eq!(
            session.join_architect(AccountId(2), None),
            Err(LanJoinError::NoArchitectSeat)
        );
        assert_eq!(session.reconnect(AccountId(1), 11), Some(desk));
        assert!(!session.architects[0].ready);
        session.set_ready(AccountId(1), true);
        session.tick(42);
        let mut launch = None;
        for _ in 0..LAN_COUNTDOWN_TICKS {
            launch = session.tick(42).or(launch);
        }
        let launch = launch.unwrap();
        assert_eq!(launch.seats.len(), 1);
        assert_eq!(launch.architects, vec![TeamId(0)]);
        assert!(
            !session.claim_architect(AccountId(1), false),
            "role is frozen during a match"
        );
        let mut race = LanSession::new(SessionId(12), 1);
        assert_eq!(
            race.join_architect(AccountId(0), None),
            Err(LanJoinError::NoArchitectSeat)
        );
        assert_eq!(race.capacity(), LAN_DEFAULT_ROSTER.seats());
    }

    #[test]
    fn sixteen_bodies_can_have_sixteen_independent_desks_without_aliasing() {
        let mut session = LanSession::with_architects(
            SessionId(13),
            32,
            LanRoster {
                teams: 16,
                members_per_team: 1,
            },
        );
        for raw in 0..16 {
            session
                .join(AccountId(raw), Some(TeamId(raw as u8)))
                .unwrap();
            assert_eq!(
                session.join_architect(AccountId(raw + 16), Some(TeamId(raw as u8))),
                Ok(LanSeatId::Architect(TeamId(raw as u8)))
            );
        }
        assert_eq!(
            (
                session.seats.len(),
                session.capacity(),
                session.human_count()
            ),
            (16, 32, 32)
        );
        let ids: std::collections::BTreeSet<_> =
            session.participants().map(|(id, _, _)| id).collect();
        assert_eq!(ids.len(), 32);
        assert_eq!(session.min_humans, 32);
    }

    #[test]
    fn team_requests_are_bounded_and_unassigned_humans_balance() {
        let mut session = LanSession::new(SessionId(7), 1);
        assert_eq!(session.join(AccountId(0), Some(TeamId(1))), Ok(PlayerId(2)));
        assert_eq!(session.join(AccountId(1), Some(TeamId(1))), Ok(PlayerId(3)));
        assert_eq!(session.join(AccountId(2), Some(TeamId(1))), Ok(PlayerId(0)));
        assert_eq!(session.request_team(AccountId(2), TeamId(1)), None);
    }

    #[test]
    fn all_humans_ready_launches_and_bots_fill_the_manifest() {
        let mut session = LanSession::new(SessionId(8), 1);
        session.join(AccountId(0), None).expect("join");
        assert!(session.set_ready(AccountId(0), true));
        assert!(session.tick(44).is_none());
        let mut launch = None;
        for _ in 0..LAN_COUNTDOWN_TICKS {
            launch = session.tick(44).or(launch);
        }
        let launch = launch.expect("launch");
        assert_eq!(launch.seats.len(), LAN_DEFAULT_ROSTER.seats());
        assert_eq!(
            launch
                .seats
                .iter()
                .filter(|seat| seat.human.is_some())
                .count(),
            1
        );
        assert_eq!(session.phase, LanPhase::InMatch);
    }

    #[test]
    fn disconnect_uses_bot_control_then_reclaims_or_expires_the_same_seat() {
        let mut session = LanSession::new(SessionId(9), 1);
        let player = session.join(AccountId(0), None).expect("join");
        assert_eq!(
            session.disconnect(AccountId(0), 10),
            Some(LanSeatId::Observer(player))
        );
        assert!(session.seats[player.index()].is_bot_controlled());
        assert_eq!(
            session.reconnect(AccountId(0), 11),
            Some(LanSeatId::Observer(player))
        );
        session.disconnect(AccountId(0), 20);
        session.expire_reservations(20 + LAN_RECONNECT_GRACE_TICKS + 1);
        assert_eq!(session.seats[player.index()].occupant, LanSeatOccupant::Bot);
    }
}
