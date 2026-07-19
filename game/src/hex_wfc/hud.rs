//! First-person HUD for the hex facility race.

use bevy::prelude::*;

use super::sim::HexWfcRuntime;
use crate::GameState;

#[derive(Component)]
pub(super) struct HexWfcHud;

pub(super) fn setup(
    mut commands: Commands,
    spectator_bot: Option<Res<crate::sim::state::SpectatorBot>>,
) {
    let is_spectator = spectator_bot.is_some();
    commands
        .spawn((
            DespawnOnExit(GameState::HexWfc),
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(18)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(30),
            Name::new("Hex WFC HUD root"),
        ))
        .with_children(|root| {
            root.spawn((
                HexWfcHud,
                Text::new("Hex facility initializing"),
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
                "SPECTATING BOT RUN\nEsc menu\n\nGOAL: descend the hex facility and reach the exit rhombus"
            } else {
                "HEX FACILITY RACE\nWASD / stick move | Shift sprint\nMouse / stick look | Space climb up | Ctrl climb down / ramp descend\nE collect cache | F deploy lantern at a looked-at threshold | R recover\nTab survivor map | PgUp/PgDn floor | Esc menu\n\nGOAL: walk the ramps, drop the silo shafts, reach the exit rhombus"
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

pub(super) fn sync(runtime: Res<HexWfcRuntime>, mut hud: Query<&mut Text, With<HexWfcHud>>) {
    if !runtime.is_changed() {
        return;
    }
    let Ok(mut text) = hud.single_mut() else {
        return;
    };
    let player = runtime.local();
    let world = &runtime.match_state.facility;
    let exit = world.config.exit();
    let escaped = runtime
        .match_state
        .players
        .values()
        .filter(|player| player.escaped)
        .count();
    let total = runtime.match_state.players.len();
    **text = format!(
        "GENERATION {}  |  tick {}\nrunners escaped {escaped}/{total}\nlanterns {}  |  Guardian {:?}\ncell q{} r{} L{}  |  exit q{} r{} L{}\n{}{}",
        world.generation,
        runtime.match_state.tick,
        runtime.match_state.lanterns.inventory(runtime.local_player),
        runtime.match_state.guardian.status,
        player.cell.q,
        player.cell.r,
        player.cell.level,
        exit.q,
        exit.r,
        exit.level,
        runtime.status,
        if player.escaped {
            "\n\nYOU ESCAPED"
        } else {
            ""
        }
    );
}
