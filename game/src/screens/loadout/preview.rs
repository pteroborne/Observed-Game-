//! Code-drawn, representative cosmetic comparison; no live match state or assets.
use crate::view::theme::{BORDER, DIM, PANEL, TITLE, text};
use bevy::prelude::*;
use observed_progression::progression::{Profile, Slot, cosmetic};
use observed_style::cosmetics::{Colorway, accent};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Look {
    pub color: u16,
    pub trail: u16,
    pub badge: u16,
}
impl Look {
    pub(crate) fn equipped(profile: &Profile) -> Self {
        let equipped = |slot, fallback| profile.equipped.get(&slot).copied().unwrap_or(fallback);
        Self {
            color: equipped(Slot::Color, 0),
            trail: equipped(Slot::Trail, 4),
            badge: equipped(Slot::Badge, 7),
        }
    }
    pub(crate) fn with_item(mut self, id: u16) -> Self {
        if let Some(item) = cosmetic(id) {
            match item.slot {
                Slot::Color => self.color = id,
                Slot::Trail => self.trail = id,
                Slot::Badge => self.badge = id,
            }
        }
        self
    }
    fn colorway(self) -> Colorway {
        match self.color {
            1 => Colorway::Ember,
            2 => Colorway::Cobalt,
            3 => Colorway::Void,
            _ => Colorway::Ash,
        }
    }
}

pub(crate) fn card(parent: &mut ChildSpawnerCommands, heading: &str, look: Look) {
    parent
        .spawn((
            Node {
                width: px(230),
                height: px(320),
                flex_shrink: 0.0,
                padding: UiRect::all(px(10)),
                border: UiRect::all(px(1)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(8),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(BORDER),
        ))
        .with_children(|card| {
            card.spawn(text(heading, 17.0, TITLE));
            card.spawn(Node {
                width: px(208),
                height: px(180),
                position_type: PositionType::Relative,
                ..default()
            })
            .with_children(|canvas| {
                let color = accent(look.colorway());
                // A trail and badge have shape as well as hue; no trail draws nothing.
                let segments = match look.trail {
                    5 => 3,
                    6 => 6,
                    _ => 0,
                };
                for i in 0..segments {
                    canvas.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(10.0 + i as f32 * 16.0),
                            top: px(95.0 + (i % 2) as f32 * 9.0),
                            width: px(if look.trail == 6 { 25.0 } else { 7.0 }),
                            height: px(7),
                            border_radius: BorderRadius::all(px(3)),
                            ..default()
                        },
                        BackgroundColor(color.with_alpha(0.25 + i as f32 * 0.10)),
                    ));
                }
                canvas
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(80),
                            top: px(26),
                            width: px(116),
                            height: px(116),
                            border: UiRect::all(px(5)),
                            border_radius: BorderRadius::all(percent(50)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(
                            observed_style::observer::finish(observed_style::observer::Part::Globe)
                                .base_color,
                        ),
                        BorderColor::all(color),
                    ))
                    .with_children(|eye| {
                        eye.spawn((
                            Node {
                                width: px(64),
                                height: px(64),
                                border_radius: BorderRadius::all(percent(50)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(color),
                        ))
                        .with_children(|iris| {
                            iris.spawn((
                                Node {
                                    width: px(30),
                                    height: px(30),
                                    border_radius: BorderRadius::all(percent(50)),
                                    ..default()
                                },
                                BackgroundColor(
                                    observed_style::observer::finish(
                                        observed_style::observer::Part::Pupil,
                                    )
                                    .base_color,
                                ),
                            ));
                        });
                    });
                canvas
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(147),
                            top: px(125),
                            width: px(48),
                            height: px(40),
                            border: UiRect::all(px(2)),
                            border_radius: BorderRadius::all(px(7)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::End,
                            padding: UiRect::all(px(7)),
                            column_gap: px(4),
                            ..default()
                        },
                        BackgroundColor(PANEL),
                        BorderColor::all(color),
                    ))
                    .with_children(|badge| {
                        let bars = match look.badge {
                            8 | 9 => 3,
                            _ => 1,
                        };
                        for i in 0..bars {
                            badge.spawn((
                                Node {
                                    width: px(7),
                                    height: px(if look.badge == 9 && i == 1 {
                                        24.0
                                    } else if look.badge == 9 {
                                        16.0
                                    } else {
                                        12.0
                                    }),
                                    ..default()
                                },
                                BackgroundColor(color),
                            ));
                        }
                    });
            });
            for (slot, id) in [
                (Slot::Color, look.color),
                (Slot::Trail, look.trail),
                (Slot::Badge, look.badge),
            ] {
                card.spawn(text(
                    format!(
                        "{}: {}",
                        slot.label(),
                        cosmetic(id).map_or("Default", |c| c.name)
                    ),
                    14.0,
                    DIM,
                ));
            }
        });
}
