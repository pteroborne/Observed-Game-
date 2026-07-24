//! Live authoritative LAN lobby. The server owns seats, teams, readiness, and launch.

use bevy::prelude::*;
use observed_core::TeamId;
use observed_progression::session::SessionLabWorld;

use super::lan::LanLobbyRosterText;
use super::{MenuAction, MenuCursor, menu_button};
use crate::GameState;
use crate::sim::state::LobbyRuntime;
use crate::view::theme::{DIM, TITLE, panel, screen_root, text};

#[derive(Component)]
pub(crate) struct LobbyButtonText(pub(crate) MenuAction);

pub(crate) fn setup_lobby(mut commands: Commands, mut cursor: ResMut<MenuCursor>) {
    cursor.0 = 0;
    commands
        .spawn(screen_root(GameState::Lobby))
        .with_children(|root| {
            root.spawn(text("LAN LOBBY", 40.0, TITLE));
            root.spawn(panel()).with_children(|panel| {
                panel.spawn((
                    LanLobbyRosterText,
                    text("Waiting for server roster...", 17.0, DIM),
                ));
            });
            root.spawn(panel()).with_children(|panel| {
                panel.spawn((
                    LobbyButtonText(MenuAction::ToggleLanReady),
                    menu_button(0, MenuAction::ToggleLanReady, "Ready: OFF"),
                ));
                panel.spawn(menu_button(
                    1,
                    MenuAction::RequestLanTeam(TeamId(0)),
                    "Request Team 1",
                ));
                panel.spawn(menu_button(
                    2,
                    MenuAction::RequestLanTeam(TeamId(1)),
                    "Request Team 2",
                ));
                panel.spawn(menu_button(3, MenuAction::LeaveLan, "Leave LAN"));
            });
            root.spawn(text(
                "Both teammates must escape. Teammates share survivor-map knowledge.",
                14.0,
                DIM,
            ));
        });

    // Retain the historical resource shape for callers/tests that inspect lobby
    // lifecycle; production LAN truth lives in `LanRuntime`.
    commands.insert_resource(LobbyRuntime {
        world: SessionLabWorld::authored(),
    });
}

pub(crate) fn lobby_update_labels(
    lan: Res<crate::lan::LanRuntime>,
    mut query: Query<(&LobbyButtonText, &mut Text)>,
) {
    for (button, mut text) in &mut query {
        if button.0 == MenuAction::ToggleLanReady {
            **text = format!("Ready: {}", if lan.ready { "ON" } else { "OFF" });
        }
    }
}
