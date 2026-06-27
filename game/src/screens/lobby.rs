//! The lobby screen: forms a real balanced session via the proven matchmaker
//! (`session_lab`) and renders it. The formed session is retained in [`LobbyRuntime`]
//! for the duration of the lobby/match.

use bevy::prelude::*;
use observed_progression::session::SessionLabWorld;

use super::*;
use crate::GameState;

pub(crate) fn setup_lobby(mut commands: Commands, mut cursor: ResMut<MenuCursor>) {
    cursor.0 = 0;
    // Form a session via the proven matchmaker, then ready it up.
    let mut world = SessionLabWorld::authored();
    for _ in 0..3 {
        world.advance_demo();
    }

    let mut lines: Vec<String> = Vec::new();
    if let Some(session) = &world.session {
        lines.push(format!(
            "{} | {} | {} players | {}",
            session.id.label(),
            session.region.label(),
            session.participants.len(),
            session.phase.label(),
        ));
        for team in 0..2u8 {
            let count = session
                .participants
                .iter()
                .filter(|p| p.team.0 == team)
                .count();
            lines.push(format!(
                "  Team {}  {} players  | rating {}",
                team + 1,
                count,
                session.team_rating(observed_core::TeamId(team)),
            ));
        }
    } else {
        lines.push("Matchmaking…".to_string());
    }

    commands
        .spawn(screen_root(GameState::Lobby))
        .with_children(|root| {
            root.spawn(text("LOBBY", 40.0, TITLE));
            root.spawn(panel()).with_children(|p| {
                for line in &lines {
                    p.spawn(text(line.clone(), 18.0, DIM));
                }
            });
            root.spawn(panel()).with_children(|p| {
                p.spawn(menu_button(0, MenuAction::Launch, "Launch match"));
                p.spawn(menu_button(
                    1,
                    MenuAction::Goto(GameState::MainMenu),
                    "Back",
                ));
            });
            root.spawn(text(
                "You are Team 1. Launch to drop into the facility. | Esc back",
                15.0,
                DIM,
            ));
        });
    commands.insert_resource(LobbyRuntime { world });
}
