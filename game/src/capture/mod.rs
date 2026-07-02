//! Evidence capture: the `OBSERVED2_CAPTURE*` screenshot systems used to regenerate
//! the docs evidence. Each is opt-in via an environment variable (see [`configure`]),
//! drives the game into the Match, frames a deterministic shot, saves it, and exits.
//! None of this runs in normal play.

pub(crate) mod bot_pov;
pub(crate) mod scenarios;
pub(crate) mod tour;

use bevy::prelude::*;

use crate::GameState;
use crate::screens;

pub(crate) use bot_pov::BotPovCaptureRequest;

/// Wire up whichever capture system the environment requests (at most one). Called from
/// [`crate::run`] after the game plugin is added; a no-op in normal play.
pub fn configure(app: &mut App) {
    if std::env::var("OBSERVED2_CAPTURE_TOUR").is_ok() {
        app.insert_resource(tour::TourCapture::new()).add_systems(
            Update,
            tour::capture_tour_progress.after(screens::present_match_camera),
        );
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE_MATCH") {
        app.insert_resource(scenarios::MatchCaptureRequest::new(path))
            .add_systems(
                Update,
                scenarios::capture_match_progress.after(screens::present_match_camera),
            );
    } else if let Ok((path, into_maze)) = std::env::var("OBSERVED2_CAPTURE_MAZE")
        .map(|p| (p, true))
        .or_else(|_| std::env::var("OBSERVED2_CAPTURE_ROOM").map(|p| (p, false)))
    {
        app.insert_resource(scenarios::MazeCaptureRequest::new(path, into_maze))
            .add_systems(
                Update,
                scenarios::capture_maze_progress.after(screens::present_match_camera),
            );
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE_KEYSTONE") {
        app.insert_resource(scenarios::KeystoneCaptureRequest::new(path))
            .add_systems(
                Update,
                scenarios::capture_keystone_progress.after(screens::present_match_camera),
            );
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE_RIVALS") {
        app.insert_resource(scenarios::RivalCaptureRequest::new(path))
            .add_systems(
                Update,
                scenarios::capture_rivals_progress.after(screens::present_match_camera),
            );
    } else if let Ok((path, from_hallway)) = std::env::var("OBSERVED2_CAPTURE_DOORWAY_HALL")
        .map(|p| (p, true))
        .or_else(|_| std::env::var("OBSERVED2_CAPTURE_DOORWAY").map(|p| (p, false)))
    {
        app.insert_resource(scenarios::DoorwayCaptureRequest::new(path, from_hallway))
            .add_systems(
                Update,
                scenarios::capture_doorway_progress.after(screens::present_match_camera),
            );
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE_CEILING") {
        app.insert_resource(scenarios::CeilingCaptureRequest::new(path))
            .add_systems(
                Update,
                scenarios::capture_ceiling_progress.after(screens::present_match_camera),
            );
    } else if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE_BOT") {
        let _ = std::fs::create_dir_all(&dir);
        app.insert_resource(bot_pov::BotPovCaptureRequest::new(dir))
            .add_systems(
                FixedUpdate,
                bot_pov::drive_bot_pov_capture
                    .before(screens::teleport_sim)
                    .run_if(in_state(GameState::Match)),
            )
            .add_systems(
                Update,
                bot_pov::capture_bot_pov_progress.after(screens::present_match_camera),
            );
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(scenarios::CaptureRequest::new(path))
            .add_systems(Update, scenarios::capture_progress);
    }
}
