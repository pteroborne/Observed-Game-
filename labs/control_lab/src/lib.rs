mod controls;
mod lab;
mod model;

use bevy::{
    prelude::*,
    window::{PresentMode, WindowFocused, WindowResolution},
};

pub use controls::{GamepadRegistry, LabCommand, apply_deadzone};
pub use model::{
    ControlSource, GamepadId, HumanDevice, KeyboardBindingSet, KeyboardBindings, KeyboardSlot,
    LabRuntime, PlayerId, PlayerIntent, ProbeState, RecordingBank, ScriptPattern,
};

#[derive(SystemSet, Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ControlSet {
    Devices,
    Rebind,
    CollectCommands,
    ApplyCommands,
    BuildIntent,
    Record,
    Consume,
    Present,
}

pub struct ControlLabPlugin;

impl Plugin for ControlLabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<KeyboardBindings>()
            .init_resource::<RecordingBank>()
            .init_resource::<LabRuntime>()
            .init_resource::<model::LabNotice>()
            .init_resource::<model::ResetRequested>()
            .init_resource::<model::RebindCapture>()
            .init_resource::<GamepadRegistry>()
            .add_message::<LabCommand>()
            .add_message::<WindowFocused>()
            .configure_sets(
                Update,
                (
                    ControlSet::Devices,
                    ControlSet::Rebind,
                    ControlSet::CollectCommands,
                    ControlSet::ApplyCommands,
                    ControlSet::BuildIntent,
                    ControlSet::Record,
                    ControlSet::Consume,
                    ControlSet::Present,
                )
                    .chain(),
            )
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    controls::update_window_focus,
                    (
                        controls::sync_gamepads,
                        controls::release_disconnected_gamepads,
                    )
                        .chain(),
                )
                    .in_set(ControlSet::Devices),
            )
            .add_systems(Update, controls::capture_rebind.in_set(ControlSet::Rebind))
            .add_systems(
                Update,
                (
                    controls::keyboard_shortcuts,
                    lab::command_buttons,
                    lab::button_visuals,
                )
                    .in_set(ControlSet::CollectCommands),
            )
            .add_systems(
                Update,
                (
                    controls::process_commands,
                    lab::perform_reset,
                    ApplyDeferred,
                )
                    .chain()
                    .in_set(ControlSet::ApplyCommands),
            )
            .add_systems(
                Update,
                controls::build_player_intents.in_set(ControlSet::BuildIntent),
            )
            .add_systems(Update, controls::record_intents.in_set(ControlSet::Record))
            .add_systems(Update, lab::consume_intents.in_set(ControlSet::Consume))
            .add_systems(
                Update,
                (lab::present_probes, lab::update_debug_ui).in_set(ControlSet::Present),
            );
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Name::new("Control Lab Camera")));
}

pub fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.012, 0.022, 0.035)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Control Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ControlLabPlugin)
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        controls::assign_device_in_slice,
        lab::{ControlLabUiRoot, ProbeVisual},
        model::{LabNotice, ResetRequested, playback_frame},
    };
    use bevy::input::{
        InputPlugin,
        gamepad::{
            GamepadAxis, GamepadButton, GamepadConnection, GamepadConnectionEvent,
            RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent, RawGamepadEvent,
        },
    };

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin))
            .insert_resource(ClearColor(Color::BLACK))
            .add_plugins(ControlLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    fn player_source(app: &mut App, target: PlayerId) -> ControlSource {
        let world = app.world_mut();
        let mut query = world.query::<(&PlayerId, &ControlSource)>();
        query
            .iter(world)
            .find(|(player, _)| **player == target)
            .map(|(_, source)| *source)
            .expect("player should exist")
    }

    fn player_intent(app: &mut App, target: PlayerId) -> PlayerIntent {
        let world = app.world_mut();
        let mut query = world.query::<(&PlayerId, &PlayerIntent)>();
        query
            .iter(world)
            .find(|(player, _)| **player == target)
            .map(|(_, intent)| *intent)
            .expect("player should exist")
    }

    #[test]
    fn keyboard_mapping_is_abstract_and_rebindable() {
        let mut keys = ButtonInput::<KeyCode>::default();
        let mut bindings = KeyboardBindingSet::keyboard_a();
        keys.press(KeyCode::KeyW);
        keys.press(KeyCode::KeyD);
        keys.press(KeyCode::Space);
        keys.press(KeyCode::ShiftLeft);

        let intent = bindings.read(&keys);
        assert!((intent.movement.length() - 1.0).abs() < 0.001);
        assert!(intent.movement.x > 0.0 && intent.movement.y > 0.0);
        assert!(intent.jump_pressed);
        assert!(intent.sprint_held);

        keys.clear();
        bindings.jump = KeyCode::KeyZ;
        keys.press(KeyCode::Space);
        assert!(!bindings.read(&keys).jump_pressed);
        keys.press(KeyCode::KeyZ);
        assert!(bindings.read(&keys).jump_pressed);
    }

    #[test]
    fn playback_repeats_exact_frames_in_order() {
        let frames = [
            PlayerIntent {
                movement: Vec2::X,
                ..default()
            },
            PlayerIntent {
                jump_pressed: true,
                ..default()
            },
        ];
        let mut cursor = 0;

        assert_eq!(playback_frame(&frames, &mut cursor), frames[0]);
        assert_eq!(playback_frame(&frames, &mut cursor), frames[1]);
        assert_eq!(playback_frame(&frames, &mut cursor), frames[0]);
        assert_eq!(cursor, 1);
    }

    #[test]
    fn device_assignment_is_exclusive() {
        let mut players = [
            (
                PlayerId(0),
                ControlSource::Human(HumanDevice::Keyboard(KeyboardSlot::A)),
            ),
            (PlayerId(1), ControlSource::Scripted(ScriptPattern::Patrol)),
        ];

        assign_device_in_slice(
            &mut players,
            PlayerId(1),
            HumanDevice::Keyboard(KeyboardSlot::A),
        );

        assert_eq!(
            players[1].1,
            ControlSource::Human(HumanDevice::Keyboard(KeyboardSlot::A))
        );
        assert!(matches!(players[0].1, ControlSource::Scripted(_)));
    }

    #[test]
    fn simulated_controller_can_claim_player_and_generate_intent() {
        let mut app = test_app();
        let gamepad_entity = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Messages<GamepadConnectionEvent>>()
            .write(GamepadConnectionEvent::new(
                gamepad_entity,
                GamepadConnection::Connected {
                    name: "Test Gamepad".to_string(),
                    vendor_id: None,
                    product_id: None,
                },
            ));
        app.update();

        assert_eq!(app.world().resource::<GamepadRegistry>().devices.len(), 1);

        app.world_mut()
            .resource_mut::<Messages<RawGamepadEvent>>()
            .write(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
                gamepad_entity,
                GamepadButton::Start,
                1.0,
            )));
        app.world_mut()
            .resource_mut::<Messages<RawGamepadEvent>>()
            .write(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                gamepad_entity,
                GamepadAxis::LeftStickX,
                0.75,
            )));
        app.update();

        assert_eq!(
            player_source(&mut app, PlayerId(0)),
            ControlSource::Human(HumanDevice::Gamepad(GamepadId(0)))
        );
        assert!(player_intent(&mut app, PlayerId(0)).movement.x > 0.5);
    }

    #[test]
    fn focus_loss_neutralizes_intent_and_freezes_playback() {
        let mut app = test_app();
        app.world_mut().resource_mut::<RecordingBank>().tracks[0] = vec![PlayerIntent {
            movement: Vec2::X,
            ..default()
        }];

        {
            let world = app.world_mut();
            let mut query = world.query::<(&PlayerId, &mut ControlSource)>();
            for (player, mut source) in query.iter_mut(world) {
                if *player == PlayerId(0) {
                    *source = ControlSource::Playback { cursor: 0 };
                }
            }
        }

        app.world_mut()
            .resource_mut::<Messages<WindowFocused>>()
            .write(WindowFocused {
                window: Entity::PLACEHOLDER,
                focused: false,
            });
        app.update();

        assert!(!app.world().resource::<LabRuntime>().focused);
        assert_eq!(app.world().resource::<LabRuntime>().focus_losses, 1);
        assert!(player_intent(&mut app, PlayerId(0)).is_neutral());
        assert_eq!(
            player_source(&mut app, PlayerId(0)),
            ControlSource::Playback { cursor: 0 }
        );
    }

    #[test]
    fn runtime_rebind_command_updates_the_active_keyboard_slot() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<Messages<LabCommand>>()
            .write(LabCommand::BeginJumpRebind);
        app.update();

        assert_eq!(
            app.world().resource::<model::RebindCapture>().player,
            Some(PlayerId(0))
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyZ);
        app.world_mut().run_schedule(Update);

        assert_eq!(
            app.world()
                .resource::<KeyboardBindings>()
                .get(KeyboardSlot::A)
                .jump,
            KeyCode::KeyZ
        );
        assert_eq!(app.world().resource::<model::RebindCapture>().player, None);
    }

    #[test]
    fn recording_commands_capture_frames_and_enter_playback() {
        let mut app = test_app();
        {
            let mut messages = app.world_mut().resource_mut::<Messages<LabCommand>>();
            messages.write(LabCommand::Select(PlayerId(2)));
            messages.write(LabCommand::ToggleRecording);
        }
        app.update();
        app.update();
        app.update();

        app.world_mut()
            .resource_mut::<Messages<LabCommand>>()
            .write(LabCommand::ToggleRecording);
        app.update();

        let frame_count = app
            .world()
            .resource::<RecordingBank>()
            .track(PlayerId(2))
            .len();
        assert!(frame_count >= 3);
        assert_eq!(app.world().resource::<RecordingBank>().recording, None);

        app.world_mut()
            .resource_mut::<Messages<LabCommand>>()
            .write(LabCommand::PlayRecording);
        app.update();

        assert!(matches!(
            player_source(&mut app, PlayerId(2)),
            ControlSource::Playback { .. }
        ));
    }

    #[test]
    fn human_scripted_switch_keeps_same_player_entity() {
        let mut app = test_app();
        let original_entity = {
            let world = app.world_mut();
            let mut query = world.query::<(Entity, &PlayerId)>();
            query
                .iter(world)
                .find(|(_, player)| **player == PlayerId(0))
                .map(|(entity, _)| entity)
                .unwrap()
        };

        app.world_mut()
            .resource_mut::<Messages<LabCommand>>()
            .write(LabCommand::ToggleHumanScripted);
        app.update();

        assert!(matches!(
            player_source(&mut app, PlayerId(0)),
            ControlSource::Scripted(_)
        ));
        let new_entity = {
            let world = app.world_mut();
            let mut query = world.query::<(Entity, &PlayerId)>();
            query
                .iter(world)
                .find(|(_, player)| **player == PlayerId(0))
                .map(|(entity, _)| entity)
                .unwrap()
        };
        assert_eq!(original_entity, new_entity);
    }

    #[test]
    fn repeated_reset_restores_one_clean_baseline() {
        let mut app = test_app();
        for expected_reset in 1..=10 {
            app.world_mut().resource_mut::<ResetRequested>().0 = true;
            app.update();

            assert_eq!(count::<ProbeVisual>(&mut app), 4);
            assert_eq!(count::<ControlLabUiRoot>(&mut app), 1);
            assert_eq!(
                app.world().resource::<LabRuntime>().reset_count,
                expected_reset
            );
            assert!(
                app.world()
                    .resource::<RecordingBank>()
                    .tracks
                    .iter()
                    .all(Vec::is_empty)
            );
            assert_eq!(
                player_source(&mut app, PlayerId(0)),
                ControlSource::Human(HumanDevice::Keyboard(KeyboardSlot::A))
            );
            assert_eq!(
                player_source(&mut app, PlayerId(1)),
                ControlSource::Human(HumanDevice::Keyboard(KeyboardSlot::B))
            );
        }
        assert!(app.world().resource::<LabNotice>().0.contains("Reset 10"));
    }

    #[test]
    fn deadzone_removes_drift_and_rescales_signal() {
        assert_eq!(apply_deadzone(Vec2::new(0.1, 0.0), 0.18), Vec2::ZERO);
        let output = apply_deadzone(Vec2::new(0.59, 0.0), 0.18);
        assert!((output.x - 0.5).abs() < 0.01);
        assert_eq!(output.y, 0.0);
    }
}
