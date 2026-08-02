//! Live-validating viewer for authored tile modules.
//!
//! `cargo run -p composition_studio --bin module-studio [-- <dir>]`
//!
//! Watches the authored corpus, re-validates on save, and draws the module with
//! whatever failed lit up on the geometry that caused it.

use bevy::prelude::*;
use composition_studio::module::app::ModuleStudioPlugin;
use composition_studio::module::watch::default_dir;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map_or_else(default_dir, std::path::PathBuf::from);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("Observed 2 - module studio - {}", dir.display()),
                resolution: (1600u32, 1000u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ModuleStudioPlugin { dir })
        .run();
}
