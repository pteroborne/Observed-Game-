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
    Interact,
    DeployAnchor,
    RecoverAnchor,
    DeployPad,
    PanLeft,
    PanRight,
    PanUp,
    PanDown,
    ZoomIn,
    ZoomOut,
    Recenter,
    ToggleBot,
    StepBot,
    Restart,
    ToggleMore,
}

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct SquadText;

#[derive(Component)]
pub struct CommandDock;

#[derive(Component)]
pub struct DesktopHudContent;

#[derive(Component)]
pub struct MobileHudContent;

#[derive(Component)]
pub struct MobileStatusText;

#[derive(Component)]
pub struct MobilePromptText;

#[derive(Component)]
pub struct MobileMorePanel;

/// Touch-sized. The mobile trajectory in the plan is a constraint on this
/// number: 44 logical pixels is the smallest control a thumb reliably hits.
const CONTROL_SIZE: f32 = 48.0;
/// Logical width shared with the camera viewport inset. One owner prevents the
/// panel and map boundary drifting apart under DPI scaling.
pub const COMMAND_DOCK_WIDTH: f32 = 340.0;
/// Below this logical width the command surface becomes a bottom dock. The
/// breakpoint is deliberately narrow enough that a phone in landscape retains
/// a useful map beside the desktop dock while portrait phones do not.
pub const MOBILE_DOCK_BREAKPOINT: f32 = 700.0;
const MOBILE_DOCK_MIN_HEIGHT: f32 = 220.0;
const MOBILE_DOCK_MAX_HEIGHT: f32 = 320.0;
const MOBILE_MAP_MIN_HEIGHT: f32 = 180.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DockPlacement {
    Right,
    Bottom,
}

/// The one layout decision consumed by both Bevy UI and the board camera.
/// Keeping the viewport and visible dock derived from the same value prevents
/// touch ownership from drifting at a responsive breakpoint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockLayout {
    pub placement: DockPlacement,
    pub viewport_size: Vec2,
    pub dock_size: Vec2,
}

impl DockLayout {
    #[must_use]
    pub fn for_window(window_size: Vec2) -> Self {
        if window_size.x < MOBILE_DOCK_BREAKPOINT {
            let available = (window_size.y - MOBILE_MAP_MIN_HEIGHT).max(0.0);
            let height = (window_size.y * 0.4)
                .clamp(MOBILE_DOCK_MIN_HEIGHT, MOBILE_DOCK_MAX_HEIGHT)
                .min(available);
            Self {
                placement: DockPlacement::Bottom,
                viewport_size: Vec2::new(window_size.x.max(1.0), (window_size.y - height).max(1.0)),
                dock_size: Vec2::new(window_size.x.max(1.0), height),
            }
        } else {
            Self {
                placement: DockPlacement::Right,
                viewport_size: Vec2::new(
                    (window_size.x - COMMAND_DOCK_WIDTH).max(1.0),
                    window_size.y.max(1.0),
                ),
                dock_size: Vec2::new(COMMAND_DOCK_WIDTH, window_size.y.max(1.0)),
            }
        }
    }

    #[must_use]
    pub fn contains_dock_point(self, point: Vec2) -> bool {
        match self.placement {
            DockPlacement::Right => point.x >= self.viewport_size.x,
            DockPlacement::Bottom => point.y >= self.viewport_size.y,
        }
    }
}

pub fn spawn(commands: &mut Commands) {
    commands
        .spawn((
            HudRoot,
            Node {
                width: percent(100.0),
                height: percent(100.0),
                justify_content: JustifyContent::FlexEnd,
                flex_direction: FlexDirection::Row,
                ..default()
            },
            Pickable::IGNORE,
            Name::new("Tactics HUD"),
        ))
        .with_children(|root| {
            root.spawn((
                CommandDock,
                ScrollPosition::default(),
                Node {
                    width: px(COMMAND_DOCK_WIDTH),
                    min_width: px(COMMAND_DOCK_WIDTH),
                    max_width: px(COMMAND_DOCK_WIDTH),
                    height: percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6.0),
                    padding: UiRect::all(px(18.0)),
                    border: UiRect::left(px(2.0)),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.025, 0.035, 0.05, 0.97)),
                BorderColor::all(Color::srgb(0.98, 0.48, 0.12)),
                Name::new("Tactical command dock"),
            ))
            .with_children(|dock| {
                dock.spawn((
                    DesktopHudContent,
                    Node {
                        width: percent(100.0),
                        flex_direction: FlexDirection::Column,
                        flex_shrink: 0.0,
                        row_gap: px(6.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|dock| {
                    dock.spawn((
                        Text::new("OBSERVED // TACTICAL"),
                        TextFont {
                            font_size: 24.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.62, 0.2)),
                        Pickable::IGNORE,
                    ));
                    dock.spawn((
                        Node {
                            width: percent(100.0),
                            height: px(170.0),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|slot| {
                        slot.spawn((
                            StatusText,
                            Text::new(String::new()),
                            TextFont {
                                font_size: 15.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                            Pickable::IGNORE,
                        ));
                    });
                    dock.spawn((
                        Node {
                            width: percent(100.0),
                            height: px(70.0),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|slot| {
                        slot.spawn((
                            SquadText,
                            Text::new(String::new()),
                            TextFont {
                                font_size: 15.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                            Pickable::IGNORE,
                        ));
                    });
                    spawn_section_label(dock, "ACTIONS");
                    dock.spawn((
                        Node {
                            width: percent(100.0),
                            height: px(220.0),
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            column_gap: px(8.0),
                            row_gap: px(8.0),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|controls| {
                        for (button, label) in [
                            (HudButton::Interact, "Interact"),
                            (HudButton::DeployAnchor, "Place anchor"),
                            (HudButton::RecoverAnchor, "Recover anchor"),
                            (HudButton::DeployPad, "Place plate"),
                            (HudButton::NextUnit, "Next unit"),
                            (HudButton::EndTurn, "End turn"),
                        ] {
                            spawn_control(controls, button, label);
                        }
                    });
                    spawn_section_label(dock, "VIEW");
                    dock.spawn((
                        Node {
                            width: percent(100.0),
                            height: px(276.0),
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            column_gap: px(8.0),
                            row_gap: px(8.0),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|controls| {
                        for (button, label) in [
                            (HudButton::PanLeft, "< pan"),
                            (HudButton::PanUp, "pan ^"),
                            (HudButton::PanRight, "pan >"),
                            (HudButton::PanDown, "pan v"),
                            (HudButton::ZoomOut, "- zoom"),
                            (HudButton::ZoomIn, "+ zoom"),
                            (HudButton::Recenter, "Recenter"),
                            (HudButton::LevelDown, "Deck -"),
                            (HudButton::LevelUp, "Deck +"),
                            (HudButton::ToggleView, "Deck / map"),
                            (HudButton::Restart, "Setup"),
                        ] {
                            spawn_control(controls, button, label);
                        }
                    });
                    spawn_section_label(dock, "SPECTATE");
                    dock.spawn((
                        Node {
                            width: percent(100.0),
                            height: px(56.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: px(8.0),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|controls| {
                        for (button, label) in [
                            (HudButton::ToggleBot, "Bot run / pause"),
                            (HudButton::StepBot, "Bot step"),
                        ] {
                            spawn_control(controls, button, label);
                        }
                    });
                    spawn_legend(dock);
                });
                spawn_mobile_content(dock);
            });
        });
}

fn spawn_mobile_content(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            MobileHudContent,
            Node {
                display: Display::None,
                width: percent(100.0),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                row_gap: px(8.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|mobile| {
            mobile.spawn((
                MobileStatusText,
                Text::new(String::new()),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(1.0, 0.68, 0.28)),
                Pickable::IGNORE,
            ));
            mobile.spawn((
                MobilePromptText,
                Text::new(String::new()),
                TextFont { font_size: 17.0, ..default() },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
            spawn_mobile_row(
                mobile,
                &[
                    (HudButton::NextUnit, "Next runner"),
                    (HudButton::Interact, "Use"),
                    (HudButton::EndTurn, "End turn"),
                ],
            );
            spawn_mobile_row(
                mobile,
                &[
                    (HudButton::ToggleView, "Deck / map"),
                    (HudButton::Recenter, "Recenter"),
                    (HudButton::ToggleMore, "More / less"),
                ],
            );
            mobile
                .spawn((
                    MobileMorePanel,
                    Node {
                        display: Display::None,
                        width: percent(100.0),
                        flex_direction: FlexDirection::Column,
                        flex_shrink: 0.0,
                        row_gap: px(8.0),
                        padding: UiRect::top(px(4.0)),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|more| {
                    spawn_mobile_row(
                        more,
                        &[
                            (HudButton::DeployAnchor, "Place anchor"),
                            (HudButton::RecoverAnchor, "Recover"),
                            (HudButton::DeployPad, "Place portal"),
                        ],
                    );
                    spawn_mobile_row(
                        more,
                        &[
                            (HudButton::LevelDown, "Deck -"),
                            (HudButton::LevelUp, "Deck +"),
                            (HudButton::ZoomOut, "Zoom -"),
                            (HudButton::ZoomIn, "Zoom +"),
                        ],
                    );
                    spawn_mobile_row(
                        more,
                        &[
                            (HudButton::ToggleBot, "Bot run / pause"),
                            (HudButton::StepBot, "Bot step"),
                            (HudButton::Restart, "Setup"),
                        ],
                    );
                    more.spawn((
                        Text::new("Touch map: tap runner, then highlighted ground. Drag to pan. Pinch to zoom."),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(0.72, 0.8, 0.88)),
                        Pickable::IGNORE,
                    ));
                });
        });
}

fn spawn_mobile_row(parent: &mut ChildSpawnerCommands, controls: &[(HudButton, &str)]) {
    parent
        .spawn((
            Node {
                width: percent(100.0),
                min_height: px(52.0),
                flex_direction: FlexDirection::Row,
                column_gap: px(8.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|row| {
            for &(button, label) in controls {
                row.spawn((
                    button,
                    Button,
                    Node {
                        min_width: px(0.0),
                        min_height: px(52.0),
                        flex_basis: px(0.0),
                        flex_grow: 1.0,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::axes(px(8.0), px(6.0)),
                        border: UiRect::all(px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.07, 0.1, 0.94)),
                    BorderColor::all(Color::srgb(0.35, 0.65, 0.82)),
                    Name::new(format!("Mobile HUD control: {label}")),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new(label.to_string()),
                        TextFont {
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        Pickable::IGNORE,
                    ));
                });
            }
        });
}

/// Apply the responsive dock orientation. This is idempotent and only changes
/// values derived from the display dimensions; game state never reflows it.
pub fn sync_layout(window_size: Vec2, root: &mut Node, dock: &mut Node) -> DockLayout {
    let layout = DockLayout::for_window(window_size);
    match layout.placement {
        DockPlacement::Right => {
            root.flex_direction = FlexDirection::Row;
            dock.width = px(layout.dock_size.x);
            dock.min_width = px(layout.dock_size.x);
            dock.max_width = px(layout.dock_size.x);
            dock.height = percent(100.0);
            dock.min_height = Val::Auto;
            dock.max_height = Val::Auto;
            dock.border = UiRect::left(px(2.0));
        }
        DockPlacement::Bottom => {
            root.flex_direction = FlexDirection::Column;
            dock.width = percent(100.0);
            dock.min_width = Val::Auto;
            dock.max_width = Val::Auto;
            dock.height = px(layout.dock_size.y);
            dock.min_height = px(layout.dock_size.y);
            dock.max_height = px(layout.dock_size.y);
            dock.border = UiRect::top(px(2.0));
        }
    }
    layout
}

pub fn sync_content_visibility(placement: DockPlacement, desktop: &mut Node, mobile: &mut Node) {
    let mobile_display = placement == DockPlacement::Bottom;
    desktop.display = if mobile_display {
        Display::None
    } else {
        Display::Flex
    };
    mobile.display = if mobile_display {
        Display::Flex
    } else {
        Display::None
    };
}

fn spawn_section_label(parent: &mut ChildSpawnerCommands, label: &str) {
    parent.spawn((
        Node {
            width: percent(100.0),
            height: px(18.0),
            ..default()
        },
        Text::new(label.to_string()),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.76, 0.82)),
        Pickable::IGNORE,
    ));
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
            for note in [
                "structure hue - district atmosphere (hover to name)",
                "ring - exit or current objective",
                "purple floor disc - unlinked portal plate",
                "cyan double ring - linked portal plate",
                "rising / descending rings - portal transit",
                "small arrow - shaft to deck above / below",
            ] {
                legend.spawn((
                    Text::new(note),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.8, 0.85, 0.9)),
                    Pickable::IGNORE,
                ));
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
        (Some(telegraph), _) => format!("{} pending", telegraph.cells().len()),
        (None, None) => "static".to_string(),
        (None, Some(_)) => "none pending".to_string(),
    };
    let last = match game.last_shift {
        Some(ShiftOutcome::Committed) => " | last: shifted",
        Some(ShiftOutcome::Held) => " | last: held",
        Some(ShiftOutcome::NothingToShift) => " | last: none",
        None => "",
    };
    let guardian = match game.guardian_status() {
        Some(HexGuardianStatus::Active) => "hunting",
        Some(HexGuardianStatus::FrozenByPlayer) => "frozen: watched",
        Some(HexGuardianStatus::FrozenByAnchor) => "frozen: anchored",
        None => "not deployed",
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
            "keystones {}/{}{station}",
            progress.keystones,
            game.keystones_required()
        )
    } else {
        "none".to_string()
    };
    let outcome = match game.status {
        MatchStatus::Running => "running".to_string(),
        MatchStatus::Escaped => format!("ESCAPED on turn {}", game.turn),
        MatchStatus::Outrun => "OUTRUN by the rival squad".to_string(),
    };
    let view = match mode {
        ViewMode::Overview => format!("Map L{level}"),
        ViewMode::Deck => format!("Deck L{level}"),
    };
    let exit_hint = if game.settings.reveal_full_map {
        let exit = game.world.config.exit();
        format!("\nFull map: EXIT L{} ({},{})", exit.level, exit.q, exit.r)
    } else {
        String::new()
    };
    format!(
        "Turn {} | {view}\n\
         Shift: {shift}{last}\n\
         Guardian: {guardian} | Observe: {} tiles\n\
         Goals: {objectives} | {outcome}{exit_hint}",
        game.turn,
        game.settings.sight.hops(),
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
                "AP{} A{} P{}",
                unit.action_points,
                game.anchors.inventory(unit.id),
                game.pads.inventory(unit.id)
            )
        };
        lines.push(format!(
            "{mark} U{} L{} ({},{}) | {state}",
            unit.id.0, unit.cell.level, unit.cell.q, unit.cell.r
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
        let line = status_line(&shifting, ViewMode::Overview, 0);
        assert!(line.contains("Shift:") && line.contains("pending"));

        let static_facility = game(MatchSettings {
            shift: ShiftCadence::Off,
            ..MatchSettings::standard()
        });
        assert!(status_line(&static_facility, ViewMode::Overview, 0).contains("Shift: static"));
    }

    #[test]
    fn a_match_without_a_guardian_keeps_the_fixed_status_slot() {
        let quiet = game(MatchSettings {
            guardian: GuardianSetting::Off,
            ..MatchSettings::standard()
        });
        assert!(status_line(&quiet, ViewMode::Overview, 0).contains("Guardian: not deployed"));
    }

    #[test]
    fn objectives_are_reported_only_when_the_match_has_them() {
        let plain = game(MatchSettings {
            objectives: Objectives::ExitOnly,
            ..MatchSettings::standard()
        });
        assert!(!status_line(&plain, ViewMode::Overview, 0).contains("keystones"));
        let full = game(MatchSettings::standard());
        assert!(status_line(&full, ViewMode::Overview, 0).contains("keystones 0/1"));
    }

    #[test]
    fn both_views_name_the_deck_they_are_showing() {
        let game = game(MatchSettings::standard());
        assert!(status_line(&game, ViewMode::Deck, 2).contains("Deck L2"));
        assert!(status_line(&game, ViewMode::Overview, 2).contains("Map L2"));
    }

    #[test]
    fn full_map_status_names_the_exit_deck_and_coordinate() {
        let game = game(MatchSettings::guided());
        let exit = game.world.config.exit();
        let line = status_line(&game, ViewMode::Overview, 0);
        assert!(line.contains(&format!("EXIT L{} ({},{})", exit.level, exit.q, exit.r)));
    }

    #[test]
    fn the_squad_line_lists_every_unit_and_marks_the_selected_one() {
        let game = game(MatchSettings::standard());
        let line = squad_line(&game, Some(observed_core::PlayerId(0)));
        assert_eq!(line.lines().count(), usize::from(game.settings.squad_size));
        assert!(line.starts_with('>'), "the selected unit is not marked");
    }

    #[test]
    fn narrow_displays_use_a_bottom_dock_without_covering_the_map() {
        let layout = DockLayout::for_window(Vec2::new(390.0, 844.0));
        assert_eq!(layout.placement, DockPlacement::Bottom);
        assert_eq!(layout.viewport_size.x, 390.0);
        assert!(layout.viewport_size.y >= MOBILE_MAP_MIN_HEIGHT);
        assert_eq!(layout.viewport_size.y + layout.dock_size.y, 844.0);
        assert!(!layout.contains_dock_point(Vec2::new(100.0, layout.viewport_size.y - 1.0)));
        assert!(layout.contains_dock_point(Vec2::new(100.0, layout.viewport_size.y + 1.0)));
    }

    #[test]
    fn landscape_and_desktop_displays_keep_the_right_dock() {
        for size in [Vec2::new(844.0, 390.0), Vec2::new(1600.0, 1000.0)] {
            let layout = DockLayout::for_window(size);
            assert_eq!(layout.placement, DockPlacement::Right);
            assert_eq!(layout.viewport_size.x + layout.dock_size.x, size.x);
            assert_eq!(layout.viewport_size.y, size.y);
        }
    }

    #[test]
    fn mobile_layout_uses_the_compact_touch_surface() {
        let mut desktop = Node::default();
        let mut mobile = Node::default();
        sync_content_visibility(DockPlacement::Bottom, &mut desktop, &mut mobile);
        assert_eq!(desktop.display, Display::None);
        assert_eq!(mobile.display, Display::Flex);

        sync_content_visibility(DockPlacement::Right, &mut desktop, &mut mobile);
        assert_eq!(desktop.display, Display::Flex);
        assert_eq!(mobile.display, Display::None);
    }
}
