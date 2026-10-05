//! Bounded projection of body and independent desk slots.
use observed_core::{PlayerId, TeamId};
use observed_net::lan::{WireArchitectSeat, WirePhase, WireSeat, WireSeatOccupant};
use std::collections::BTreeSet;
pub(super) const ROSTER_ROWS: usize = 16;

pub(crate) fn lobby_roster_text(lan: &crate::lan::LanRuntime) -> String {
    let Some(client) = lan.client.as_ref() else {
        return format!("{}\nWaiting for roster...", lan.status);
    };
    let Some(lobby) = client.lobby.as_ref() else {
        return format!("{}\nWaiting for roster...", lan.status);
    };
    format_roster(
        lobby.phase,
        lobby.countdown_ticks,
        &lobby.seats,
        (
            client.player,
            client.is_architect().then_some(client.team).flatten(),
        ),
        lobby.ascent,
        lobby.fill_empty_seats,
        &lobby.architect_seats,
    )
}

pub(super) fn format_roster(
    phase: WirePhase,
    countdown: u16,
    seats: &[WireSeat],
    local: (Option<PlayerId>, Option<TeamId>),
    ascent: bool,
    fill_empty_seats: bool,
    architects: &[WireArchitectSeat],
) -> String {
    let (local_player, local_desk) = local;
    let countdown = if countdown > 0 {
        format!(" | launch in {:.1}s", f32::from(countdown) / 60.0)
    } else {
        String::new()
    };
    let teams = seats.iter().map(|seat| seat.team).collect::<BTreeSet<_>>();
    let mut lines = vec![
        format!(
            "{} | {} {}",
            if ascent {
                "Architect Ascent"
            } else {
                "Facility race"
            },
            seats.len(),
            if ascent {
                "Observer bodies + separate desks"
            } else {
                "seats"
            }
        ),
        format!(
            "{phase:?}{countdown} | {}",
            if fill_empty_seats {
                "empty seats: bots"
            } else {
                "all connection seats need humans"
            }
        ),
    ];
    for team in teams {
        for desk in architects.iter().filter(|seat| seat.team == team) {
            let you = if local_desk == Some(team) {
                " (YOU)"
            } else {
                ""
            };
            let ready = if desk.ready { " | READY" } else { "" };
            lines.push(format!(
                "{} | Architect{you}: {}{ready}",
                team.label().to_uppercase(),
                occupant_label(desk.occupant)
            ));
        }
        let team_seats = seats
            .iter()
            .filter(|seat| seat.team == team)
            .collect::<Vec<_>>();
        for seat in team_seats {
            let occupant = occupant_label(seat.occupant);
            let you = if local_player == Some(seat.player) {
                " (YOU)"
            } else {
                ""
            };
            let ready = if seat.ready { " | READY" } else { "" };
            lines.push(format!(
                "{} | {}{you}: {occupant}{ready}",
                team.label().to_uppercase(),
                seat.player.label()
            ));
        }
    }
    lines.join("\n")
}

fn occupant_label(occupant: WireSeatOccupant) -> &'static str {
    match occupant {
        WireSeatOccupant::Bot => "BOT",
        WireSeatOccupant::Human => "HUMAN",
        WireSeatOccupant::ReservedHuman => "RESERVED",
        WireSeatOccupant::SynchronizingHuman => "PREPARING",
        WireSeatOccupant::Empty => "EMPTY",
    }
}
pub(super) fn roster_page_teams(lan: &crate::lan::LanRuntime, page: usize) -> BTreeSet<TeamId> {
    let Some(lobby) = lan.client.as_ref().and_then(|client| client.lobby.as_ref()) else {
        return BTreeSet::new();
    };
    let teams: BTreeSet<_> = lobby.seats.iter().map(|seat| seat.team).collect();
    let mut rows = Vec::new();
    for team in teams {
        rows.extend(
            lobby
                .architect_seats
                .iter()
                .filter(|seat| seat.team == team)
                .map(|seat| seat.team),
        );
        rows.extend(
            lobby
                .seats
                .iter()
                .filter(|seat| seat.team == team)
                .map(|seat| seat.team),
        );
    }
    rows.into_iter()
        .skip(page * ROSTER_ROWS)
        .take(ROSTER_ROWS)
        .collect()
}

pub(super) fn roster_page_count(text: &str) -> usize {
    text.lines()
        .count()
        .saturating_sub(2)
        .div_ceil(ROSTER_ROWS)
        .max(1)
}
pub(crate) fn roster_page_text(text: &str, page: usize) -> String {
    let count = roster_page_count(text);
    let page = page.min(count - 1);
    let mut lines: Vec<_> = text.lines().take(2).map(String::from).collect();
    lines.push(format!("Roster and teams page {} / {count}", page + 1));
    lines.extend(
        text.lines()
            .skip(2 + page * ROSTER_ROWS)
            .take(ROSTER_ROWS)
            .map(String::from),
    );
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maximum_ascent_roster_pages_every_body_and_desk_without_hiding_their_roles() {
        let bodies: Vec<_> = (0..16)
            .map(|raw| WireSeat {
                player: PlayerId(raw),
                team: TeamId(raw as u8),
                occupant: WireSeatOccupant::Human,
                ready: true,
            })
            .collect();
        let desks: Vec<_> = (0..16)
            .map(|raw| WireArchitectSeat {
                team: TeamId(raw),
                occupant: WireSeatOccupant::Human,
                ready: true,
            })
            .collect();
        let text = format_roster(
            WirePhase::Lobby,
            0,
            &bodies,
            (None, Some(TeamId(15))),
            true,
            false,
            &desks,
        );
        assert_eq!(roster_page_count(&text), 2);
        let pages = [roster_page_text(&text, 0), roster_page_text(&text, 1)];
        assert!(
            pages
                .iter()
                .all(|page| page.lines().count() <= ROSTER_ROWS + 3)
        );
        for team in 1..=16 {
            assert_eq!(
                pages
                    .iter()
                    .flat_map(|page| page.lines())
                    .filter(|line| line.starts_with(&format!("TEAM {team} |")))
                    .count(),
                2
            );
        }
        assert!(pages[1].contains("TEAM 16 | Architect (YOU): HUMAN | READY"));
        assert_eq!(roster_page_text(&text, usize::MAX), pages[1]);
    }
}
