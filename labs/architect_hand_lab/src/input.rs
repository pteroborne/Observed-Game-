//! Pointer, touch and keyboard parity. A card-origin drag owns the gesture;
//! otherwise the fitted map remains a simple tap surface.

use bevy::prelude::*;

use crate::board::{BoardCamera, cell_at};
use crate::model::LabState;
use crate::ui::{Control, HandSlot};

const DRAG_THRESHOLD: f32 = 14.0;

#[derive(Resource, Default)]
pub struct TrialClock {
    pub elapsed: f32,
}

#[derive(Resource, Default)]
pub struct CardDrag {
    armed: bool,
    pub active: bool,
    pub start: Vec2,
    pub current: Vec2,
}

pub fn tick(time: Res<Time>, state: Res<LabState>, mut clock: ResMut<TrialClock>) {
    if !state.complete {
        clock.elapsed += time.delta_secs();
    }
}

fn pointer_position(windows: &Query<&Window>, touches: &Touches) -> Option<Vec2> {
    touches
        .iter_just_pressed()
        .next()
        .map(|touch| touch.position())
        .or_else(|| windows.single().ok()?.cursor_position())
}

fn cell_from_screen(
    state: &LabState,
    camera: &Query<(&Camera, &GlobalTransform), With<BoardCamera>>,
    point: Vec2,
) -> Option<observed_hex::coords::HexCoord> {
    let (camera, transform) = camera.single().ok()?;
    let world = camera.viewport_to_world_2d(transform, point).ok()?;
    cell_at(state, world)
}

pub fn cards(
    mut state: ResMut<LabState>,
    mut drag: ResMut<CardDrag>,
    pressed: Query<(&Interaction, &HandSlot), Changed<Interaction>>,
    windows: Query<&Window>,
    touches: Res<Touches>,
) {
    for (interaction, slot) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(card) = state.cards.get(slot.0).copied() else {
            continue;
        };
        state.select(card.id);
        if let Some(point) = pointer_position(&windows, &touches) {
            drag.armed = true;
            drag.active = false;
            drag.start = point;
            drag.current = point;
        }
    }
}

pub fn drag_card(
    mut state: ResMut<LabState>,
    mut drag: ResMut<CardDrag>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform), With<BoardCamera>>,
) {
    if !drag.armed {
        return;
    }
    if let Some(point) = touches
        .iter()
        .next()
        .map(|touch| touch.position())
        .or_else(|| windows.single().ok()?.cursor_position())
    {
        drag.current = point;
        if point.distance(drag.start) >= DRAG_THRESHOLD {
            drag.active = true;
        }
    }

    let released = touches
        .iter_just_released()
        .next()
        .map(|touch| touch.position())
        .or_else(|| {
            mouse
                .just_released(MouseButton::Left)
                .then(|| windows.single().ok()?.cursor_position())
                .flatten()
        });
    let Some(point) = released else {
        return;
    };
    if drag.active {
        if let Some(cell) = cell_from_screen(&state, &camera, point) {
            state.aim(cell, true);
        } else {
            state.notice = "Card returned to the hand — drop it on a hex to preview.".to_string();
        }
    }
    drag.armed = false;
    drag.active = false;
}

pub fn board_taps(
    mut state: ResMut<LabState>,
    drag: Res<CardDrag>,
    camera: Query<(&Camera, &GlobalTransform), With<BoardCamera>>,
    windows: Query<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    over_ui: Query<&Interaction, With<Button>>,
) {
    if drag.armed
        || over_ui
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
    if let Some(cell) = cell_from_screen(&state, &camera, point) {
        state.aim(cell, false);
    }
}

pub fn hover_board(
    mut state: ResMut<LabState>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform), With<BoardCamera>>,
) {
    let hovered = windows
        .single()
        .ok()
        .and_then(Window::cursor_position)
        .and_then(|point| cell_from_screen(&state, &camera, point));
    if state.hovered == hovered {
        return;
    }
    state.hovered = hovered;
    if state.target.is_some() {
        return;
    }
    let Some(cell) = hovered else {
        return;
    };
    let Some(preview) = state.preview_at(cell) else {
        return;
    };
    state.notice = match preview.refusal {
        Some(reason) => format!(
            "{} — {}",
            reason.label().to_uppercase(),
            "tap for a fixed explanation"
        ),
        None => format!(
            "VALID SITE — {} boundaries would change",
            preview.changes.len()
        ),
    };
}

pub fn controls(
    mut state: ResMut<LabState>,
    mut clock: ResMut<TrialClock>,
    pressed: Query<(&Interaction, &Control), Changed<Interaction>>,
) {
    for (interaction, control) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        activate(*control, &mut state, &mut clock);
    }
}

fn activate(control: Control, state: &mut LabState, clock: &mut TrialClock) {
    match control {
        Control::PreviousScenario => {
            state.change_scenario(state.scenario.previous());
            clock.elapsed = 0.0;
        }
        Control::NextScenario => {
            state.change_scenario(state.scenario.next());
            clock.elapsed = 0.0;
        }
        Control::Place => state.commit(),
        Control::Cancel => state.cancel(),
        Control::Undo => state.undo(),
        Control::Reset => {
            state.change_scenario(state.scenario);
            clock.elapsed = 0.0;
        }
        Control::RotateLeft => state.rotate(false),
        Control::RotateRight => state.rotate(true),
        Control::CopyReport => copy_report(state, clock.elapsed),
    }
}

pub fn keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<LabState>,
    mut clock: ResMut<TrialClock>,
) {
    for (key, slot) in [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
    ] {
        if keys.just_pressed(key)
            && let Some(card) = state.cards.get(slot)
        {
            let id = card.id;
            state.select(id);
        }
    }
    if keys.just_pressed(KeyCode::KeyQ) {
        state.rotate(false);
    }
    if keys.just_pressed(KeyCode::KeyE) {
        state.rotate(true);
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        state.commit();
    }
    if keys.just_pressed(KeyCode::Escape) {
        state.cancel();
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        state.undo();
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        activate(Control::PreviousScenario, &mut state, &mut clock);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        activate(Control::NextScenario, &mut state, &mut clock);
    }
}

type ButtonVisualQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static mut BackgroundColor),
    (Changed<Interaction>, With<Button>),
>;

pub fn button_visuals(mut buttons: ButtonVisualQuery) {
    for (interaction, mut background) in &mut buttons {
        match interaction {
            Interaction::Pressed => background.0 = background.0.lighter(0.16),
            Interaction::Hovered => background.0 = background.0.lighter(0.08),
            Interaction::None => {}
        }
    }
}

fn copy_report(state: &mut LabState, elapsed: f32) {
    let report = state.report(elapsed);
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let _ = window.navigator().clipboard().write_text(&report);
            state.notice = "Local trial report copied to the clipboard.".to_string();
            return;
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        println!("{report}");
        state.notice = "Local trial report printed to the terminal.".to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_requires_deliberate_travel() {
        assert!(Vec2::new(15.0, 0.0).length() >= DRAG_THRESHOLD);
        assert!(Vec2::new(4.0, 4.0).length() < DRAG_THRESHOLD);
    }

    #[test]
    fn scenario_navigation_wraps() {
        assert_eq!(
            crate::model::Scenario::ThroughLine.previous(),
            crate::model::Scenario::FreePlay
        );
        assert_eq!(
            crate::model::Scenario::FreePlay.next(),
            crate::model::Scenario::ThroughLine
        );
    }

    #[test]
    fn card_ids_are_not_slot_numbers_after_a_spend() {
        let mut state = LabState::default();
        let second = state.cards[1].id;
        state.cards.remove(0);
        assert_eq!(state.cards[0].id, second);
        assert_ne!(state.cards[0].id, crate::model::CardId(0));
    }
}
