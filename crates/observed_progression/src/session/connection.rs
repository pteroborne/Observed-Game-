//! Session connection states, tickers, and host transitions.

use super::{
    AccountId, COUNTDOWN_TICKS, ConnectionState, LaunchManifest, LaunchSeat, POST_MATCH_TICKS,
    PROTOCOL_VERSION, RECONNECT_GRACE_TICKS, Session, SessionPhase,
};

impl Session {
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
}
