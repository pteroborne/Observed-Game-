//! The Architect's desk: the chrome around the board, laid out as `architect_lab` lays
//! out its own.
//!
//! - **The top bar** names the seat and the team, says whether the hand is charged, and
//!   switches floors.
//! - **The side panel** holds the team's Observers, the climb (`stack`) and the key.
//! - **The hand** runs along the bottom: five cards, each a miniature of the real tile it
//!   will build (`cards`), with the controls in a line beneath.
//! - **The card panel** stands at the right while a card is picked up: the tile large,
//!   where it is aimed and how it is turned, the rules' verdict, and the buttons that turn
//!   it, play it and put it down. A play is aimed and then confirmed, so the amber preview
//!   can be inspected before it is built.
//!
//! In the lab's palette (`observed_style::architect`). The UI belongs to the board's
//! camera, which draws over the world; `readout` keeps what it says current.

use bevy::prelude::*;
use bevy::ui::UiTargetCamera;
use observed_style::architect::{Role, color};

use super::board::BoardCamera;
use super::cards::{CardArt, HAND};
use crate::GameState;

/// The desk's measures, in pixels, which the board frames itself inside.
pub(super) const TOP_BAR: f32 = 56.0;
pub(super) const PANEL_WIDTH: f32 = 250.0;
pub(super) const CARD_PANEL_WIDTH: f32 = 270.0;
pub(super) const GAP: f32 = 16.0;
const CARD: Vec2 = Vec2::new(176.0, 176.0);
/// Under the cards: the line of controls.
const STRIP: f32 = 28.0;
pub(super) const HAND_HEIGHT: f32 = CARD.y + STRIP + 14.0;
/// A card's miniature, and the card panel's larger one, in the image's proportions.
const ART: Vec2 = Vec2::new(156.0, 98.0);
const PANEL_ART: Vec2 = Vec2::new(234.0, 158.0);

#[derive(Component)]
pub(super) struct DeskUi;

/// A text the desk keeps current.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Line {
    Team,
    Phase,
    Floor,
    Observers,
    Message,
    CardName,
    CardDistrict,
    CardWhere,
    Verdict,
}

/// One card's place in the hand.
#[derive(Component)]
pub(super) struct Slot(pub(super) usize);

/// A text on a card.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CardLine {
    Name,
    District,
    Detail,
}

/// What a desk button does; `input` acts on the press.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DeskButton {
    FloorDown,
    FloorUp,
    TurnLeft,
    TurnRight,
    Play,
    Cancel,
}

#[derive(Component)]
pub(super) struct CardPanel;

#[derive(Component)]
pub(super) struct CardPanelArt;

#[derive(Component)]
pub(super) struct PlayLabel;

/// A button's text, which names the key or the controller button (`words::button_label`).
#[derive(Component)]
pub(super) struct ButtonLabel(pub(super) DeskButton);

/// The line of controls under the hand.
#[derive(Component)]
pub(super) struct ControlStrip;

pub(super) fn spawn(
    mut commands: Commands,
    camera: Query<Entity, With<BoardCamera>>,
    art: Option<Res<CardArt>>,
    existing: Query<(), With<DeskUi>>,
) {
    let (Ok(camera), Some(art)) = (camera.single(), art) else {
        return;
    };
    if !existing.is_empty() {
        return;
    }
    commands
        .spawn((
            DeskUi,
            UiTargetCamera(camera),
            DespawnOnExit(GameState::HexWfc),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            top_bar(root);
            side_panel(root);
            hand(root, &art);
            card_panel(root, &art);
            root.spawn((
                Line::Message,
                Text::new(""),
                text_font(15.0),
                TextColor(color(Role::Guardian)),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(TOP_BAR + 14.0),
                    left: px(PANEL_WIDTH + 24.0),
                    ..default()
                },
            ));
        });
}

fn text_font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

fn label(text: &str, size: f32, role: Role) -> impl Bundle {
    (Text::new(text), text_font(size), TextColor(color(role)))
}

/// A bordered button saying `text`.
fn button(parent: &mut ChildSpawnerCommands, action: DeskButton, grow: bool) {
    let text = super::words::button_label(action, false);
    parent
        .spawn((
            action,
            Button,
            Node {
                padding: UiRect::axes(px(12), px(8)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                justify_content: JustifyContent::Center,
                flex_grow: if grow { 1.0 } else { 0.0 },
                ..default()
            },
            BackgroundColor(color(Role::Card)),
            BorderColor::all(color(Role::Border)),
        ))
        .with_children(|button| {
            let mut text = button.spawn((ButtonLabel(action), label(text, 13.0, Role::Text)));
            if action == DeskButton::Play {
                text.insert(PlayLabel);
            }
        });
}

fn top_bar(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            height: px(TOP_BAR),
            padding: UiRect::horizontal(px(20)),
            align_items: AlignItems::Center,
            column_gap: px(28),
            border: UiRect::bottom(px(1)),
            ..default()
        },
        BackgroundColor(color(Role::Panel)),
        BorderColor::all(color(Role::Border)),
    ))
    .with_children(|bar| {
        bar.spawn(Node {
            width: px(PANEL_WIDTH - 48.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|name| {
            name.spawn(label("ARCHITECT", 22.0, Role::Text));
            name.spawn((Line::Team, label("", 12.0, Role::Muted)));
        });
        // A fixed width, so the floor switcher does not move as the charge counts down.
        bar.spawn((
            Line::Phase,
            label("", 14.0, Role::Valid),
            TextLayout::no_wrap(),
            Node {
                width: px(190),
                ..default()
            },
        ));
        // The floor switcher, centred in what is left.
        bar.spawn(Node {
            flex_grow: 1.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: px(10),
            ..default()
        })
        .with_children(|switcher| {
            button(switcher, DeskButton::FloorDown, false);
            switcher.spawn((
                Line::Floor,
                label("", 13.0, Role::Text),
                TextLayout::justify(Justify::Center),
                Node {
                    min_width: px(220),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
            ));
            button(switcher, DeskButton::FloorUp, false);
        });
        bar.spawn(Node {
            width: px(PANEL_WIDTH - 48.0),
            ..default()
        });
    });
}

fn side_panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(TOP_BAR),
            bottom: px(0),
            width: px(PANEL_WIDTH),
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            padding: UiRect::all(px(18)),
            border: UiRect::right(px(1)),
            ..default()
        },
        BackgroundColor(color(Role::Panel)),
        BorderColor::all(color(Role::Border)),
    ))
    .with_children(|panel| {
        panel.spawn(label("THE TEAM", 12.0, Role::Muted));
        panel.spawn((Line::Observers, label("", 13.0, Role::Text)));
        panel.spawn((
            label("THE CLIMB  (click a floor)", 12.0, Role::Muted),
            Node {
                margin: UiRect::top(px(10)),
                ..default()
            },
        ));
        // Every floor at once: the stack's camera draws into this space.
        panel.spawn((
            super::stack::StackSpace,
            Node {
                width: percent(100),
                height: px(250),
                ..default()
            },
        ));
        panel.spawn((
            label(
                "KEY\nCyan eye     Observer\nRed pyramid  Guardian\nGreen ring   can build\nAmber        your play\nRed ring     contradiction\nViolet ring  prison lobby\nChevron      stair or ramp",
                12.0,
                Role::Muted,
            ),
            Node {
                margin: UiRect::top(Val::Auto),
                ..default()
            },
        ));
    });
}

fn hand(root: &mut ChildSpawnerCommands, art: &CardArt) {
    // The hand's own table, so the board ends where the hand begins.
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(PANEL_WIDTH),
            right: px(0),
            bottom: px(0),
            height: px(HAND_HEIGHT),
            border: UiRect::top(px(1)),
            ..default()
        },
        BackgroundColor(color(Role::Panel)),
        BorderColor::all(color(Role::Border)),
    ));
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: px(PANEL_WIDTH),
        right: px(0),
        bottom: px(STRIP),
        justify_content: JustifyContent::Center,
        column_gap: px(14),
        ..default()
    })
    .with_children(|row| {
        for index in 0..HAND {
            row.spawn((
                Slot(index),
                Button,
                Node {
                    width: px(CARD.x),
                    height: px(CARD.y),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    padding: UiRect::all(px(10)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BackgroundColor(color(Role::Card)),
                BorderColor::all(color(Role::Border)),
            ))
            .with_children(|card| {
                card.spawn(Node {
                    column_gap: px(8),
                    ..default()
                })
                .with_children(|head| {
                    head.spawn(label(&format!("{}", index + 1), 15.0, Role::Muted));
                    head.spawn((CardLine::Name, label("", 15.0, Role::Text)));
                });
                card.spawn((CardLine::District, label("", 11.0, Role::Muted)));
                // The tile itself, drawn by the card's own camera (`cards`).
                card.spawn((
                    ImageNode::new(art.images[index].clone()),
                    Node {
                        width: px(ART.x),
                        height: px(ART.y),
                        margin: UiRect::vertical(px(2)),
                        ..default()
                    },
                ));
                card.spawn((CardLine::Detail, label("", 11.0, Role::Muted)));
            });
        }
    });
    root.spawn((
        ControlStrip,
        label(super::words::controls(false), 12.0, Role::Muted),
        Node {
            position_type: PositionType::Absolute,
            left: px(PANEL_WIDTH),
            right: px(0),
            bottom: px(7),
            justify_content: JustifyContent::Center,
            ..default()
        },
        TextLayout::justify(Justify::Center),
    ));
}

fn card_panel(root: &mut ChildSpawnerCommands, art: &CardArt) {
    root.spawn((
        CardPanel,
        Node {
            position_type: PositionType::Absolute,
            right: px(GAP),
            top: px(TOP_BAR + GAP),
            width: px(CARD_PANEL_WIDTH),
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            padding: UiRect::all(px(18)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(color(Role::Panel)),
        BorderColor::all(color(Role::Border)),
        Visibility::Hidden,
    ))
    .with_children(|panel| {
        panel.spawn((Line::CardName, label("", 22.0, Role::Text)));
        panel.spawn((Line::CardDistrict, label("", 12.0, Role::Muted)));
        panel.spawn((
            CardPanelArt,
            ImageNode::new(art.images[0].clone()),
            Node {
                width: px(PANEL_ART.x),
                height: px(PANEL_ART.y),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
        ));
        panel.spawn((Line::CardWhere, label("", 13.0, Role::Muted)));
        panel.spawn((Line::Verdict, label("", 15.0, Role::Valid)));
        panel
            .spawn(Node {
                column_gap: px(8),
                ..default()
            })
            .with_children(|turn| {
                button(turn, DeskButton::TurnLeft, true);
                button(turn, DeskButton::TurnRight, true);
            });
        button(panel, DeskButton::Play, false);
        button(panel, DeskButton::Cancel, false);
        panel.spawn(label(
            "Aim at a cell, inspect the amber preview, then play the card.",
            12.0,
            Role::Muted,
        ));
    });
}
