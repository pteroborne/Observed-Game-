//! The Architect's desk: the side panel and the hand, over the board.
//!
//! The panel says who and where the Architect is, whether the hand is charged, where the
//! team's Observers are, and what the rules would say to the play under the cursor. The
//! hand is five cards; each card's glyph is a hub with a spoke for every doorway the tile
//! would have, turned to the rotation it would be played at, so the card shows the tile
//! it will make. The UI belongs to the board's camera, which draws over the world.

use bevy::prelude::*;
use bevy::ui::UiTargetCamera;
use observed_hex::HexFace;
use observed_match::ascent::sim::{ArchitectCommand, CardKind, ObserverState, floor_title};

use super::ArchitectDesk;
use super::board::BoardCamera;
use super::pick::face_angle;
use super::words;
use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;
use crate::view::theme::{ACCENT, DIM, PANEL, TITLE, WARNING};

/// Cards in a hand.
const HAND: usize = 5;
const PANEL_WIDTH: f32 = 310.0;
const CARD_WIDTH: f32 = 150.0;
const CARD_HEIGHT: f32 = 196.0;
/// The glyph's box, and a spoke's length from its hub.
const GLYPH: f32 = 76.0;
const SPOKE: f32 = 30.0;

#[derive(Component)]
pub(super) struct DeskUi;

/// A text of the panel.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Line {
    Heading,
    Charge,
    Observers,
    Target,
    Message,
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

/// A spoke of a card's glyph, for the doorway on face `.1`.
#[derive(Component)]
pub(super) struct Spoke(usize, usize);

pub(super) fn spawn(
    mut commands: Commands,
    camera: Query<Entity, With<BoardCamera>>,
    existing: Query<(), With<DeskUi>>,
) {
    let Ok(camera) = camera.single() else {
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
            panel(root);
            hand(root);
            root.spawn((
                Line::Message,
                Text::new(""),
                text_font(16.0),
                TextColor(WARNING),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(18),
                    left: px(PANEL_WIDTH + 30.0),
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

fn panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: px(PANEL_WIDTH),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(14),
            padding: UiRect::all(px(20)),
            border: UiRect::right(px(1)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(ACCENT.with_alpha(0.35)),
    ))
    .with_children(|panel| {
        panel.spawn((Text::new("ARCHITECT"), text_font(30.0), TextColor(TITLE)));
        for (line, size, color) in [
            (Line::Heading, 15.0, ACCENT),
            (Line::Charge, 15.0, TITLE),
            (Line::Observers, 14.0, DIM),
        ] {
            panel.spawn((line, Text::new(""), text_font(size), TextColor(color)));
        }
        // Every floor at once: the stack's camera draws into this space.
        panel.spawn((
            Text::new("THE CLIMB  (click a floor)"),
            text_font(12.0),
            TextColor(DIM),
        ));
        panel.spawn((
            super::stack::StackSpace,
            Node {
                width: percent(100),
                height: px(270),
                ..default()
            },
        ));
        panel.spawn((Line::Target, Text::new(""), text_font(15.0), TextColor(TITLE)));
        panel.spawn((
            Text::new(
                "1-5  pick up a card\nQ / E  turn it\nClick  play it here\nRight click  put it down\n[ / ]  floor below / above\nR  emergency requisition",
            ),
            text_font(13.0),
            TextColor(DIM),
            Node {
                margin: UiRect::top(Val::Auto),
                ..default()
            },
        ));
    });
}

fn hand(root: &mut ChildSpawnerCommands) {
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: px(PANEL_WIDTH),
        right: px(0),
        bottom: px(18),
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
                    width: px(CARD_WIDTH),
                    height: px(CARD_HEIGHT),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(6),
                    padding: UiRect::all(px(10)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(DIM.with_alpha(0.5)),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new(format!("{}", index + 1)),
                    text_font(13.0),
                    TextColor(DIM),
                    Node {
                        align_self: AlignSelf::FlexEnd,
                        ..default()
                    },
                ));
                card.spawn((
                    CardLine::Name,
                    Text::new(""),
                    text_font(17.0),
                    TextColor(TITLE),
                ));
                card.spawn((
                    CardLine::District,
                    Text::new(""),
                    text_font(12.0),
                    TextColor(DIM),
                ));
                glyph(card, index);
                card.spawn((
                    CardLine::Detail,
                    Text::new(""),
                    text_font(12.0),
                    TextColor(DIM),
                ));
            });
        }
    });
}

/// A hub, a ring, and a spoke for each of the six faces, shown when the tile opens it.
fn glyph(card: &mut ChildSpawnerCommands, slot: usize) {
    card.spawn(Node {
        width: px(GLYPH),
        height: px(GLYPH),
        ..default()
    })
    .with_children(|glyph| {
        let centre = GLYPH * 0.5;
        glyph.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(centre - 26.0),
                top: px(centre - 26.0),
                width: px(52),
                height: px(52),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor::all(DIM.with_alpha(0.6)),
        ));
        for face in HexFace::LATERAL {
            let angle = face_angle(face);
            let mid = Vec2::new(angle.cos(), angle.sin()) * SPOKE * 0.5;
            glyph.spawn((
                Spoke(slot, face.index()),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(centre + mid.x - SPOKE * 0.5),
                    top: px(centre + mid.y - 2.5),
                    width: px(SPOKE),
                    height: px(5),
                    border_radius: BorderRadius::all(px(2)),
                    ..default()
                },
                UiTransform {
                    rotation: Rot2::radians(angle),
                    ..UiTransform::IDENTITY
                },
                BackgroundColor(ACCENT),
                Visibility::Hidden,
            ));
        }
        glyph.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(centre - 6.0),
                top: px(centre - 6.0),
                width: px(12),
                height: px(12),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(TITLE),
        ));
    });
}

type Lines<'w, 's> = Query<'w, 's, (&'static Line, &'static mut Text)>;
type Cards<'w, 's> = Query<
    'w,
    's,
    (
        &'static Slot,
        &'static mut BorderColor,
        &'static mut UiTransform,
        &'static mut Visibility,
    ),
>;
type CardLines<'w, 's> =
    Query<'w, 's, (&'static CardLine, &'static ChildOf, &'static mut Text), Without<Line>>;
type Spokes<'w, 's> = Query<
    'w,
    's,
    (
        &'static Spoke,
        &'static mut Visibility,
        &'static mut BackgroundColor,
    ),
    Without<Slot>,
>;

pub(super) fn sync(
    desk: Res<ArchitectDesk>,
    runtime: Res<HexWfcRuntime>,
    mut lines: Lines,
    mut cards: Cards,
    mut card_lines: CardLines,
    slots: Query<&Slot>,
    mut spokes: Spokes,
) {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let rules = ascent.rules();
    let Some(hand) = ascent.session().hands.get(&desk.team) else {
        return;
    };
    let selected_card = desk.selected.and_then(|index| hand.deck.hand.get(index));
    for (line, mut text) in &mut lines {
        **text = match line {
            Line::Heading => format!(
                "TEAM {}  /  FLOOR {} OF {}\n{}",
                desk.team.0 + 1,
                desk.floor + 1,
                rules.world.config.levels,
                floor_title(desk.floor).to_ascii_uppercase(),
            ),
            Line::Charge => words::cooldown(hand.cooldown),
            Line::Observers => rules
                .observers
                .values()
                .filter(|observer| observer.team == desk.team)
                .map(|observer| {
                    let doing = match observer.state {
                        ObserverState::Active => format!("floor {}", observer.cell.level + 1),
                        ObserverState::Jailed => "in the prison".to_owned(),
                        ObserverState::Corrupted => "lost to the void".to_owned(),
                    };
                    format!("Observer {}   {doing}", observer.id.0 + 1)
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Line::Target => match (selected_card, desk.hovered) {
                (None, _) => "Pick up a card to build.".to_owned(),
                (Some(_), None) => "Point at a cell.".to_owned(),
                (Some(card), Some(cell)) => format!(
                    "Cell {}, {}\n{}",
                    cell.q,
                    cell.r,
                    words::verdict(ascent.session().architect_refusal(
                        desk.seat,
                        ArchitectCommand::Play {
                            card: card.id,
                            target: cell,
                            rotation: desk.rotation,
                        },
                    ))
                ),
            },
            Line::Message => desk
                .last_refusal
                .map(|refusal| {
                    format!(
                        "The rules refused that play: {}.",
                        words::refusal_words(refusal)
                    )
                })
                .unwrap_or_default(),
        };
    }
    for (slot, mut border, mut transform, mut visibility) in &mut cards {
        let card = hand.deck.hand.get(slot.0);
        *visibility = if card.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let lifted = desk.selected == Some(slot.0);
        *border = BorderColor::all(if lifted { ACCENT } else { DIM.with_alpha(0.5) });
        transform.translation = Val2::px(0.0, if lifted { -14.0 } else { 0.0 });
    }
    for (line, parent, mut text) in &mut card_lines {
        let Some(card) = slots
            .get(parent.parent())
            .ok()
            .and_then(|slot| hand.deck.hand.get(slot.0))
        else {
            continue;
        };
        **text = match line {
            CardLine::Name => words::card_name(card.kind).to_owned(),
            CardLine::District => card
                .district
                .map_or("any floor", |district| district.label())
                .to_ascii_uppercase(),
            CardLine::Detail => words::card_detail(card.kind).to_owned(),
        };
    }
    for (spoke, mut visibility, mut color) in &mut spokes {
        let Some(card) = hand.deck.hand.get(spoke.0) else {
            continue;
        };
        let lifted = desk.selected == Some(spoke.0);
        // A lifted card shows the rotation it would be played at.
        let rotation = if lifted { desk.rotation } else { 0 };
        let open = match card.kind {
            CardKind::Tile(shape) => shape.doors(rotation) & (1 << spoke.1) != 0,
            CardKind::Door => spoke.1 == usize::from(rotation % 6),
        };
        *visibility = if open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        *color = BackgroundColor(if lifted { TITLE } else { ACCENT });
    }
}
