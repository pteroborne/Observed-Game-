//! The overlay: what mode is running, what the turn is about to do, and what
//! every mark on the board means.
//!
//! Two rules shape it. **Every mark is named** — the legend is built from
//! `CellPaint::ALL` and `MarkPaint::ALL`, so a new paint state cannot ship
//! without appearing here. And **every action has a control**, because this
//! build is judged on a phone and a lab that can only be driven from a keyboard
//! would not survive the move.

use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui::{percent, px};
use observed_style::ColorVisionMode;

use crate::sim::state::Outcome;
use crate::spec::{ModeSpec, Rules};

use super::Session;
use super::board::{CellPaint, MarkPaint};

#[derive(Component)]
pub struct HudRoot;

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct ModeText;

#[derive(Component)]
pub struct LegendPanel;

#[derive(Component)]
pub struct ModeMenu;

/// One row of the mode menu, carrying the preset it loads.
#[derive(Component, Clone, Copy)]
pub struct ModeChoice(pub usize);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub enum HudButton {
    Resolve,
    RotateLeft,
    RotateRight,
    FaceOnly,
    Plant,
    Hold,
    Restart,
    Modes,
    Vision,
    Legend,
}

impl HudButton {
    const fn label(self) -> &'static str {
        match self {
            HudButton::Resolve => "Resolve turn",
            HudButton::FaceOnly => "Turn in place",
            HudButton::Plant => "Plant",
            HudButton::Hold => "Hold",
            HudButton::Restart => "Restart",
            HudButton::RotateLeft => "< Turn",
            HudButton::RotateRight => "Turn >",
            HudButton::Modes => "Modes",
            HudButton::Vision => "Vision",
            HudButton::Legend => "Legend",
        }
    }
}

const INK: Color = Color::srgb(0.92, 0.95, 0.98);
const PANEL: Color = Color::srgba(0.02, 0.03, 0.045, 0.94);

pub fn spawn(mut commands: Commands) {
    commands
        .spawn((
            HudRoot,
            Node {
                width: percent(100.0),
                height: percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            // Top: what is running and what the turn did.
            root.spawn((
                Node {
                    width: percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(7.0)),
                    row_gap: px(2.0),
                    ..default()
                },
                BackgroundColor(PANEL),
                Pickable::IGNORE,
            ))
            .with_children(|bar| {
                bar.spawn((
                    ModeText,
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.62, 0.2)),
                    Pickable::IGNORE,
                ));
                bar.spawn((
                    StatusText,
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(INK),
                    Pickable::IGNORE,
                ));
            });

            // The legend, hidden until asked for: it is reference, not chrome.
            root.spawn((
                LegendPanel,
                Node {
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(10.0)),
                    row_gap: px(2.0),
                    ..default()
                },
                BackgroundColor(PANEL),
                Pickable::IGNORE,
            ))
            .with_children(|legend| {
                for paint in CellPaint::ALL {
                    swatch(legend, paint.color(), paint.label());
                }
                for paint in MarkPaint::ALL {
                    swatch(legend, paint.color(), paint.label());
                }
            });

            // The mode menu, over everything. A list rather than a cycler:
            // comparing whole rule sets is the point of the bench, and stepping
            // blindly through seven of them to find one is not comparing.
            root.spawn((
                ModeMenu,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(0.0),
                    left: px(0.0),
                    width: percent(100.0),
                    height: percent(100.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(12.0)),
                    row_gap: px(6.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.015, 0.025, 0.985)),
            ))
            .with_children(|menu| {
                menu.spawn((
                    Text::new("MODES"),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.62, 0.2)),
                    Pickable::IGNORE,
                ));
                for (index, spec) in ModeSpec::presets().iter().enumerate() {
                    menu.spawn((
                        ModeChoice(index),
                        Button,
                        Node {
                            width: percent(100.0),
                            padding: UiRect::all(px(10.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(3.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.09, 0.12, 0.16)),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Text::new(spec.name.clone()),
                            TextFont {
                                font_size: FontSize::Px(14.0),
                                ..default()
                            },
                            TextColor(INK),
                            Pickable::IGNORE,
                        ));
                        row.spawn((
                            Text::new(Rules::from_spec(spec).summary()),
                            TextFont {
                                font_size: FontSize::Px(11.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.58, 0.66, 0.74)),
                            Pickable::IGNORE,
                        ));
                    });
                }
                control(menu, HudButton::Modes);
            });

            // Bottom dock. Everything reachable with a thumb.
            root.spawn((
                Node {
                    width: percent(100.0),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    padding: UiRect::all(px(6.0)),
                    column_gap: px(4.0),
                    row_gap: px(4.0),
                    ..default()
                },
                BackgroundColor(PANEL),
                Pickable::IGNORE,
            ))
            .with_children(|dock| {
                for button in [
                    HudButton::Resolve,
                    HudButton::RotateLeft,
                    HudButton::RotateRight,
                    HudButton::Hold,
                    HudButton::Plant,
                    HudButton::FaceOnly,
                    HudButton::Modes,
                    HudButton::Restart,
                    HudButton::Vision,
                    HudButton::Legend,
                ] {
                    control(dock, button);
                }
            });
        });
}

fn swatch(parent: &mut ChildSpawnerCommands, color: Color, label: &str) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(8.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    width: px(16.0),
                    height: px(10.0),
                    ..default()
                },
                BackgroundColor(color),
                Pickable::IGNORE,
            ));
            row.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(INK),
                Pickable::IGNORE,
            ));
        });
}

fn control(parent: &mut ChildSpawnerCommands, button: HudButton) {
    parent
        .spawn((
            button,
            Button,
            Node {
                min_width: px(62.0),
                height: px(40.0),
                padding: UiRect::horizontal(px(7.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.10, 0.13, 0.17)),
        ))
        .with_children(|face| {
            face.spawn((
                Text::new(button.label()),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(INK),
                Pickable::IGNORE,
            ));
        });
}

pub fn sync(
    session: Res<Session>,
    mut mode: Query<&mut Text, (With<ModeText>, Without<StatusText>)>,
    mut status: Query<&mut Text, With<StatusText>>,
    mut buttons: Query<(&HudButton, &mut BackgroundColor)>,
) {
    if !session.is_changed() {
        return;
    }
    let state = &session.state;
    if let Ok(mut text) = mode.single_mut() {
        **text = format!(
            "{}  ·  preview {:?}  ·  vision {}",
            session.spec.name,
            session.spec.preview,
            session.vision.label(),
        );
    }
    if let Ok(mut text) = status.single_mut() {
        let planted: Vec<String> = state
            .teams()
            .iter()
            .map(|&team| format!("T{}:{}", team.0, state.flags_held_by(team)))
            .collect();
        let verdict = match state.outcome {
            Some(Outcome::Won(team)) if team == session.human => "  YOU WIN".to_string(),
            Some(Outcome::Won(team)) => format!("  TEAM {} WINS", team.0),
            Some(Outcome::Lost(reason)) => format!("  LOST: {reason:?}"),
            Some(Outcome::Draw) => "  DRAW".to_string(),
            None => String::new(),
        };
        let ordered = session.queued.len();
        let commandable = session.commandable().len();
        **text = format!(
            "turn {}/{}   flags {}   orders {ordered}/{commandable}   changing {}{verdict}\n{}",
            state.turn,
            session.spec.turn_limit,
            planted.join(" "),
            state.telegraph.len(),
            session.notice,
        );
    }
    for (button, mut background) in &mut buttons {
        let lit = match button {
            HudButton::FaceOnly => session.face_only,
            HudButton::Modes => session.menu_open,
            HudButton::Vision => session.vision != ColorVisionMode::Normal,
            HudButton::Resolve => session.queued.len() == session.commandable().len(),
            _ => false,
        };
        background.0 = if lit {
            Color::srgb(0.22, 0.36, 0.44)
        } else {
            Color::srgb(0.10, 0.13, 0.17)
        };
    }
}

pub fn sync_menu(session: Res<Session>, mut menu: Query<&mut Node, With<ModeMenu>>) {
    if !session.is_changed() {
        return;
    }
    if let Ok(mut node) = menu.single_mut() {
        node.display = if session.menu_open {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub fn toggle_legend(mut panel: Query<&mut Node, With<LegendPanel>>) {
    if let Ok(mut node) = panel.single_mut() {
        node.display = match node.display {
            Display::None => Display::Flex,
            _ => Display::None,
        };
    }
}
