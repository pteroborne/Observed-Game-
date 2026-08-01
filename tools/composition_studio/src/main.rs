//! Binary entry point for `composition_studio`.

use bevy::prelude::*;
use composition_studio::StudioPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Composition Studio".to_string(),
                resolution: (1600u32, 1000u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(StudioPlugin)
        .run();
}
