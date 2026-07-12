//! Evidence capture: the `OBSERVED2_CAPTURE*` screenshot systems used to regenerate
//! the docs evidence. Each is opt-in via an environment variable (see [`configure`]),
//! drives the game into the Match, frames a deterministic shot, saves it, and exits.
//! None of this runs in normal play.
//!
//! Drivers must not assume their first `NextState` write survives boot: on a slow
//! first frame (dx12 shader compilation) the splash timer fires in the same frame
//! and clobbers the transition. Each driver therefore keeps asserting its target
//! state until the Match resources actually exist — and stops once they do, so an
//! identity transition never re-runs the Match `OnEnter` teardown/setup.

pub(crate) mod bot_pov;
pub(crate) mod scenarios;
pub(crate) mod tour;

use bevy::prelude::*;

use crate::GameState;

pub(crate) use bot_pov::BotPovCaptureRequest;

/// Wire up whichever capture system the environment requests (at most one). Called from
/// [`crate::run`] after the game plugin is added; a no-op in normal play.
pub(super) fn configure_captures(app: &mut App) {
    if std::env::var("OBSERVED2_CAPTURE_TOUR").is_ok() {
        app.insert_resource(tour::TourCapture::new()).add_systems(
            Update,
            tour::capture_tour_progress.after(crate::screens::place::present_match_camera),
        );
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE_MATCH") {
        app.insert_resource(scenarios::MatchCaptureRequest::new(path))
            .add_systems(
                Update,
                scenarios::capture_match_progress
                    .after(crate::screens::place::present_match_camera),
            );
    } else if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE_MAP_AUDIT") {
        let _ = std::fs::create_dir_all(&dir);
        app.insert_resource(scenarios::MapAuditCaptureRequest::new(dir))
            .add_systems(
                Update,
                scenarios::capture_map_audit_progress
                    .after(crate::screens::place::present_match_camera),
            );
    } else if let Ok((path, into_maze)) = std::env::var("OBSERVED2_CAPTURE_MAZE")
        .map(|p| (p, true))
        .or_else(|_| std::env::var("OBSERVED2_CAPTURE_ROOM").map(|p| (p, false)))
    {
        app.insert_resource(scenarios::MazeCaptureRequest::new(path, into_maze))
            .add_systems(
                Update,
                scenarios::capture_maze_progress.after(crate::screens::place::present_match_camera),
            );
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE_KEYSTONE") {
        app.insert_resource(scenarios::KeystoneCaptureRequest::new(path))
            .add_systems(
                Update,
                scenarios::capture_keystone_progress
                    .after(crate::screens::place::present_match_camera),
            );
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE_RIVALS") {
        app.insert_resource(scenarios::RivalCaptureRequest::new(path))
            .add_systems(
                Update,
                scenarios::capture_rivals_progress
                    .after(crate::screens::place::present_match_camera),
            );
    } else if let Ok((path, from_hallway)) = std::env::var("OBSERVED2_CAPTURE_DOORWAY_HALL")
        .map(|p| (p, true))
        .or_else(|_| std::env::var("OBSERVED2_CAPTURE_DOORWAY").map(|p| (p, false)))
    {
        app.insert_resource(scenarios::DoorwayCaptureRequest::new(path, from_hallway))
            .add_systems(
                Update,
                scenarios::capture_doorway_progress
                    .after(crate::screens::place::present_match_camera),
            );
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE_CEILING") {
        app.insert_resource(scenarios::CeilingCaptureRequest::new(path))
            .add_systems(
                Update,
                scenarios::capture_ceiling_progress
                    .after(crate::screens::place::present_match_camera),
            );
    } else if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE_WELLSHAFT") {
        let _ = std::fs::create_dir_all(&dir);
        app.insert_resource(scenarios::WellshaftCaptureRequest::new(dir))
            .add_systems(
                Update,
                scenarios::capture_wellshaft_progress
                    .after(crate::screens::place::present_match_camera),
            );
    } else if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE_RESULTS") {
        let _ = std::fs::create_dir_all(&dir);
        app.insert_resource(scenarios::ResultsCaptureRequest::new(dir))
            .add_systems(Update, scenarios::capture_results_progress);
    } else if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE_BOT")
        .map(|d| (d, false))
        .or_else(|_| std::env::var("OBSERVED2_CAPTURE_WELLSHAFT_POV").map(|d| (d, true)))
    {
        let (dir, force_wellshaft) = dir;
        let _ = std::fs::create_dir_all(&dir);
        let request = if force_wellshaft {
            bot_pov::BotPovCaptureRequest::new_wellshaft(dir)
        } else {
            bot_pov::BotPovCaptureRequest::new(dir)
        };
        app.insert_resource(request)
            .add_systems(
                FixedUpdate,
                bot_pov::drive_bot_pov_capture
                    .before(crate::screens::match_runtime::teleport_sim)
                    .run_if(in_state(GameState::Match)),
            )
            .add_systems(
                Update,
                bot_pov::capture_bot_pov_progress
                    .after(crate::screens::place::present_match_camera),
            );
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE_REBIND") {
        app.insert_resource(scenarios::RebindCaptureRequest::new(path))
            .add_systems(Update, scenarios::capture_rebind_progress);
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(scenarios::CaptureRequest::new(path))
            .add_systems(Update, scenarios::capture_progress);
    }
}
