//! Continuous first-person kinetic playground. Run `cargo dev-run -p kinetic_lab`.
pub mod arena;
pub mod demo;
mod evidence;
pub mod model;
pub mod physics;
mod runtime;
mod view;

use bevy::{
    asset::AssetPlugin,
    prelude::*,
    window::{PresentMode, WindowResolution},
};
use runtime::Runtime;

pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "Kinetic / The Impulse Chamber\n  --encounter   Start with finite charge and three waves\n  --help        Show this help\n\n1 practice / 2 encounter / R reset / P pause / Esc release cursor\nWASD move / Shift sprint / Space jump / LMB push / RMB pull / E operate\nF3 diagnostics / N one paused tick\n\nOBSERVED2_CAPTURE=<png> or OBSERVED2_CAPTURE_SEQUENCE=<directory>\nThe former discrete schematic, siege and opposition flags are retired."
        );
        return;
    }
    if let Some(arg) = args.iter().find(|arg| arg.as_str() != "--encounter") {
        eprintln!("Unsupported option {arg}. Run --help for the rebuilt lab's options.");
        std::process::exit(2);
    }
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Kinetic / The Impulse Chamber".into(),
                    resolution: WindowResolution::new(1440, 900),
                    present_mode: PresentMode::AutoVsync,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: format!("{}/../../assets", env!("CARGO_MANIFEST_DIR")),
                ..default()
            }),
    )
    .insert_resource(Time::<Fixed>::from_hz(60.))
    .insert_resource(Runtime::new(if args.iter().any(|a| a == "--encounter") {
        model::Mode::Encounter
    } else {
        model::Mode::Practice
    }))
    .add_systems(FixedUpdate, runtime::fixed_step);
    for (env, sequence) in [
        ("OBSERVED2_CAPTURE", false),
        ("OBSERVED2_CAPTURE_SEQUENCE", true),
    ] {
        if let Ok(destination) = std::env::var(env) {
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
                target: None,
                report: String::new(),
            });
            break;
        }
    }
    view::plugin(&mut app);
    app.run();
}
/// Compatibility launcher for the former first-person binary.
pub fn run_fps() {
    run();
}
