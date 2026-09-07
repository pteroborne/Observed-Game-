//! Turning taps into intents.
//!
//! Simultaneous resolution means orders are *declared* and then all resolve
//! together, so the interaction is two-stage by nature: select a pawn, give it
//! an order, repeat, then resolve. That is also what makes it work on a phone —
//! nothing is timed, and every order is visible and revisable before it counts.

use bevy::prelude::*;
use observed_hex::faces::HexFace;

use crate::sim::state::{Action, Intent, PawnId};
use crate::sim::{bot, step::step};
use crate::spec::Stacking;

use super::hud::{HudButton, ModeChoice};
use super::{Session, cell_at};

/// A tap on the board: pick a pawn, or order the picked one.
pub fn board_taps(
    mut session: ResMut<Session>,
    camera: Query<(&Camera, &GlobalTransform)>,
    windows: Query<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    over_ui: Query<&Interaction, With<Button>>,
) {
    // The dock owns its own pixels; a tap that lands on a control is not a tap
    // on the board underneath it.
    if over_ui
        .iter()
        .any(|interaction| *interaction != Interaction::None)
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

    // What a tap means depends on whether it lands next to the selected pawn.
    //
    // The old rule — "a tap on your own pawn selects it" — made ordering a move
    // onto a teammate impossible: the tap selected them instead, so the order
    // silently became a selection change. Adjacency decides it now, and `Next`
    // cycles selection when several pawns share one cell.
    let selected = session
        .selected
        .filter(|id| !session.state.pawn(*id).jailed);
    let adjacent = selected.and_then(|id| {
        let from = session.state.pawn(id).at;
        HexFace::LATERAL
            .into_iter()
            .find(|&face| session.state.board.size().neighbor(from, face) == Some(cell))
            .map(|face| (id, from, face))
    });

    let Some((selected, from, face)) = adjacent else {
        // Not adjacent: this is a selection.
        let mine: Vec<_> = session
            .state
            .occupants(cell)
            .into_iter()
            .filter(|id| session.state.pawn(*id).team == session.human)
            .collect();
        match mine.first() {
            Some(&id) => {
                session.selected = Some(id);
                session.notice = if mine.len() > 1 {
                    format!(
                        "pawn {} selected - {} here, Next to cycle",
                        id.0,
                        mine.len()
                    )
                } else {
                    format!("pawn {} selected", id.0)
                };
            }
            None => session.notice = "tap one of your pawns, then a neighbour".to_string(),
        }
        return;
    };

    // A tap through a wall can only ever mean "look that way": the pawn cannot
    // go there, and refusing the tap outright would just feel broken.
    let blocked = !session.state.board.passable(from, face);
    // Teammates block a move only when the mode forbids stacking.
    let teammate = session
        .state
        .occupants(cell)
        .into_iter()
        .any(|id| session.state.pawn(id).team == session.human);
    let crowded = teammate && session.spec.stacking == Stacking::Forbidden;

    let action = if session.face_only || blocked || crowded {
        Action::Hold
    } else {
        Action::Step(face)
    };
    session.set_order(Intent {
        pawn: selected,
        facing: face,
        action,
    });
    session.notice = match (action, blocked, crowded) {
        (Action::Hold, true, _) => format!("wall that way - pawn {} faces it", selected.0),
        (Action::Hold, _, true) => {
            format!(
                "teammate there - pawn {} faces it, no room to stack",
                selected.0
            )
        }
        (Action::Hold, _, _) => format!("pawn {} turns to face {face:?}", selected.0),
        _ => format!("pawn {} steps {face:?}", selected.0),
    };
}

/// Loading a mode from the menu: rebuild `Rules` from the chosen `ModeSpec`
/// and deal a fresh match. Swapping a mechanic at runtime is data replacement,
/// never mutation of a live strategy.
pub fn mode_choices(
    mut session: ResMut<Session>,
    pressed: Query<(&Interaction, &ModeChoice), Changed<Interaction>>,
) {
    for (interaction, choice) in &pressed {
        if *interaction == Interaction::Pressed {
            session.load_mode(choice.0);
            session.notice = format!("loaded {}", session.presets[choice.0].name);
        }
    }
}

pub fn buttons(
    mut session: ResMut<Session>,
    pressed: Query<(&Interaction, &HudButton), Changed<Interaction>>,
    mut legend: Query<&mut Node, With<super::hud::LegendPanel>>,
) {
    for (interaction, button) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            HudButton::Resolve => resolve(&mut session),
            HudButton::Restart => session.restart(),
            HudButton::Next => next_pawn(&mut session),
            HudButton::Modes => session.menu_open = !session.menu_open,
            HudButton::Vision => {
                session.vision = session.vision.next();
                session.notice = format!("colour vision: {}", session.vision.label());
            }
            HudButton::RotateLeft | HudButton::RotateRight => rotate(&mut session, *button),
            HudButton::FaceOnly => {
                session.face_only = !session.face_only;
                session.notice = if session.face_only {
                    "turn in place: a tap sets facing without moving".to_string()
                } else {
                    "move: a tap steps and faces that way".to_string()
                };
            }
            HudButton::Hold => {
                if let Some(selected) = session.selected {
                    let facing = session.state.pawn(selected).facing;
                    session.set_order(Intent {
                        pawn: selected,
                        facing,
                        action: Action::Hold,
                    });
                    session.notice = format!("pawn {} holds", selected.0);
                }
            }
            HudButton::Plant => {
                if let Some(selected) = session.selected {
                    let facing = session.state.pawn(selected).facing;
                    session.set_order(Intent {
                        pawn: selected,
                        facing,
                        action: Action::Plant,
                    });
                    session.notice = format!("pawn {} will plant", selected.0);
                }
            }
            HudButton::Legend => {
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

/// Cycle selection. Pawns sharing a cell come first, so a stack is walked
/// through before moving on — without this there is no way to reach the pawn
/// underneath one that is standing on top of it.
fn next_pawn(session: &mut Session) {
    let order: Vec<PawnId> = session.commandable();
    if order.is_empty() {
        session.notice = "no pawn can act".to_string();
        return;
    }
    let next = match session.selected {
        Some(current) => {
            let index = order.iter().position(|id| *id == current).unwrap_or(0);
            order[(index + 1) % order.len()]
        }
        None => order[0],
    };
    session.selected = Some(next);
    let sharing = session.state.occupants(session.state.pawn(next).at).len();
    session.notice = if sharing > 1 {
        format!("pawn {} selected - {sharing} in this cell", next.0)
    } else {
        format!("pawn {} selected", next.0)
    };
}

/// Turn the selected pawn one face and declare it, so the cone updates live
/// and the order is already recorded. Facing costs no action, so a rotation
/// never consumes the pawn's turn — but it is still declared before resolution,
/// which is what keeps it a blind commitment.
fn rotate(session: &mut Session, button: HudButton) {
    let Some(selected) = session.selected else {
        session.notice = "select a pawn first".to_string();
        return;
    };
    let delta = if button == HudButton::RotateLeft {
        5
    } else {
        1
    };
    let current = session.state.pawn(selected).facing.index();
    let facing = HexFace::LATERAL[(current + delta) % 6];
    session.state.pawn_mut(selected).facing = facing;

    // Keep whatever the pawn was already told to do; only the facing changes.
    let action = session
        .order_for(selected)
        .map_or(Action::Hold, |intent| intent.action);
    session.set_order(Intent {
        pawn: selected,
        facing,
        action,
    });
    session.notice = format!("pawn {} faces {facing:?}", selected.0);
}

/// Resolve the turn: the human's declared orders, the bot's for every other
/// team, all through the one pipeline.
fn resolve(session: &mut Session) {
    if session.state.outcome.is_some() {
        session.notice = "the match is over - Restart or change Mode".to_string();
        return;
    }
    let mut intents = session.queued.clone();
    // A pawn with no order holds rather than vanishing from the turn.
    for pawn in session.commandable() {
        if session.order_for(pawn).is_none() {
            let facing = session.state.pawn(pawn).facing;
            intents.push(Intent {
                pawn,
                facing,
                action: Action::Hold,
            });
        }
    }
    let human = session.human;
    for team in session.state.teams() {
        if team != human {
            intents.extend(bot::team_intents(&session.state, team));
        }
    }
    intents.sort_by_key(|intent| intent.pawn);

    let Session { state, rules, .. } = session;
    step(state, rules, &intents);
    session.queued.clear();
    session.notice = report(session);
}

fn report(session: &Session) -> String {
    let report = &session.state.report;
    let mut parts = Vec::new();
    if !report.refused_moves.is_empty() {
        parts.push(format!("{} moves refused", report.refused_moves.len()));
    }
    if !report.refused_rewires.is_empty() {
        parts.push(format!("{} held off", report.refused_rewires.len()));
    }
    if !report.rewired.is_empty() {
        parts.push(format!("{} boundaries rewired", report.rewired.len()));
    }
    if !report.taken.is_empty() {
        parts.push(format!("{} taken", report.taken.len()));
    }
    if !report.released.is_empty() {
        parts.push(format!("{} freed", report.released.len()));
    }
    if !report.planted.is_empty() {
        parts.push(format!("{} planted", report.planted.len()));
    }
    parts.join(", ")
}
