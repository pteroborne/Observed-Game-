//! One script tick per rendered frame, so a recording is the run.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::view::Run;

#[derive(Resource)]
pub struct Capture {
    pub destination: String,
    pub sequence: bool,
    pub frame: u32,
}

pub fn advance(
    mut commands: Commands,
    capture: Option<ResMut<Capture>>,
    mut run: ResMut<Run>,
    time: Res<Time>,
    mut exit: MessageWriter<AppExit>,
    mut accumulated: Local<f32>,
) {
    let Some(mut capture) = capture else {
        // Interactive: advance at the fixed rate in real time.
        *accumulated += time.delta_secs();
        while *accumulated > observed_traversal::FIXED_DT {
            *accumulated -= observed_traversal::FIXED_DT;
            run.advance();
        }
        return;
    };
    capture.frame += 1;
    // Let the room finish streaming in before anything is recorded.
    if capture.frame < 30 {
        return;
    }
    let frame = capture.frame - 30;
    if !capture.sequence {
        if frame < 90 {
            run.advance();
        }
        if frame == 95 {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(capture.destination.clone()));
        }
        if frame > 125 {
            exit.write(AppExit::Success);
        }
        return;
    }
    if run.finished {
        if frame > 0 {
            exit.write(AppExit::Success);
        }
        return;
    }
    run.advance();
    // Every second tick: at 30 fps that plays back in real time.
    if frame.is_multiple_of(2) {
        let path = format!("{}/frame_{:04}.png", capture.destination, frame / 2);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}
