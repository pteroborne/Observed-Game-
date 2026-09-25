//! Seat-specific chrome. Each chair gets only the controls it can actually use,
//! which is the clearest possible statement of what the roles are.

use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui::{percent, px};
use observed_mechanics::tiles::TileShape;

use crate::seat::{Seat, Session};

use super::board::Fog;

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct SeatText;

#[derive(Component)]
pub struct LegendPanel;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub enum Control {
    Submit,
    Restart,
    SwapSeat,
    Legend,
    // Operator
    Next,
    TurnLeft,
    TurnRight,
    Hold,
    Plant,
    // Architect
    Rotate,
    Clear,
}

impl Control {
    const fn label(self) -> &'static str {
        match self {
            Control::Submit => "Submit",
            Control::Restart => "Restart",
            Control::SwapSeat => "Swap seat",
            Control::Legend => "Legend",
            Control::Next => "Next",
            Control::TurnLeft => "< Turn",
            Control::TurnRight => "Turn >",
            Control::Hold => "Hold",
            Control::Plant => "Plant",
            Control::Rotate => "Rotate",
            Control::Clear => "Clear",
        }
    }

    const fn seat(self) -> Option<Seat> {
        match self {
            Control::Next
            | Control::TurnLeft
            | Control::TurnRight
            | Control::Hold
            | Control::Plant => Some(Seat::Operator),
            Control::Rotate | Control::Clear => Some(Seat::Architect),
            _ => None,
        }
    }
}

/// A card in the architect's hand.
#[derive(Component, Clone, Copy)]
pub struct HandCard(pub usize);

#[derive(Component)]
pub struct HandRow;

const INK: Color = Color::srgb(0.92, 0.95, 0.98);
const PANEL: Color = Color::srgba(0.02, 0.03, 0.045, 0.94);

pub fn spawn(mut commands: Commands) {
    commands
        .spawn((
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
                    SeatText,
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Px(15.0),
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

            root.spawn((
                LegendPanel,
                Node {
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(10.0)),
                    row_gap: px(3.0),
                    ..default()
                },
                BackgroundColor(PANEL),
                Pickable::IGNORE,
            ))
            .with_children(|legend| {
                for fog in Fog::ALL {
                    legend.spawn((
                        Text::new(fog.label().to_string()),
                        TextFont {
                            font_size: FontSize::Px(12.0),
                            ..default()
                        },
                        TextColor(INK),
                        Pickable::IGNORE,
                    ));
                }
            });

            root.spawn((
                Node {
                    width: percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(6.0)),
                    row_gap: px(6.0),
                    ..default()
                },
                BackgroundColor(PANEL),
                Pickable::IGNORE,
            ))
            .with_children(|dock| {
                dock.spawn((
                    HandRow,
                    Node {
                        width: percent(100.0),
                        display: Display::None,
                        flex_direction: FlexDirection::Row,
                        column_gap: px(4.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|row| {
                    for slot in 0..6 {
                        row.spawn((
                            HandCard(slot),
                            Button,
                            Node {
                                flex_grow: 1.0,
                                flex_basis: px(0.0),
                                height: px(46.0),
                                display: Display::None,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.10, 0.13, 0.17)),
                        ))
                        .with_children(|face| {
                            face.spawn((
                                Text::new(""),
                                TextFont {
                                    font_size: FontSize::Px(11.0),
                                    ..default()
                                },
                                TextColor(INK),
                                Pickable::IGNORE,
                            ));
                        });
                    }
                });

                for group in [
                    [Control::TurnLeft, Control::Next, Control::TurnRight],
                    [Control::Hold, Control::Plant, Control::Rotate],
                    [Control::Submit, Control::Clear, Control::Restart],
                    [Control::SwapSeat, Control::Legend, Control::Legend],
                ] {
                    dock.spawn((
                        Node {
                            width: percent(100.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: px(6.0),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|row| {
                        let mut seen: Vec<Control> = Vec::new();
                        for control in group {
                            if seen.contains(&control) {
                                continue;
                            }
                            seen.push(control);
                            control_button(row, control);
                        }
                    });
                }
            });
        });
}

fn control_button(parent: &mut ChildSpawnerCommands, control: Control) {
    parent
        .spawn((
            control,
            Button,
            Node {
                flex_grow: 1.0,
                flex_basis: px(0.0),
                height: px(52.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.10, 0.13, 0.17)),
        ))
        .with_children(|face| {
            face.spawn((
                Text::new(control.label()),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(INK),
                Pickable::IGNORE,
            ));
        });
}

/// The dock's disjoint node queries, named so the signature stays readable.
type HandRowQuery<'w, 's> =
    Query<'w, 's, &'static mut Node, (With<HandRow>, Without<Control>, Without<HandCard>)>;
type HandCardQuery<'w, 's> = Query<
    'w,
    's,
    (&'static HandCard, &'static mut Node, &'static Children),
    (Without<Control>, Without<HandRow>),
>;

/// Show each seat only what it can use.
pub fn sync(
    session: Res<Session>,
    mut seat_text: Query<&mut Text, (With<SeatText>, Without<StatusText>)>,
    mut status: Query<&mut Text, With<StatusText>>,
    mut controls: Query<(&Control, &mut Node, &mut BackgroundColor), Without<HandCard>>,
    mut hand_row: HandRowQuery,
    mut cards: HandCardQuery,
    mut labels: Query<&mut Text, (Without<SeatText>, Without<StatusText>)>,
) {
    if !session.is_changed() {
        return;
    }
    let architect = session.seat == Seat::Architect;
    let state = &session.state;

    if let Ok(mut text) = seat_text.single_mut() {
        **text = format!(
            "{}   turn {}/{}   {}",
            session.seat.label(),
            state.turn,
            session.spec.turn_limit,
            match state.outcome {
                Some(outcome) => format!("{outcome:?}"),
                None => String::new(),
            },
        );
    }
    if let Ok(mut text) = status.single_mut() {
        let known = session.known.known_count();
        let total = state.board.cells().count();
        let picture = if architect {
            format!("whole lattice ({total} cells)")
        } else {
            format!("{known}/{total} cells known")
        };
        let orders = if architect {
            format!("{} tiles placed", session.plays.len())
        } else {
            format!(
                "{}/{} ordered",
                session.queued.len(),
                session.commandable().len()
            )
        };
        **text = format!("{picture}   {orders}\n{}", session.notice);
    }

    for (control, mut node, mut bg) in &mut controls {
        node.display = match control.seat() {
            Some(seat) if seat != session.seat => Display::None,
            _ => Display::Flex,
        };
        bg.0 = Color::srgb(0.10, 0.13, 0.17);
    }

    if let Ok(mut node) = hand_row.single_mut() {
        node.display = if architect {
            Display::Flex
        } else {
            Display::None
        };
    }
    let hand: Vec<TileShape> = session
        .state
        .hand_of(session.team)
        .map(|hand| hand.cards.clone())
        .unwrap_or_default();
    for (card, mut node, children) in &mut cards {
        let shape = hand.get(card.0).copied();
        node.display = if architect && shape.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if let Some(shape) = shape
            && let Some(&child) = children.first()
            && let Ok(mut text) = labels.get_mut(child)
        {
            **text = shape.label().to_string();
        }
    }
}
