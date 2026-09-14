//! The Architect's physical-looking five-card hand.

use bevy::prelude::*;
use observed_style::{SchematicRole, TacticsRole};
use observed_ui::theme::{ChromeRole, chrome};

use super::{
    CARD_ART_SIZE, CardAccent, CardArtArm, CardArtCore, CardArtFrame, CardArtMotif,
    CardArtMotifKind, CardButton, CardText, CardTextField, DynamicText, HandDock,
};

pub(super) fn spawn_hand(root: &mut ChildSpawnerCommands) {
    root.spawn((
        HandDock,
        Node {
            position_type: PositionType::Absolute,
            left: px(304.0),
            right: px(0.0),
            bottom: px(0.0),
            height: px(270.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(px(18.0), px(10.0)),
            border: UiRect::top(px(2.0)),
            row_gap: px(7.0),
            ..default()
        },
        BackgroundColor(chrome(ChromeRole::Surface)),
        BorderColor::all(chrome(ChromeRole::Border)),
        Name::new("Architect card hand"),
    ))
    .with_children(|dock| {
        dock.spawn((
            Node {
                width: percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::End,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|header| {
            header.spawn((
                Text::new("ROGUE'S HAND"),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(observed_style::schematic(SchematicRole::Selected).base_color),
            ));
            header.spawn((
                DynamicText::HandStatus,
                Text::new("5 CARDS / DRAW AFTER PLAY"),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(chrome(ChromeRole::TextDim)),
            ));
        });
        dock.spawn((
            Node {
                width: percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Start,
                justify_content: JustifyContent::Center,
                column_gap: px(8.0),
                ..default()
            },
            Name::new("Five card fan"),
        ))
        .with_children(|hand| {
            for index in 0..5 {
                spawn_card(hand, index);
            }
        });
    });
}

fn spawn_card(hand: &mut ChildSpawnerCommands, index: usize) {
    hand.spawn((
        CardButton(index),
        Button,
        Node {
            width: percent(18.0),
            min_width: px(146.0),
            max_width: px(202.0),
            height: px(202.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::new(px(15.0), px(11.0), px(11.0), px(10.0)),
            margin: UiRect::top(px(12.0)),
            border: UiRect::all(px(3.0)),
            border_radius: BorderRadius::all(px(14.0)),
            row_gap: px(4.0),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(chrome(ChromeRole::Control)),
        BorderColor::all(chrome(ChromeRole::Border)),
        UiTransform::IDENTITY,
        Name::new(format!("Card slot {}", index + 1)),
    ))
    .with_children(|card| {
        card.spawn((
            CardAccent(index),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                bottom: px(0.0),
                width: px(6.0),
                ..default()
            },
            BackgroundColor(observed_style::tactics(TacticsRole::DevGrid).base_color),
            Pickable::IGNORE,
        ));
        card.spawn((
            Node {
                width: percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|top| {
            top.spawn((
                CardText {
                    index,
                    field: CardTextField::District,
                },
                Text::new("DISTRICT"),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(chrome(ChromeRole::TextDim)),
            ));
            top.spawn((
                CardAccent(index),
                Node {
                    width: px(27.0),
                    height: px(27.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(px(2.0)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(chrome(ChromeRole::Surface)),
                BorderColor::all(observed_style::tactics(TacticsRole::DevGrid).base_color),
                Pickable::IGNORE,
            ))
            .with_children(|pip| {
                pip.spawn((
                    Text::new(format!("{}", index + 1)),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(chrome(ChromeRole::TextMain)),
                    Pickable::IGNORE,
                ));
            });
        });
        card.spawn((
            CardText {
                index,
                field: CardTextField::Title,
            },
            Text::new("JUNCTION"),
            TextFont {
                font_size: FontSize::Px(17.0),
                ..default()
            },
            TextColor(chrome(ChromeRole::TextMain)),
            Pickable::IGNORE,
        ));
        spawn_card_art(card, index);
        card.spawn((
            CardText {
                index,
                field: CardTextField::Meta,
            },
            Text::new("3 BRANCHES"),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(chrome(ChromeRole::TextDim)),
            Pickable::IGNORE,
        ));
    });
}

fn spawn_card_art(card: &mut ChildSpawnerCommands, index: usize) {
    card.spawn((
        Node {
            width: px(CARD_ART_SIZE.x),
            height: px(CARD_ART_SIZE.y),
            align_self: AlignSelf::Center,
            position_type: PositionType::Relative,
            border: UiRect::all(px(1.0)),
            border_radius: BorderRadius::all(px(10.0)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(chrome(ChromeRole::Surface).with_alpha(0.72)),
        BorderColor::all(chrome(ChromeRole::Border)),
        Pickable::IGNORE,
        Name::new(format!("Card {} tile illustration", index + 1)),
    ))
    .with_children(|art| {
        spawn_hex_frame(art, index);
        spawn_district_motifs(art, index);
        for face in 0..6 {
            let world_angle = -(face as f32) * std::f32::consts::TAU / 6.0;
            let direction = Vec2::new(world_angle.cos(), -world_angle.sin());
            art.spawn((
                CardArtArm { index, face },
                Node {
                    position_type: PositionType::Absolute,
                    left: px(CARD_ART_SIZE.x * 0.5 + direction.x * 17.0 - 17.0),
                    top: px(CARD_ART_SIZE.y * 0.5 + direction.y * 17.0 - 2.0),
                    width: px(34.0),
                    height: px(4.0),
                    border_radius: BorderRadius::all(px(2.0)),
                    ..default()
                },
                BackgroundColor(observed_style::tactics(TacticsRole::DevGrid).base_color),
                UiTransform::from_rotation(Rot2::radians(-world_angle)),
                Pickable::IGNORE,
            ));
        }
        art.spawn((
            CardArtCore(index),
            Node {
                position_type: PositionType::Absolute,
                left: px(CARD_ART_SIZE.x * 0.5 - 10.0),
                top: px(CARD_ART_SIZE.y * 0.5 - 10.0),
                width: px(20.0),
                height: px(20.0),
                border: UiRect::all(px(3.0)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(chrome(ChromeRole::Control)),
            BorderColor::all(observed_style::tactics(TacticsRole::DevGrid).base_color),
            Pickable::IGNORE,
        ));
    });
}

fn spawn_hex_frame(art: &mut ChildSpawnerCommands, index: usize) {
    for edge in 0..6 {
        let angle = edge as f32 * std::f32::consts::TAU / 6.0;
        let direction = Vec2::new(angle.cos(), angle.sin());
        art.spawn((
            CardArtFrame(index),
            Node {
                position_type: PositionType::Absolute,
                left: px(CARD_ART_SIZE.x * 0.5 + direction.x * 25.0 - 14.0),
                top: px(CARD_ART_SIZE.y * 0.5 + direction.y * 25.0 - 1.0),
                width: px(28.0),
                height: px(2.0),
                border_radius: BorderRadius::all(px(1.0)),
                ..default()
            },
            BackgroundColor(observed_style::tactics(TacticsRole::DevGrid).base_color),
            UiTransform::from_rotation(Rot2::radians(angle + std::f32::consts::FRAC_PI_2)),
            Pickable::IGNORE,
        ));
    }
}

fn spawn_district_motifs(art: &mut ChildSpawnerCommands, index: usize) {
    for offset in [-13.0, 0.0, 13.0] {
        spawn_motif(
            art,
            index,
            CardArtMotifKind::Institutional,
            Vec2::new(CARD_ART_SIZE.x * 0.5 + offset - 1.5, 20.0),
            Vec2::new(3.0, 32.0),
            1.5,
        );
    }
    for row in 0..2 {
        for column in 0..3 {
            spawn_motif(
                art,
                index,
                CardArtMotifKind::LiminalGrid,
                Vec2::new(36.0 + column as f32 * 13.0, 23.0 + row as f32 * 19.0),
                Vec2::splat(7.0),
                2.0,
            );
        }
    }
    for (position, size) in [
        (Vec2::new(34.0, 16.0), Vec2::new(5.0, 40.0)),
        (Vec2::new(65.0, 16.0), Vec2::new(5.0, 40.0)),
        (Vec2::new(34.0, 33.0), Vec2::new(36.0, 6.0)),
    ] {
        spawn_motif(art, index, CardArtMotifKind::Door, position, size, 2.0);
    }
}

fn spawn_motif(
    art: &mut ChildSpawnerCommands,
    index: usize,
    kind: CardArtMotifKind,
    position: Vec2,
    size: Vec2,
    radius: f32,
) {
    art.spawn((
        CardArtMotif { index, kind },
        Node {
            position_type: PositionType::Absolute,
            left: px(position.x),
            top: px(position.y),
            width: px(size.x),
            height: px(size.y),
            border_radius: BorderRadius::all(px(radius)),
            ..default()
        },
        BackgroundColor(observed_style::tactics(TacticsRole::DevGrid).base_color),
        Pickable::IGNORE,
    ));
}
