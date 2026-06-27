//! archie_input_lab -- Phase A6 of the Bevy asset-integration roadmap.
//!
//! It answers one question: **can richer device support (`bevy_archie`) feed the
//! existing `player_input::PlayerIntent` boundary without rewriting gameplay
//! systems?** The lab adds archie's real `ControllerPlugin` (detection, controller
//! ownership, haptics, the data-driven `ActionMap`) and routes keyboard, gamepad,
//! scripted, and replayed control for four local players into the *same*
//! `PlayerIntent`. Gameplay never reads a device directly.
//!
//! Compatibility gate: `bevy_archie 0.2.4` is the Bevy `0.18` line (the latest
//! `0.3.0` targets Bevy `0.19`), pinned exactly with `default-features = false`.

mod adapter;
mod lab;

pub use adapter::{
    ActionReading, DeviceSample, ScriptPattern, evaluate, intent_from, lab_action_map, look_vector,
    movement_vector, playback_frame, rebind_key, scripted_intent,
};
pub use lab::{
    ArchieLabCamera, ArchieLabUiRoot, LabNotice, LabRuntime, PLAYER_COUNT, PLAYERS, ProbeVisual,
    RebindCapture, RecordingBank, ResetRequested, Source,
};

use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowFocused, WindowResolution},
};
use bevy_archie::plugin::ControllerPlugin;
use observed_style::{SurfaceRole, surface};

#[derive(SystemSet, Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ControlSet {
    Devices,
    Focus,
    Rebind,
    Commands,
    Apply,
    BuildIntent,
    Record,
    Haptics,
    Present,
}

pub struct ArchieInputLabPlugin;

impl Plugin for ArchieInputLabPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ControllerPlugin::default())
            // Ensure the focus message exists even under MinimalPlugins (tests).
            .add_message::<WindowFocused>()
            .init_resource::<lab::LabRuntime>()
            .init_resource::<lab::LabNotice>()
            .init_resource::<lab::ResetRequested>()
            .init_resource::<lab::RebindCapture>()
            .init_resource::<lab::RecordingBank>()
            .configure_sets(
                Update,
                (
                    ControlSet::Devices,
                    ControlSet::Focus,
                    ControlSet::Rebind,
                    ControlSet::Commands,
                    ControlSet::Apply,
                    ControlSet::BuildIntent,
                    ControlSet::Record,
                    ControlSet::Haptics,
                    ControlSet::Present,
                )
                    .chain(),
            )
            .add_systems(Startup, lab::setup_lab)
            .add_systems(
                Update,
                (lab::assign_gamepads, lab::handle_unassigned).in_set(ControlSet::Devices),
            )
            .add_systems(Update, lab::update_focus.in_set(ControlSet::Focus))
            .add_systems(Update, lab::capture_rebind.in_set(ControlSet::Rebind))
            .add_systems(Update, lab::keyboard_shortcuts.in_set(ControlSet::Commands))
            .add_systems(
                Update,
                (lab::perform_reset, ApplyDeferred)
                    .chain()
                    .in_set(ControlSet::Apply),
            )
            .add_systems(Update, lab::build_intents.in_set(ControlSet::BuildIntent))
            .add_systems(Update, lab::record_intents.in_set(ControlSet::Record))
            .add_systems(Update, lab::drive_haptics.in_set(ControlSet::Haptics))
            .add_systems(
                Update,
                (lab::present_probes, lab::update_overlay).in_set(ControlSet::Present),
            );
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(surface(SurfaceRole::Ceiling).base_color))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Archie Input Lab (Phase A6)".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ArchieInputLabPlugin);

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, phase: 0 })
            .add_systems(Update, capture_progress);
    }

    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 && elapsed >= 0.9 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 1.8 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::{
        InputPlugin,
        gamepad::{GamepadConnection, GamepadConnectionEvent},
    };
    use lab::Source;
    use player_input::{PlayerId, PlayerIntent};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin))
            .insert_resource(ClearColor(Color::BLACK))
            .add_plugins(ArchieInputLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    fn source_label(app: &mut App, target: PlayerId) -> String {
        let world = app.world_mut();
        let mut query = world.query::<(&PlayerId, &Source)>();
        query
            .iter(world)
            .find(|(player, _)| **player == target)
            .map(|(_, source)| source.label())
            .expect("player exists")
    }

    fn intent_of(app: &mut App, target: PlayerId) -> PlayerIntent {
        let world = app.world_mut();
        let mut query = world.query::<(&PlayerId, &PlayerIntent)>();
        query
            .iter(world)
            .find(|(player, _)| **player == target)
            .map(|(_, intent)| *intent)
            .expect("player exists")
    }

    #[test]
    fn boots_with_camera_ui_and_four_player_probes() {
        let mut app = test_app();
        assert_eq!(count::<ArchieLabCamera>(&mut app), 1);
        assert_eq!(count::<ArchieLabUiRoot>(&mut app), 1);
        assert_eq!(count::<ProbeVisual>(&mut app), PLAYER_COUNT);
        // Player 1 is the keyboard human; the rest start scripted.
        assert_eq!(source_label(&mut app, PlayerId(0)), "keyboard");
        assert!(source_label(&mut app, PlayerId(1)).starts_with("scripted"));
    }

    #[test]
    fn reset_rebuilds_the_scene_without_leaking_entities() {
        let mut app = test_app();
        let baseline = count::<lab::LabSpawned>(&mut app);
        assert!(baseline > 0);

        for expected in 1..=3 {
            app.world_mut().resource_mut::<ResetRequested>().0 = true;
            app.update();
            assert_eq!(count::<lab::LabSpawned>(&mut app), baseline);
            assert_eq!(count::<ProbeVisual>(&mut app), PLAYER_COUNT);
            assert_eq!(count::<ArchieLabCamera>(&mut app), 1);
            assert_eq!(app.world().resource::<LabRuntime>().reset_count, expected);
        }
    }

    #[test]
    fn focus_loss_neutralizes_every_intent() {
        let mut app = test_app();
        // Let a scripted player accumulate a non-neutral intent first.
        for _ in 0..4 {
            app.update();
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
        assert!(intent_of(&mut app, PlayerId(1)).is_neutral());
        assert!(intent_of(&mut app, PlayerId(2)).is_neutral());
    }

    #[test]
    fn intent_records_and_replays_at_the_intent_layer() {
        let mut app = test_app();
        // Record player 2 (a moving scripted player) for several frames.
        app.world_mut()
            .resource_mut::<RecordingBank>()
            .begin(PlayerId(1));
        for _ in 0..5 {
            app.update();
        }
        let recorded = app
            .world()
            .resource::<RecordingBank>()
            .track(PlayerId(1))
            .to_vec();
        assert!(recorded.len() >= 5, "frames were captured");

        // Stop recording and switch player 2 to replay the tape.
        {
            let mut bank = app.world_mut().resource_mut::<RecordingBank>();
            bank.recording = None;
        }
        {
            let world = app.world_mut();
            let mut query = world.query::<(&PlayerId, &mut Source)>();
            for (player, mut source) in query.iter_mut(world) {
                if *player == PlayerId(1) {
                    *source = Source::Playback { cursor: 0 };
                }
            }
        }
        app.update();
        // The first replayed frame equals the first recorded frame.
        assert_eq!(intent_of(&mut app, PlayerId(1)), recorded[0]);
        assert!(source_label(&mut app, PlayerId(1)).starts_with("playback"));
    }

    #[test]
    fn a_connected_controller_claims_a_slot_and_falls_back_on_disconnect() {
        let mut app = test_app();
        let gamepad = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Messages<GamepadConnectionEvent>>()
            .write(GamepadConnectionEvent::new(
                gamepad,
                GamepadConnection::Connected {
                    name: "Test Controller".to_string(),
                    vendor_id: None,
                    product_id: None,
                },
            ));
        app.update();
        app.update();

        // The controller claimed the first free scripted slot (player 2).
        assert_eq!(source_label(&mut app, PlayerId(1)), "gamepad");

        // Disconnect -> archie reports it -> the player reverts to script.
        app.world_mut()
            .resource_mut::<Messages<GamepadConnectionEvent>>()
            .write(GamepadConnectionEvent::new(
                gamepad,
                GamepadConnection::Disconnected,
            ));
        app.update();
        app.update();

        assert!(source_label(&mut app, PlayerId(1)).starts_with("scripted"));
    }

    #[test]
    fn a_hazard_pulse_never_changes_intent() {
        let mut app = test_app();
        // Pin player 2 to a constant single-frame replay so its intent is fixed
        // regardless of time; any change would have to come from the haptics path.
        let fixed = PlayerIntent {
            movement: Vec2::new(0.5, 0.0),
            jump_pressed: true,
            ..Default::default()
        };
        app.world_mut().resource_mut::<RecordingBank>().tracks[1] = vec![fixed];
        {
            let world = app.world_mut();
            let mut query = world.query::<(&PlayerId, &mut Source)>();
            for (player, mut source) in query.iter_mut(world) {
                if *player == PlayerId(1) {
                    *source = Source::Playback { cursor: 0 };
                }
            }
        }
        app.update();
        let before = intent_of(&mut app, PlayerId(1));
        assert_eq!(before, fixed);

        // Fire a hazard pulse (presentation-only haptics path).
        app.world_mut().resource_mut::<LabRuntime>().pulse_requested = true;
        app.update();

        assert_eq!(
            app.world().resource::<LabRuntime>().pulses,
            1,
            "pulse consumed"
        );
        assert_eq!(
            intent_of(&mut app, PlayerId(1)),
            fixed,
            "haptics never touch intent"
        );
    }
}
