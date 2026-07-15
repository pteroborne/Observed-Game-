use bevy::prelude::*;

use super::sim::FullWfcRuntime;
use crate::GameState;

#[derive(Component)]
pub(super) struct FullWfcHud;

pub(super) fn setup(
    mut commands: Commands,
    spectator_bot: Option<Res<crate::sim::state::SpectatorBot>>,
) {
    let is_spectator = spectator_bot.is_some();
    commands
        .spawn((
            DespawnOnExit(GameState::FullWfc),
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(18)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(30),
            Name::new("Full WFC HUD root"),
        ))
        .with_children(|root| {
            root.spawn((
                FullWfcHud,
                Text::new("Full WFC initializing"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.95, 1.0)),
                Node {
                    width: px(470),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.008, 0.018, 0.035, 0.9)),
                BorderColor::all(Color::srgba(0.35, 0.9, 1.0, 0.6)),
            ));
            let help_text = if is_spectator {
                "SPECTATING BOT MATCH\nEsc menu\n\nGOAL: 2 keystones + dual station + both teammates exit\nCyan frame: mutable | Purple: anchored | Red: exit chain sealed"
            } else {
                "FULL WFC MATCH\nWASD / stick move | Shift sprint\nMouse / stick look | Space climb up | Ctrl down\nE interact / use pad | F anchor | C pad | R recover\nTab survivor map | Esc menu\n\nGOAL: 2 keystones + dual station + both teammates exit\nCyan frame: mutable | Purple: anchored | Red: exit chain sealed"
            };
            root.spawn((
                Text::new(help_text),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.70, 0.78, 0.9)),
                Node {
                    width: px(330),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.008, 0.018, 0.035, 0.88)),
                BorderColor::all(Color::srgba(0.35, 0.9, 1.0, 0.45)),
            ));
            if !is_spectator {
                root.spawn((
                    Text::new("+"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.95, 1.0)),
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(50),
                        top: percent(48),
                        ..default()
                    },
                ));
            }
        });
}

pub(super) fn sync(runtime: Res<FullWfcRuntime>, mut hud: Query<&mut Text, With<FullWfcHud>>) {
    if !runtime.is_changed() {
        return;
    }
    let Ok(mut text) = hud.single_mut() else {
        return;
    };
    let player = runtime.local();
    let world = &runtime.match_state.facility;
    let proximity = world.candle_proximity(player.cell);
    let observed = world
        .observation
        .visible_thresholds
        .iter()
        .next()
        .map_or("none".to_string(), |key| format!("{:?}", key.face));
    let exit_rule = world
        .exit_claim
        .as_ref()
        .map_or("exit unclaimed".to_string(), |claim| {
            format!("exit chain claimed by {}", claim.owner.label())
        });
    let space = world.placements[&player.cell].space;
    let team = &runtime.match_state.teams[&player.team];
    let pressure = runtime.match_state.guardian_pressure(runtime.local_player);
    **text = format!(
        "GENERATION {}  |  tick {}\nTEAM {}  |  keys {}/2  |  dual station {}\nlevel {}  |  {:?} ({}, {})  |  candle {:.0}%\nGuardian pressure {:.0}%  |  faced threshold {}\n{}\n{}{}",
        world.generation,
        runtime.match_state.tick,
        player.team.0 + 1,
        team.keystones,
        if team.dual_station_complete {
            "READY"
        } else {
            "pending"
        },
        player.cell.level,
        space,
        player.cell.x,
        player.cell.z,
        proximity * 100.0,
        pressure * 100.0,
        observed,
        exit_rule,
        runtime.status,
        if team.escaped {
            "\n\nTEAM ESCAPED - collapse countdown active"
        } else {
            ""
        }
    );
}
