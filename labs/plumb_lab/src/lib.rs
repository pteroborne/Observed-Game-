//! Plumb / does a redirected gravity give you a surface to walk on?
//!
//! A single room, a single Modron, and one question asked five times: if the
//! subject is told that "down" is some other direction, does it fall to that
//! surface, stay on it, and walk along it?
//!
//! Run it with `cargo dev-run -p plumb_lab`, or `--report` for the headless
//! answer.

pub mod evidence;
pub mod model;
pub mod report;
pub mod room;
pub mod script;
pub mod view;

use bevy::{
    prelude::*,
    window::{PresentMode, WindowResolution},
};

pub fn run() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Plumb / 01 - Which way is down".into(),
            resolution: WindowResolution::new(1440, 900),
            present_mode: PresentMode::AutoVsync,
            ..default()
        }),
        ..default()
    }));
    for (variable, sequence) in [
        ("OBSERVED2_CAPTURE", false),
        ("OBSERVED2_CAPTURE_SEQUENCE", true),
    ] {
        if let Ok(destination) = std::env::var(variable) {
            let parent = if sequence {
                std::path::Path::new(&destination)
            } else {
                std::path::Path::new(&destination)
                    .parent()
                    .unwrap_or(std::path::Path::new("."))
            };
            std::fs::create_dir_all(parent).expect("create evidence directory");
            app.insert_resource(evidence::Capture {
                destination,
                sequence,
                frame: 0,
            });
            break;
        }
    }
    view::plugin(&mut app);
    app.run();
}
