use std::fmt;

use bevy::prelude::*;
pub use player_input::{PlayerId, PlayerIntent};
pub type RebindCapture = player_input::RebindCapture<PlayerId>;

pub const PLAYER_COUNT: usize = 4;
pub const PLAYERS: [PlayerId; PLAYER_COUNT] = [PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)];
pub const MAX_RECORDING_FRAMES: usize = 900;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardSlot {
    A,
    B,
}

impl fmt::Display for KeyboardSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::A => formatter.write_str("Keyboard A"),
            Self::B => formatter.write_str("Keyboard B"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GamepadId(pub u8);

impl fmt::Display for GamepadId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Gamepad {}", self.0 + 1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HumanDevice {
    Keyboard(KeyboardSlot),
    Gamepad(GamepadId),
}

impl fmt::Display for HumanDevice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keyboard(slot) => slot.fmt(formatter),
            Self::Gamepad(id) => id.fmt(formatter),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptPattern {
    Orbit,
    Patrol,
    ActionPulse,
    FigureEight,
}

impl ScriptPattern {
    pub fn for_player(player: PlayerId) -> Self {
        match player.index() {
            0 => Self::Orbit,
            1 => Self::Patrol,
            2 => Self::ActionPulse,
            _ => Self::FigureEight,
        }
    }
}

impl fmt::Display for ScriptPattern {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Orbit => formatter.write_str("Script: Orbit"),
            Self::Patrol => formatter.write_str("Script: Patrol"),
            Self::ActionPulse => formatter.write_str("Script: Action pulse"),
            Self::FigureEight => formatter.write_str("Script: Figure eight"),
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlSource {
    Human(HumanDevice),
    Scripted(ScriptPattern),
    Playback { cursor: usize },
}

impl ControlSource {
    pub fn label(self) -> String {
        match self {
            Self::Human(device) => device.to_string(),
            Self::Scripted(pattern) => pattern.to_string(),
            Self::Playback { cursor } => format!("Playback frame {cursor}"),
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct ProbeState {
    pub position: Vec2,
    pub spawn_position: Vec2,
}

#[derive(Clone, Copy, Debug)]
pub struct KeyboardBindingSet {
    pub move_up: KeyCode,
    pub move_down: KeyCode,
    pub move_left: KeyCode,
    pub move_right: KeyCode,
    pub look_up: KeyCode,
    pub look_down: KeyCode,
    pub look_left: KeyCode,
    pub look_right: KeyCode,
    pub jump: KeyCode,
    pub sprint: KeyCode,
    pub interact: KeyCode,
    pub climb: KeyCode,
}

impl KeyboardBindingSet {
    pub fn keyboard_a() -> Self {
        Self {
            move_up: KeyCode::KeyW,
            move_down: KeyCode::KeyS,
            move_left: KeyCode::KeyA,
            move_right: KeyCode::KeyD,
            look_up: KeyCode::KeyI,
            look_down: KeyCode::KeyK,
            look_left: KeyCode::KeyJ,
            look_right: KeyCode::KeyL,
            jump: KeyCode::Space,
            sprint: KeyCode::ShiftLeft,
            interact: KeyCode::KeyE,
            climb: KeyCode::KeyQ,
        }
    }

    pub fn keyboard_b() -> Self {
        Self {
            move_up: KeyCode::ArrowUp,
            move_down: KeyCode::ArrowDown,
            move_left: KeyCode::ArrowLeft,
            move_right: KeyCode::ArrowRight,
            look_up: KeyCode::Numpad8,
            look_down: KeyCode::Numpad5,
            look_left: KeyCode::Numpad4,
            look_right: KeyCode::Numpad6,
            jump: KeyCode::Numpad0,
            sprint: KeyCode::ControlRight,
            interact: KeyCode::Numpad1,
            climb: KeyCode::Numpad2,
        }
    }

    pub fn read(self, keyboard: &ButtonInput<KeyCode>) -> PlayerIntent {
        PlayerIntent {
            movement: digital_axis(
                keyboard.pressed(self.move_left),
                keyboard.pressed(self.move_right),
                keyboard.pressed(self.move_down),
                keyboard.pressed(self.move_up),
            ),
            look: digital_axis(
                keyboard.pressed(self.look_left),
                keyboard.pressed(self.look_right),
                keyboard.pressed(self.look_down),
                keyboard.pressed(self.look_up),
            ),
            jump_pressed: keyboard.just_pressed(self.jump),
            sprint_held: keyboard.pressed(self.sprint),
            interact_pressed: keyboard.just_pressed(self.interact),
            interact_held: keyboard.pressed(self.interact),
            climb_pressed: keyboard.just_pressed(self.climb),
        }
        .sanitized()
    }
}

fn digital_axis(left: bool, right: bool, down: bool, up: bool) -> Vec2 {
    Vec2::new(
        f32::from(u8::from(right)) - f32::from(u8::from(left)),
        f32::from(u8::from(up)) - f32::from(u8::from(down)),
    )
}

#[derive(Resource, Clone, Debug)]
pub struct KeyboardBindings {
    pub slots: [KeyboardBindingSet; 2],
}

impl Default for KeyboardBindings {
    fn default() -> Self {
        Self {
            slots: [
                KeyboardBindingSet::keyboard_a(),
                KeyboardBindingSet::keyboard_b(),
            ],
        }
    }
}

impl KeyboardBindings {
    pub fn get(&self, slot: KeyboardSlot) -> KeyboardBindingSet {
        self.slots[match slot {
            KeyboardSlot::A => 0,
            KeyboardSlot::B => 1,
        }]
    }

    pub fn get_mut(&mut self, slot: KeyboardSlot) -> &mut KeyboardBindingSet {
        &mut self.slots[match slot {
            KeyboardSlot::A => 0,
            KeyboardSlot::B => 1,
        }]
    }
}

#[derive(Resource, Debug)]
pub struct RecordingBank {
    pub tracks: [Vec<PlayerIntent>; PLAYER_COUNT],
    pub recording: Option<PlayerId>,
}

impl Default for RecordingBank {
    fn default() -> Self {
        Self {
            tracks: std::array::from_fn(|_| Vec::new()),
            recording: None,
        }
    }
}

impl RecordingBank {
    pub fn track(&self, player: PlayerId) -> &[PlayerIntent] {
        &self.tracks[player.index()]
    }

    pub fn track_mut(&mut self, player: PlayerId) -> &mut Vec<PlayerIntent> {
        &mut self.tracks[player.index()]
    }

    pub fn begin(&mut self, player: PlayerId) {
        self.track_mut(player).clear();
        self.recording = Some(player);
    }

    pub fn stop(&mut self) {
        self.recording = None;
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct LabRuntime {
    pub selected_player: PlayerId,
    pub focused: bool,
    pub focus_losses: u32,
    pub reset_count: u32,
}

impl Default for LabRuntime {
    fn default() -> Self {
        Self {
            selected_player: PlayerId(0),
            focused: true,
            focus_losses: 0,
            reset_count: 0,
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct LabNotice(pub String);

#[derive(Resource, Debug, Default)]
pub struct ResetRequested(pub bool);

pub fn scripted_intent(pattern: ScriptPattern, elapsed: f32) -> PlayerIntent {
    let cycle = elapsed % 4.0;
    match pattern {
        ScriptPattern::Orbit => PlayerIntent {
            movement: Vec2::new(elapsed.cos(), elapsed.sin()),
            look: Vec2::new((-elapsed * 0.7).sin(), (elapsed * 0.7).cos()),
            sprint_held: cycle > 2.0,
            jump_pressed: crossed_pulse(elapsed, 2.0, 0.04),
            ..default()
        },
        ScriptPattern::Patrol => PlayerIntent {
            movement: Vec2::new(if cycle < 2.0 { 1.0 } else { -1.0 }, 0.0),
            look: Vec2::Y,
            interact_pressed: crossed_pulse(elapsed, 3.0, 0.04),
            interact_held: crossed_pulse(elapsed, 3.0, 0.20),
            ..default()
        },
        ScriptPattern::ActionPulse => PlayerIntent {
            movement: Vec2::new(0.0, (elapsed * 1.7).sin()),
            look: Vec2::new((elapsed * 1.2).cos(), (elapsed * 1.2).sin()),
            jump_pressed: crossed_pulse(elapsed, 1.0, 0.04),
            interact_pressed: crossed_pulse(elapsed + 0.25, 1.0, 0.04),
            interact_held: crossed_pulse(elapsed + 0.25, 1.0, 0.24),
            climb_pressed: crossed_pulse(elapsed + 0.5, 1.0, 0.04),
            sprint_held: cycle > 3.0,
        },
        ScriptPattern::FigureEight => PlayerIntent {
            movement: Vec2::new(elapsed.sin(), (elapsed * 2.0).sin()).normalize_or_zero(),
            look: Vec2::new(elapsed.cos(), (elapsed * 0.5).sin()).normalize_or_zero(),
            climb_pressed: crossed_pulse(elapsed, 2.5, 0.04),
            ..default()
        },
    }
}

fn crossed_pulse(elapsed: f32, period: f32, width: f32) -> bool {
    elapsed.rem_euclid(period) < width
}

pub fn playback_frame(track: &[PlayerIntent], cursor: &mut usize) -> PlayerIntent {
    if track.is_empty() {
        return PlayerIntent::default();
    }

    let frame = track[*cursor % track.len()];
    *cursor = (*cursor + 1) % track.len();
    frame
}
