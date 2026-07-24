//! Pure dedicated/listen-server lobby state with stable 2v2 seats.

use observed_core::{PlayerId, TeamId};

use super::{AccountId, SessionId};

pub const LAN_TEAM_COUNT: u8 = 2;
pub const LAN_MEMBERS_PER_TEAM: u8 = 2;
pub const LAN_ROSTER_SIZE: usize = 4;
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanSession {
    pub id: SessionId,
    pub seats: Vec<LanSeat>,
    pub phase: LanPhase,
    pub min_humans: u8,
    pub match_number: u32,
}

impl LanSession {
    #[must_use]
    pub fn new(id: SessionId, min_humans: u8) -> Self {
        let seats = (0..LAN_ROSTER_SIZE as u16)
            .map(|raw| LanSeat {
                player: PlayerId(raw),
                team: TeamId((raw / u16::from(LAN_MEMBERS_PER_TEAM)) as u8),
                occupant: LanSeatOccupant::Bot,
                ready: false,
            })
            .collect();
        Self {
            id,
            seats,
            phase: LanPhase::Lobby,
            min_humans: min_humans.clamp(1, LAN_ROSTER_SIZE as u8),
            match_number: 0,
        }
    }

    #[must_use]
    pub fn human_count(&self) -> usize {
        self.seats
            .iter()
            .filter(|seat| seat.connected_human().is_some())
            .count()
    }

    #[must_use]
    pub fn joinable(&self) -> bool {
        !matches!(self.phase, LanPhase::PostMatch { .. })
            && self
                .seats
                .iter()
                .any(|seat| seat.occupant == LanSeatOccupant::Bot)
    }

    pub fn join(
        &mut self,
        account: AccountId,
        requested_team: Option<TeamId>,
    ) -> Result<PlayerId, LanJoinError> {
        if matches!(self.phase, LanPhase::PostMatch { .. }) {
            return Err(LanJoinError::PostMatch);
        }
        if self.account_seat(account).is_some() {
            return Err(LanJoinError::AlreadyJoined);
        }
        let team = requested_team
            .filter(|team| team.0 < LAN_TEAM_COUNT && self.open_bot_on_team(*team).is_some())
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
            || team.0 >= LAN_TEAM_COUNT
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

    pub fn set_ready(&mut self, account: AccountId, ready: bool) -> bool {
        if !matches!(self.phase, LanPhase::Lobby | LanPhase::Countdown { .. }) {
            return false;
        }
        let Some(index) = self.account_seat(account) else {
            return false;
        };
        if self.seats[index].connected_human().is_none() {
            return false;
        }
        self.seats[index].ready = ready;
        if !ready {
            self.cancel_countdown();
        }
        true
    }

    pub fn disconnect(&mut self, account: AccountId, now_tick: u64) -> Option<PlayerId> {
        let index = self.account_seat(account)?;
        self.seats[index].occupant = LanSeatOccupant::Human {
            account,
            connected: false,
            reserved_until: Some(now_tick.saturating_add(LAN_RECONNECT_GRACE_TICKS)),
        };
        self.seats[index].ready = false;
        self.cancel_countdown();
        Some(self.seats[index].player)
    }

    pub fn reconnect(&mut self, account: AccountId, now_tick: u64) -> Option<PlayerId> {
        let index = self.account_seat(account)?;
        let LanSeatOccupant::Human {
            reserved_until: Some(until),
            connected: false,
            ..
        } = self.seats[index].occupant
        else {
            return None;
        };
        if now_tick > until {
            return None;
        }
        self.seats[index].occupant = LanSeatOccupant::Human {
            account,
            connected: true,
            reserved_until: None,
        };
        self.seats[index].ready = matches!(self.phase, LanPhase::InMatch);
        Some(self.seats[index].player)
    }

    pub fn expire_reservations(&mut self, now_tick: u64) {
        for seat in &mut self.seats {
            if matches!(
                seat.occupant,
                LanSeatOccupant::Human {
                    connected: false,
                    reserved_until: Some(until),
                    ..
                } if now_tick > until
            ) {
                seat.occupant = LanSeatOccupant::Bot;
                seat.ready = false;
            }
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
                for seat in &mut self.seats {
                    seat.ready = false;
                }
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
        }
    }

    fn can_count_down(&self) -> bool {
        self.human_count() >= usize::from(self.min_humans)
            && self
                .seats
                .iter()
                .filter(|seat| seat.connected_human().is_some())
                .all(|seat| seat.ready)
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
        (0..LAN_TEAM_COUNT)
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
        assert_eq!(launch.seats.len(), LAN_ROSTER_SIZE);
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
        assert_eq!(session.disconnect(AccountId(0), 10), Some(player));
        assert!(session.seats[player.index()].is_bot_controlled());
        assert_eq!(session.reconnect(AccountId(0), 11), Some(player));
        session.disconnect(AccountId(0), 20);
        session.expire_reservations(20 + LAN_RECONNECT_GRACE_TICKS + 1);
        assert_eq!(session.seats[player.index()].occupant, LanSeatOccupant::Bot);
    }
}
