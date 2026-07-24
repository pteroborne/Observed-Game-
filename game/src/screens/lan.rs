//! LAN browser input and state transitions.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use super::{MenuAction, MenuCursor, menu_button};
use crate::GameState;
use crate::view::theme::{ACCENT, DIM, TITLE, panel, screen_root, text};

#[derive(Component)]
pub(crate) struct LanAddressText;
#[derive(Component)]
pub(crate) struct LanServerListText;
#[derive(Component)]
pub(crate) struct LanStatusText;
#[derive(Component)]
pub(crate) struct LanLobbyRosterText;

type AddressTextFilter = (
    With<LanAddressText>,
    Without<LanServerListText>,
    Without<LanStatusText>,
);
type ServerListTextFilter = (
    With<LanServerListText>,
    Without<LanAddressText>,
    Without<LanStatusText>,
);
type StatusTextFilter = (
    With<LanStatusText>,
    Without<LanAddressText>,
    Without<LanServerListText>,
);

pub(crate) fn setup_browser(
    mut commands: Commands,
    mut cursor: ResMut<MenuCursor>,
    lan: Res<crate::lan::LanRuntime>,
) {
    cursor.0 = 0;
    commands.spawn(screen_root(GameState::LanBrowser)).with_children(|root| {
        root.spawn(text("LAN PLAY", 42.0, TITLE));
        root.spawn(panel()).with_children(|panel| {
            panel.spawn((LanAddressText, text(format!("Direct: {}", lan.direct_address), 18.0, ACCENT)));
            panel.spawn((LanServerListText, text("Searching for servers...", 16.0, DIM)));
            panel.spawn((LanStatusText, text(lan.status.clone(), 15.0, DIM)));
        });
        root.spawn(panel()).with_children(|panel| {
            panel.spawn(menu_button(0, MenuAction::HostLan, "Host LAN"));
            panel.spawn(menu_button(1, MenuAction::JoinLan, "Join discovered"));
            panel.spawn(menu_button(2, MenuAction::JoinLanDirect, "Join direct IP"));
            panel.spawn(menu_button(3, MenuAction::RefreshLan, "Refresh"));
            panel.spawn(menu_button(4, MenuAction::Goto(GameState::MainMenu), "Back"));
        });
        root.spawn(text(
            "Type digits, '.', or ':' to edit the direct address | broadcast discovery is automatic",
            14.0,
            DIM,
        ));
    });
}

pub(crate) fn poll_lan(
    mut lan: ResMut<crate::lan::LanRuntime>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    lan.poll();
    if *state.get() == GameState::LanBrowser
        && lan
            .client
            .as_ref()
            .is_some_and(|client| client.token.is_some())
    {
        next.set(GameState::Lobby);
    }
    if *state.get() == GameState::Lobby
        && let Some((_, match_number, _, _)) = lan.client.as_ref().and_then(|client| client.launch)
        && lan.consumed_match != Some(match_number)
    {
        lan.consumed_match = Some(match_number);
        next.set(GameState::HexWfc);
    }
}

pub(crate) fn edit_direct_address(
    mut inputs: MessageReader<KeyboardInput>,
    mut lan: ResMut<crate::lan::LanRuntime>,
) {
    for input in inputs.read().filter(|input| input.state.is_pressed()) {
        match &input.logical_key {
            Key::Backspace => {
                lan.direct_address.pop();
            }
            _ => {
                let Some(text) = input.text.as_deref() else {
                    continue;
                };
                if text
                    .chars()
                    .all(|character| character.is_ascii_digit() || matches!(character, '.' | ':'))
                    && lan.direct_address.len() + text.len() <= 64
                {
                    lan.direct_address.push_str(text);
                }
            }
        }
    }
}

pub(crate) fn refresh_browser_text(
    lan: Res<crate::lan::LanRuntime>,
    mut address: Query<&mut Text, AddressTextFilter>,
    mut list: Query<&mut Text, ServerListTextFilter>,
    mut status: Query<&mut Text, StatusTextFilter>,
) {
    if let Ok(mut text) = address.single_mut() {
        **text = format!("Direct: {}", lan.direct_address);
    }
    if let Ok(mut text) = status.single_mut() {
        **text = lan.status.clone();
    }
    if let Ok(mut text) = list.single_mut() {
        let servers = lan
            .browser
            .as_ref()
            .map_or_else(Vec::new, |browser| browser.servers());
        **text = if servers.is_empty() {
            "No broadcasts yet; direct IP remains available.".to_string()
        } else {
            servers
                .into_iter()
                .map(|server| {
                    format!(
                        "{} | {} | {:?} | {}/4 humans | {}",
                        server.name,
                        server.address,
                        server.phase,
                        server.humans,
                        if server.joinable { "JOINABLE" } else { "FULL" }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
    }
}

pub(crate) fn refresh_lobby_text(
    lan: Res<crate::lan::LanRuntime>,
    mut roster: Query<&mut Text, With<LanLobbyRosterText>>,
) {
    let Ok(mut text) = roster.single_mut() else {
        return;
    };
    let Some((_, phase, countdown, seats)) =
        lan.client.as_ref().and_then(|client| client.lobby.clone())
    else {
        **text = format!("{}\nWaiting for roster...", lan.status);
        return;
    };
    let mut lines = vec![format!(
        "{:?}{}",
        phase,
        if countdown > 0 {
            format!(" | launch in {:.1}s", f32::from(countdown) / 60.0)
        } else {
            String::new()
        }
    )];
    for team in 0..2 {
        lines.push(format!("TEAM {}", team + 1));
        for seat in seats.iter().filter(|seat| seat.team.0 == team) {
            let occupant = match seat.occupant {
                0 => "BOT",
                1 => "HUMAN",
                2 => "BOT TAKEOVER / RESERVED",
                3 => "SYNCHRONIZING",
                _ => "UNKNOWN",
            };
            let is_you = lan
                .client
                .as_ref()
                .and_then(|client| client.player)
                .is_some_and(|player| player == seat.player);
            let you = if is_you { " (YOU)" } else { "" };
            lines.push(format!(
                "  {}{you}: {occupant}{}",
                seat.player.label(),
                if seat.ready { " | READY" } else { "" }
            ));
        }
    }
    **text = lines.join("\n");
}
