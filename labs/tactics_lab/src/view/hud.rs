//! The overlay: what the squad has left, what the facility is about to do, and
//! what every colour on the board means.
//!
//! Two rules shape this file. **Every mark on screen is named** — the Legibility
//! Contract forbids an unlabelled coloured marker, so the legend is built from
//! [`CellPaint::ALL`] and a new paint state cannot be added without appearing
//! here. And **every action has a control**: End Turn and the view toggle are
//! buttons, not just key bindings, because a build that can only be played from
//! a keyboard cannot become the thing this prototype is pointed at.

use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use observed_match::hex_wfc::HexGuardianStatus;
use observed_style::schematic_screen;

use crate::sim::relayout::ShiftOutcome;
use crate::sim::unit::PLAYER_TEAM;
use crate::sim::{MatchStatus, TacticsGame};

use super::{CellPaint, HudRoot, ViewMode};

/// Buttons the HUD owns. Handled by the lab's input systems, which apply the
/// same commands the keyboard accelerators do.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub enum HudButton {
    EndTurn,
    ToggleView,
    LevelUp,
    LevelDown,
    NextUnit,
    Restart,
}

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct SquadText;

/// Touch-sized. The mobile trajectory in the plan is a constraint on this
/// number: 44 logical pixels is the smallest control a thumb reliably hits.
const CONTROL_SIZE: f32 = 48.0;

pub fn spawn(commands: &mut Commands) {
    commands
        .spawn((
            HudRoot,
            Node {
                width: percent(100.0),
                height: percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(14.0)),
                ..default()
            },
            // The HUD must never eat a click meant for the board.
            Pickable::IGNORE,
            Name::new("Tactics HUD"),
        ))
        .with_children(|root| {
            root.spawn((
                StatusText,
                Text::new(String::new()),
                TextFont {
                    font_size: 17.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::FlexEnd,
                    width: percent(100.0),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|row| {
                row.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|column| {
                    spawn_legend(column);
                    column.spawn((
                        SquadText,
                        Text::new(String::new()),
                        TextFont {
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
                row.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: px(8.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|controls| {
                    for (button, label) in [
                        (HudButton::NextUnit, "Next unit"),
                        (HudButton::LevelDown, "-"),
                        (HudButton::LevelUp, "+"),
                        (HudButton::ToggleView, "View"),
                        (HudButton::Restart, "Setup"),
                        (HudButton::EndTurn, "End turn"),
                    ] {
                        spawn_control(controls, button, label);
                    }
                });
            });
        });
}

fn spawn_control(parent: &mut ChildSpawnerCommands, button: HudButton, label: &str) {
    parent
        .spawn((
            button,
            Button,
            Node {
                min_width: px(CONTROL_SIZE * 1.6),
                min_height: px(CONTROL_SIZE),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(14.0), px(8.0)),
                border: UiRect::all(px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.07, 0.1, 0.9)),
            BorderColor::all(Color::srgb(0.35, 0.55, 0.7)),
            Name::new(format!("HUD control: {label}")),
        ))
        .with_children(|control| {
            control.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });
}

/// One row per paint state, coloured by the same treatment the board uses.
fn spawn_legend(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(2.0),
                margin: UiRect::bottom(px(8.0)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|legend| {
            for state in CellPaint::ALL {
                legend
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: px(6.0),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Node {
                                width: px(14.0),
                                height: px(14.0),
                                ..default()
                            },
                            BackgroundColor(swatch(state)),
                            ImageNode::default().with_mode(NodeImageMode::Auto),
                            Pickable::IGNORE,
                        ));
                        row.spawn((
                            Text::new(state.legend().to_string()),
                            TextFont {
                                font_size: 13.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.8, 0.85, 0.9)),
                            Pickable::IGNORE,
                        ));
                    });
            }
        });
}

/// The legend swatch for a state. `Unknown` draws nothing on the board, so it is
/// shown as the screen's own background — the legend says what "nothing" means
/// rather than leaving the player to infer it.
fn swatch(state: CellPaint) -> Color {
    match state {
        CellPaint::Unknown => schematic_screen(),
        other => other.treatment().base_color,
    }
}

/// The status line: turn, what the facility is about to do, and how the match
/// stands.
#[must_use]
pub fn status_line(game: &TacticsGame, mode: ViewMode, level: u8) -> String {
    let shift = match (&game.telegraph, game.settings.shift.interval()) {
        (Some(telegraph), _) => format!("{} cells will re-collapse", telegraph.cells().len()),
        (None, None) => "facility static".to_string(),
        (None, Some(_)) => "no shift this turn".to_string(),
    };
    let last = match game.last_shift {
        Some(ShiftOutcome::Committed) => " | last turn: the facility shifted",
        Some(ShiftOutcome::Held) => " | last turn: you held it",
        Some(ShiftOutcome::NothingToShift) => " | last turn: nothing shifted",
        None => "",
    };
    let guardian = match game.guardian_status() {
        Some(HexGuardianStatus::Active) => " | Guardian hunting",
        Some(HexGuardianStatus::FrozenByPlayer) => " | Guardian frozen: watched",
        Some(HexGuardianStatus::FrozenByAnchor) => " | Guardian frozen: anchored",
        None => "",
    };
    let objectives = if game.keystones_required() > 0 || game.settings.objectives.stations() {
        let progress = game.objectives.team(PLAYER_TEAM);
        let station = if game.settings.objectives.stations() {
            if progress.station_complete {
                ", station done"
            } else {
                ", station pending"
            }
        } else {
            ""
        };
        format!(
            " | keystones {}/{}{station}",
            progress.keystones,
            game.keystones_required()
        )
    } else {
        String::new()
    };
    let outcome = match game.status {
        MatchStatus::Running => String::new(),
        MatchStatus::Escaped => format!(" | ESCAPED on turn {}", game.turn),
        MatchStatus::Outrun => " | OUTRUN by the rival squad".to_string(),
    };
    let view = match mode {
        ViewMode::Isometric => format!("{} (all levels)", mode.label()),
        ViewMode::Flat => format!("{} (level {level})", mode.label()),
    };
    format!(
        "Turn {} | {view} | {shift}{last}{guardian}{objectives}{outcome}\n\
         click a cell to move the selected unit | Tab next unit | Space end turn | V view | R setup",
        game.turn
    )
}

/// One line per unit: where it is, what it has left, and what it is carrying.
#[must_use]
pub fn squad_line(game: &TacticsGame, selected: Option<observed_core::PlayerId>) -> String {
    let mut lines = Vec::new();
    for unit in game.units.values().filter(|unit| unit.team == PLAYER_TEAM) {
        let mark = if Some(unit.id) == selected { ">" } else { " " };
        let state = if unit.escaped {
            "escaped".to_string()
        } else {
            format!(
                "{} AP  anchors {}  pads {}",
                unit.action_points,
                game.anchors.inventory(unit.id),
                game.pads.inventory(unit.id)
            )
        };
        lines.push(format!(
            "{mark} unit {}  ({},{},L{})  {state}",
            unit.id.0, unit.cell.q, unit.cell.r, unit.cell.level
        ));
    }
    lines.join("\n")
}

const fn px(value: f32) -> Val {
    Val::Px(value)
}

const fn percent(value: f32) -> Val {
    Val::Percent(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{GuardianSetting, MatchSettings, Objectives, ShiftCadence};

    fn game(settings: MatchSettings) -> TacticsGame {
        TacticsGame::new(settings).expect("solves")
    }

    /// The status line is the only place a player is told what is about to
    /// happen, so it has to say so in words rather than relying on the colour.
    #[test]
    fn the_status_line_names_what_the_facility_will_do() {
        let shifting = game(MatchSettings::standard());
        let line = status_line(&shifting, ViewMode::Isometric, 0);
        assert!(
            line.contains("re-collapse") || line.contains("no shift"),
            "status line said nothing about the shift: {line}"
        );

        let static_facility = game(MatchSettings {
            shift: ShiftCadence::Off,
            ..MatchSettings::standard()
        });
        assert!(status_line(&static_facility, ViewMode::Isometric, 0).contains("facility static"));
    }

    #[test]
    fn a_match_without_a_guardian_says_nothing_about_one() {
        let quiet = game(MatchSettings {
            guardian: GuardianSetting::Off,
            ..MatchSettings::standard()
        });
        assert!(!status_line(&quiet, ViewMode::Isometric, 0).contains("Guardian"));
    }

    #[test]
    fn objectives_are_reported_only_when_the_match_has_them() {
        let plain = game(MatchSettings {
            objectives: Objectives::ExitOnly,
            ..MatchSettings::standard()
        });
        assert!(!status_line(&plain, ViewMode::Isometric, 0).contains("keystones"));
        let full = game(MatchSettings::standard());
        assert!(status_line(&full, ViewMode::Isometric, 0).contains("keystones 0/2"));
    }

    #[test]
    fn the_flat_view_names_the_level_it_is_showing() {
        let game = game(MatchSettings::standard());
        assert!(status_line(&game, ViewMode::Flat, 2).contains("level 2"));
        assert!(status_line(&game, ViewMode::Isometric, 2).contains("all levels"));
    }

    #[test]
    fn the_squad_line_lists_every_unit_and_marks_the_selected_one() {
        let game = game(MatchSettings::standard());
        let line = squad_line(&game, Some(observed_core::PlayerId(0)));
        assert_eq!(line.lines().count(), usize::from(game.settings.squad_size));
        assert!(line.starts_with('>'), "the selected unit is not marked");
    }
}
