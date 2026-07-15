//! Selectable continuous-facility experiment. The legacy `Match` state remains the
//! shipping teleport game; this state assembles the lab-proven full-WFC simulation
//! into a deliberately small first-person traversal slice.

mod input;
mod sim;
mod view;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::GameState;

pub(crate) struct FullWfcPlugin;

impl Plugin for FullWfcPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::FullWfc),
            (sim::setup_runtime, view::setup_view, input::grab_cursor).chain(),
        )
        .add_systems(
            FixedUpdate,
            sim::step_runtime.run_if(in_state(GameState::FullWfc)),
        )
        .add_systems(
            Update,
            (
                input::map_input,
                input::mode_hotkeys,
                view::sync_changed_geometry,
                view::sync_threshold_signals,
                view::sync_camera_and_candle,
                view::sync_hud,
            )
                .chain()
                .run_if(in_state(GameState::FullWfc)),
        )
        .add_systems(
            OnExit(GameState::FullWfc),
            (
                input::release_cursor,
                view::clear_view,
                sim::cleanup_runtime,
            )
                .chain(),
        );
        if let Ok(path) = std::env::var("OBSERVED2_CAPTURE_FULL_WFC") {
            app.insert_resource(FullWfcCapture { path, frame: 0 })
                .add_systems(Startup, autostart_capture)
                .add_systems(
                    Update,
                    capture_progress
                        .after(view::sync_camera_and_candle)
                        .run_if(in_state(GameState::FullWfc)),
                );
        }
    }
}

#[derive(Resource)]
struct FullWfcCapture {
    path: String,
    frame: u16,
}

fn autostart_capture(mut next: ResMut<NextState<GameState>>) {
    next.set(GameState::FullWfc);
}

fn capture_progress(
    mut request: ResMut<FullWfcCapture>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    request.frame = request.frame.saturating_add(1);
    if request.frame == 60 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
    } else if request.frame == 120 {
        exit.write(AppExit::Success);
    }
}
