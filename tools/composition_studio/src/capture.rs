//! Screenshot capture pipeline for `OBSERVED2_CAPTURE` environment variable.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

#[derive(Resource)]
pub struct CaptureState {
    pub dir: String,
    /// File stem, so the two binaries in this crate do not overwrite each
    /// other's evidence.
    pub name: String,
    pub timer: f32,
    pub step: u8,
}

pub fn capture_system(
    time: Res<Time>,
    mut capture: ResMut<CaptureState>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    capture.timer += time.delta_secs();

    match capture.step {
        0 => {
            // Settle time. Generous because the studio builds the whole generated
            // tile library before its first frame, and a short wait silently
            // captures an empty black window rather than failing.
            if capture.timer >= 6.0 {
                let path = format!("{}/{}.png", capture.dir, capture.name);
                std::fs::create_dir_all(&capture.dir).ok();
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path));
                capture.step = 1;
                capture.timer = 0.0;
            }
        }
        1 if capture.timer >= 0.8 => {
            info!("Capture saved to dir '{}', exiting.", capture.dir);
            capture.step = 2;
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
