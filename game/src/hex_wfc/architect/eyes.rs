//! Looking through an Observer's eyes, from the Architect's desk.
//!
//! The board is the building as the team knows it; what one of the team is looking at
//! right now is another thing, and the one that tells an Architect what a corridor is
//! like to stand in. V (the controller's View button, or the panel's LOOK button) steps
//! off the desk into the first active Observer's eyes; Q / E (LB / RB) go to the next
//! Observer; V, Escape or B come back to the desk.
//!
//! Nothing is simulated for it. The world view already follows one body - the runtime's
//! viewed body ([`HexWfcRuntime::viewed`]), normally the local one - so while looking,
//! that body is the Observer's: the camera rides its eye, the facility streams in around
//! it, the light and the district are its, and a jailed Observer is seen in its maze. The
//! board, the climb and the desk step aside (their cameras rest), and a slim bar over the
//! view says whose eyes these are and how to go back. The desk hears nothing else until
//! then.

use bevy::input::gamepad::GamepadButton;
use bevy::prelude::*;
use bevy::ui::UiTargetCamera;
use observed_core::PlayerId;
use observed_match::ascent::sim::ObserverState;
use observed_match::hex_wfc::HexPlayerState;
use observed_style::architect::{Role, color};

use super::ArchitectDesk;
use super::board::BoardCamera;
use super::desk::{DeskButton, DeskUi};
use super::input::desk_live;
use super::stack::StackCamera;
use crate::GameState;
use crate::hex_wfc::overlay::MatchOverlayState;
use crate::hex_wfc::sim::HexWfcRuntime;
use crate::screens::widgets::UiInputCapture;
use crate::view::components::GameCam;

impl HexWfcRuntime {
    /// The body the world is presented from: the local player's, unless the Architect is
    /// looking through another's eyes (`viewed_player`).
    #[must_use]
    pub fn viewed(&self) -> &HexPlayerState {
        self.viewed_player
            .and_then(|player| self.match_state.players.get(&player))
            .unwrap_or_else(|| self.local())
    }
}

/// The team's bodies an Architect can look through: its active Observers, in order.
#[must_use]
pub(super) fn eyes_for(runtime: &HexWfcRuntime, desk: &ArchitectDesk) -> Vec<PlayerId> {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return Vec::new();
    };
    let rules = ascent.rules();
    runtime
        .match_state
        .players
        .keys()
        .copied()
        .filter(|&player| {
            ascent
                .observer_for(player)
                .and_then(|id| rules.observers.get(&id))
                .is_some_and(|observer| {
                    observer.team == desk.team && observer.state != ObserverState::Corrupted
                })
        })
        .collect()
}

/// The next of `eyes` `step` along from `now`, wrapping; the first from none.
#[must_use]
pub(super) fn next_eye(eyes: &[PlayerId], now: Option<PlayerId>, step: i32) -> Option<PlayerId> {
    let count = i32::try_from(eyes.len()).ok().filter(|&n| n > 0)?;
    let at = now
        .and_then(|now| eyes.iter().position(|&eye| eye == now))
        .and_then(|at| i32::try_from(at).ok());
    let next = at.map_or(0, |at| (at + step).rem_euclid(count));
    eyes.get(usize::try_from(next).ok()?).copied()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    runtime: Res<HexWfcRuntime>,
    mut desk: ResMut<ArchitectDesk>,
    look: Query<(&DeskButton, &Interaction)>,
    (overlay, capture): (Res<MatchOverlayState>, Res<UiInputCapture>),
) {
    if !desk_live(&overlay, &capture) {
        return;
    }
    let pad = |button: GamepadButton| gamepads.iter().any(|pad| pad.just_pressed(button));
    let eyes = eyes_for(&runtime, &desk);
    // A body that can no longer see - lost to the void - hands the view back.
    if desk.eyes.is_some_and(|eye| !eyes.contains(&eye)) {
        desk.eyes = None;
    }
    let clicked_look = buttons.just_pressed(MouseButton::Left)
        && look.iter().any(|(action, interaction)| {
            *action == DeskButton::Look && *interaction == Interaction::Pressed
        });
    let toggle = keys.just_pressed(KeyCode::KeyV) || pad(GamepadButton::Select) || clicked_look;
    if desk.eyes.is_none() {
        if toggle {
            desk.eyes = next_eye(&eyes, None, 1);
        }
        return;
    }
    if toggle || keys.just_pressed(KeyCode::Escape) || pad(GamepadButton::East) {
        desk.eyes = None;
        return;
    }
    let step = i32::from(keys.just_pressed(KeyCode::KeyE) || pad(GamepadButton::RightTrigger))
        - i32::from(keys.just_pressed(KeyCode::KeyQ) || pad(GamepadButton::LeftTrigger));
    if step != 0 {
        desk.eyes = next_eye(&eyes, desk.eyes, step);
    }
}

/// The bar over the view while looking through an Observer's eyes.
#[derive(Component)]
pub(super) struct EyesBar;

#[derive(Component)]
pub(super) struct EyesText;

pub(super) fn spawn(
    mut commands: Commands,
    camera: Query<Entity, With<GameCam>>,
    existing: Query<(), With<EyesBar>>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    if !existing.is_empty() {
        return;
    }
    commands
        .spawn((
            EyesBar,
            UiTargetCamera(camera),
            DespawnOnExit(GameState::HexWfc),
            Node {
                position_type: PositionType::Absolute,
                top: px(16),
                left: percent(50),
                padding: UiRect::axes(px(18), px(10)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
            BackgroundColor(color(Role::Panel).with_alpha(0.92)),
            BorderColor::all(color(Role::Observer)),
            Visibility::Hidden,
            Name::new("Architect looking through an Observer"),
        ))
        .with_children(|bar| {
            bar.spawn((
                EyesText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(color(Role::Text)),
                TextLayout::justify(Justify::Center),
            ));
        });
}

/// What the bar says, looking through `eye` (its EYE number) on `floor` (0-based).
#[must_use]
pub(super) fn words(eye: u16, floor: u8, pad: bool) -> String {
    let (other, back) = if pad {
        ("LB / RB  other eye", "VIEW / B  back to the desk")
    } else {
        ("Q / E  other eye", "V / Esc  back to the desk")
    };
    format!(
        "LOOKING THROUGH EYE {:02}  /  FLOOR {}\n{other}      {back}",
        eye + 1,
        floor + 1
    )
}

#[allow(clippy::type_complexity)]
pub(super) fn sync(
    desk: Res<ArchitectDesk>,
    mut runtime: ResMut<HexWfcRuntime>,
    mut cameras: Query<&mut Camera, Or<(With<BoardCamera>, With<StackCamera>)>>,
    mut desk_ui: Query<&mut Visibility, (With<DeskUi>, Without<EyesBar>)>,
    mut bar: Query<&mut Visibility, (With<EyesBar>, Without<DeskUi>)>,
    mut text: Query<&mut Text, With<EyesText>>,
) {
    if runtime.viewed_player != desk.eyes {
        runtime.viewed_player = desk.eyes;
    }
    let looking = desk.eyes.is_some();
    for mut camera in &mut cameras {
        if camera.is_active == looking {
            camera.is_active = !looking;
        }
    }
    let (desk_shown, bar_shown) = if looking {
        (Visibility::Hidden, Visibility::Inherited)
    } else {
        (Visibility::Inherited, Visibility::Hidden)
    };
    for mut visibility in &mut desk_ui {
        visibility.set_if_neq(desk_shown);
    }
    for mut visibility in &mut bar {
        visibility.set_if_neq(bar_shown);
    }
    if let Some(eye) = desk.eyes {
        let floor = runtime.viewed().cell.level;
        let said = words(eye.0, floor, desk.pad);
        for mut text in &mut text {
            if **text != said {
                **text = said.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_eyes_go_round_the_team_and_start_at_the_first() {
        let eyes = [PlayerId(0), PlayerId(1), PlayerId(4)];
        assert_eq!(next_eye(&eyes, None, 1), Some(PlayerId(0)));
        assert_eq!(next_eye(&eyes, Some(PlayerId(1)), 1), Some(PlayerId(4)));
        assert_eq!(next_eye(&eyes, Some(PlayerId(4)), 1), Some(PlayerId(0)));
        assert_eq!(next_eye(&eyes, Some(PlayerId(0)), -1), Some(PlayerId(4)));
        assert_eq!(
            next_eye(&eyes, Some(PlayerId(9)), 1),
            Some(PlayerId(0)),
            "a body no longer among them starts again at the first"
        );
        assert_eq!(next_eye(&[], None, 1), None);
    }

    #[test]
    fn the_bar_says_whose_eyes_and_how_to_go_back() {
        let keys = words(0, 2, false);
        assert!(
            keys.starts_with("LOOKING THROUGH EYE 01  /  FLOOR 3"),
            "{keys}"
        );
        assert!(keys.contains("V / Esc  back to the desk"), "{keys}");
        assert!(words(1, 0, true).contains("VIEW / B"));
        assert!(keys.is_ascii(), "the shipped font");
    }
}
