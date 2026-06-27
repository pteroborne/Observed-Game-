//! The **pure adapter** for Phase A6: it turns `bevy_archie`'s data-driven input
//! model into the project's durable [`player_input::PlayerIntent`], *per device*,
//! with no Bevy ECS, no resources, and no rendering. Everything here is unit
//! testable from constructed samples.
//!
//! Why an adapter at all? `bevy_archie` ships its own `ActionState` resource, but
//! that resource is **global**: its `update_action_state` system merges every
//! connected gamepad and the keyboard into one set of action booleans. That is
//! perfect for a single-player menu, but it cannot answer "what does *player 2's*
//! controller want?" — which the project's multiplayer rule (up to four local
//! players) requires. So this lab keeps archie's genuinely valuable pieces — the
//! remappable [`ActionMap`] binding table, the [`GameAction`] vocabulary, and the
//! [`ControllerConfig`] deadzone/sensitivity model — and evaluates them against a
//! *single* device sample at a time. The result is per-player isolation through
//! one code path, and gameplay still only ever sees a `PlayerIntent`.

use std::collections::HashMap;

use bevy::math::Vec2;
use bevy::prelude::{GamepadAxis, GamepadButton, KeyCode};
use bevy_archie::actions::{ActionMap, GameAction};
use bevy_archie::config::ControllerConfig;
use player_input::{PlayerId, PlayerIntent};

/// A pure snapshot of one device's raw state. The ECS layer fills this from a
/// `ButtonInput<KeyCode>` or a single `&Gamepad`; the adapter never touches the
/// hardware itself.
#[derive(Clone, Debug, PartialEq)]
pub enum DeviceSample {
    /// One keyboard's currently held keys.
    Keyboard { keys: Vec<KeyCode> },
    /// One gamepad's pressed buttons and analog axes.
    Gamepad {
        buttons: Vec<GamepadButton>,
        axes: Vec<(GamepadAxis, f32)>,
    },
}

impl DeviceSample {
    /// A neutral keyboard sample (nothing held).
    pub fn idle_keyboard() -> Self {
        Self::Keyboard { keys: Vec::new() }
    }
}

/// The digital action booleans evaluated for one device against an [`ActionMap`].
/// Stored on each player so the *next* frame can detect just-pressed edges (the
/// same `was_pressed` diff archie's own `ActionState` performs).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionReading {
    pressed: HashMap<GameAction, bool>,
}

impl ActionReading {
    /// Is this action currently held on the sampled device?
    pub fn pressed(&self, action: GameAction) -> bool {
        self.pressed.get(&action).copied().unwrap_or(false)
    }

    /// Did this action transition from released to pressed since `prev`?
    pub fn just_pressed(&self, prev: &ActionReading, action: GameAction) -> bool {
        self.pressed(action) && !prev.pressed(action)
    }

    fn set(&mut self, action: GameAction, value: bool) {
        self.pressed.insert(action, value);
    }
}

/// Build the lab's [`ActionMap`]: archie's defaults plus the few keyboard bindings
/// the default table leaves to the gamepad (look, sprint, interact, climb). This is
/// the *whole* binding surface the lab exposes, and it is what runtime remapping
/// edits — proving the data-driven table, not hard-coded `KeyCode`s, owns input.
pub fn lab_action_map() -> ActionMap {
    let mut map = ActionMap::default();
    // Keyboard look (gamepad uses the right stick by default).
    map.bind_key(GameAction::LookUp, KeyCode::KeyI);
    map.bind_key(GameAction::LookDown, KeyCode::KeyK);
    map.bind_key(GameAction::LookLeft, KeyCode::KeyJ);
    map.bind_key(GameAction::LookRight, KeyCode::KeyL);
    // Keyboard sprint / interact / climb (gamepad faces/shoulders are bound already).
    map.bind_key(GameAction::LeftShoulder, KeyCode::ShiftLeft);
    map.bind_key(GameAction::Primary, KeyCode::KeyF);
    map.bind_key(GameAction::Secondary, KeyCode::KeyC);
    map
}

/// Replace every key bound to `action` with a single new key. This is how the lab's
/// runtime rebind works: it edits archie's `ActionMap`, so the next evaluation reads
/// the new binding with no code change.
pub fn rebind_key(map: &mut ActionMap, action: GameAction, key: KeyCode) {
    map.key_bindings.insert(action, vec![key]);
}

/// Evaluate archie's bindings for one device into digital action booleans. Keyboard
/// samples consult the key bindings; gamepad samples consult the button bindings.
/// Analog sticks are handled separately in [`movement_vector`] / [`look_vector`].
pub fn evaluate(map: &ActionMap, sample: &DeviceSample) -> ActionReading {
    let mut reading = ActionReading::default();
    for &action in GameAction::all() {
        let pressed = match sample {
            DeviceSample::Keyboard { keys } => map
                .key_bindings
                .get(&action)
                .is_some_and(|bound| bound.iter().any(|key| keys.contains(key))),
            DeviceSample::Gamepad { buttons, .. } => map
                .gamepad_bindings
                .get(&action)
                .is_some_and(|bound| bound.iter().any(|button| buttons.contains(button))),
        };
        reading.set(action, pressed);
    }
    reading
}

/// The full per-device derivation: archie bindings + config deadzone in, one
/// `PlayerIntent` out, plus the reading to carry into next frame's edge detection.
pub fn intent_from(
    map: &ActionMap,
    config: &ControllerConfig,
    sample: &DeviceSample,
    prev: &ActionReading,
) -> (PlayerIntent, ActionReading) {
    let reading = evaluate(map, sample);
    let intent = PlayerIntent {
        movement: movement_vector(&reading, sample, config),
        look: look_vector(&reading, sample, config),
        jump_pressed: reading.just_pressed(prev, GameAction::Confirm),
        sprint_held: reading.pressed(GameAction::LeftShoulder),
        interact_pressed: reading.just_pressed(prev, GameAction::Primary),
        interact_held: reading.pressed(GameAction::Primary),
        climb_pressed: reading.just_pressed(prev, GameAction::Secondary),
    }
    .sanitized();
    (intent, reading)
}

/// Movement intent: keyboard/d-pad digital axes, plus the left analog stick run
/// through archie's [`ControllerConfig`] circular deadzone for gamepad samples.
pub fn movement_vector(
    reading: &ActionReading,
    sample: &DeviceSample,
    config: &ControllerConfig,
) -> Vec2 {
    let digital = Vec2::new(
        bool_axis(reading, GameAction::Right, GameAction::Left),
        bool_axis(reading, GameAction::Up, GameAction::Down),
    );
    match sample {
        DeviceSample::Keyboard { .. } => digital,
        DeviceSample::Gamepad { axes, .. } => {
            let stick = config.apply_deadzone_2d(
                axis_value(axes, GamepadAxis::LeftStickX),
                axis_value(axes, GamepadAxis::LeftStickY),
                true,
            );
            digital + stick
        }
    }
}

/// Look intent: keyboard look keys, plus the right analog stick through the config
/// deadzone for gamepad samples.
pub fn look_vector(
    reading: &ActionReading,
    sample: &DeviceSample,
    config: &ControllerConfig,
) -> Vec2 {
    let digital = Vec2::new(
        bool_axis(reading, GameAction::LookRight, GameAction::LookLeft),
        bool_axis(reading, GameAction::LookUp, GameAction::LookDown),
    );
    match sample {
        DeviceSample::Keyboard { .. } => digital,
        DeviceSample::Gamepad { axes, .. } => {
            let stick = config.apply_deadzone_2d(
                axis_value(axes, GamepadAxis::RightStickX),
                axis_value(axes, GamepadAxis::RightStickY),
                false,
            );
            digital + stick
        }
    }
}

fn bool_axis(reading: &ActionReading, positive: GameAction, negative: GameAction) -> f32 {
    f32::from(reading.pressed(positive)) - f32::from(reading.pressed(negative))
}

fn axis_value(axes: &[(GamepadAxis, f32)], axis: GamepadAxis) -> f32 {
    axes.iter()
        .find(|(candidate, _)| *candidate == axis)
        .map_or(0.0, |(_, value)| *value)
}

/// A deterministic synthetic controller, used as the **scripted fallback** when a
/// player has no live device. Output depends only on the pattern and elapsed time,
/// so a scripted player stays reproducible (and recordable at the intent layer).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptPattern {
    Orbit,
    Patrol,
    Pulse,
}

impl ScriptPattern {
    /// The fallback pattern a given player drops into when its device disconnects.
    pub fn for_player(player: PlayerId) -> Self {
        match player.index() {
            1 => Self::Patrol,
            2 => Self::Pulse,
            _ => Self::Orbit,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Orbit => "scripted: orbit",
            Self::Patrol => "scripted: patrol",
            Self::Pulse => "scripted: pulse",
        }
    }
}

/// Deterministic intent for a scripted player at `elapsed` seconds.
pub fn scripted_intent(pattern: ScriptPattern, elapsed: f32) -> PlayerIntent {
    let cycle = elapsed.rem_euclid(4.0);
    match pattern {
        ScriptPattern::Orbit => PlayerIntent {
            movement: Vec2::new(elapsed.cos(), elapsed.sin()),
            look: Vec2::new((-elapsed * 0.7).sin(), (elapsed * 0.7).cos()),
            jump_pressed: pulse(elapsed, 2.0),
            sprint_held: cycle > 2.0,
            ..Default::default()
        }
        .sanitized(),
        ScriptPattern::Patrol => PlayerIntent {
            movement: Vec2::new(if cycle < 2.0 { 1.0 } else { -1.0 }, 0.0),
            look: Vec2::Y,
            interact_pressed: pulse(elapsed, 3.0),
            interact_held: pulse_held(elapsed, 3.0, 0.2),
            ..Default::default()
        }
        .sanitized(),
        ScriptPattern::Pulse => PlayerIntent {
            movement: Vec2::new(0.0, (elapsed * 1.7).sin()),
            look: Vec2::new((elapsed * 1.2).cos(), (elapsed * 1.2).sin()),
            jump_pressed: pulse(elapsed, 1.0),
            climb_pressed: pulse(elapsed + 0.5, 1.0),
            sprint_held: cycle > 3.0,
            ..Default::default()
        }
        .sanitized(),
    }
}

fn pulse(elapsed: f32, period: f32) -> bool {
    elapsed.rem_euclid(period) < 0.05
}

fn pulse_held(elapsed: f32, period: f32, width: f32) -> bool {
    elapsed.rem_euclid(period) < width
}

/// Replay one recorded `PlayerIntent` frame and advance the looping cursor. Proves
/// that recording/replay stays possible at the durable intent layer — exactly where
/// the project records tapes and sends network packets.
pub fn playback_frame(track: &[PlayerIntent], cursor: &mut usize) -> PlayerIntent {
    if track.is_empty() {
        return PlayerIntent::default();
    }
    let frame = track[*cursor % track.len()];
    *cursor = (*cursor + 1) % track.len();
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ControllerConfig {
        ControllerConfig::default()
    }

    #[test]
    fn keyboard_bindings_are_abstract_and_drive_intent() {
        let map = lab_action_map();
        let prev = ActionReading::default();
        let sample = DeviceSample::Keyboard {
            keys: vec![
                KeyCode::KeyW,
                KeyCode::KeyD,
                KeyCode::Space,
                KeyCode::ShiftLeft,
                KeyCode::KeyF,
            ],
        };

        let (intent, reading) = intent_from(&map, &config(), &sample, &prev);
        assert!((intent.movement.length() - 1.0).abs() < 0.001);
        assert!(intent.movement.x > 0.0 && intent.movement.y > 0.0);
        assert!(intent.jump_pressed, "Space is bound to Confirm = jump");
        assert!(
            intent.sprint_held,
            "Shift is bound to LeftShoulder = sprint"
        );
        assert!(intent.interact_held, "F is bound to Primary = interact");

        // Holding the same keys a second frame must not re-fire the jump edge.
        let (next, _) = intent_from(&map, &config(), &sample, &reading);
        assert!(!next.jump_pressed);
        assert!(next.interact_held);
    }

    #[test]
    fn remapping_the_action_map_moves_the_binding() {
        let mut map = lab_action_map();
        // Default interact key is F; G does nothing yet.
        let g_only = DeviceSample::Keyboard {
            keys: vec![KeyCode::KeyG],
        };
        let (before, _) = intent_from(&map, &config(), &g_only, &ActionReading::default());
        assert!(!before.interact_held);

        rebind_key(&mut map, GameAction::Primary, KeyCode::KeyG);
        let (after, _) = intent_from(&map, &config(), &g_only, &ActionReading::default());
        assert!(
            after.interact_held,
            "interact now answers to its rebound key"
        );

        // Old key no longer triggers interact.
        let f_only = DeviceSample::Keyboard {
            keys: vec![KeyCode::KeyF],
        };
        let (old, _) = intent_from(&map, &config(), &f_only, &ActionReading::default());
        assert!(!old.interact_held);
    }

    #[test]
    fn gamepad_stick_and_buttons_become_intent() {
        let map = lab_action_map();
        let prev = ActionReading::default();
        let sample = DeviceSample::Gamepad {
            buttons: vec![GamepadButton::South, GamepadButton::West],
            axes: vec![
                (GamepadAxis::LeftStickX, 0.9),
                (GamepadAxis::LeftStickY, 0.0),
            ],
        };

        let (intent, _) = intent_from(&map, &config(), &sample, &prev);
        assert!(intent.movement.x > 0.4, "left stick pushes movement right");
        assert!(intent.jump_pressed, "South is bound to Confirm = jump");
        assert!(intent.interact_held, "West is bound to Primary = interact");
    }

    #[test]
    fn analog_deadzone_swallows_stick_drift() {
        let map = lab_action_map();
        let drift = DeviceSample::Gamepad {
            buttons: Vec::new(),
            axes: vec![
                (GamepadAxis::LeftStickX, 0.05),
                (GamepadAxis::LeftStickY, 0.05),
            ],
        };
        let (intent, _) = intent_from(&map, &config(), &drift, &ActionReading::default());
        assert_eq!(
            intent.movement,
            Vec2::ZERO,
            "drift below deadzone is ignored"
        );
    }

    #[test]
    fn scripted_patterns_are_deterministic() {
        let a = scripted_intent(ScriptPattern::Patrol, 1.5);
        let b = scripted_intent(ScriptPattern::Patrol, 1.5);
        assert_eq!(a, b);
        // Same input, different time, generally differs.
        let c = scripted_intent(ScriptPattern::Patrol, 3.5);
        assert_ne!(a.movement, c.movement);
    }

    #[test]
    fn playback_loops_recorded_frames_in_order() {
        let track = [
            PlayerIntent {
                movement: Vec2::X,
                ..Default::default()
            },
            PlayerIntent {
                jump_pressed: true,
                ..Default::default()
            },
        ];
        let mut cursor = 0;
        assert_eq!(playback_frame(&track, &mut cursor), track[0]);
        assert_eq!(playback_frame(&track, &mut cursor), track[1]);
        assert_eq!(playback_frame(&track, &mut cursor), track[0]);
        assert_eq!(cursor, 1);
    }

    #[test]
    fn for_player_assigns_distinct_fallbacks() {
        assert_eq!(
            ScriptPattern::for_player(PlayerId(1)),
            ScriptPattern::Patrol
        );
        assert_eq!(ScriptPattern::for_player(PlayerId(2)), ScriptPattern::Pulse);
        assert_eq!(ScriptPattern::for_player(PlayerId(3)), ScriptPattern::Orbit);
    }
}
