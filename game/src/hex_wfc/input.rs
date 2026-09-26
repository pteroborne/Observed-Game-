//! Local input capture for the hex match: keyboard/mouse/gamepad into a sanitized
//! [`PlayerIntent`]. The hex match is a spawn→exit traversal race — movement, look,
//! sprint, ordinary jump, and interaction.

use bevy::ecs::system::SystemParam;
use bevy::input::gamepad::GamepadButton;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use observed_match::hex_wfc::HexActionButtons;
use player_input::PlayerIntent;

use super::overlay::{MatchOverlayState, OverlayHotkeys, OverlayRoot, reduce_hotkeys};
use super::sim::HexWfcIntent;
use crate::screens::widgets::UiInputCapture;

const KEY_LOOK_STEP: f32 = 0.035;
const OVERLAY_TRANSITION_CAPTURE: &str = "hex_overlay_transition";

#[derive(SystemParam)]
pub(super) struct HexInputContext<'w, 's> {
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    mouse: Option<Res<'w, AccumulatedMouseMotion>>,
    gamepads: Query<'w, 's, &'static Gamepad>,
    settings: Res<'w, crate::settings::Settings>,
    overlay: Res<'w, MatchOverlayState>,
    capture: Res<'w, UiInputCapture>,
    onboarding: Option<Res<'w, crate::screens::onboarding::OnboardingState>>,
    intent: ResMut<'w, HexWfcIntent>,
    spectator_bot: Option<Res<'w, crate::sim::state::SpectatorBot>>,
}

pub(super) fn map_input(context: HexInputContext) {
    let HexInputContext {
        keyboard,
        mouse,
        gamepads,
        settings,
        overlay,
        capture,
        onboarding,
        mut intent,
        spectator_bot,
    } = context;
    if spectator_bot.is_some() {
        neutralize(&mut intent);
        return;
    }
    let bindings = &settings.bindings;
    if onboarding.is_some() || capture.is_active() || overlay.captures_local_input() {
        neutralize(&mut intent);
        if *overlay == MatchOverlayState::SurvivorMap {
            intent.browse_map_level = map_level_browse(&keyboard, &gamepads);
        }
        return;
    }
    let axis = |negative: KeyCode, positive: KeyCode| {
        (keyboard.pressed(positive) as i32 - keyboard.pressed(negative) as i32) as f32
    };
    let movement = Vec2::new(
        axis(bindings.move_left, bindings.move_right),
        axis(bindings.move_back, bindings.move_forward),
    )
    .normalize_or_zero();
    let mouse_delta = mouse.map_or(Vec2::ZERO, |motion| motion.delta);
    let mut gamepad_intent = PlayerIntent::default();
    let mut gamepad_deploy = false;
    let mut gamepad_recover = false;
    let mut gamepad_pad = false;
    for gamepad in &gamepads {
        let (command, items) = crate::screens::input::read_gamepad_match(gamepad);
        gamepad_intent.movement += command.movement;
        gamepad_intent.look += command.look;
        gamepad_intent.jump_pressed |= command.jump_pressed;
        gamepad_intent.sprint_held |= command.sprint_held;
        gamepad_intent.interact_held |= command.interact_held;
        gamepad_intent.interact_pressed |= command.interact_pressed;
        gamepad_deploy |= items.torch_action;
        // `items.pad_action` was already produced by the shared reader and
        // dropped on the floor here; a pad fires on contact, so `activate_pad`
        // stays unread on purpose.
        gamepad_pad |= items.pad_action;
        gamepad_recover |= gamepad.just_pressed(GamepadButton::East);
    }
    intent.intent = PlayerIntent {
        movement: (movement + gamepad_intent.movement).clamp_length_max(1.0),
        look: mouse_delta * (settings.mouse_sensitivity * 0.018_333)
            + Vec2::new(
                axis(bindings.look_left, bindings.look_right) * KEY_LOOK_STEP,
                axis(bindings.look_up, bindings.look_down) * KEY_LOOK_STEP,
            )
            + gamepad_intent.look,
        // Jump always uses the ordinary physical controller. Vertical routes
        // are walked on authored ramps and stairs.
        jump_pressed: keyboard.pressed(bindings.jump) || gamepad_intent.jump_pressed,
        sprint_held: keyboard.pressed(bindings.sprint)
            || keyboard.pressed(bindings.sprint_alt)
            || gamepad_intent.sprint_held,
        interact_held: keyboard.pressed(bindings.interact) || gamepad_intent.interact_held,
        ..Default::default()
    };
    intent.actions = HexActionButtons {
        interact: keyboard.just_pressed(bindings.interact) || gamepad_intent.interact_pressed,
        deploy_lantern: keyboard.just_pressed(bindings.torch) || gamepad_deploy,
        recover_lantern: keyboard.just_pressed(bindings.recover_lantern) || gamepad_recover,
        deploy_pad: keyboard.just_pressed(bindings.pad) || gamepad_pad,
    };
    intent.browse_map_level = 0;
}

fn map_level_browse(keyboard: &ButtonInput<KeyCode>, gamepads: &Query<&Gamepad>) -> i8 {
    let keyboard_up =
        keyboard.just_pressed(KeyCode::PageUp) || keyboard.just_pressed(KeyCode::BracketRight);
    let keyboard_down =
        keyboard.just_pressed(KeyCode::PageDown) || keyboard.just_pressed(KeyCode::BracketLeft);
    let gamepad_up = gamepads
        .iter()
        .any(|gamepad| gamepad.just_pressed(GamepadButton::DPadUp));
    let gamepad_down = gamepads
        .iter()
        .any(|gamepad| gamepad.just_pressed(GamepadButton::DPadDown));
    match (keyboard_up || gamepad_up, keyboard_down || gamepad_down) {
        (true, false) => 1,
        (false, true) => -1,
        _ => 0,
    }
}

/// What the Architect's desk claims of the match's hotkeys this frame.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct DeskClaim {
    /// The local player is at the Architect's desk.
    pub present: bool,
    /// The desk holds a card, in play.
    pub holds_a_card: bool,
    pub pause_is_escape: bool,
    /// Escape itself was pressed (`raw.back` may also be the controller's East).
    pub escape: bool,
}

/// The match's hotkeys once the Architect's desk has had its claim on `raw` (the pause
/// key, the map keys, Escape or East), with Start (`start`) pausing whatever the desk holds.
///
/// While the desk holds a card, Escape and East step back at the desk, so neither pauses
/// nor goes back in the match; the desk's systems run after this one, so it still holds the
/// card this frame. The desk is the Architect's map, and its controller turns cards on the
/// shoulder buttons, so the map never opens from it.
#[must_use]
pub(super) fn hotkeys_beside_desk(
    raw: OverlayHotkeys,
    desk: DeskClaim,
    start: bool,
) -> OverlayHotkeys {
    let desks_escape = desk.holds_a_card && desk.escape;
    OverlayHotkeys {
        pause: (raw.pause && !(desks_escape && desk.pause_is_escape)) || start,
        map: raw.map && !desk.present,
        back: raw.back && !desk.holds_a_card,
    }
}

fn neutralize(intent: &mut HexWfcIntent) {
    intent.intent = PlayerIntent::default();
    intent.actions = HexActionButtons::default();
    intent.browse_map_level = 0;
}

pub(super) fn mode_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    settings: Res<crate::settings::Settings>,
    mut capture: ResMut<UiInputCapture>,
    onboarding: Option<Res<crate::screens::onboarding::OnboardingState>>,
    overlay_roots: Query<(), With<OverlayRoot>>,
    (mut overlay, architect): (
        ResMut<MatchOverlayState>,
        Option<Res<super::architect::ArchitectDesk>>,
    ),
) {
    // A higher-priority match overlay (for example first-run onboarding) owns
    // Escape/East while captured. Its semantic widget handles dismissal, and this
    // press must never leak through into Pause on the same frame.
    if onboarding.is_some() || capture.is_active() {
        return;
    }
    let escape = keyboard.just_pressed(KeyCode::Escape);
    let hotkeys = hotkeys_beside_desk(
        OverlayHotkeys {
            pause: keyboard.just_pressed(settings.bindings.pause),
            map: keyboard.just_pressed(settings.bindings.tac_map)
                || crate::screens::input::gamepad_map_pressed(&gamepads),
            back: escape
                || gamepads
                    .iter()
                    .any(|gamepad| gamepad.just_pressed(GamepadButton::East)),
        },
        DeskClaim {
            present: architect.is_some(),
            // Only in play: a card still in hand does not stop Escape closing a pause page.
            holds_a_card: *overlay == MatchOverlayState::Playing
                && architect
                    .as_ref()
                    .is_some_and(|desk| desk.selected.is_some() || desk.aimed.is_some()),
            pause_is_escape: settings.bindings.pause == KeyCode::Escape,
            escape,
        },
        gamepads
            .iter()
            .any(|gamepad| gamepad.just_pressed(GamepadButton::Start)),
    );
    // Escape/East on a rendered pause page belongs to that page's semantic Back
    // widget. This guard makes the result independent of whether the shared focus
    // system happens to run before or after this adapter in Update.
    if hotkeys.back && !overlay_roots.is_empty() {
        return;
    }
    let next = reduce_hotkeys(*overlay, hotkeys);
    if next != *overlay {
        if !matches!(*overlay, MatchOverlayState::Pause(_))
            && matches!(next, MatchOverlayState::Pause(_))
        {
            capture.capture(OVERLAY_TRANSITION_CAPTURE);
        }
        *overlay = next;
    }
}

/// Release the one-frame guard used when a pause page is created from the same
/// input edge that semantic widgets also observe. The owner-qualified release cannot
/// disturb onboarding or rebind capture.
pub(super) fn release_overlay_transition_capture(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    settings: Res<crate::settings::Settings>,
    mut capture: ResMut<UiInputCapture>,
) {
    let opening_edge = keyboard.just_pressed(settings.bindings.pause)
        || gamepads
            .iter()
            .any(|gamepad| gamepad.just_pressed(GamepadButton::Start));
    if !opening_edge {
        capture.release(OVERLAY_TRANSITION_CAPTURE);
    }
}

pub(super) fn grab_cursor(
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
    spectator_bot: Option<Res<crate::sim::state::SpectatorBot>>,
    settings: Res<crate::settings::Settings>,
) {
    if spectator_bot.is_none()
        && !settings.needs_onboarding()
        && let Ok(mut cursor) = cursors.single_mut()
    {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

pub(super) fn release_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor) = cursors.single_mut() {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

/// Match cursor ownership is derived from modal state, never from the button that
/// happened to open or close it. This also repairs ownership after pointer activation.
pub(super) fn sync_cursor(
    overlay: Res<MatchOverlayState>,
    capture: Res<UiInputCapture>,
    onboarding: Option<Res<crate::screens::onboarding::OnboardingState>>,
    spectator_bot: Option<Res<crate::sim::state::SpectatorBot>>,
    architect: Option<Res<super::architect::ArchitectDesk>>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let Ok(mut cursor) = cursors.single_mut() else {
        return;
    };
    let grab = *overlay == MatchOverlayState::Playing
        && !capture.is_active()
        && onboarding.is_none()
        && spectator_bot.is_none()
        // An Architect points at a board.
        && architect.is_none();
    cursor.grab_mode = if grab {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    cursor.visible = !grab;
}

#[cfg(test)]
mod tests {
    use super::super::overlay::PausePage;
    use super::*;

    fn escape_pressed() -> OverlayHotkeys {
        // The default bindings: Escape is both the pause key and back.
        OverlayHotkeys {
            pause: true,
            map: false,
            back: true,
        }
    }

    fn desk(holds_a_card: bool) -> DeskClaim {
        DeskClaim {
            present: true,
            holds_a_card,
            pause_is_escape: true,
            escape: true,
        }
    }

    #[test]
    fn escape_with_a_card_in_hand_is_the_desk_s_and_does_not_pause() {
        let hotkeys = hotkeys_beside_desk(escape_pressed(), desk(true), false);
        assert_eq!(hotkeys, OverlayHotkeys::default());
        assert_eq!(
            reduce_hotkeys(MatchOverlayState::Playing, hotkeys),
            MatchOverlayState::Playing
        );
    }

    #[test]
    fn escape_with_nothing_in_hand_pauses_as_ever() {
        let hotkeys = hotkeys_beside_desk(escape_pressed(), desk(false), false);
        assert!(matches!(
            reduce_hotkeys(MatchOverlayState::Playing, hotkeys),
            MatchOverlayState::Pause(_)
        ));
        let without_a_desk = hotkeys_beside_desk(escape_pressed(), DeskClaim::default(), false);
        assert_eq!(without_a_desk, escape_pressed());
    }

    #[test]
    fn start_pauses_whatever_the_desk_holds() {
        let hotkeys = hotkeys_beside_desk(OverlayHotkeys::default(), desk(true), true);
        assert!(hotkeys.pause);
    }

    #[test]
    fn a_rebound_pause_key_still_pauses_with_a_card_in_hand() {
        let claim = DeskClaim {
            pause_is_escape: false,
            escape: false,
            ..desk(true)
        };
        let pause_key = OverlayHotkeys {
            pause: true,
            ..OverlayHotkeys::default()
        };
        assert!(hotkeys_beside_desk(pause_key, claim, false).pause);
    }

    #[test]
    fn east_with_a_card_is_the_desk_s_and_the_map_never_opens_at_the_desk() {
        let east = OverlayHotkeys {
            back: true,
            map: true,
            pause: false,
        };
        let claim = DeskClaim {
            escape: false,
            ..desk(true)
        };
        let hotkeys = hotkeys_beside_desk(east, claim, false);
        assert!(!hotkeys.back);
        assert!(!hotkeys.map, "the board is the Architect's map");
        let observer = hotkeys_beside_desk(east, DeskClaim::default(), false);
        assert!(observer.map && observer.back, "an Observer keeps both");
    }

    #[test]
    fn pausing_neutralizes_every_held_and_one_shot_input() {
        let mut app = App::new();
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyW);
        keyboard.press(KeyCode::Space);
        app.insert_resource(keyboard)
            .insert_resource(crate::settings::Settings::default())
            .insert_resource(MatchOverlayState::Pause(PausePage::Root))
            .insert_resource(UiInputCapture::default())
            .insert_resource(HexWfcIntent {
                intent: PlayerIntent {
                    movement: Vec2::ONE,
                    look: Vec2::ONE,
                    jump_pressed: true,
                    sprint_held: true,
                    interact_held: true,
                    ..Default::default()
                },
                actions: HexActionButtons {
                    interact: true,
                    deploy_lantern: true,
                    recover_lantern: true,
                    deploy_pad: true,
                },
                browse_map_level: 1,
            })
            .add_systems(Update, map_input);

        app.update();

        let intent = app.world().resource::<HexWfcIntent>();
        assert!(intent.intent.is_neutral());
        assert_eq!(intent.actions, HexActionButtons::default());
        assert_eq!(intent.browse_map_level, 0);
    }

    #[test]
    fn higher_priority_input_capture_consumes_escape_before_pause() {
        let mut app = App::new();
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::Escape);
        let mut capture = UiInputCapture::default();
        capture.capture("hex.onboarding.test");
        app.insert_resource(keyboard)
            .insert_resource(crate::settings::Settings::default())
            .insert_resource(MatchOverlayState::Playing)
            .insert_resource(capture)
            .add_systems(Update, mode_hotkeys);

        app.update();

        assert_eq!(
            *app.world().resource::<MatchOverlayState>(),
            MatchOverlayState::Playing
        );
    }

    #[test]
    fn opening_pause_holds_capture_through_the_triggering_edge() {
        let mut app = App::new();
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::Escape);
        app.insert_resource(keyboard)
            .insert_resource(crate::settings::Settings::default())
            .insert_resource(MatchOverlayState::Playing)
            .insert_resource(UiInputCapture::default())
            .add_systems(
                Update,
                (mode_hotkeys, release_overlay_transition_capture).chain(),
            );

        app.update();

        assert_eq!(
            *app.world().resource::<MatchOverlayState>(),
            MatchOverlayState::Pause(PausePage::Root)
        );
        assert!(app.world().resource::<UiInputCapture>().is_active());

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        assert!(!app.world().resource::<UiInputCapture>().is_active());
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::*;

    #[test]
    fn controller_interact_reaches_the_canonical_action_frame() {
        let mut app = App::new();
        app.insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(crate::settings::Settings::default())
            .init_resource::<MatchOverlayState>()
            .init_resource::<UiInputCapture>()
            .init_resource::<HexWfcIntent>()
            .add_systems(Update, map_input);
        let mut gamepad = Gamepad::default();
        gamepad.digital_mut().press(GamepadButton::West);
        app.world_mut().spawn(gamepad);
        app.update();
        let intent = app.world().resource::<HexWfcIntent>();
        assert!(
            intent.actions.interact,
            "X must collect and operate, as E does"
        );
        assert!(
            intent.intent.interact_held,
            "holding X must also synchronize stations"
        );
        *app.world_mut().resource_mut::<MatchOverlayState>() = MatchOverlayState::SurvivorMap;
        app.update();
        let intent = app.world().resource::<HexWfcIntent>();
        assert!(
            !intent.actions.interact,
            "map focus must consume gameplay actions"
        );
        assert!(!intent.intent.interact_held);
    }
}
