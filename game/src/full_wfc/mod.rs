//! Canonical continuous-facility match adapter. Pure authoritative simulation lives
//! in `observed_match::full_wfc`; this module owns only Bevy input, presentation,
//! feedback, replay, and screen lifecycle integration.

mod audio;
mod cues;
mod entities;
mod feedback;
mod hud;
mod input;
pub mod sim;
mod tacmap;
mod view;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_content::ArchitectureRegister;

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
                view::sync_lighting_and_atmosphere,
                view::normalize_imported_materials,
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
        let capture = std::env::var("OBSERVED2_CAPTURE_FULL_WFC_STYLE")
            .map(|path| (path, FullWfcCaptureMode::Style))
            .or_else(|_| {
                std::env::var("OBSERVED2_CAPTURE_FULL_WFC")
                    .map(|path| (path, FullWfcCaptureMode::Gameplay))
            })
            .or_else(|_| {
                std::env::var("OBSERVED2_CAPTURE_FULL_WFC_MAP")
                    .map(|path| (path, FullWfcCaptureMode::Map))
            });
        if let Ok((path, mode)) = capture {
            if mode == FullWfcCaptureMode::Style {
                std::fs::create_dir_all(&path)
                    .expect("full-WFC style capture directory must be creatable");
            }
            app.insert_resource(FullWfcCapture {
                path,
                frame: 0,
                mode,
            })
            .add_systems(Startup, autostart_capture)
            .add_systems(
                Update,
                capture_progress
                    .after(view::sync_lighting_and_atmosphere)
                    .run_if(in_state(GameState::FullWfc)),
            );
        }
    }
}

#[derive(Resource)]
pub(super) struct FullWfcCapture {
    path: String,
    frame: u16,
    mode: FullWfcCaptureMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FullWfcCaptureMode {
    Gameplay,
    Map,
    Style,
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
        && request.mode == FullWfcCaptureMode::Map
        && let Some(runtime) = runtime.as_deref_mut()
    {
        runtime.map_open = true;
    }
    if request.mode == FullWfcCaptureMode::Style {
        let shot = usize::from(request.frame / 45);
        if request.frame % 45 == 35 && shot < ArchitectureRegister::ALL.len() {
            let register = ArchitectureRegister::ALL[shot];
            let path = std::path::Path::new(&request.path)
                .join(format!("full_wfc_{}.png", register.slug()));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        } else if usize::from(request.frame) >= ArchitectureRegister::ALL.len() * 45 + 20 {
            exit.write(AppExit::Success);
        }
    } else if request.frame == 60 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
    } else if request.frame == 120 {
        exit.write(AppExit::Success);
    }
}
