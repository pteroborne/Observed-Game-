//! Optional walking evidence with the ordinary clock and streaming budgets.
//! Timestamped frames preserve real pauses when encoded with the concat manifest.
use crate::GameState;
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};
use std::{io::Write, path::PathBuf, time::Instant};

#[derive(Resource)]
struct Recorder {
    directory: PathBuf,
    started: Option<Instant>,
    next: f64,
    frames: u32,
}

pub(super) fn install(app: &mut App) {
    let Some(directory) = std::env::var_os("OBSERVED2_CAPTURE_SOLO_ROUTE_VIDEO").map(PathBuf::from)
    else {
        return;
    };
    std::fs::create_dir_all(directory.join("frames")).expect("route evidence directory");
    std::fs::write(directory.join("frames.csv"), "frame,seconds\n").expect("route timestamps");
    app.insert_resource(Recorder {
        directory,
        started: None,
        next: 0.0,
        frames: 0,
    })
    .add_systems(Update, record);
}

fn record(mut commands: Commands, state: Res<State<GameState>>, mut recorder: ResMut<Recorder>) {
    if recorder.started.is_none() {
        if *state.get() != GameState::HexWfc {
            return;
        }
        recorder.started = Some(Instant::now());
    }
    let elapsed = recorder.started.unwrap().elapsed().as_secs_f64();
    if elapsed < recorder.next {
        return;
    }
    recorder.next = elapsed + 1.0 / 15.0;
    let name = format!("frame_{:05}.png", recorder.frames);
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(recorder.directory.join("frames").join(&name)));
    let mut log = std::fs::OpenOptions::new()
        .append(true)
        .open(recorder.directory.join("frames.csv"))
        .expect("timestamp log");
    writeln!(log, "{name},{elapsed:.6}").expect("frame timestamp");
    recorder.frames += 1;
}
