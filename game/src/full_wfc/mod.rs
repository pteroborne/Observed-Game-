//! Canonical continuous-facility match adapter. Pure authoritative simulation lives
//! in `observed_match::full_wfc`; this module owns only Bevy input, presentation,
//! feedback, replay, and screen lifecycle integration.

mod audio;
mod cues;
mod entities;
mod feedback;
mod hud;
mod input;
mod sim;
mod tacmap;
mod view;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::GameState;

pub(crate) struct FullWfcPlugin;

impl Plugin for FullWfcPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::FullWfc),
            (
                sim::setup_runtime,
                view::setup_view,
                hud::setup,
                tacmap::setup,
                feedback::setup,
                audio::setup,
                entities::setup,
                input::grab_cursor,
            )
                .chain(),
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
                view::sync_streamed_cells,
                view::sync_threshold_signals,
                view::sync_camera_and_candle,
                hud::sync,
                tacmap::sync,
                feedback::sync,
                audio::sync,
                entities::sync,
                sim::finish_runtime,
            )
                .chain()
                .run_if(in_state(GameState::FullWfc)),
        )
        .add_systems(
            OnExit(GameState::FullWfc),
            (
                input::release_cursor,
                view::clear_view,
                tacmap::cleanup,
                feedback::cleanup,
                audio::cleanup,
                entities::cleanup,
                sim::cleanup_runtime,
            )
                .chain(),
        );
        let capture = std::env::var("OBSERVED2_CAPTURE_FULL_WFC")
            .map(|path| (path, false))
            .or_else(|_| std::env::var("OBSERVED2_CAPTURE_FULL_WFC_MAP").map(|path| (path, true)));
        if let Ok((path, show_map)) = capture {
            app.insert_resource(FullWfcCapture {
                path,
                frame: 0,
                show_map,
            })
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
    show_map: bool,
}

fn autostart_capture(mut next: ResMut<NextState<GameState>>) {
    next.set(GameState::FullWfc);
}

fn capture_progress(
    mut request: ResMut<FullWfcCapture>,
    mut runtime: Option<ResMut<sim::FullWfcRuntime>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    request.frame = request.frame.saturating_add(1);
    if request.frame == 30
        && request.show_map
        && let Some(runtime) = runtime.as_deref_mut()
    {
        runtime.map_open = true;
    }
    if request.frame == 60 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
    } else if request.frame == 120 {
        exit.write(AppExit::Success);
    }
}
