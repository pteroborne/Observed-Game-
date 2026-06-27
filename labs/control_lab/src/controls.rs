use bevy::{
    ecs::system::SystemParam,
    input::gamepad::{Gamepad, GamepadButton},
    prelude::*,
    window::WindowFocused,
};

use crate::model::{
    ControlSource, GamepadId, HumanDevice, KeyboardBindings, KeyboardSlot, LabNotice, LabRuntime,
    MAX_RECORDING_FRAMES, PlayerId, PlayerIntent, RebindCapture, RecordingBank, ResetRequested,
    ScriptPattern, playback_frame, scripted_intent,
};

#[derive(Clone, Copy, Debug)]
pub struct GamepadDevice {
    pub id: GamepadId,
    pub entity: Entity,
}

#[derive(Resource, Debug, Default)]
pub struct GamepadRegistry {
    pub devices: Vec<GamepadDevice>,
    next_id: u8,
}

impl GamepadRegistry {
    pub fn entity_for(&self, id: GamepadId) -> Option<Entity> {
        self.devices
            .iter()
            .find(|device| device.id == id)
            .map(|device| device.entity)
    }

    pub fn connected_ids(&self) -> impl Iterator<Item = GamepadId> + '_ {
        self.devices.iter().map(|device| device.id)
    }
}

#[derive(Message, Clone, Copy, Debug)]
pub enum LabCommand {
    Select(PlayerId),
    ToggleHumanScripted,
    CycleDevice,
    ToggleRecording,
    PlayRecording,
    BeginJumpRebind,
    AssignGamepad(GamepadId),
    Reset,
}

#[derive(SystemParam)]
pub(crate) struct CommandContext<'w, 's> {
    runtime: ResMut<'w, LabRuntime>,
    registry: Res<'w, GamepadRegistry>,
    recordings: ResMut<'w, RecordingBank>,
    rebind: ResMut<'w, RebindCapture>,
    reset: ResMut<'w, ResetRequested>,
    notice: ResMut<'w, LabNotice>,
    players: Query<'w, 's, (&'static PlayerId, &'static mut ControlSource)>,
}

#[derive(SystemParam)]
pub(crate) struct IntentContext<'w, 's> {
    time: Res<'w, Time>,
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    bindings: Res<'w, KeyboardBindings>,
    runtime: Res<'w, LabRuntime>,
    registry: Res<'w, GamepadRegistry>,
    recordings: Res<'w, RecordingBank>,
    gamepads: Query<'w, 's, &'static Gamepad>,
    players: Query<
        'w,
        's,
        (
            &'static PlayerId,
            &'static mut ControlSource,
            &'static mut PlayerIntent,
        ),
    >,
}

pub(crate) fn update_window_focus(
    mut events: MessageReader<WindowFocused>,
    mut runtime: ResMut<LabRuntime>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut notice: ResMut<LabNotice>,
) {
    for event in events.read() {
        if runtime.focused == event.focused {
            continue;
        }

        runtime.focused = event.focused;
        if event.focused {
            notice.0 = "Window focus restored; hardware input is live.".to_string();
        } else {
            runtime.focus_losses += 1;
            keyboard.clear();
            notice.0 = "Focus lost; all intents neutralized and playback frozen.".to_string();
        }
    }
}

pub(crate) fn sync_gamepads(
    gamepads: Query<(Entity, &Gamepad)>,
    mut registry: ResMut<GamepadRegistry>,
    runtime: Res<LabRuntime>,
    mut commands: MessageWriter<LabCommand>,
    mut notice: ResMut<LabNotice>,
) {
    let connected_entities = gamepads
        .iter()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    let removed = registry
        .devices
        .iter()
        .filter(|device| !connected_entities.contains(&device.entity))
        .map(|device| device.id)
        .collect::<Vec<_>>();
    registry
        .devices
        .retain(|device| connected_entities.contains(&device.entity));

    for id in removed {
        notice.0 = format!("{id} disconnected; affected player will fall back to script.");
    }

    for (entity, gamepad) in &gamepads {
        let id = if let Some(device) = registry
            .devices
            .iter()
            .find(|device| device.entity == entity)
        {
            device.id
        } else {
            let id = GamepadId(registry.next_id);
            registry.next_id = registry.next_id.saturating_add(1);
            registry.devices.push(GamepadDevice { id, entity });
            notice.0 = format!("{id} connected. Press controller Start to claim a player.");
            id
        };

        if gamepad.just_pressed(GamepadButton::Start) {
            commands.write(LabCommand::Select(runtime.selected_player));
            commands.write(LabCommand::AssignGamepad(id));
        }
    }
}

pub(crate) fn release_disconnected_gamepads(
    registry: Res<GamepadRegistry>,
    mut players: Query<(&PlayerId, &mut ControlSource)>,
) {
    for (player, mut source) in &mut players {
        let ControlSource::Human(HumanDevice::Gamepad(id)) = *source else {
            continue;
        };

        if registry.entity_for(id).is_none() {
            *source = ControlSource::Scripted(ScriptPattern::for_player(*player));
        }
    }
}

pub(crate) fn capture_rebind(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut capture: ResMut<RebindCapture>,
    players: Query<(&PlayerId, &ControlSource)>,
    mut bindings: ResMut<KeyboardBindings>,
    mut notice: ResMut<LabNotice>,
) {
    let Some(player) = capture.player else {
        return;
    };

    if keyboard.just_pressed(KeyCode::Escape) {
        capture.player = None;
        notice.0 = "Jump rebind cancelled.".to_string();
        return;
    }

    let Some(key) = keyboard.get_just_pressed().next().copied() else {
        return;
    };

    let slot = players
        .iter()
        .find(|(candidate, _)| **candidate == player)
        .and_then(|(_, source)| match source {
            ControlSource::Human(HumanDevice::Keyboard(slot)) => Some(*slot),
            _ => None,
        });

    if let Some(slot) = slot {
        bindings.get_mut(slot).jump = key;
        notice.0 = format!("{} Jump rebound to {key:?}.", player.label());
    } else {
        notice.0 = "Rebinding requires a human keyboard source.".to_string();
    }
    capture.player = None;
}

pub(crate) fn keyboard_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    capture: Res<RebindCapture>,
    mut commands: MessageWriter<LabCommand>,
) {
    if capture.player.is_some() {
        return;
    }

    for (key, player) in [
        (KeyCode::Digit1, PlayerId(0)),
        (KeyCode::Digit2, PlayerId(1)),
        (KeyCode::Digit3, PlayerId(2)),
        (KeyCode::Digit4, PlayerId(3)),
    ] {
        if keyboard.just_pressed(key) {
            commands.write(LabCommand::Select(player));
        }
    }

    for (key, command) in [
        (KeyCode::Tab, LabCommand::ToggleHumanScripted),
        (KeyCode::KeyC, LabCommand::CycleDevice),
        (KeyCode::F5, LabCommand::ToggleRecording),
        (KeyCode::F6, LabCommand::PlayRecording),
        (KeyCode::F7, LabCommand::BeginJumpRebind),
        (KeyCode::F8, LabCommand::Reset),
    ] {
        if keyboard.just_pressed(key) {
            commands.write(command);
        }
    }
}

pub(crate) fn process_commands(
    mut commands: MessageReader<LabCommand>,
    mut context: CommandContext,
) {
    for command in commands.read() {
        match *command {
            LabCommand::Select(player) => {
                context.runtime.selected_player = player;
                context.notice.0 = format!("{} selected.", player.label());
            }
            LabCommand::ToggleHumanScripted => {
                let target = context.runtime.selected_player;
                let current = source_for(&mut context.players, target);
                match current {
                    Some(ControlSource::Human(_)) => {
                        set_source(
                            &mut context.players,
                            target,
                            ControlSource::Scripted(ScriptPattern::for_player(target)),
                        );
                        context.notice.0 =
                            format!("{} switched to scripted control.", target.label());
                    }
                    _ => {
                        let device =
                            first_available_device(&mut context.players, &context.registry);
                        assign_device(&mut context.players, target, device);
                        context.notice.0 = format!("{} switched to {device}.", target.label());
                    }
                }
            }
            LabCommand::CycleDevice => {
                let target = context.runtime.selected_player;
                let devices = available_devices(&context.registry);
                let current =
                    source_for(&mut context.players, target).and_then(|source| match source {
                        ControlSource::Human(device) => Some(device),
                        _ => None,
                    });
                let next_index = current
                    .and_then(|device| devices.iter().position(|candidate| *candidate == device))
                    .map_or(0, |index| (index + 1) % devices.len());
                let device = devices[next_index];
                assign_device(&mut context.players, target, device);
                context.notice.0 = format!("{} assigned to {device}.", target.label());
            }
            LabCommand::ToggleRecording => {
                let target = context.runtime.selected_player;
                if context.recordings.recording == Some(target) {
                    let frames = context.recordings.track(target).len();
                    context.recordings.stop();
                    context.notice.0 = format!("Recorded {frames} frames for {}.", target.label());
                } else {
                    context.recordings.begin(target);
                    context.notice.0 = format!("Recording {} from a fresh tape.", target.label());
                }
            }
            LabCommand::PlayRecording => {
                let target = context.runtime.selected_player;
                if context.recordings.track(target).is_empty() {
                    context.notice.0 = format!("{} has no recording yet.", target.label());
                } else {
                    context.recordings.stop();
                    set_source(
                        &mut context.players,
                        target,
                        ControlSource::Playback { cursor: 0 },
                    );
                    context.notice.0 = format!("{} started looping playback.", target.label());
                }
            }
            LabCommand::BeginJumpRebind => {
                let target = context.runtime.selected_player;
                let source = source_for(&mut context.players, target);
                if matches!(source, Some(ControlSource::Human(HumanDevice::Keyboard(_)))) {
                    context.rebind.player = Some(target);
                    context.notice.0 = format!(
                        "Press a new Jump key for {} (Escape cancels).",
                        target.label()
                    );
                } else {
                    context.notice.0 =
                        "Select a human keyboard player before rebinding.".to_string();
                }
            }
            LabCommand::AssignGamepad(id) => {
                let target = context.runtime.selected_player;
                assign_device(&mut context.players, target, HumanDevice::Gamepad(id));
                context.notice.0 = format!("{id} assigned exclusively to {}.", target.label());
            }
            LabCommand::Reset => {
                context.reset.0 = true;
            }
        }
    }
}

pub(crate) fn build_player_intents(mut context: IntentContext) {
    for (player, mut source, mut intent) in &mut context.players {
        if !context.runtime.focused {
            *intent = PlayerIntent::default();
            continue;
        }

        *intent = match &mut *source {
            ControlSource::Human(HumanDevice::Keyboard(slot)) => {
                context.bindings.get(*slot).read(&context.keyboard)
            }
            ControlSource::Human(HumanDevice::Gamepad(id)) => context
                .registry
                .entity_for(*id)
                .and_then(|entity| context.gamepads.get(entity).ok())
                .map(read_gamepad)
                .unwrap_or_default(),
            ControlSource::Scripted(pattern) => {
                scripted_intent(*pattern, context.time.elapsed_secs())
            }
            ControlSource::Playback { cursor } => {
                playback_frame(context.recordings.track(*player), cursor)
            }
        };
    }
}

pub(crate) fn record_intents(
    mut recordings: ResMut<RecordingBank>,
    players: Query<(&PlayerId, &PlayerIntent)>,
    mut notice: ResMut<LabNotice>,
) {
    let Some(target) = recordings.recording else {
        return;
    };
    let Some((_, intent)) = players.iter().find(|(player, _)| **player == target) else {
        return;
    };

    let track = recordings.track_mut(target);
    track.push(*intent);
    if track.len() >= MAX_RECORDING_FRAMES {
        recordings.stop();
        notice.0 = format!(
            "{} recording reached the {} frame limit.",
            target.label(),
            MAX_RECORDING_FRAMES
        );
    }
}

pub fn read_gamepad(gamepad: &Gamepad) -> PlayerIntent {
    PlayerIntent {
        movement: apply_deadzone(gamepad.left_stick(), 0.18),
        look: apply_deadzone(gamepad.right_stick(), 0.18),
        jump_pressed: gamepad.just_pressed(GamepadButton::South),
        sprint_held: gamepad.pressed(GamepadButton::LeftTrigger2)
            || gamepad.pressed(GamepadButton::LeftThumb),
        interact_pressed: gamepad.just_pressed(GamepadButton::West),
        interact_held: gamepad.pressed(GamepadButton::West),
        climb_pressed: gamepad.just_pressed(GamepadButton::North),
    }
    .sanitized()
}

pub fn apply_deadzone(value: Vec2, deadzone: f32) -> Vec2 {
    let length = value.length();
    if length <= deadzone {
        return Vec2::ZERO;
    }
    let normalized_magnitude = ((length - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0);
    value.normalize_or_zero() * normalized_magnitude
}

fn available_devices(registry: &GamepadRegistry) -> Vec<HumanDevice> {
    let mut devices = vec![
        HumanDevice::Keyboard(KeyboardSlot::A),
        HumanDevice::Keyboard(KeyboardSlot::B),
    ];
    devices.extend(registry.connected_ids().map(HumanDevice::Gamepad));
    devices
}

fn first_available_device(
    players: &mut Query<(&PlayerId, &mut ControlSource)>,
    registry: &GamepadRegistry,
) -> HumanDevice {
    available_devices(registry)
        .into_iter()
        .find(|device| {
            !players
                .iter_mut()
                .any(|(_, source)| *source == ControlSource::Human(*device))
        })
        .unwrap_or(HumanDevice::Keyboard(KeyboardSlot::A))
}

fn source_for(
    players: &mut Query<(&PlayerId, &mut ControlSource)>,
    target: PlayerId,
) -> Option<ControlSource> {
    players
        .iter_mut()
        .find(|(player, _)| **player == target)
        .map(|(_, source)| *source)
}

fn set_source(
    players: &mut Query<(&PlayerId, &mut ControlSource)>,
    target: PlayerId,
    new_source: ControlSource,
) {
    for (player, mut source) in players.iter_mut() {
        if *player == target {
            *source = new_source;
            return;
        }
    }
}

fn assign_device(
    players: &mut Query<(&PlayerId, &mut ControlSource)>,
    target: PlayerId,
    device: HumanDevice,
) {
    for (player, mut source) in players.iter_mut() {
        if *player != target && *source == ControlSource::Human(device) {
            *source = ControlSource::Scripted(ScriptPattern::for_player(*player));
        }
    }
    set_source(players, target, ControlSource::Human(device));
}

#[cfg(test)]
pub(crate) fn assign_device_in_slice(
    players: &mut [(PlayerId, ControlSource)],
    target: PlayerId,
    device: HumanDevice,
) {
    for (player, source) in players.iter_mut() {
        if *player != target && *source == ControlSource::Human(device) {
            *source = ControlSource::Scripted(ScriptPattern::for_player(*player));
        }
    }
    if let Some((_, source)) = players.iter_mut().find(|(player, _)| *player == target) {
        *source = ControlSource::Human(device);
    }
}
