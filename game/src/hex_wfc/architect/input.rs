//! The Architect's hands: keys and the mouse, turned into the desk's state and plays.
//!
//! A click on the board aims the card in hand; a second click on the aim, Space, Enter or
//! PLAY confirms it. A confirmed play goes to the rules only when their own inspection
//! would take it; otherwise the desk shows why not and nothing is sent. Either way the
//! rules decide on the tick.

use bevy::prelude::*;
use observed_match::ascent::sim::ArchitectCommand;

use super::ArchitectDesk;
use super::board::{Board, margins as board_margins};
use super::desk::{DeskButton, Slot};
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
    controls: Query<(&DeskButton, &Interaction)>,
) {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let Some(hand) = ascent.session().hands.get(&desk.team) else {
        return;
    };
    let held = hand.deck.hand.len();
    let clicked = buttons.just_pressed(MouseButton::Left);
    for (index, key) in CARD_KEYS.into_iter().enumerate() {
        if keys.just_pressed(key) {
            desk.pick_up(index, held);
        }
    }
    for (slot, interaction) in &cards {
        if *interaction == Interaction::Pressed && clicked {
            desk.pick_up(slot.0, held);
        }
    }
    // The desk's buttons do what their keys do.
    let pressed = |action: DeskButton| {
        clicked
            && controls.iter().any(|(button, interaction)| {
                *button == action && *interaction == Interaction::Pressed
            })
    };
    let over_the_ui = cards
        .iter()
        .map(|(_, interaction)| interaction)
        .chain(controls.iter().map(|(_, interaction)| interaction))
        .any(|interaction| *interaction != Interaction::None);

    if keys.just_pressed(KeyCode::KeyQ) || pressed(DeskButton::TurnLeft) {
        desk.rotation = (desk.rotation + 5) % 6;
    }
    if keys.just_pressed(KeyCode::KeyE) || pressed(DeskButton::TurnRight) {
        desk.rotation = (desk.rotation + 1) % 6;
    }
    let levels = runtime.match_state.facility.config.levels;
    if keys.just_pressed(KeyCode::BracketLeft) || pressed(DeskButton::FloorDown) {
        let floor = desk.floor.saturating_sub(1);
        desk.look_at(floor);
    }
    if keys.just_pressed(KeyCode::BracketRight) || pressed(DeskButton::FloorUp) {
        let floor = (desk.floor + 1).min(levels.saturating_sub(1));
        desk.look_at(floor);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        desk.pending = Some(ArchitectCommand::Requisition);
    }
    if buttons.just_pressed(MouseButton::Right) {
        desk.put_down();
    }
    if keys.just_pressed(KeyCode::Escape) || pressed(DeskButton::Cancel) {
        desk.cancel();
    }

    // The cell under the cursor. A cursor off the window leaves the last one in place; one
    // over the desk is not over the board, whatever is drawn beneath it.
    let cursor = windows.single().ok().and_then(Window::cursor_position);
    let margins = board_margins(desk.selected.is_some());
    let over_the_desk = over_the_ui
        || cursor
            .zip(board.framing)
            .is_some_and(|(pixel, (_, size))| !margins.contain(size, pixel));
    if over_the_desk {
        desk.hovered = None;
    } else if let (Some(pixel), Some((framing, size))) = (cursor, board.framing) {
        // Traced onto the floor in view, so the pick is exact at the isometric pitch.
        let ray = pick::ray(board.camera, framing.metres_per_pixel, size, pixel);
        desk.hovered = pick::on_deck(ray, desk.floor).and_then(|point| {
            pick::cell_at(runtime.match_state.facility.config, point, desk.floor)
        });
    }

    // A click on the board aims; a second click on the aim, Space, Enter or PLAY confirms.
    let mut confirm = keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::Enter)
        || pressed(DeskButton::Play);
    if clicked
        && !over_the_desk
        && let Some(cell) = desk.hovered
    {
        confirm |= desk.click_cell(cell);
    }
    if confirm
        && let (Some(index), Some(target)) = (desk.selected, desk.aimed)
        && let Some(card) = hand.deck.hand.get(index)
    {
        let play = ArchitectCommand::Play {
            card: card.id,
            target,
            rotation: desk.rotation,
        };
        let refusal = ascent.session().architect_refusal(desk.seat, play);
        desk.settle(play, refusal);
    }
}
