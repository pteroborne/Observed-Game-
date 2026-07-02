//! Session unit tests.

#[cfg(test)]
mod tests {
    use super::super::{
        AccountId, COUNTDOWN_TICKS, CURRENT_BUILD, Matchmaker, POST_MATCH_TICKS, QueueError,
        QueueTicket, RECONNECT_GRACE_TICKS, ROSTER_SIZE, Session, SessionLabWorld, SessionPhase,
        TEAM_COUNT, TEAM_SIZE, lobby::authored_tickets,
    };
    use observed_core::{PlayerId, TeamId};

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
                .any(|ticket| ticket.region == super::super::Region::East)
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
