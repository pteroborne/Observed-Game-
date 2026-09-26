//! The team's requests, as the Architect's desk reads them.
//!
//! The rules hold every request (`AscentSession::requests`): who asked, for what, where,
//! when, and whether the Architect has answered. The desk lists its own team's, oldest
//! first, marks each on the board and the climb, and answers the oldest unanswered one on
//! F (or the controller's Y, or the panel's button), which acknowledges it to the team
//! through the rules and puts the board on its floor.

use observed_match::ascent::session::{
    AscentSession, REQUEST_LIFETIME_TICKS, RequestKind, TeamRequest,
};
use observed_match::ascent::sim::TeamId;
use observed_style::architect::Role;

use super::ArchitectDesk;
use crate::hex_wfc::sim::HexWfcRuntime;

/// `team`'s live requests, oldest first.
#[must_use]
pub(super) fn team_requests(session: &AscentSession, team: TeamId) -> Vec<TeamRequest> {
    let mut requests: Vec<TeamRequest> = session
        .requests
        .values()
        .filter(|request| request.team == team)
        .copied()
        .collect();
    requests.sort_by_key(|request| (request.created_at, request.author));
    requests
}

/// The oldest of `team`'s requests the Architect has not answered.
#[must_use]
pub(super) fn oldest_unanswered(session: &AscentSession, team: TeamId) -> Option<TeamRequest> {
    team_requests(session, team)
        .into_iter()
        .find(|request| request.acknowledged_by.is_none())
}

/// Answer the oldest of the desk's team's unanswered requests, if there is one.
pub(super) fn answer_oldest(desk: &mut ArchitectDesk, runtime: &HexWfcRuntime) {
    if let Some(request) = runtime
        .ascent
        .as_ref()
        .and_then(|ascent| oldest_unanswered(ascent.session(), desk.team))
    {
        desk.answer(&request);
    }
}

/// What a request looks like on the board and the climb.
#[must_use]
pub(super) const fn role(kind: RequestKind) -> Role {
    match kind {
        RequestKind::Route => Role::Observer,
        RequestKind::Power => Role::Fixture,
        RequestKind::Rescue => Role::Prison,
        RequestKind::Recharge | RequestKind::HoldObservation | RequestKind::ReleaseObservation => {
            Role::Muted
        }
    }
}

/// A request as the panel lists it, at `tick`: who, what, which floor, and how long it
/// has left.
#[must_use]
pub(super) fn line(request: &TeamRequest, tick: u64) -> String {
    let left = REQUEST_LIFETIME_TICKS.saturating_sub(tick.saturating_sub(request.created_at));
    format!(
        "EYE {:02}  {}\n    floor {}  {} s{}",
        request.author.0 + 1,
        request.kind.label(),
        request.target.level + 1,
        left.div_ceil(60),
        if request.acknowledged_by.is_some() {
            "  answered"
        } else {
            ""
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_core::PlayerId;
    use observed_hex::HexCoord;

    fn request(author: u16, created_at: u64, answered: bool) -> TeamRequest {
        TeamRequest {
            author: PlayerId(author),
            team: TeamId(0),
            kind: RequestKind::Route,
            target: HexCoord {
                q: 1,
                r: 2,
                level: 1,
            },
            created_at,
            acknowledged_by: answered.then_some(PlayerId(200)),
        }
    }

    #[test]
    fn a_request_says_who_what_where_and_how_long() {
        let fresh = line(&request(0, 100, false), 100);
        assert!(fresh.starts_with("EYE 01  Build a route"), "{fresh}");
        assert!(fresh.contains("floor 2"), "{fresh}");
        assert!(fresh.contains("15 s"), "{fresh}");
        assert!(!fresh.contains("answered"));
        let old = line(&request(0, 100, true), 100 + 14 * 60 + 1);
        assert!(old.contains(" 1 s"), "{old}");
        assert!(old.contains("answered"), "{old}");
        assert!(old.is_ascii(), "the shipped font");
    }

    #[test]
    fn every_kind_has_a_colour_of_its_own_on_the_board() {
        assert_ne!(role(RequestKind::Route), role(RequestKind::Power));
        assert_ne!(role(RequestKind::Power), role(RequestKind::Rescue));
        assert_ne!(role(RequestKind::Route), role(RequestKind::Rescue));
    }
}
