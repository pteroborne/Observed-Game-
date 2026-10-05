//! What the Architect's desk says: every text, card and button of the desk (`desk`)
//! brought up to date with the rules and the desk's state each frame.

use bevy::prelude::*;
use observed_match::ascent::sim::{ArchitectCommand, CardKind, ObserverState, floor_title};
use observed_style::architect::{Role, color};

use super::ArchitectDesk;
use super::cards::CardArt;
use super::desk::{
    ButtonLabel, CardLine, CardPanel, CardPanelArt, ControlStrip, DeskButton, Line, PlayLabel, Slot,
};
use super::words;
use crate::hex_wfc::sim::HexWfcRuntime;

type Lines<'w, 's> = Query<'w, 's, (&'static Line, &'static mut Text, &'static mut TextColor)>;
type Cards<'w, 's> = Query<
    'w,
    's,
    (
        &'static Slot,
        &'static mut BorderColor,
        &'static mut BackgroundColor,
        &'static mut UiTransform,
        &'static mut Visibility,
    ),
>;
type CardLines<'w, 's> =
    Query<'w, 's, (&'static CardLine, &'static ChildOf, &'static mut Text), Without<Line>>;
type Buttons<'w, 's> = Query<
    'w,
    's,
    (
        &'static DeskButton,
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    Without<Slot>,
>;
type PanelArt<'w, 's> = Query<'w, 's, &'static mut ImageNode, With<CardPanelArt>>;
type PlayLabels<'w, 's> = Query<'w, 's, &'static mut TextColor, (With<PlayLabel>, Without<Line>)>;
type Panel<'w, 's> = Query<
    'w,
    's,
    (&'static mut Visibility, &'static mut BorderColor),
    (With<CardPanel>, Without<Slot>, Without<DeskButton>),
>;

#[allow(clippy::too_many_arguments)]
pub(super) fn sync(
    desk: Res<ArchitectDesk>,
    runtime: Res<HexWfcRuntime>,
    art: Res<CardArt>,
    mut lines: Lines,
    mut cards: Cards,
    (mut card_lines, slots, parents): (CardLines, Query<&Slot>, Query<&ChildOf>),
    (mut buttons, mut panel, mut panel_art, mut play_label): (Buttons, Panel, PanelArt, PlayLabels),
) {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let rules = ascent.rules();
    let Some(hand) = desk.hand(ascent.session()) else {
        return;
    };
    let lifted = desk
        .selected
        .and_then(|index| Some((index, *hand.deck.hand.get(index)?)));
    // The rules' verdict on the play being made, where it is aimed or pointed.
    let refusal = lifted.zip(desk.focus()).map(|((_, card), target)| {
        ascent.session().architect_refusal(
            desk.seat,
            ArchitectCommand::Play {
                card: card.id,
                target,
                rotation: desk.rotation,
            },
        )
    });
    let playable = desk.aimed.is_some() && refusal == Some(None);
    for (line, mut text, mut tint) in &mut lines {
        let (said, role) = match line {
            // By a Guardian or a sensor.
            Line::Heading if desk.rogue => ("DETECTED".to_owned(), Role::Muted),
            Line::SeatName => (
                (if desk.rogue { "ROGUE" } else { "ARCHITECT" }).to_owned(),
                Role::Text,
            ),
            Line::Heading => ("THE TEAM".to_owned(), Role::Muted),
            Line::Team if desk.rogue => ("ROGUE AI".to_owned(), Role::Guardian),
            Line::Team => (format!("TEAM {}", desk.team.0 + 1), Role::Muted),
            Line::Phase => (
                words::cooldown(hand.cooldown),
                if hand.cooldown == 0 {
                    Role::Valid
                } else {
                    Role::Muted
                },
            ),
            Line::Floor => (
                format!(
                    "{:02} / {:02}\n{}",
                    desk.floor + 1,
                    rules.world.config.levels,
                    floor_title(desk.floor, rules.world.config.levels),
                ),
                Role::Text,
            ),
            Line::Pressure => {
                let pressure = rules.economy.disturbance(desk.floor);
                let warning = rules
                    .next_retraction()
                    .filter(|cell| cell.level == desk.floor)
                    .and(rules.next_retraction_tick)
                    .map(|due| {
                        format!(
                            "  /  RETRACTS IN {:.1} S",
                            due.saturating_sub(rules.tick) as f64 / 60.0
                        )
                    })
                    .unwrap_or_default();
                (
                    format!("FLOOR PRESSURE {pressure} / 100{warning}"),
                    Role::Guardian,
                )
            }
            Line::Observers => (
                rules
                    .observers
                    .values()
                    .filter(|observer| {
                        desk.knowledge(rules)
                            .is_some_and(|knowledge| desk.shows(observer, knowledge))
                    })
                    .map(|observer| {
                        let doing = match observer.state {
                            ObserverState::Active => {
                                format!("floor {}", observer.cell.level + 1)
                            }
                            ObserverState::Jailed => "in the prison".to_owned(),
                            ObserverState::Corrupted => "Rogue operator".to_owned(),
                        };
                        format!("EYE {:02}   {doing}", observer.id.0 + 1)
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                Role::Text,
            ),
            Line::Requests => {
                let requests = super::requests::team_requests(ascent.session(), &desk);
                if requests.is_empty() {
                    ("None.".to_owned(), Role::Muted)
                } else {
                    (
                        requests
                            .iter()
                            .take(3)
                            .map(|request| super::requests::line(request, rules.tick))
                            .collect::<Vec<_>>()
                            .join("\n"),
                        Role::Text,
                    )
                }
            }
            Line::Message => {
                let detail = desk.last_refusal.map_or_else(
                    || {
                        if desk.rogue {
                            words::rogue_orders(rules)
                        } else {
                            String::new()
                        }
                    },
                    |refusal| {
                        format!(
                            "The rules refused that play: {}.",
                            words::refusal_words(refusal)
                        )
                    },
                );
                (
                    if desk.rogue {
                        format!(
                            "You now play for the Rogue AI. Jail every remaining loyal Observer to win.\n{detail}"
                        )
                    } else {
                        detail
                    },
                    Role::Guardian,
                )
            }
            Line::CardName => (
                lifted.map_or_else(String::new, |(_, card)| {
                    words::card_name(card.kind).to_owned()
                }),
                Role::Text,
            ),
            Line::CardDistrict => (
                lifted.map_or_else(String::new, |(_, card)| {
                    format!(
                        "{}  /  {}",
                        card.district
                            .map_or("any floor", |district| district.label()),
                        words::card_detail(card.kind)
                    )
                }),
                Role::Muted,
            ),
            Line::CardWhere => (
                match desk.focus() {
                    Some(cell) => {
                        let pointing = if desk.aimed.is_some() {
                            ""
                        } else {
                            "  (pointing)"
                        };
                        let orientation = if lifted.is_some_and(|(_, c)| c.kind == CardKind::Stair)
                        {
                            let (heading, turn, exit) =
                                observed_facility::hex_wfc::stair_shape(desk.rotation);
                            let heading_idx = observed_hex::HexFace::LATERAL
                                .iter()
                                .position(|&f| f == heading)
                                .unwrap_or(0)
                                + 1;
                            let shape_name = stair_turn_description(turn, exit);
                            format!("Heading {heading_idx} / 6  /  {shape_name} [T]")
                        } else {
                            format!("Orientation {} / 6", (desk.rotation % 6) + 1)
                        };
                        format!(
                            "Floor {:02}  /  cell {}, {}{pointing}\n{orientation}",
                            cell.level + 1,
                            cell.q,
                            cell.r,
                        )
                    }
                    None => {
                        let orientation = if lifted.is_some_and(|(_, c)| c.kind == CardKind::Stair)
                        {
                            let (heading, turn, exit) =
                                observed_facility::hex_wfc::stair_shape(desk.rotation);
                            let heading_idx = observed_hex::HexFace::LATERAL
                                .iter()
                                .position(|&f| f == heading)
                                .unwrap_or(0)
                                + 1;
                            let shape_name = stair_turn_description(turn, exit);
                            format!("Heading {heading_idx} / 6  /  {shape_name} [T]")
                        } else {
                            format!("Orientation {} / 6", (desk.rotation % 6) + 1)
                        };
                        format!("No cell aimed\n{orientation}")
                    }
                },
                Role::Muted,
            ),
            Line::Verdict => match refusal {
                None => ("Point at a cell to aim.".to_owned(), Role::Muted),
                Some(None) if desk.aimed.is_none() => {
                    ("Legal here: aim to play.".to_owned(), Role::Valid)
                }
                Some(verdict) => (
                    words::verdict(verdict),
                    if verdict.is_none() {
                        Role::Valid
                    } else {
                        Role::Guardian
                    },
                ),
            },
        };
        if **text != said {
            **text = said;
        }
        tint.0 = color(role);
    }

    for (slot, mut border, mut fill, mut transform, mut visibility) in &mut cards {
        *visibility = if hand.deck.hand.get(slot.0).is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let up = desk.selected == Some(slot.0);
        *border = BorderColor::all(color(if up { Role::Selected } else { Role::Border }));
        fill.0 = color(if up { Role::Hover } else { Role::Card });
        transform.translation = Val2::px(0.0, if up { -10.0 } else { 0.0 });
    }
    for (line, parent, mut text) in &mut card_lines {
        // A card's texts sit on the card or on its heading row.
        let slot = slots.get(parent.parent()).ok().or_else(|| {
            parents
                .get(parent.parent())
                .ok()
                .and_then(|row| slots.get(row.parent()).ok())
        });
        let Some(card) = slot.and_then(|slot| hand.deck.hand.get(slot.0)) else {
            continue;
        };
        let said = match line {
            CardLine::Name => words::card_name(card.kind).to_owned(),
            CardLine::District => card
                .district
                .map_or("any floor", |district| district.label())
                .to_ascii_uppercase(),
            CardLine::Detail => words::card_detail(card.kind).to_owned(),
        };
        if **text != said {
            **text = said;
        }
    }

    for (mut visibility, mut border) in &mut panel {
        *visibility = if lifted.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        *border = BorderColor::all(color(if playable {
            Role::Selected
        } else {
            Role::Border
        }));
    }
    if let Some((index, _)) = lifted {
        for mut image in &mut panel_art {
            if image.image != art.images[index] {
                image.image = art.images[index].clone();
            }
        }
    }
    for (action, interaction, mut fill, mut border) in &mut buttons {
        let hot = *interaction != Interaction::None;
        let (back, edge) = match action {
            // PLAY lights amber only when there is a play the rules would take.
            DeskButton::Play if playable => (Role::Selected, Role::Selected),
            DeskButton::Play => (Role::Card, Role::Border),
            _ if hot => (Role::Hover, Role::Muted),
            _ => (Role::Card, Role::Border),
        };
        fill.0 = color(back);
        *border = BorderColor::all(color(edge));
    }
    for mut tint in &mut play_label {
        tint.0 = color(if playable {
            Role::Background
        } else {
            Role::Muted
        });
    }
}

/// Name the keys or the controller's buttons, whichever the last hand on the desk used,
/// and take out what only a team's Architect has at the Rogue board.
pub(super) fn prompts(
    desk: Res<ArchitectDesk>,
    mut labels: Query<(&ButtonLabel, &mut Text)>,
    mut strip: Query<&mut Text, (With<ControlStrip>, Without<ButtonLabel>)>,
    mut team_only: Query<&mut Node, With<super::desk::TeamOnly>>,
) {
    let display = if desk.rogue {
        Display::None
    } else {
        Display::Flex
    };
    for mut node in &mut team_only {
        if node.display != display {
            node.display = display;
        }
    }
    for (label, mut text) in &mut labels {
        let said = words::button_label(label.0, desk.pad);
        if **text != said {
            said.clone_into(&mut **text);
        }
    }
    for mut text in &mut strip {
        let said = words::controls(desk.pad, desk.rogue);
        if **text != said {
            said.clone_into(&mut **text);
        }
    }
}

fn stair_turn_description(
    turn: observed_facility::hex_wfc::ClimbTurn,
    exit: observed_facility::hex_wfc::ClimbTurn,
) -> &'static str {
    use observed_facility::hex_wfc::ClimbTurn;
    match (turn, exit) {
        (ClimbTurn::Ahead, ClimbTurn::Ahead) => "Straight",
        (ClimbTurn::Ahead, ClimbTurn::Left) => "Straight, exit left",
        (ClimbTurn::Ahead, ClimbTurn::Right) => "Straight, exit right",
        (ClimbTurn::Ahead, ClimbTurn::Back) => "Switchback",
        (ClimbTurn::Left, ClimbTurn::Ahead) => "Left bend",
        (ClimbTurn::Left, ClimbTurn::Left) => "Left bend, exit left",
        (ClimbTurn::Left, ClimbTurn::Right) => "Left bend, exit right",
        (ClimbTurn::Left, ClimbTurn::Back) => "Left winder",
        (ClimbTurn::Right, ClimbTurn::Ahead) => "Right bend",
        (ClimbTurn::Right, ClimbTurn::Left) => "Right bend, exit left",
        (ClimbTurn::Right, ClimbTurn::Right) => "Right bend, exit right",
        (ClimbTurn::Right, ClimbTurn::Back) => "Right winder",
        (ClimbTurn::SharpLeft, ClimbTurn::Ahead) => "Sharp left bend",
        (ClimbTurn::SharpLeft, ClimbTurn::Left) => "Sharp left, exit left",
        (ClimbTurn::SharpLeft, ClimbTurn::Right) => "Sharp left, exit right",
        (ClimbTurn::SharpLeft, ClimbTurn::Back) => "Sharp left winder",
        (ClimbTurn::SharpRight, ClimbTurn::Ahead) => "Sharp right bend",
        (ClimbTurn::SharpRight, ClimbTurn::Left) => "Sharp right, exit left",
        (ClimbTurn::SharpRight, ClimbTurn::Right) => "Sharp right, exit right",
        (ClimbTurn::SharpRight, ClimbTurn::Back) => "Sharp right winder",
        _ => "Turned stair",
    }
}
