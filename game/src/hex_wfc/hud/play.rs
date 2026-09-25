//! Restrained first-person chrome. All world eligibility comes from the match brain.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use observed_match::hex_wfc::{DUAL_STATION_HOLD_TICKS, HexInteractionAction, HexMatchEventKind};

use super::super::{HexOnboardingGate, overlay::MatchOverlayState, sim::HexWfcRuntime};
use crate::{
    GameState,
    settings::{Settings, key_name},
    view::theme::{ACCENT, DIM, PANEL, TITLE, WARNING},
};

#[derive(Component)]
pub(in crate::hex_wfc) struct PlayHud;
#[derive(Component, Clone, Copy)]
pub(in crate::hex_wfc) enum Readout {
    Objective,
    Equipment,
    Interaction,
    Notice,
}
#[derive(Resource, Default)]
pub(in crate::hex_wfc) struct HudNotice {
    tick: u64,
    until: f64,
    text: &'static str,
}

pub(in crate::hex_wfc) fn setup(mut commands: Commands) {
    commands.insert_resource(HudNotice::default());
    commands
        .spawn((
            PlayHud,
            DespawnOnExit(GameState::HexWfc),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(31),
            Name::new("Contextual gameplay HUD"),
        ))
        .with_children(|root| {
            for (kind, top, bottom, left, right, width, size, color) in [
                (
                    Readout::Objective,
                    px(20),
                    Val::Auto,
                    px(20),
                    Val::Auto,
                    percent(34),
                    18.0,
                    TITLE,
                ),
                (
                    Readout::Equipment,
                    Val::Auto,
                    px(20),
                    px(20),
                    Val::Auto,
                    percent(30),
                    16.0,
                    DIM,
                ),
                (
                    Readout::Interaction,
                    Val::Auto,
                    percent(20),
                    percent(30),
                    percent(30),
                    percent(40),
                    20.0,
                    ACCENT,
                ),
                (
                    Readout::Notice,
                    px(20),
                    Val::Auto,
                    percent(38),
                    percent(28),
                    percent(34),
                    20.0,
                    WARNING,
                ),
            ] {
                root.spawn((
                    kind,
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Px(size),
                        ..default()
                    },
                    TextColor(color),
                    Node {
                        position_type: PositionType::Absolute,
                        top,
                        bottom,
                        left,
                        right,
                        width,
                        padding: UiRect::axes(px(14), px(10)),
                        ..default()
                    },
                    BackgroundColor(PANEL),
                    Visibility::Hidden,
                ));
            }
        });
}

pub(in crate::hex_wfc) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<HudNotice>();
}

#[derive(SystemParam)]
pub(in crate::hex_wfc) struct HudContext<'w, 's> {
    runtime: Res<'w, HexWfcRuntime>,
    settings: Res<'w, Settings>,
    time: Res<'w, Time>,
    overlay: Res<'w, MatchOverlayState>,
    onboarding: Res<'w, HexOnboardingGate>,
    spectator: Option<Res<'w, crate::sim::state::SpectatorBot>>,
    notice: ResMut<'w, HudNotice>,
    readouts: Query<
        'w,
        's,
        (
            &'static Readout,
            &'static mut Text,
            &'static mut Visibility,
            &'static mut TextFont,
        ),
    >,
}

pub(in crate::hex_wfc) fn sync(context: HudContext) {
    let HudContext {
        runtime,
        settings,
        time,
        overlay,
        onboarding,
        spectator,
        mut notice,
        mut readouts,
    } = context;
    let game = &runtime.match_state;
    let player = runtime.local();
    let team = &game.teams[&player.team];
    let hidden = *overlay != MatchOverlayState::Playing || onboarding.active || spectator.is_some();
    if notice.tick != game.tick {
        notice.tick = game.tick;
        if let Some(message) = game
            .recent_events
            .iter()
            .rev()
            .filter(|event| event.player == Some(runtime.local_player))
            .find_map(|event| match event.kind {
                HexMatchEventKind::AnchorDeployed => Some("Connection anchored"),
                HexMatchEventKind::AnchorRecovered => {
                    Some("Lantern recovered | connection released")
                }
                HexMatchEventKind::KeystoneCollected => Some("Team keystone collected"),
                HexMatchEventKind::MonitorSurveyed => Some("Team map updated"),
                HexMatchEventKind::LanternCacheCollected => Some("Anchor lanterns collected"),
                HexMatchEventKind::PlayerRecovered => Some("Recovered to safe ground"),
                HexMatchEventKind::ExitDenied => {
                    Some("Complete the team's objectives before leaving")
                }
                HexMatchEventKind::GuardianCatch => {
                    Some("Guardian caught you | regroup with your team")
                }
                HexMatchEventKind::DualStationCompleted => {
                    Some("Station synchronized | head for the exit")
                }
                _ => None,
            })
        {
            notice.text = message;
            notice.until = time.elapsed_secs_f64() + 3.0;
        }
    }
    for (kind, mut text, mut visibility, mut font) in &mut readouts {
        let size = match kind {
            Readout::Objective => 18.0,
            Readout::Equipment => 16.0,
            _ => 20.0,
        } * settings.gameplay_text_scale;
        let next_font = FontSize::Px(size);
        if font.font_size != next_font {
            font.font_size = next_font;
        }
        let line = match kind {
            Readout::Objective => {
                let next = if player.escaped {
                    "Escaped | waiting for the team"
                } else if game.objectives.enabled
                    && team.objectives.keystones < game.objectives.keystones_required
                {
                    "Explore and collect team keystones"
                } else if game.objectives.enabled && !team.objectives.dual_station_complete {
                    if team.members.len() == 1 {
                        "Find and hold a station console"
                    } else {
                        "Synchronize a station with your teammate"
                    }
                } else {
                    "Regroup at the exit"
                };
                let next = if game.objectives.enabled && team.objectives.keystones < game.objectives.keystones_required {
                    format!("Find keystones | {} / {}", team.objectives.keystones, game.objectives.keystones_required)
                } else { next.to_owned() };
                format!(
                    "FLOOR {}  |  TEAM {}\n{next}",
                    player.cell.level + 1,
                    player.team.0 + 1
                )
            }
            Readout::Equipment => format!(
                "Lanterns {}  |  Plates {}\n[{} / Y] Place teleport plate\n[{} / View] Team map\n[{} / Menu] Pause",
                game.lanterns.inventory(runtime.local_player),
                game.pads.inventory(runtime.local_player),
                key_name(settings.bindings.pad),
                key_name(settings.bindings.tac_map),
                key_name(settings.bindings.pause)
            ),
            Readout::Interaction => {
                game.interaction(runtime.local_player)
                    .map_or_else(String::new, |prompt| {
                        let (key, controller, hold) = match prompt.action {
                            HexInteractionAction::Interact => (settings.bindings.interact, "X", ""),
                            HexInteractionAction::HoldInteract => {
                                (settings.bindings.interact, "X", "Hold ")
                            }
                            HexInteractionAction::DeployLantern => {
                                (settings.bindings.torch, "LB", "")
                            }
                            HexInteractionAction::RecoverLantern => {
                                (settings.bindings.recover_lantern, "B", "")
                            }
                        };
                        let progress = if prompt.action == HexInteractionAction::HoldInteract
                            && team.objectives.dual_station_ticks > 0
                        {
                            format!(
                                "{}% | ",
                                u32::from(team.objectives.dual_station_ticks) * 100
                                    / u32::from(DUAL_STATION_HOLD_TICKS)
                            )
                        } else {
                            String::new()
                        };
                        format!(
                            "{hold}[{} / {controller}] {}\n{progress}{}",
                            key_name(key),
                            prompt.title,
                            prompt.detail
                        )
                    })
            }
            Readout::Notice => {
                if time.elapsed_secs_f64() < notice.until {
                    notice.text.to_owned()
                } else {
                    String::new()
                }
            }
        };
        *visibility = if hidden || line.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if text.0 != line {
            text.0 = line;
        }
    }
}
