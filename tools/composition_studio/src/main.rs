//! Binary entry point for `composition_studio`.

use bevy::prelude::*;
use composition_studio::StudioPlugin;

/// The desktop window, sized for the authoring layout.
#[cfg(not(target_arch = "wasm32"))]
fn primary_window() -> Window {
    Window {
        title: "Observed 2 - Composition Studio".to_string(),
        resolution: (1600u32, 1000u32).into(),
        ..default()
    }
}

/// The browser canvas.
///
/// `fit_canvas_to_parent` hands sizing to CSS so the page decides the viewport
/// rather than a resolution baked in here - the studio is hosted for review on
/// whatever screen is to hand, including a phone. `prevent_default_event_handling`
/// stops the browser from claiming the gestures the viewport needs to pan.
#[cfg(target_arch = "wasm32")]
fn primary_window() -> Window {
    Window {
        title: "Observed 2 - Composition Studio".to_string(),
        canvas: Some("#studio-canvas".to_string()),
        fit_canvas_to_parent: true,
        prevent_default_event_handling: true,
        ..default()
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(primary_window()),
        ..default()
    }))
    .add_plugins(StudioPlugin);

    // Keep the studio's most important visual dependency executable rather than
    // merely documented. Bevy 0.19 dropped UI rendering from its `2d`/`3d`
    // feature groups: without the explicit `ui` feature the studio still builds,
    // lays out, and accepts input while drawing nothing at all. A black canvas
    // is indistinguishable from a slow load, so assert the plugin is really
    // here - this also catches a future change in how `DefaultPlugins` is
    // composed, which no Cargo feature could.
    assert!(
        app.is_plugin_added::<bevy::ui_render::UiRenderPlugin>(),
        "composition_studio requires Bevy's UiRenderPlugin"
    );

    app.run();
}
