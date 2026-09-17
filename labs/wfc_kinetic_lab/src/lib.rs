//! The kinetic tool, inside a floor the production WFC solver actually built.
//!
//! `kinetic_lab` proved the tool against an authored chamber: a rectangle whose
//! every ledge was placed by hand to be shoved off. This lab asks the next
//! question, which is whether any of that survives contact with architecture
//! nobody arranged for it — real authored tiles, chosen by the real solver,
//! with the doorways, columns, decks and holes that come with them.
//!
//! Run it with `cargo dev-run -p wfc_kinetic_lab`.

pub mod demo;
pub mod evidence;
pub mod model;
pub mod runtime;
pub mod site;
mod view;

use std::sync::Arc;

use bevy::{
    asset::AssetPlugin,
    prelude::*,
    window::{PresentMode, WindowResolution},
};

use runtime::Runtime;

/// The floor the lab opens on.
///
/// Chosen rather than left at zero: this seed solves into the arena with the
/// most faces open onto void, which is the one property the kinetic tool needs
/// from its architecture. Any other seed still works — [`site::Site::solve`]
/// searches forward until it finds a usable seven-cell floor.
pub const DEFAULT_SEED: u64 = 8;

const HELP: &str = "\
Kinetic / 02 — The Solved Floor

  --seed <n>    Solve the floor from this seed (default 8). The search steps
                forward from it until a seven-cell floor with a shovable ledge
                appears, so every value works.
  --encounter   Start with finite charge and three waves
  --help        Show this help

1 practice / 2 encounter / R reset / G next floor / P pause / Esc release cursor
WASD move / Shift sprint / Space jump / LMB push / RMB pull / E operate
F3 diagnostics / N one paused tick

OBSERVED2_CAPTURE=<png>, OBSERVED2_CAPTURE_SEQUENCE=<dir> (staged scenes),
OBSERVED2_CAPTURE_LOOP=<dir> (one encounter, played by the director)
";

struct Options {
    seed: u64,
    mode: model::Mode,
}

/// Parse the command line, or explain why it could not be parsed.
fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        seed: DEFAULT_SEED,
        mode: model::Mode::Practice,
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--encounter" => options.mode = model::Mode::Encounter,
            "--seed" => {
                let value = rest.next().ok_or("--seed needs a number")?;
                options.seed = value
                    .parse()
                    .map_err(|_| format!("{value} is not a seed"))?;
            }
            other => return Err(format!("unsupported option {other}")),
        }
    }
    Ok(options)
}

pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print!("{HELP}");
        return;
    }
    let options = match parse(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}. Run --help for this lab's options.");
            std::process::exit(2);
        }
    };

    // Solve before opening a window. A floor that cannot be built is a failure
    // worth seeing in the terminal, not behind a blank viewport.
    let content = site::load_content();
    let site = match site::Site::solve(options.seed, &content) {
        Ok(site) => Arc::new(site),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    println!(
        "seed {} (requested {}): {} cells, {} holes, {} ledges, {} hulls — plan {}",
        site.seed,
        site.requested_seed,
        site.cells.len(),
        site.voids.len(),
        site.ledges.len(),
        site.colliders().len(),
        site.plan(),
    );

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Kinetic / 02 — The Solved Floor".into(),
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
    .insert_resource(Runtime::new(site, options.mode))
    .add_systems(FixedUpdate, runtime::fixed_step);

    for (variable, mode) in [
        ("OBSERVED2_CAPTURE", evidence::Mode2::Still),
        ("OBSERVED2_CAPTURE_SEQUENCE", evidence::Mode2::Scenes),
        ("OBSERVED2_CAPTURE_LOOP", evidence::Mode2::Loop),
    ] {
        let sequence = mode != evidence::Mode2::Still;
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
                mode,
                frame: 0,
                target: None,
                report: String::new(),
                settled: None,
            });
            break;
        }
    }
    view::plugin(&mut app);
    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_line_accepts_a_seed_and_rejects_nonsense() {
        let parsed = parse(&["--seed".into(), "1234".into(), "--encounter".into()])
            .expect("valid options parse");
        assert_eq!(parsed.seed, 1234);
        assert_eq!(parsed.mode, model::Mode::Encounter);

        assert_eq!(parse(&[]).expect("no options is valid").seed, DEFAULT_SEED);
        assert!(parse(&["--seed".into()]).is_err(), "--seed without a value");
        assert!(parse(&["--seed".into(), "x".into()]).is_err());
        assert!(parse(&["--siege".into()]).is_err(), "retired flag accepted");
    }
}
