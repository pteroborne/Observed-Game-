//! Turning taps into whatever this seat is allowed to do.

use bevy::prelude::*;
use observed_hex::faces::HexFace;
use observed_mechanics::state::{Action, Intent, PawnId};
use observed_mechanics::tiles::TilePlay;

use crate::seat::{Seat, Session};

use super::hud::{Control, HandCard, LegendPanel};
use super::{cell_at, world_of};

/// A tap on the board means something different in each chair: the operator is
/// pointing at a pawn or a step, the architect is pointing at a building site.
pub fn board_taps(
    mut session: ResMut<Session>,
    camera: Query<(&Camera, &GlobalTransform)>,
    windows: Query<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    over_ui: Query<&Interaction, With<Button>>,
) {
    if over_ui
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        return;
    }
    let point = touches
        .iter_just_pressed()
        .next()
        .map(|touch| touch.position())
        .or_else(|| {
            mouse
                .just_pressed(MouseButton::Left)
                .then(|| windows.single().ok()?.cursor_position())
                .flatten()
        });
    let Some(point) = point else {
        return;
    };
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    let Ok(world) = camera.viewport_to_world_2d(camera_transform, point) else {
        return;
    };
    let Some(cell) = cell_at(&session.state, world) else {
        return;
    };
    let _ = world_of(&session.state, cell);

    match session.seat {
        Seat::Architect => {
            let Some(shape) = session.selected_tile else {
                session.notice = "pick a tile from your hand first".to_string();
                return;
            };
            let rotation = session.rotation;
            session.plays.clear();
            session.plays.push(TilePlay {
                cell,
                shape,
                rotation,
            });
            session.notice = format!(
                "{} at ({}, {}) rot {rotation} - Submit to commit",
                shape.label(),
                cell.q,
                cell.r
            );
        }
        Seat::Operator => operator_tap(&mut session, cell),
    }
}

fn operator_tap(session: &mut Session, cell: observed_hex::coords::HexCoord) {
    let selected = session
        .selected_pawn
        .filter(|id| !session.state.pawn(*id).jailed);
    let adjacent = selected.and_then(|id| {
        let from = session.state.pawn(id).at;
        HexFace::LATERAL
            .into_iter()
            .find(|&face| session.state.board.size().neighbor(from, face) == Some(cell))
            .map(|face| (id, from, face))
    });

    let Some((selected, from, face)) = adjacent else {
        let mine: Vec<PawnId> = session
            .state
            .occupants(cell)
            .into_iter()
            .filter(|id| session.state.pawn(*id).team == session.team)
            .collect();
        match mine.first() {
            Some(&id) => {
                session.selected_pawn = Some(id);
                session.notice = format!("pawn {} selected", id.0);
            }
            None => session.notice = "tap one of your pawns, then a neighbour".to_string(),
        }
        return;
    };

    let blocked = !session.state.board.passable(from, face);
    let action = if session.face_only || blocked {
        Action::Hold
    } else {
        Action::Step(face)
    };
    session.set_order(Intent {
        pawn: selected,
        facing: face,
        action,
    });
    session.notice = if blocked {
        format!("wall that way - pawn {} faces it", selected.0)
    } else {
        format!("pawn {} steps {face:?}", selected.0)
    };
}

pub fn hand_taps(
    mut session: ResMut<Session>,
    pressed: Query<(&Interaction, &HandCard), Changed<Interaction>>,
) {
    for (interaction, card) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let shape = session
            .state
            .hand_of(session.team)
            .and_then(|hand| hand.cards.get(card.0).copied());
        if let Some(shape) = shape {
            session.selected_tile = Some(shape);
            session.notice = format!("{} selected - tap a cell to place it", shape.label());
        }
    }
}

pub fn controls(
    mut session: ResMut<Session>,
    pressed: Query<(&Interaction, &Control), Changed<Interaction>>,
    mut legend: Query<&mut Node, With<LegendPanel>>,
) {
    for (interaction, control) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match control {
            Control::Submit => {
                let plays = session.plays.clone();
                session.resolve(&[], plays);
                session.notice = "turn resolved".to_string();
            }
            Control::Restart => session.restart(),
            Control::SwapSeat => {
                session.seat = session.seat.other();
                session.selected_pawn = None;
                session.selected_tile = None;
                session.notice = format!("now sitting as {}", session.seat.label());
            }
            Control::Clear => {
                session.plays.clear();
                session.queued.clear();
                session.notice = "orders cleared".to_string();
            }
            Control::Rotate => {
                session.rotation = (session.rotation + 1) % 6;
                let rotation = session.rotation;
                if let Some(play) = session.plays.last_mut() {
                    play.rotation = rotation;
                }
                session.notice = format!("rotation {rotation}");
            }
            Control::Next => next_pawn(&mut session),
            Control::TurnLeft | Control::TurnRight => {
                rotate_pawn(&mut session, *control == Control::TurnLeft);
            }
            Control::Hold | Control::Plant => {
                if let Some(id) = session.selected_pawn {
                    let facing = session.state.pawn(id).facing;
                    let action = if *control == Control::Plant {
                        Action::Plant
                    } else {
                        Action::Hold
                    };
                    session.set_order(Intent {
                        pawn: id,
                        facing,
                        action,
                    });
                    session.notice = format!("pawn {} will {action:?}", id.0);
                }
            }
            Control::Legend => {
                if let Ok(mut node) = legend.single_mut() {
                    node.display = match node.display {
                        Display::None => Display::Flex,
                        _ => Display::None,
                    };
                }
            }
        }
    }
}

fn next_pawn(session: &mut Session) {
    let order = session.commandable();
    if order.is_empty() {
        return;
    }
    let next = match session.selected_pawn {
        Some(current) => {
            let index = order.iter().position(|id| *id == current).unwrap_or(0);
            order[(index + 1) % order.len()]
        }
        None => order[0],
    };
    session.selected_pawn = Some(next);
    session.notice = format!("pawn {} selected", next.0);
}

fn rotate_pawn(session: &mut Session, left: bool) {
    let Some(id) = session.selected_pawn else {
        return;
    };
    let delta = if left { 5 } else { 1 };
    let current = session.state.pawn(id).facing.index();
    let facing = HexFace::LATERAL[(current + delta) % 6];
    session.state.pawn_mut(id).facing = facing;
    let action = session
        .order_for(id)
        .map_or(Action::Hold, |intent| intent.action);
    session.set_order(Intent {
        pawn: id,
        facing,
        action,
    });
    session.notice = format!("pawn {} faces {facing:?}", id.0);
}
