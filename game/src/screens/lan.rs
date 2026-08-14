//! LAN browser presentation, input, and state transitions.
//!
//! Discovery and transport remain owned by [`crate::lan::LanRuntime`]. This screen
//! owns only its editable address and the exact server address the player selected.

use std::collections::BTreeMap;
use std::net::SocketAddr;

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input_focus::{FocusCause, InputFocus, tab_navigation::TabIndex};
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Activate;
use observed_net::lan::DiscoveredServer;

use super::widgets::{
    self, FocusScope, FocusScopeId, FocusTarget, WidgetId, WidgetLabel, WidgetSpec,
    activation_enabled,
};
use crate::GameState;
use crate::hex_wfc::launch::{HexLaunchSpec, HexSeedPolicy};
use crate::hex_wfc::loading::HexLaunchRequestSequence;
use crate::play_setup::{LaunchContext, PlaySetupDraft};
use crate::view::theme::{ACCENT, BORDER, DIM, PANEL, TITLE, screen_root, text};

const SCOPE: FocusScopeId = FocusScopeId("lan_browser");
const DIRECT_ADDRESS: WidgetId = WidgetId::named("lan.direct_address");
const HOST: WidgetId = WidgetId::named("lan.host");
const JOIN_SELECTED: WidgetId = WidgetId::named("lan.join_selected");
const JOIN_DIRECT: WidgetId = WidgetId::named("lan.join_direct");
const REFRESH: WidgetId = WidgetId::named("lan.refresh");
const BACK: WidgetId = WidgetId::named("lan.back");
const PREVIOUS_PAGE: WidgetId = WidgetId::named("lan.previous_page");
const NEXT_PAGE: WidgetId = WidgetId::named("lan.next_page");

const SERVER_PAGE_SIZE: usize = 4;
const SERVER_ORDER_START: u16 = 10;
const PREVIOUS_PAGE_ORDER: u16 = 20;
const NEXT_PAGE_ORDER: u16 = 21;
const HOST_ORDER: u16 = 30;
const JOIN_SELECTED_ORDER: u16 = 31;
const JOIN_DIRECT_ORDER: u16 = 32;
const REFRESH_ORDER: u16 = 33;
const BACK_ORDER: u16 = 34;

#[derive(Component, Debug)]
pub(crate) struct LanBrowserDraft {
    direct_address: String,
    selected_server: Option<SocketAddr>,
    server_page: usize,
}

#[derive(Component)]
pub(crate) struct LanServerListText;

#[derive(Component)]
pub(crate) struct LanStatusText;

#[derive(Component)]
pub(crate) struct LanLobbyRosterText;

#[derive(Component)]
pub(crate) struct DirectAddressWidget;

#[derive(Component)]
pub(crate) struct ServerListPanel;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ServerOption(SocketAddr);

#[derive(Component)]
pub(crate) struct JoinSelectedWidget;

#[derive(Component)]
pub(crate) struct JoinDirectWidget;

#[derive(Component)]
pub(crate) struct PreviousPageWidget;

#[derive(Component)]
pub(crate) struct NextPageWidget;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LanBrowserAction {
    EditDirectAddress,
    SelectServer(SocketAddr),
    Host,
    JoinSelected,
    JoinDirect,
    PreviousPage,
    NextPage,
    Refresh,
    Back,
}

pub(crate) fn setup_browser(mut commands: Commands, lan: Res<crate::lan::LanRuntime>) {
    let servers = discovered_servers(&lan);
    let selected_server = servers
        .iter()
        .find(|server| server.joinable)
        .map(|server| server.address);
    let draft = LanBrowserDraft {
        direct_address: lan.direct_address.clone(),
        selected_server,
        server_page: selected_server
            .and_then(|selected| servers.iter().position(|server| server.address == selected))
            .map_or(0, |index| index / SERVER_PAGE_SIZE),
    };
    commands
        .spawn((
            screen_root(GameState::LanBrowser),
            widgets::focus_scope(FocusScope::screen(SCOPE, DIRECT_ADDRESS, BACK)),
            draft,
        ))
        .with_children(|root| {
            root.spawn(text("LAN PLAY", 42.0, TITLE));
            root.spawn(text(
                "Choose a discovered host, or focus the direct address field and type an IP:port.",
                15.0,
                DIM,
            ));
            root.spawn(lan_status_panel()).with_children(|summary| {
                summary.spawn((LanStatusText, text(lan.status.clone(), 15.0, ACCENT)));
            });
            root.spawn(Node {
                width: px(1120),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexStart,
                column_gap: px(18),
                ..default()
            })
            .with_children(|body| {
                body.spawn(lan_panel(720.0)).with_children(|browser| {
                    widgets::spawn_button(
                        browser,
                        WidgetSpec::enabled(
                            DIRECT_ADDRESS,
                            SCOPE,
                            0,
                            direct_address_label(&lan.direct_address),
                        )
                        .with_size(680.0, 44.0),
                        (LanBrowserAction::EditDirectAddress, DirectAddressWidget),
                    );
                    browser
                        .spawn((
                            ServerListPanel,
                            Node {
                                width: px(680),
                                min_height: px(238),
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                row_gap: px(2),
                                ..default()
                            },
                        ))
                        .with_children(|list| {
                            list.spawn((
                                LanServerListText,
                                text("Searching for LAN servers...", 15.0, DIM),
                            ));
                        });
                    browser
                        .spawn(Node {
                            width: px(680),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::Center,
                            column_gap: px(12),
                            ..default()
                        })
                        .with_children(|pager| {
                            widgets::spawn_button(
                                pager,
                                WidgetSpec::disabled(
                                    PREVIOUS_PAGE,
                                    SCOPE,
                                    PREVIOUS_PAGE_ORDER,
                                    "Previous servers | first page",
                                )
                                .with_size(326.0, 44.0),
                                (LanBrowserAction::PreviousPage, PreviousPageWidget),
                            );
                            widgets::spawn_button(
                                pager,
                                WidgetSpec::disabled(
                                    NEXT_PAGE,
                                    SCOPE,
                                    NEXT_PAGE_ORDER,
                                    "Next servers | last page",
                                )
                                .with_size(326.0, 44.0),
                                (LanBrowserAction::NextPage, NextPageWidget),
                            );
                        });
                });
                body.spawn(lan_panel(382.0)).with_children(|actions| {
                    widgets::spawn_button(
                        actions,
                        WidgetSpec::enabled(HOST, SCOPE, HOST_ORDER, "Host this setup")
                            .with_size(342.0, 44.0),
                        LanBrowserAction::Host,
                    );
                    widgets::spawn_button(
                        actions,
                        WidgetSpec::disabled(
                            JOIN_SELECTED,
                            SCOPE,
                            JOIN_SELECTED_ORDER,
                            "Join selected | choose a server",
                        )
                        .with_size(342.0, 44.0),
                        (LanBrowserAction::JoinSelected, JoinSelectedWidget),
                    );
                    let direct_spec = if lan.direct_address.parse::<SocketAddr>().is_ok() {
                        WidgetSpec::enabled(
                            JOIN_DIRECT,
                            SCOPE,
                            JOIN_DIRECT_ORDER,
                            "Join direct address",
                        )
                    } else {
                        WidgetSpec::disabled(
                            JOIN_DIRECT,
                            SCOPE,
                            JOIN_DIRECT_ORDER,
                            "Join direct | invalid IP:port",
                        )
                    };
                    widgets::spawn_button(
                        actions,
                        direct_spec.with_size(342.0, 44.0),
                        (LanBrowserAction::JoinDirect, JoinDirectWidget),
                    );
                    widgets::spawn_button(
                        actions,
                        WidgetSpec::enabled(REFRESH, SCOPE, REFRESH_ORDER, "Refresh now")
                            .with_size(342.0, 44.0),
                        LanBrowserAction::Refresh,
                    );
                    widgets::spawn_button(
                        actions,
                        WidgetSpec::enabled(BACK, SCOPE, BACK_ORDER, "Back to Play")
                            .with_size(342.0, 44.0),
                        LanBrowserAction::Back,
                    );
                });
            });
            root.spawn(text(
                "Typing edits only while the direct address field is focused | Enter / A selects",
                14.0,
                DIM,
            ));
        });
}

/// Keep transport alive in every frontend/loading state. State transitions are
/// deliberately conditional, but polling is not: authoritative frames may arrive
/// while the canonical facility is being prepared.
pub(crate) fn poll_lan(
    mut commands: Commands,
    mut lan: ResMut<crate::lan::LanRuntime>,
    state: Res<State<GameState>>,
    mut launch_sequence: ResMut<HexLaunchRequestSequence>,
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
    let launch = lan.client.as_ref().and_then(|client| {
        client
            .launch
            .zip(client.player)
            .filter(|(launch, _)| lan.consumed_match != Some(launch.match_number))
    });
    if *state.get() == GameState::Lobby
        && let Some((launch, local_player)) = launch
    {
        let request = launch_sequence.issue(
            LaunchContext::Lan,
            local_player,
            false,
            true,
            HexLaunchSpec {
                requested_seed: launch.seed,
                config: launch.config,
                seed_policy: HexSeedPolicy::Exact {
                    expected_content_hash: launch.simulation_content_hash,
                },
            },
        );
        commands.insert_resource(request);
        lan.consumed_match = Some(launch.match_number);
        next.set(GameState::Loading);
    }
}

/// Raw text enters the address draft only while the address widget itself owns
/// focus. Number keys can therefore never mutate it while a server or action is
/// selected.
pub(crate) fn edit_direct_address(
    mut inputs: MessageReader<KeyboardInput>,
    focus: Res<InputFocus>,
    address_widgets: Query<Entity, With<DirectAddressWidget>>,
    mut drafts: Query<&mut LanBrowserDraft>,
    mut lan: ResMut<crate::lan::LanRuntime>,
) {
    let Ok(address_widget) = address_widgets.single() else {
        return;
    };
    if focus.get() != Some(address_widget) {
        // Drain the messages while this screen is active, but do not apply them.
        for _ in inputs.read() {}
        return;
    }
    let Ok(mut draft) = drafts.single_mut() else {
        return;
    };
    let mut changed = false;
    for input in inputs.read().filter(|input| input.state.is_pressed()) {
        match &input.logical_key {
            Key::Backspace => changed |= draft.direct_address.pop().is_some(),
            Key::Delete => {
                changed |= !draft.direct_address.is_empty();
                draft.direct_address.clear();
            }
            _ => {
                let Some(input_text) = input.text.as_deref() else {
                    continue;
                };
                if valid_address_fragment(input_text)
                    && draft.direct_address.len() + input_text.len() <= 64
                {
                    draft.direct_address.push_str(input_text);
                    changed = true;
                }
            }
        }
    }
    if changed {
        lan.direct_address.clone_from(&draft.direct_address);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn refresh_browser_text(
    mut commands: Commands,
    lan: Res<crate::lan::LanRuntime>,
    mut drafts: Query<&mut LanBrowserDraft>,
    list_panels: Query<Entity, With<ServerListPanel>>,
    server_widgets: Query<(Entity, &ServerOption)>,
    address_widgets: Query<Entity, With<DirectAddressWidget>>,
    join_selected: Query<Entity, With<JoinSelectedWidget>>,
    join_direct: Query<Entity, With<JoinDirectWidget>>,
    previous_page: Query<Entity, With<PreviousPageWidget>>,
    next_page: Query<Entity, With<NextPageWidget>>,
    mut list_text: Query<&mut Text, (With<LanServerListText>, Without<LanStatusText>)>,
    mut status_text: Query<&mut Text, (With<LanStatusText>, Without<LanServerListText>)>,
    mut focus: ResMut<InputFocus>,
) {
    let Ok(mut draft) = drafts.single_mut() else {
        return;
    };
    let servers = discovered_servers(&lan);
    let old_selection = draft.selected_server;
    if draft
        .selected_server
        .is_some_and(|selected| !servers.iter().any(|server| server.address == selected))
    {
        draft.selected_server = servers
            .iter()
            .find(|server| server.joinable)
            .map(|server| server.address);
    } else if draft.selected_server.is_none() {
        draft.selected_server = servers
            .iter()
            .find(|server| server.joinable)
            .map(|server| server.address);
    }
    let pages = server_page_count(servers.len());
    draft.server_page = draft.server_page.min(pages.saturating_sub(1));
    if draft.selected_server != old_selection
        && let Some(selected) = draft.selected_server
        && let Some(index) = servers.iter().position(|server| server.address == selected)
    {
        draft.server_page = index / SERVER_PAGE_SIZE;
    }
    let range = server_page_range(servers.len(), draft.server_page);
    let visible_servers = &servers[range];

    if let Ok(address_widget) = address_widgets.single() {
        commands
            .entity(address_widget)
            .insert(WidgetLabel(direct_address_label(&draft.direct_address)));
    }
    if let Ok(mut status) = status_text.single_mut() {
        **status = lan.status.clone();
    }
    if let Ok(mut empty) = list_text.single_mut() {
        **empty = if servers.is_empty() {
            "No broadcasts yet. Direct address remains available.".to_string()
        } else {
            format!(
                "{} {} discovered | page {} / {}",
                servers.len(),
                if servers.len() == 1 {
                    "server"
                } else {
                    "servers"
                },
                draft.server_page + 1,
                pages,
            )
        };
    }

    let existing = server_widgets
        .iter()
        .map(|(entity, option)| (option.0, entity))
        .collect::<BTreeMap<_, _>>();
    for (address, entity) in &existing {
        if !visible_servers
            .iter()
            .any(|server| server.address == *address)
        {
            if focus.get() == Some(*entity)
                && let Ok(address_widget) = address_widgets.single()
            {
                focus.set(address_widget, FocusCause::Navigated);
            }
            commands.entity(*entity).despawn();
        }
    }

    let Ok(list_panel) = list_panels.single() else {
        return;
    };
    for (index, server) in visible_servers.iter().enumerate() {
        let order = SERVER_ORDER_START + u16::try_from(index).expect("page size fits in u16");
        let label = server_label(server, draft.selected_server == Some(server.address));
        if let Some(entity) = existing.get(&server.address).copied() {
            commands.entity(entity).insert((
                WidgetLabel(label),
                FocusTarget {
                    scope: SCOPE,
                    order,
                },
            ));
        } else {
            commands.entity(list_panel).with_children(|panel| {
                widgets::spawn_button(
                    panel,
                    WidgetSpec::enabled(server_widget_id(server.address), SCOPE, order, label)
                        .with_size(720.0, 46.0),
                    (
                        LanBrowserAction::SelectServer(server.address),
                        ServerOption(server.address),
                    ),
                );
            });
        }
    }

    if let Ok(previous) = previous_page.single() {
        let enabled = draft.server_page > 0;
        let label = if enabled {
            format!("Previous servers | page {}", draft.server_page)
        } else {
            "Previous servers | first page".to_string()
        };
        set_widget_availability(&mut commands, previous, PREVIOUS_PAGE_ORDER, enabled, label);
    }
    if let Ok(next_page) = next_page.single() {
        let enabled = draft.server_page + 1 < pages;
        let label = if enabled {
            format!("Next servers | page {}", draft.server_page + 2)
        } else {
            "Next servers | last page".to_string()
        };
        set_widget_availability(&mut commands, next_page, NEXT_PAGE_ORDER, enabled, label);
    }

    if let Ok(join) = join_selected.single() {
        let selected = draft
            .selected_server
            .and_then(|selected| servers.iter().find(|server| server.address == selected));
        let enabled = selected.is_some_and(|server| server.joinable);
        let label = selected.map_or_else(
            || "Join selected | choose an available server".to_string(),
            |server| {
                if server.joinable {
                    format!("Join {}", server.name)
                } else {
                    format!("Join {} | unavailable", server.name)
                }
            },
        );
        set_widget_availability(&mut commands, join, JOIN_SELECTED_ORDER, enabled, label);
    }
    if let Ok(join) = join_direct.single() {
        let enabled = draft.direct_address.parse::<SocketAddr>().is_ok();
        let label = if enabled {
            "Join direct address"
        } else {
            "Join direct address | enter a valid IP:port"
        };
        set_widget_availability(
            &mut commands,
            join,
            JOIN_DIRECT_ORDER,
            enabled,
            label.to_string(),
        );
    }
}

pub(crate) fn activate_browser(
    activation: On<Activate>,
    actions: Query<&LanBrowserAction>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut drafts: Query<&mut LanBrowserDraft>,
    mut lan: ResMut<crate::lan::LanRuntime>,
    setup: Res<PlaySetupDraft>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !activation_enabled(&activation, &disabled) {
        return;
    }
    let Ok(action) = actions.get(activation.entity) else {
        return;
    };
    match *action {
        LanBrowserAction::EditDirectAddress => {}
        LanBrowserAction::SelectServer(address) => {
            if let Ok(mut draft) = drafts.single_mut() {
                draft.selected_server = Some(address);
            }
        }
        LanBrowserAction::Host => match setup.validate() {
            Ok(setup) => set_lan_result(&mut lan, |lan| lan.host_with_setup(setup)),
            Err(error) => lan.status = error.to_string(),
        },
        LanBrowserAction::JoinSelected => {
            let Some(address) = drafts.single().ok().and_then(|draft| draft.selected_server) else {
                return;
            };
            set_lan_result(&mut lan, |lan| lan.join_server(address));
        }
        LanBrowserAction::JoinDirect => {
            let Some(address) = drafts
                .single()
                .ok()
                .and_then(|draft| draft.direct_address.parse::<SocketAddr>().ok())
            else {
                lan.status = "Enter a valid IP:port".to_string();
                return;
            };
            set_lan_result(&mut lan, |lan| lan.join_server(address));
        }
        LanBrowserAction::PreviousPage => {
            if let Ok(mut draft) = drafts.single_mut() {
                draft.server_page = draft.server_page.saturating_sub(1);
            }
        }
        LanBrowserAction::NextPage => {
            let pages = server_page_count(discovered_servers(&lan).len());
            if let Ok(mut draft) = drafts.single_mut() {
                draft.server_page = (draft.server_page + 1).min(pages.saturating_sub(1));
            }
        }
        LanBrowserAction::Refresh => {
            if let Some(browser) = lan.browser.as_ref() {
                let _ = browser.probe();
            }
            lan.status = "Refreshing LAN discovery...".to_string();
        }
        LanBrowserAction::Back => {
            if lan.client.is_some() || lan.listen_server.is_some() {
                lan.leave();
            }
            next.set(GameState::Play);
        }
    }
}

/// Retained as a narrow compatibility system until the screen plugin moves the
/// roster refresh registration entirely beside the lobby's local observer.
pub(crate) fn refresh_lobby_text(
    lan: Res<crate::lan::LanRuntime>,
    mut roster: Query<&mut Text, With<LanLobbyRosterText>>,
) {
    let Ok(mut roster) = roster.single_mut() else {
        return;
    };
    **roster = super::lobby::lobby_roster_text(&lan);
}

fn discovered_servers(lan: &crate::lan::LanRuntime) -> Vec<DiscoveredServer> {
    let mut servers = lan
        .browser
        .as_ref()
        .map_or_else(Vec::new, |browser| browser.servers());
    servers.sort_unstable_by_key(|server| server.address);
    servers
}

fn server_page_count(server_count: usize) -> usize {
    server_count.div_ceil(SERVER_PAGE_SIZE).max(1)
}

fn server_page_range(server_count: usize, page: usize) -> std::ops::Range<usize> {
    let start = (page * SERVER_PAGE_SIZE).min(server_count);
    let end = (start + SERVER_PAGE_SIZE).min(server_count);
    start..end
}

fn lan_status_panel() -> impl Bundle {
    (
        Node {
            width: px(1120),
            min_height: px(44),
            padding: UiRect::axes(px(18), px(10)),
            border: UiRect::all(px(1)),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(BORDER),
    )
}

fn lan_panel(width: f32) -> impl Bundle {
    (
        Node {
            width: px(width),
            padding: UiRect::all(px(18)),
            border: UiRect::all(px(1)),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(4),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(BORDER),
    )
}

fn direct_address_label(address: &str) -> String {
    format!("Direct address: {address}_")
}

fn valid_address_fragment(fragment: &str) -> bool {
    fragment.chars().all(|character| {
        character.is_ascii_hexdigit() || matches!(character, '.' | ':' | '[' | ']')
    })
}

fn server_label(server: &DiscoveredServer, selected: bool) -> String {
    let marker = if selected { ">" } else { " " };
    let availability = if server.joinable {
        "accepting players"
    } else {
        "not accepting players"
    };
    format!(
        "{marker} {} | {} | {:?} | {} humans | {availability}",
        server.name, server.address, server.phase, server.humans
    )
}

fn server_widget_id(address: SocketAddr) -> WidgetId {
    // FNV-1a keeps identity stable across discovery order and process launches.
    // The full address remains on `ServerOption`; this compact key is presentation-only.
    let mut key = 0xcbf2_9ce4_8422_2325_u64;
    for byte in address.to_string().bytes() {
        key ^= u64::from(byte);
        key = key.wrapping_mul(0x0000_0100_0000_01b3);
    }
    WidgetId::keyed("lan.server", key)
}

fn set_widget_availability(
    commands: &mut Commands,
    entity: Entity,
    order: u16,
    enabled: bool,
    label: String,
) {
    let mut widget = commands.entity(entity);
    widget.insert(WidgetLabel(label));
    if enabled {
        widget
            .remove::<InteractionDisabled>()
            .insert(TabIndex(i32::from(order)));
    } else {
        widget.insert(InteractionDisabled).remove::<TabIndex>();
    }
}

fn set_lan_result(
    lan: &mut crate::lan::LanRuntime,
    operation: impl FnOnce(&mut crate::lan::LanRuntime) -> Result<(), String>,
) {
    if let Err(error) = operation(lan) {
        lan.status = error;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use observed_net::lan::WirePhase;

    use super::*;

    fn server(address: &str, humans: u8, joinable: bool) -> DiscoveredServer {
        DiscoveredServer {
            address: address.parse().expect("test address"),
            name: "Workshop".to_string(),
            phase: WirePhase::Lobby,
            humans,
            joinable,
            last_seen: Instant::now(),
        }
    }

    #[test]
    fn discovery_copy_reports_truth_without_a_fake_fixed_capacity() {
        let label = server_label(&server("127.0.0.1:47624", 7, true), false);
        assert!(label.contains("7 humans"));
        assert!(label.contains("accepting players"));
        assert!(!label.contains("/4"));
        assert!(!label.contains("/16"));
    }

    #[test]
    fn server_widget_identity_follows_the_address_not_list_order() {
        let first: SocketAddr = "127.0.0.1:47624".parse().expect("first address");
        let second: SocketAddr = "127.0.0.2:47624".parse().expect("second address");
        assert_eq!(server_widget_id(first), server_widget_id(first));
        assert_ne!(server_widget_id(first), server_widget_id(second));
    }

    #[test]
    fn address_editor_accepts_ipv4_and_ipv6_syntax_but_not_arbitrary_text() {
        assert!(valid_address_fragment("127.0.0.1:47624"));
        assert!(valid_address_fragment("[::1]:47624"));
        assert!(!valid_address_fragment("server.example.com"));
        assert!(!valid_address_fragment("launch!"));
    }

    #[test]
    fn discovery_pages_are_bounded_and_cover_every_server_once() {
        assert_eq!(server_page_count(0), 1);
        assert_eq!(server_page_count(4), 1);
        assert_eq!(server_page_count(5), 2);
        assert_eq!(server_page_range(11, 0), 0..4);
        assert_eq!(server_page_range(11, 1), 4..8);
        assert_eq!(server_page_range(11, 2), 8..11);
        assert_eq!(server_page_range(11, 99), 11..11);
    }

    #[test]
    fn selection_keeps_the_exact_socket_address_across_pages() {
        let first: SocketAddr = "127.0.0.1:47624".parse().expect("first address");
        let selected: SocketAddr = "127.0.0.9:47624".parse().expect("selected address");
        let draft = LanBrowserDraft {
            direct_address: first.to_string(),
            selected_server: Some(selected),
            server_page: 2,
        };

        assert_eq!(draft.selected_server, Some(selected));
        assert_ne!(draft.selected_server, Some(first));
    }
}
