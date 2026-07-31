//! Live authoritative LAN lobby presentation.
//!
//! The server owns seats, teams, readiness, and launch. This module projects that
//! truth into an N-team roster and screen-local actions; it never fabricates a
//! historical two-team session model.

use std::collections::BTreeSet;

use bevy::input_focus::tab_navigation::TabIndex;
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Activate;
use observed_core::TeamId;
use observed_net::lan::{WirePhase, WireSeat, WireSeatOccupant};
use player_input::PlayerId;

use super::lan::LanLobbyRosterText;
use super::widgets::{
    self, FocusScope, FocusScopeId, WidgetId, WidgetLabel, WidgetSpec, activation_enabled,
};
use crate::GameState;
use crate::view::theme::{ACCENT, BORDER, DIM, PANEL, TITLE, screen_root, text};

const SCOPE: FocusScopeId = FocusScopeId("lan_lobby");
const READY: WidgetId = WidgetId::named("lobby.ready");
const LEAVE: WidgetId = WidgetId::named("lobby.leave");
const LEAVE_ORDER: u16 = 60_000;
const LOBBY_BODY_WIDTH: f32 = 920.0;
const LOBBY_BODY_HEIGHT: f32 = 500.0;
const ACTION_COLUMN_WIDTH: f32 = 344.0;
const TEAM_BUTTON_WIDTH: f32 = 160.0;
const TEAM_BUTTON_HEIGHT: f32 = 36.0;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LobbyAction {
    ToggleReady,
    RequestTeam(TeamId),
    Leave,
}

#[derive(Component)]
pub(crate) struct ReadyWidget;

#[derive(Component, Default)]
pub(crate) struct TeamActionsPanel {
    spawned: BTreeSet<TeamId>,
}

pub(crate) fn setup_lobby(mut commands: Commands) {
    commands
        .spawn((
            screen_root(GameState::Lobby),
            widgets::focus_scope(FocusScope::screen(SCOPE, READY, LEAVE)),
        ))
        .with_children(|root| {
            root.spawn(text("LAN LOBBY", 40.0, TITLE));
            root.spawn(text(
                "The server owns this roster. Every visible seat enters the same authoritative run.",
                15.0,
                DIM,
            ));
            root.spawn(compact_lobby_body()).with_children(|body| {
                body.spawn(Node {
                    width: percent(100),
                    height: percent(100),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexStart,
                    row_gap: px(10),
                    overflow: Overflow::clip(),
                    ..default()
                })
                .with_children(|roster| {
                    roster.spawn((
                        LanLobbyRosterText,
                        text("Waiting for server roster...", 15.0, DIM),
                    ));
                    roster.spawn(text(
                        "Seat legend: HUMAN | BOT | RESERVED | PREPARING | EMPTY",
                        12.0,
                        ACCENT,
                    ));
                });
                body.spawn(Node {
                    width: px(ACTION_COLUMN_WIDTH),
                    height: percent(100),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|actions| {
                    widgets::spawn_button(
                        actions,
                        WidgetSpec::disabled(READY, SCOPE, 0, "Ready | waiting for roster")
                            .with_size(320.0, 42.0),
                        (LobbyAction::ToggleReady, ReadyWidget),
                    );
                    actions.spawn((
                        Node {
                            width: px(ACTION_COLUMN_WIDTH),
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            align_content: AlignContent::FlexStart,
                            justify_content: JustifyContent::Center,
                            column_gap: px(4),
                            row_gap: px(2),
                            ..default()
                        },
                        TeamActionsPanel::default(),
                    ));
                    widgets::spawn_button(
                        actions,
                        WidgetSpec::enabled(LEAVE, SCOPE, LEAVE_ORDER, "Leave lobby")
                            .with_size(320.0, 42.0),
                        LobbyAction::Leave,
                    );
                });
            });
            root.spawn(text(
                "Choose any listed team | every teammate must escape | Esc / B leaves",
                14.0,
                ACCENT,
            ));
        });
}

fn compact_lobby_body() -> impl Bundle {
    (
        Node {
            width: px(LOBBY_BODY_WIDTH),
            max_width: percent(96),
            height: px(LOBBY_BODY_HEIGHT),
            padding: UiRect::all(px(20)),
            border: UiRect::all(px(1)),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexStart,
            column_gap: px(20),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(BORDER),
    )
}

pub(crate) fn activate(
    activation: On<Activate>,
    actions: Query<&LobbyAction>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut commands: Commands,
    mut lan: ResMut<crate::lan::LanRuntime>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !activation_enabled(&activation, &disabled) {
        return;
    }
    let Ok(action) = actions.get(activation.entity) else {
        return;
    };
    let result = match *action {
        LobbyAction::ToggleReady => lan.toggle_ready(),
        LobbyAction::RequestTeam(team) => lan.request_team(team),
        LobbyAction::Leave => {
            commands.remove_resource::<crate::hex_wfc::loading::HexLaunchRequest>();
            lan.leave();
            next.set(GameState::LanBrowser);
            return;
        }
    };
    if let Err(error) = result {
        lan.status = error;
    }
}

/// Refreshes action labels and adds one stable team-selection widget for every
/// team the authoritative roster exposes. Server rosters are immutable within a
/// lobby generation, so widgets only need to be added, never churned by index.
pub(crate) fn lobby_update_labels(
    mut commands: Commands,
    lan: Res<crate::lan::LanRuntime>,
    ready_widgets: Query<Entity, With<ReadyWidget>>,
    mut team_panels: Query<(Entity, &mut TeamActionsPanel)>,
    action_widgets: Query<(Entity, &LobbyAction)>,
) {
    let connected = lan
        .client
        .as_ref()
        .is_some_and(|client| client.token.is_some() && client.lobby.is_some());
    if let Ok(entity) = ready_widgets.single() {
        let label = if connected {
            format!("Ready: {}", if lan.ready { "ON" } else { "OFF" })
        } else {
            "Ready | waiting for roster".to_string()
        };
        set_widget_availability(&mut commands, entity, 0, connected, label);
    }

    let teams = lobby_teams(&lan);
    let current_team = lan.client.as_ref().and_then(|client| client.team);
    for (entity, action) in &action_widgets {
        let LobbyAction::RequestTeam(team) = *action else {
            continue;
        };
        commands
            .entity(entity)
            .insert(WidgetLabel(team_action_label(team, current_team)));
    }

    let Ok((panel, mut known)) = team_panels.single_mut() else {
        return;
    };
    for team in teams {
        if !known.spawned.insert(team) {
            continue;
        }
        let order = 1_u16.saturating_add(u16::from(team.0));
        commands.entity(panel).with_children(|panel| {
            widgets::spawn_button(
                panel,
                WidgetSpec::enabled(
                    WidgetId::keyed("lobby.team", u64::from(team.0)),
                    SCOPE,
                    order,
                    team_action_label(team, current_team),
                )
                .with_size(TEAM_BUTTON_WIDTH, TEAM_BUTTON_HEIGHT),
                LobbyAction::RequestTeam(team),
            );
        });
    }
}

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
        client.player,
    )
}

fn format_roster(
    phase: WirePhase,
    countdown: u16,
    seats: &[WireSeat],
    local_player: Option<PlayerId>,
) -> String {
    let countdown = if countdown > 0 {
        format!(" | launch in {:.1}s", f32::from(countdown) / 60.0)
    } else {
        String::new()
    };
    let teams = seats.iter().map(|seat| seat.team).collect::<BTreeSet<_>>();
    let mut lines = vec![format!("{phase:?}{countdown} | {} seats", seats.len())];
    for team in teams {
        let team_seats = seats
            .iter()
            .filter(|seat| seat.team == team)
            .collect::<Vec<_>>();
        let mut entries = Vec::with_capacity(team_seats.len());
        for seat in team_seats {
            let occupant = match seat.occupant {
                WireSeatOccupant::Bot => "BOT",
                WireSeatOccupant::Human => "HUMAN",
                WireSeatOccupant::ReservedHuman => "RESERVED",
                WireSeatOccupant::SynchronizingHuman => "PREPARING",
                WireSeatOccupant::Empty => "EMPTY",
            };
            let you = if local_player == Some(seat.player) {
                " (YOU)"
            } else {
                ""
            };
            let ready = if seat.ready { " | READY" } else { "" };
            entries.push(format!("{}{you}: {occupant}{ready}", seat.player.label()));
        }
        lines.push(format!(
            "{} | {} seats | {}",
            team.label().to_uppercase(),
            entries.len(),
            entries.join(" | ")
        ));
    }
    lines.join("\n")
}

fn lobby_teams(lan: &crate::lan::LanRuntime) -> BTreeSet<TeamId> {
    lan.client
        .as_ref()
        .and_then(|client| client.lobby.as_ref())
        .map_or_else(BTreeSet::new, |lobby| {
            lobby.seats.iter().map(|seat| seat.team).collect()
        })
}

fn team_action_label(team: TeamId, current: Option<TeamId>) -> String {
    if current == Some(team) {
        format!("> {} | current", team.label())
    } else {
        format!("Request {}", team.label())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn four_by_four() -> Vec<WireSeat> {
        (0..16)
            .map(|index| WireSeat {
                player: PlayerId(index),
                team: TeamId((index / 4) as u8),
                occupant: if index % 4 == 0 {
                    WireSeatOccupant::Human
                } else {
                    WireSeatOccupant::Bot
                },
                ready: index % 4 == 0,
            })
            .collect()
    }

    #[test]
    fn four_by_four_roster_renders_every_team_and_all_sixteen_seats() {
        let text = format_roster(WirePhase::Lobby, 0, &four_by_four(), Some(PlayerId(0)));
        for team in 1..=4 {
            assert!(text.contains(&format!("TEAM {team} | 4 seats")));
        }
        for player in 1..=16 {
            assert!(text.contains(&format!("P{player}")));
        }
        assert!(text.contains("P1 (YOU): HUMAN | READY"));
        assert!(!text.contains("TEAM 5"));
    }

    #[test]
    fn roster_groups_sparse_team_ids_in_stable_order() {
        let seats = vec![
            WireSeat {
                player: PlayerId(0),
                team: TeamId(3),
                occupant: WireSeatOccupant::Bot,
                ready: false,
            },
            WireSeat {
                player: PlayerId(1),
                team: TeamId(1),
                occupant: WireSeatOccupant::Human,
                ready: true,
            },
        ];
        let text = format_roster(WirePhase::Countdown, 120, &seats, None);
        assert!(text.find("TEAM 2").expect("team 2") < text.find("TEAM 4").expect("team 4"));
        assert!(text.contains("launch in 2.0s"));
    }

    #[test]
    fn humans_only_empty_seat_is_not_presented_as_a_bot() {
        let seats = [WireSeat {
            player: PlayerId(0),
            team: TeamId(0),
            occupant: WireSeatOccupant::Empty,
            ready: false,
        }];
        let text = format_roster(WirePhase::Lobby, 0, &seats, None);
        assert!(text.contains(": EMPTY"));
        assert!(!text.contains(": BOT"));
    }

    #[test]
    fn sixteen_teams_fit_the_bounded_roster_and_wrapped_action_column() {
        let team_count = observed_net::lan::MAX_SEATS;
        let roster_height = (team_count + 1) as f32 * 18.0;
        let team_rows = team_count.div_ceil(2) as f32;
        let actions_height = team_rows * (TEAM_BUTTON_HEIGHT + 6.0) + 2.0 * 48.0;

        assert!(roster_height < LOBBY_BODY_HEIGHT - 40.0);
        assert!(actions_height < LOBBY_BODY_HEIGHT - 40.0);
    }
}
