//! The Architect's hands: keys and the mouse, turned into the desk's state and plays.
//!
//! A play goes to the rules only when their own inspection would take it; otherwise the
//! desk shows why not and nothing is sent. Either way the rules decide on the tick.

use bevy::prelude::*;
use observed_match::ascent::sim::ArchitectCommand;

use super::ArchitectDesk;
use super::board::{Board, MARGINS};
use super::desk::Slot;
use super::pick;
use crate::hex_wfc::sim::HexWfcRuntime;

const CARD_KEYS: [KeyCode; 5] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
];

#[allow(clippy::too_many_arguments)]
pub(super) fn input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    board: Res<Board>,
    runtime: Res<HexWfcRuntime>,
    mut desk: ResMut<ArchitectDesk>,
    cards: Query<(&Slot, &Interaction)>,
) {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let Some(hand) = ascent.session().hands.get(&desk.team) else {
        return;
    };
    let held = hand.deck.hand.len();
    let pick_up = |desk: &mut ArchitectDesk, index: usize| {
        if index < held {
            desk.selected = (desk.selected != Some(index)).then_some(index);
        }
    };
    for (index, key) in CARD_KEYS.into_iter().enumerate() {
        if keys.just_pressed(key) {
            pick_up(&mut desk, index);
        }
    }
    let over_a_card = cards
        .iter()
        .any(|(_, interaction)| *interaction != Interaction::None);
    for (slot, interaction) in &cards {
        if *interaction == Interaction::Pressed && buttons.just_pressed(MouseButton::Left) {
            pick_up(&mut desk, slot.0);
        }
    }
    if keys.just_pressed(KeyCode::KeyQ) {
        desk.rotation = (desk.rotation + 5) % 6;
    }
    if keys.just_pressed(KeyCode::KeyE) {
        desk.rotation = (desk.rotation + 1) % 6;
    }
    let levels = runtime.match_state.facility.config.levels;
    if keys.just_pressed(KeyCode::BracketLeft) {
        desk.floor = desk.floor.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        desk.floor = (desk.floor + 1).min(levels.saturating_sub(1));
    }
    if keys.just_pressed(KeyCode::KeyR) {
        desk.pending = Some(ArchitectCommand::Requisition);
    }
    if buttons.just_pressed(MouseButton::Right) {
        desk.selected = None;
    }

    // The cell under the cursor. A cursor off the window leaves the last one in place; one
    // over the panel or the hand is not over the board, whatever is drawn beneath them.
    let cursor = windows.single().ok().and_then(Window::cursor_position);
    let on_board = |pixel: Vec2, size: Vec2| {
        pixel.x > MARGINS.left && pixel.y > MARGINS.top && pixel.y < size.y - MARGINS.bottom
    };
    let over_the_desk = cursor
        .zip(board.framing)
        .is_some_and(|(pixel, (_, size))| !on_board(pixel, size));
    if over_a_card || over_the_desk {
        desk.hovered = None;
    } else if let (Some(pixel), Some((framing, size))) = (cursor, board.framing) {
        desk.hovered = pick::cell_at(
            runtime.match_state.facility.config,
            pick::floor_point(framing, size, pixel),
            desk.floor,
        );
    }

    if buttons.just_pressed(MouseButton::Left)
        && !over_a_card
        && !over_the_desk
        && let (Some(index), Some(target)) = (desk.selected, desk.hovered)
        && let Some(card) = hand.deck.hand.get(index)
    {
        let play = ArchitectCommand::Play {
            card: card.id,
            target,
            rotation: desk.rotation,
        };
        match ascent.session().architect_refusal(desk.seat, play) {
            None => {
                desk.pending = Some(play);
                desk.selected = None;
                desk.last_refusal = None;
            }
            Some(refusal) => desk.last_refusal = Some(refusal),
        }
    }
}
