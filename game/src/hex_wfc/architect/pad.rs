//! The Architect's desk on a controller.
//!
//! The left stick moves a cursor across the floor in view, in the board's own screen
//! directions, and the cell under it is the one pointed at, as under the mouse. The rest
//! is the desk's keys on buttons, through the same desk methods:
//!
//! - **A** aims at the cell pointed at; **A** on the aim confirms the play.
//! - **B** steps back: from the aim to the card in hand, and from the card to none.
//!   (Start still pauses.)
//! - **LB / RB** turn the card; **D-pad left / right** go along the hand; **D-pad up /
//!   down** change floor.
//! - **R3** is the emergency requisition, off the fingers' way on purpose.
//!
//! The last hand on the desk decides which the prompts name (`ArchitectDesk::pad`).

use bevy::input::gamepad::GamepadButton;
use bevy::prelude::*;
use observed_facility::hex_wfc::HexWfcConfig;
use observed_hex::{HexCoord, hex_origin};
use observed_match::ascent::sim::ArchitectCommand;

use super::ArchitectDesk;
use super::board::Board;
use super::input::{confirm_play, desk_live};
use super::pick::{self, CELL_RADIUS};
use crate::hex_wfc::overlay::MatchOverlayState;
use crate::hex_wfc::sim::HexWfcRuntime;
use crate::screens::input::apply_deadzone;
use crate::screens::widgets::UiInputCapture;

/// How fast the cursor crosses the board at full stick, in screen pixels a second.
const CURSOR_SPEED: f32 = 560.0;

/// The plan directions that read as screen right and screen up on the board.
#[must_use]
pub(super) fn screen_axes() -> (Vec2, Vec2) {
    let rotation = pick::rotation();
    let flat = |v: Vec3| Vec2::new(v.x, v.z).normalize_or_zero();
    (flat(rotation * Vec3::X), flat(rotation * Vec3::NEG_Z))
}

/// The cursor moved by `stick` for `metres` at full deflection, kept on the lattice.
#[must_use]
pub(super) fn step_cursor(from: Vec2, stick: Vec2, metres: f32, bounds: (Vec2, Vec2)) -> Vec2 {
    let (right, up) = screen_axes();
    (from + (right * stick.x + up * stick.y) * metres).clamp(bounds.0, bounds.1)
}

/// The plan rectangle the lattice's cells cover.
#[must_use]
pub(super) fn lattice_bounds(config: HexWfcConfig) -> (Vec2, Vec2) {
    let mut low = Vec2::splat(f32::MAX);
    let mut high = Vec2::splat(f32::MIN);
    for (q, r) in [
        (0, 0),
        (config.cols - 1, 0),
        (0, config.rows - 1),
        (config.cols - 1, config.rows - 1),
    ] {
        let at = hex_origin(HexCoord { q, r, level: 0 });
        let at = Vec2::new(at[0], at[2]);
        low = low.min(at);
        high = high.max(at);
    }
    (
        low - Vec2::splat(CELL_RADIUS * 0.5),
        high + Vec2::splat(CELL_RADIUS * 0.5),
    )
}

fn centre(cell: HexCoord) -> Vec2 {
    let at = hex_origin(cell);
    Vec2::new(at[0], at[2])
}

#[allow(clippy::too_many_arguments)]
pub(super) fn input(
    gamepads: Query<&Gamepad>,
    time: Res<Time>,
    board: Option<Res<Board>>,
    runtime: Res<HexWfcRuntime>,
    mut desk: ResMut<ArchitectDesk>,
    overlay: Res<MatchOverlayState>,
    capture: Res<UiInputCapture>,
) {
    if !desk_live(&overlay, &capture) {
        return;
    }
    let Some(held) = runtime
        .ascent
        .as_ref()
        .and_then(|ascent| ascent.session().hands.get(&desk.team))
        .map(|hand| hand.deck.hand.len())
    else {
        return;
    };
    let pressed = |button: GamepadButton| gamepads.iter().any(|pad| pad.just_pressed(button));
    let stick = gamepads
        .iter()
        .map(|pad| apply_deadzone(pad.left_stick()))
        .fold(Vec2::ZERO, |sum, stick| sum + stick)
        .clamp_length_max(1.0);
    let any_button = gamepads
        .iter()
        .any(|pad| pad.get_just_pressed().next().is_some());
    if stick == Vec2::ZERO && !any_button {
        return;
    }
    desk.pad = true;
    let config = runtime.match_state.facility.config;

    if stick != Vec2::ZERO
        && let Some((framing, _)) = board.and_then(|board| board.framing)
    {
        // From where the play is about, or the middle of the board.
        let from = desk.pad_cursor.unwrap_or_else(|| {
            desk.focus()
                .map_or(Vec2::new(framing.focus.x, framing.focus.z), centre)
        });
        let metres = CURSOR_SPEED * framing.metres_per_pixel * time.delta_secs();
        let at = step_cursor(from, stick, metres, lattice_bounds(config));
        desk.pad_cursor = Some(at);
        desk.hovered = pick::cell_at(config, at, desk.floor);
    }

    if pressed(GamepadButton::DPadRight) {
        desk.cycle(1, held);
    }
    if pressed(GamepadButton::DPadLeft) {
        desk.cycle(-1, held);
    }
    if pressed(GamepadButton::LeftTrigger) {
        desk.rotation = (desk.rotation + 5) % 6;
    }
    if pressed(GamepadButton::RightTrigger) {
        desk.rotation = (desk.rotation + 1) % 6;
    }
    if pressed(GamepadButton::DPadUp) {
        let floor = (desk.floor + 1).min(config.levels.saturating_sub(1));
        desk.look_at(floor);
    }
    if pressed(GamepadButton::DPadDown) {
        let floor = desk.floor.saturating_sub(1);
        desk.look_at(floor);
    }
    if pressed(GamepadButton::RightThumb) {
        desk.pending = Some(ArchitectCommand::Requisition);
    }
    if pressed(GamepadButton::East) {
        desk.cancel();
    }
    if pressed(GamepadButton::South)
        && let Some(cell) = desk.hovered
        && desk.click_cell(cell)
    {
        confirm_play(&mut desk, &runtime);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stick_moves_the_cursor_the_way_it_is_pushed_on_screen() {
        let rotation = pick::rotation();
        let bounds = (Vec2::splat(-1e4), Vec2::splat(1e4));
        let on_screen = |plan: Vec2| {
            let world = Vec3::new(plan.x, 0.0, plan.y);
            let local = rotation.inverse() * world;
            Vec2::new(local.x, local.y)
        };
        let right = step_cursor(Vec2::ZERO, Vec2::X, 10.0, bounds);
        let up = step_cursor(Vec2::ZERO, Vec2::Y, 10.0, bounds);
        assert!(on_screen(right).x > 5.0, "right is right: {right}");
        assert!(on_screen(right).y.abs() < 1e-3, "and only right");
        assert!(on_screen(up).y > 1.0, "up is up: {up}");
        assert!(on_screen(up).x.abs() < 1e-3, "and only up");
    }

    #[test]
    fn the_cursor_stays_on_the_lattice() {
        let config = HexWfcConfig::arc_default();
        let bounds = lattice_bounds(config);
        let far = step_cursor(Vec2::ZERO, Vec2::new(-1.0, -1.0), 1e5, bounds);
        assert_eq!(far, far.clamp(bounds.0, bounds.1));
        let corner = HexCoord {
            q: 0,
            r: 0,
            level: 0,
        };
        assert!(
            pick::cell_at(config, centre(corner), 0).is_some(),
            "a corner cell is inside"
        );
    }
}
