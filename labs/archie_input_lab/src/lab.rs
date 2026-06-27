//! The ECS layer: `bevy_archie`'s live systems (detection, ownership, haptics) feed
//! four local players, each of which derives a [`PlayerIntent`] through the pure
//! [`crate::adapter`]. Presentation is a 2D dashboard of intent probes and a
//! diagnostics overlay — never the source of truth.

use std::time::Duration;

use bevy::{ecs::system::SystemParam, input::ButtonInput, prelude::*, window::WindowFocused};
use bevy_archie::actions::{ActionMap, GameAction};
use bevy_archie::config::ControllerConfig;
use bevy_archie::detection::{InputDevice, InputDeviceState};
use bevy_archie::haptics::{RumblePattern, RumbleRequest};
use bevy_archie::multiplayer::{
    ControllerOwnership, ControllerUnassigned, PlayerId as ArchiePlayerId,
};
use observed_style::{MarkerRole, SurfaceRole, marker, surface};
use player_input::{PlayerId, PlayerIntent};

use crate::adapter::{
    ActionReading, DeviceSample, ScriptPattern, intent_from, lab_action_map, playback_frame,
    rebind_key, scripted_intent,
};

/// Four local players, no single-player assumptions.
pub const PLAYER_COUNT: usize = 4;
/// The canonical player ids in iteration order.
pub const PLAYERS: [PlayerId; PLAYER_COUNT] = [PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)];
const MAX_RECORDING_FRAMES: usize = 600;
const PROBE_RADIUS: f32 = 70.0;

#[derive(Component)]
pub struct LabSpawned;

#[derive(Component)]
pub struct ArchieLabCamera;

#[derive(Component)]
pub struct ArchieLabUiRoot;

#[derive(Component)]
pub struct ProbeVisual;

#[derive(Component)]
pub(crate) struct OverlayPanel;

#[derive(Component)]
pub(crate) struct DiagnosticsText;

#[derive(Component, Clone, Copy)]
pub(crate) struct Anchor(Vec2);

#[derive(Component, Clone)]
pub struct PrevReading(pub ActionReading);

/// How a player's intent is currently produced. The whole point of the lab is that
/// every variant lands in the same `PlayerIntent`, so gameplay never branches on it.
#[derive(Component, Clone, Debug)]
pub enum Source {
    /// The local keyboard, evaluated through archie's `ActionMap`.
    Keyboard,
    /// An owned gamepad entity, evaluated through archie's `ActionMap` + config.
    Gamepad(Entity),
    /// A deterministic synthetic controller (fallback / demo).
    Scripted(ScriptPattern),
    /// Looping replay of a recorded intent tape.
    Playback { cursor: usize },
}

impl Source {
    pub fn label(&self) -> String {
        match self {
            Source::Keyboard => "keyboard".to_string(),
            Source::Gamepad(_) => "gamepad".to_string(),
            Source::Scripted(pattern) => pattern.label().to_string(),
            Source::Playback { cursor } => format!("playback @{cursor}"),
        }
    }

    fn is_scripted(&self) -> bool {
        matches!(self, Source::Scripted(_))
    }

    fn is_human(&self) -> bool {
        matches!(self, Source::Keyboard | Source::Gamepad(_))
    }
}

#[derive(Resource)]
pub struct LabRuntime {
    pub selected: PlayerId,
    pub focused: bool,
    pub focus_losses: u32,
    pub reset_count: u32,
    pub debug_visible: bool,
    pub haptics_enabled: bool,
    pub pulse_requested: bool,
    pub pulses: u32,
    pub last_pulse: Option<PlayerId>,
}

impl Default for LabRuntime {
    fn default() -> Self {
        Self {
            selected: PlayerId(0),
            focused: true,
            focus_losses: 0,
            reset_count: 0,
            debug_visible: true,
            haptics_enabled: true,
            pulse_requested: false,
            pulses: 0,
            last_pulse: None,
        }
    }
}

#[derive(Resource, Default)]
pub struct LabNotice(pub String);

#[derive(Resource, Default)]
pub struct ResetRequested(pub bool);

#[derive(Resource, Default)]
pub struct RebindCapture {
    pub active: bool,
}

/// Per-player recorded intent tapes — recording/replay lives at the durable intent
/// layer, exactly where the project's replay and networking already operate.
#[derive(Resource)]
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

    fn track_mut(&mut self, player: PlayerId) -> &mut Vec<PlayerIntent> {
        &mut self.tracks[player.index()]
    }

    pub fn begin(&mut self, player: PlayerId) {
        self.track_mut(player).clear();
        self.recording = Some(player);
    }

    fn stop(&mut self) {
        self.recording = None;
    }
}

/// (Re)build the whole scene: archie's binding table + ownership reset, the camera,
/// four player probes, and the overlay. Called at startup and on every reset.
pub fn setup_lab(
    mut commands: Commands,
    mut action_map: ResMut<ActionMap>,
    mut ownership: ResMut<ControllerOwnership>,
) {
    // The lab owns assignment policy, so disable archie auto-assign and start clean.
    *action_map = lab_action_map();
    ownership.auto_assign = false;
    ownership.owners.clear();
    ownership.assignments.clear();

    commands.spawn((
        Camera2d,
        ArchieLabCamera,
        LabSpawned,
        Name::new("Archie Input Lab Camera"),
    ));

    let anchors = [
        Vec2::new(-380.0, 200.0),
        Vec2::new(380.0, 200.0),
        Vec2::new(-380.0, -210.0),
        Vec2::new(380.0, -210.0),
    ];
    let roles = [
        MarkerRole::You,
        MarkerRole::Teammate,
        MarkerRole::Rival,
        MarkerRole::Director,
    ];

    for (idx, player) in PLAYERS.iter().enumerate() {
        let source = if idx == 0 {
            Source::Keyboard
        } else {
            Source::Scripted(ScriptPattern::for_player(*player))
        };
        let color = marker(roles[idx]).base_color;

        // The translucent "field" the probe roams within.
        commands.spawn((
            LabSpawned,
            Sprite {
                color: color.with_alpha(0.12),
                custom_size: Some(Vec2::splat(2.0 * PROBE_RADIUS)),
                ..default()
            },
            Transform::from_translation(anchors[idx].extend(-1.0)),
            Name::new("probe field"),
        ));

        commands.spawn((
            LabSpawned,
            ProbeVisual,
            *player,
            source,
            PlayerIntent::default(),
            PrevReading(ActionReading::default()),
            Anchor(anchors[idx]),
            Sprite {
                color,
                custom_size: Some(Vec2::splat(46.0)),
                ..default()
            },
            Transform::from_translation(anchors[idx].extend(0.0)),
            Name::new(format!("probe {}", player.label())),
        ));
    }

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            LabSpawned,
            ArchieLabUiRoot,
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(16)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(20),
            Name::new("Archie Input Lab UI Root"),
        ))
        .with_children(|root| {
            root.spawn((
                OverlayPanel,
                Node {
                    width: px(580),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(surface(SurfaceRole::Wall).base_color.with_alpha(0.94)),
                BorderColor::all(marker(MarkerRole::You).base_color.with_alpha(0.6)),
                children![(
                    DiagnosticsText,
                    Text::new("Archie input diagnostics starting..."),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.95, 1.0)),
                )],
            ));
            root.spawn((
                OverlayPanel,
                Node {
                    width: px(430),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(surface(SurfaceRole::Wall).base_color.with_alpha(0.94)),
                BorderColor::all(marker(MarkerRole::Control).base_color.with_alpha(0.6)),
                children![(
                    Text::new(help_text()),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.86, 0.93, 1.0)),
                )],
            ));
        });
}

fn help_text() -> String {
    "ARCHIE INPUT LAB (Phase A6)\n\
     1-4 select player   Tab keyboard/scripted\n\n\
     P1 keyboard:\n\
       WASD move   IJKL look\n\
       Space jump  Shift sprint\n\
       F interact  C climb\n\
     P2-P4 scripted until a controller\n\
     claims a free slot.\n\n\
     F5 record   F6 replay (intent layer)\n\
     F7 rebind interact\n\
     H haptics toggle   G hazard pulse\n\
     R reset   F1 overlay\n\n\
     Controllers feed the SAME PlayerIntent\n\
     as the keyboard; gameplay never reads\n\
     a device directly."
        .to_string()
}

/// Claim any connected-but-unowned gamepad into the lowest free scripted slot
/// (players 2-4; player 1 stays the keyboard human). Idempotent, so it also
/// re-claims a still-connected pad after a reset.
pub fn assign_gamepads(
    gamepads: Query<Entity, With<Gamepad>>,
    mut ownership: ResMut<ControllerOwnership>,
    mut players: Query<(&PlayerId, &mut Source)>,
    mut notice: ResMut<LabNotice>,
) {
    for gamepad in &gamepads {
        if ownership.is_assigned(gamepad) {
            continue;
        }
        let target = (1..PLAYER_COUNT).find(|idx| {
            players
                .iter()
                .any(|(player, source)| player.index() == *idx && source.is_scripted())
        });
        let Some(idx) = target else { continue };

        ownership.assign(gamepad, ArchiePlayerId(idx as u8));
        for (player, mut source) in &mut players {
            if player.index() == idx {
                *source = Source::Gamepad(gamepad);
            }
        }
        notice.0 = format!("Controller claimed {}.", PlayerId(idx as u16).label());
    }
}

/// When archie reports a controller unassigned (disconnect), drop that player back
/// to its scripted fallback — proving graceful per-player recovery.
pub fn handle_unassigned(
    mut events: MessageReader<ControllerUnassigned>,
    mut players: Query<(&PlayerId, &mut Source)>,
    mut notice: ResMut<LabNotice>,
) {
    for event in events.read() {
        let idx = event.player.0 as usize;
        for (player, mut source) in &mut players {
            if player.index() == idx {
                *source = Source::Scripted(ScriptPattern::for_player(*player));
                notice.0 = format!("{} controller lost; reverted to script.", player.label());
            }
        }
    }
}

pub fn update_focus(
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
            notice.0 = "Focus restored; live input resumes.".to_string();
        } else {
            runtime.focus_losses += 1;
            keyboard.clear();
            notice.0 = "Focus lost; all intents neutralized.".to_string();
        }
    }
}

/// Capture the next key as the new interact (Primary) binding in archie's
/// `ActionMap`. Runs *before* the shortcut system so the F7 trigger itself is not
/// consumed as the new binding.
pub fn capture_rebind(
    mut rebind: ResMut<RebindCapture>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut action_map: ResMut<ActionMap>,
    mut notice: ResMut<LabNotice>,
) {
    if !rebind.active {
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        rebind.active = false;
        notice.0 = "Rebind cancelled.".to_string();
        return;
    }
    let Some(key) = keyboard
        .get_just_pressed()
        .find(|key| **key != KeyCode::F7)
        .copied()
    else {
        return;
    };
    rebind_key(&mut action_map, GameAction::Primary, key);
    rebind.active = false;
    notice.0 = format!("Interact rebound to {key:?}.");
}

pub fn keyboard_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<LabRuntime>,
    mut reset: ResMut<ResetRequested>,
    mut recordings: ResMut<RecordingBank>,
    mut rebind: ResMut<RebindCapture>,
    mut notice: ResMut<LabNotice>,
    mut players: Query<(&PlayerId, &mut Source)>,
) {
    if rebind.active {
        return;
    }

    for (key, idx) in [
        (KeyCode::Digit1, 0u16),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
    ] {
        if keyboard.just_pressed(key) {
            runtime.selected = PlayerId(idx);
            notice.0 = format!("{} selected.", PlayerId(idx).label());
        }
    }

    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        reset.0 = true;
    }
    if keyboard.just_pressed(KeyCode::KeyH) {
        runtime.haptics_enabled = !runtime.haptics_enabled;
        notice.0 = format!(
            "Haptics {}.",
            if runtime.haptics_enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
    }
    if keyboard.just_pressed(KeyCode::KeyG) {
        runtime.pulse_requested = true;
    }
    if keyboard.just_pressed(KeyCode::F7) {
        rebind.active = true;
        notice.0 = "Press a new interact key (Esc cancels).".to_string();
    }

    let selected = runtime.selected;

    if keyboard.just_pressed(KeyCode::Tab) {
        for (player, mut source) in &mut players {
            if *player != selected {
                continue;
            }
            if source.is_human() || matches!(*source, Source::Playback { .. }) {
                *source = Source::Scripted(ScriptPattern::for_player(selected));
                notice.0 = format!("{} switched to scripted.", selected.label());
            } else {
                *source = Source::Keyboard;
                notice.0 = format!("{} switched to keyboard.", selected.label());
            }
        }
    }

    if keyboard.just_pressed(KeyCode::F5) {
        if recordings.recording == Some(selected) {
            let frames = recordings.track(selected).len();
            recordings.stop();
            notice.0 = format!("Recorded {frames} frames for {}.", selected.label());
        } else {
            recordings.begin(selected);
            notice.0 = format!("Recording {} intent...", selected.label());
        }
    }

    if keyboard.just_pressed(KeyCode::F6) {
        if recordings.track(selected).is_empty() {
            notice.0 = format!("{} has no recording yet.", selected.label());
        } else {
            recordings.stop();
            for (player, mut source) in &mut players {
                if *player == selected {
                    *source = Source::Playback { cursor: 0 };
                }
            }
            notice.0 = format!("{} replaying recorded intent.", selected.label());
        }
    }
}

pub fn perform_reset(
    mut commands: Commands,
    mut reset: ResMut<ResetRequested>,
    mut runtime: ResMut<LabRuntime>,
    mut recordings: ResMut<RecordingBank>,
    mut rebind: ResMut<RebindCapture>,
    mut notice: ResMut<LabNotice>,
    spawned: Query<Entity, With<LabSpawned>>,
) {
    if !reset.0 {
        return;
    }
    reset.0 = false;
    runtime.reset_count += 1;
    runtime.selected = PlayerId(0);
    runtime.pulse_requested = false;
    runtime.last_pulse = None;
    rebind.active = false;
    *recordings = RecordingBank::default();

    for entity in &spawned {
        commands.entity(entity).despawn();
    }
    commands.run_system_cached(setup_lab);
    notice.0 = format!(
        "Reset {} - keyboard P1, scripted P2-P4.",
        runtime.reset_count
    );
}

#[derive(SystemParam)]
pub(crate) struct IntentInputs<'w, 's> {
    runtime: Res<'w, LabRuntime>,
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    action_map: Res<'w, ActionMap>,
    config: Res<'w, ControllerConfig>,
    time: Res<'w, Time>,
    recordings: Res<'w, RecordingBank>,
    gamepads: Query<'w, 's, &'static Gamepad>,
    players: Query<
        'w,
        's,
        (
            &'static PlayerId,
            &'static mut Source,
            &'static mut PlayerIntent,
            &'static mut PrevReading,
        ),
    >,
}

/// The heart of the lab: derive every player's `PlayerIntent`, isolated per device,
/// through the pure adapter. Focus loss neutralizes everyone.
pub(crate) fn build_intents(mut inputs: IntentInputs) {
    let focused = inputs.runtime.focused;
    let elapsed = inputs.time.elapsed_secs();
    for (player, mut source, mut intent, mut prev) in &mut inputs.players {
        if !focused {
            *intent = PlayerIntent::default();
            continue;
        }
        match &mut *source {
            Source::Keyboard => {
                let sample = DeviceSample::Keyboard {
                    keys: inputs.keyboard.get_pressed().copied().collect(),
                };
                let (new_intent, reading) =
                    intent_from(&inputs.action_map, &inputs.config, &sample, &prev.0);
                *intent = new_intent;
                prev.0 = reading;
            }
            Source::Gamepad(entity) => {
                if let Ok(gamepad) = inputs.gamepads.get(*entity) {
                    let sample = gamepad_sample(&inputs.action_map, gamepad);
                    let (new_intent, reading) =
                        intent_from(&inputs.action_map, &inputs.config, &sample, &prev.0);
                    *intent = new_intent;
                    prev.0 = reading;
                } else {
                    *intent = PlayerIntent::default();
                }
            }
            Source::Scripted(pattern) => {
                *intent = scripted_intent(*pattern, elapsed);
            }
            Source::Playback { cursor } => {
                *intent = playback_frame(inputs.recordings.track(*player), cursor);
            }
        }
    }
}

fn gamepad_sample(map: &ActionMap, gamepad: &Gamepad) -> DeviceSample {
    let mut buttons = Vec::new();
    for bound in map.gamepad_bindings.values() {
        for button in bound {
            if gamepad.pressed(*button) && !buttons.contains(button) {
                buttons.push(*button);
            }
        }
    }
    let axes = [
        GamepadAxis::LeftStickX,
        GamepadAxis::LeftStickY,
        GamepadAxis::RightStickX,
        GamepadAxis::RightStickY,
    ]
    .into_iter()
    .filter_map(|axis| gamepad.get(axis).map(|value| (axis, value)))
    .collect();
    DeviceSample::Gamepad { buttons, axes }
}

pub fn record_intents(
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
    let frame = *intent;
    let track = recordings.track_mut(target);
    track.push(frame);
    if track.len() >= MAX_RECORDING_FRAMES {
        recordings.stop();
        notice.0 = format!(
            "{} recording hit the {} frame cap.",
            target.label(),
            MAX_RECORDING_FRAMES
        );
    }
}

/// Presentation-only haptics: a hazard pulse rumbles every gamepad player. It never
/// reads or writes `PlayerIntent`, so it cannot affect the deterministic simulation.
pub fn drive_haptics(
    mut runtime: ResMut<LabRuntime>,
    players: Query<(&PlayerId, &Source)>,
    mut rumble: MessageWriter<RumbleRequest>,
    mut notice: ResMut<LabNotice>,
) {
    if !runtime.pulse_requested {
        return;
    }
    runtime.pulse_requested = false;
    runtime.pulses += 1;

    if !runtime.haptics_enabled {
        notice.0 = "Hazard pulse fired (haptics disabled - no rumble).".to_string();
        return;
    }

    let mut buzzed = None;
    for (player, source) in &players {
        if let Source::Gamepad(entity) = source {
            rumble.write(RumbleRequest::with_pattern(
                *entity,
                RumblePattern::HeavyImpact,
                0.85,
                Duration::from_millis(220),
            ));
            buzzed = Some(*player);
        }
    }
    runtime.last_pulse = buzzed;
    notice.0 = match buzzed {
        Some(player) => format!(
            "Hazard pulse -> rumble on gamepad players (e.g. {}).",
            player.label()
        ),
        None => "Hazard pulse (no controller connected to rumble).".to_string(),
    };
}

pub(crate) fn present_probes(
    mut probes: Query<(&Anchor, &PlayerIntent, &mut Transform), With<ProbeVisual>>,
) {
    for (anchor, intent, mut transform) in &mut probes {
        let offset = intent.movement.clamp_length_max(1.0) * PROBE_RADIUS;
        transform.translation = (anchor.0 + offset).extend(0.0);
    }
}

#[derive(SystemParam)]
pub(crate) struct OverlayInputs<'w, 's> {
    runtime: Res<'w, LabRuntime>,
    notice: Res<'w, LabNotice>,
    recordings: Res<'w, RecordingBank>,
    rebind: Res<'w, RebindCapture>,
    device_state: Res<'w, InputDeviceState>,
    config: Res<'w, ControllerConfig>,
    action_map: Res<'w, ActionMap>,
    players: Query<'w, 's, (&'static PlayerId, &'static Source, &'static PlayerIntent)>,
    cameras: Query<'w, 's, (), With<ArchieLabCamera>>,
    ui_roots: Query<'w, 's, (), With<ArchieLabUiRoot>>,
    probes: Query<'w, 's, (), With<ProbeVisual>>,
}

pub(crate) fn update_overlay(
    inputs: OverlayInputs,
    mut panels: Query<&mut Visibility, With<OverlayPanel>>,
    mut texts: Query<&mut Text, With<DiagnosticsText>>,
) {
    let visibility = if inputs.runtime.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut panel in &mut panels {
        *panel = visibility;
    }

    let camera_count = inputs.cameras.iter().count();
    let ui_count = inputs.ui_roots.iter().count();
    let probe_count = inputs.probes.iter().count();
    let player_count = inputs.players.iter().count();
    let healthy = camera_count == 1
        && ui_count == 1
        && probe_count == PLAYER_COUNT
        && player_count == PLAYER_COUNT
        && inputs.runtime.focused;

    let mut rows: Vec<(u16, String)> = inputs
        .players
        .iter()
        .map(|(player, source, intent)| {
            (
                player.0,
                format!(
                    "  {}  {:<14} move({:+.2},{:+.2}) look({:+.2},{:+.2}) [{}{}{}{}]",
                    player.label(),
                    source.label(),
                    intent.movement.x,
                    intent.movement.y,
                    intent.look.x,
                    intent.look.y,
                    if intent.jump_pressed { "J" } else { "-" },
                    if intent.sprint_held { "S" } else { "-" },
                    if intent.interact_held { "E" } else { "-" },
                    if intent.climb_pressed { "C" } else { "-" },
                ),
            )
        })
        .collect();
    rows.sort_by_key(|(id, _)| *id);
    let player_lines = rows
        .into_iter()
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join("\n");

    let device = match inputs.device_state.active_device {
        InputDevice::Mouse => "mouse".to_string(),
        InputDevice::Keyboard => "keyboard".to_string(),
        InputDevice::Gamepad(_) => format!("gamepad ({:?})", inputs.config.layout()),
    };
    let interact_keys = inputs
        .action_map
        .key_bindings
        .get(&GameAction::Primary)
        .cloned()
        .unwrap_or_default();

    for mut text in &mut texts {
        *text = Text::new(format!(
            "ARCHIE INPUT LAB  {pass}\n\
             bevy_archie 0.2.4 -> player_input::PlayerIntent\n\n\
             {players}\n\n\
             selected      {selected}\n\
             active device {device}   gamepads {pads}\n\
             interact bind {interact:?}   deadzone {deadzone:.2}\n\
             haptics {haptics}   pulses {pulses}{last_pulse}\n\
             recording {recording}   rebind {rebind}\n\
             focus {focus} (losses {losses})   resets {resets}\n\
             cam {cam}  ui {ui}  probes {probes}  players {players_n}\n\n\
             {notice}",
            pass = if healthy { "[PASS]" } else { "[FAIL]" },
            players = player_lines,
            selected = inputs.runtime.selected.label(),
            device = device,
            pads = inputs.device_state.connected_gamepads.len(),
            interact = interact_keys,
            deadzone = inputs.config.effective_deadzone(),
            haptics = if inputs.runtime.haptics_enabled {
                "on"
            } else {
                "off"
            },
            pulses = inputs.runtime.pulses,
            last_pulse = inputs
                .runtime
                .last_pulse
                .map(|player| format!(" (last {})", player.label()))
                .unwrap_or_default(),
            recording = inputs
                .recordings
                .recording
                .map(|player| player.label())
                .unwrap_or_else(|| "-".to_string()),
            rebind = if inputs.rebind.active {
                "capturing..."
            } else {
                "idle"
            },
            focus = inputs.runtime.focused,
            losses = inputs.runtime.focus_losses,
            resets = inputs.runtime.reset_count,
            cam = camera_count,
            ui = ui_count,
            probes = probe_count,
            players_n = player_count,
            notice = inputs.notice.0,
        ));
    }
}
