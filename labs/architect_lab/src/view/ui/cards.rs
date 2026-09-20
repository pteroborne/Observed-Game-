//! Five aligned cards, each showing the same 3D model as its board preview.
use super::spawn::{label, row};
use super::{CardButton, CardText, CardTextField, ChargePip, DynamicText, HandDock};
use crate::view::scene::Previews;
use bevy::prelude::*;
use observed_style::architect::{Role, color};
pub(super) fn spawn_hand(root: &mut ChildSpawnerCommands, previews: &Previews) {
    root.spawn((
        HandDock,
        Node {
            position_type: PositionType::Absolute,
            left: px(18.0),
            right: px(18.0),
            bottom: px(0.0),
            height: px(210.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(14.0)),
            row_gap: px(10.0),
            border: UiRect::top(px(1.0)),
            ..default()
        },
        BackgroundColor(color(Role::Panel)),
        BorderColor::all(color(Role::Border)),
        Name::new("Architect card hand"),
    ))
    .with_children(|dock| {
        row(dock, |r| {
            r.spawn((
                DynamicText::Guidance,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(color(Role::Text)),
                Pickable::IGNORE,
            ));
            r.spawn(Node {
                flex_grow: 1.0,
                ..default()
            });
            for i in 0..5 {
                r.spawn((
                    ChargePip(i),
                    Node {
                        width: px(12.0),
                        height: px(4.0),
                        border_radius: BorderRadius::all(px(2.0)),
                        ..default()
                    },
                    BackgroundColor(color(Role::Valid)),
                ));
            }
            r.spawn((
                DynamicText::HandStatus,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(color(Role::Muted)),
            ));
        });
        dock.spawn(Node {
            width: percent(100.0),
            flex_grow: 1.0,
            min_height: px(0.0),
            column_gap: px(16.0),
            ..default()
        })
        .with_children(|hand| {
            for index in 0..5 {
                hand.spawn((
                    CardButton(index),
                    Button,
                    Node {
                        flex_basis: px(0.0),
                        flex_grow: 1.0,
                        min_width: px(0.0),
                        height: percent(100.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::axes(px(12.0), px(8.0)),
                        row_gap: px(3.0),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(6.0)),
                        ..default()
                    },
                    BackgroundColor(color(Role::Card)),
                    BorderColor::all(color(Role::Border)),
                    UiTransform::IDENTITY,
                    Name::new(format!("Card {}", index + 1)),
                ))
                .with_children(|card| {
                    row(card, |r| {
                        label(r, &format!("{}", index + 1), 16.0, Role::Muted);
                        r.spawn((
                            CardText {
                                index,
                                field: CardTextField::Title,
                            },
                            Text::new(""),
                            TextFont {
                                font_size: FontSize::Px(16.0),
                                ..default()
                            },
                            TextColor(color(Role::Text)),
                            Pickable::IGNORE,
                        ));
                    });
                    card.spawn((
                        CardText {
                            index,
                            field: CardTextField::District,
                        },
                        Text::new(""),
                        TextFont {
                            font_size: FontSize::Px(11.0),
                            ..default()
                        },
                        TextColor(color(Role::Muted)),
                        Pickable::IGNORE,
                    ));
                    card.spawn((
                        ImageNode::new(previews.0[index].clone()),
                        Node {
                            width: percent(100.0),
                            flex_grow: 1.0,
                            min_height: px(0.0),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ));
                });
            }
        });
    });
}
