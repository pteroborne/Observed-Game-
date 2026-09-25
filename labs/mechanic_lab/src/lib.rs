//! `mechanic_lab` — a mechanic bench.
//!
//! The technical question is not "is rule X fun" but **"can rules be swapped
//! fast enough to find out"**. Arc T recorded four findings that sat on *fix
//! landed, awaiting human verification* and then all four failed at once,
//! because trying an alternative meant editing the lab that implemented it.
//!
//! See `docs/mechanic_lab_plan.md` for the seams, the conflict table, and the
//! rule that keeps the framework from eating the lab: **a trait ships with two
//! implementations or it is not a trait yet.**

pub mod view;

pub use observed_mechanics::spec;

use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

use crate::view::Session;
use observed_mechanics::spec::ModeSpec;

/// Launch the lab.
pub fn run() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Observed - Mechanic Lab".to_string(),
            // Pin the scale factor to 1. `fit_canvas_to_parent` sizes the
            // backing store to the element's CSS pixels, but the window still
            // reports the display's device pixel ratio, so Bevy's *logical*
            // viewport came out at half the CSS width — 187px on a phone. Every
            // `px` in the HUD then meant two, the dock stacked one control per
            // row, and the board was squeezed behind it. With the override,
            // one UI pixel is one CSS pixel and the layout means what it says.
            resolution: WindowResolution::new(900, 1000).with_scale_factor_override(1.0),
            present_mode: PresentMode::AutoVsync,
            canvas: Some("#mechanic-canvas".to_string()),
            fit_canvas_to_parent: true,
            prevent_default_event_handling: true,
            ..default()
        }),
        ..default()
    }));

    // Bevy 0.19 no longer includes UI rendering in its `2d`/`3d` feature
    // groups, and the loss is silent: the app still builds and still accepts
    // input, and the canvas simply clears black. Asserting on the plugin makes
    // dropping the feature a failure you can see.
    assert!(
        app.is_plugin_added::<bevy::ui_render::UiRenderPlugin>(),
        "mechanic_lab requires Bevy's UiRenderPlugin"
    );

    configure(&mut app);
    app.run();
}

/// Everything but the window, so a test can build the same app headlessly.
pub fn configure(app: &mut App) {
    app.insert_resource(ClearColor(Color::srgb(0.024, 0.031, 0.043)))
        .insert_resource(Session::new(ModeSpec::presets(), 0))
        .init_resource::<view::input::WatchClock>()
        .add_systems(Startup, (spawn_camera, view::art::load, view::hud::spawn))
        .add_systems(
            Update,
            (
                view::input::spectate,
                view::input::board_taps,
                view::input::buttons,
                view::input::mode_choices,
                view::board::redraw,
                view::hud::sync,
                view::hud::sync_menu,
            )
                .chain(),
        )
        // Separate from the board redraw on purpose: the redraw only runs when
        // the match changes, and an animation that forced a full respawn every
        // frame would be paying entity churn for a sine wave.
        .add_systems(Update, (view::animate::pulse, view::animate::glide));
}

fn spawn_camera(mut commands: Commands) {
    // `AutoMin` is what replaces the whole pan/zoom layer: it guarantees the
    // board fits on any viewport, so no gesture can lose it. The extra vertical
    // room is the HUD's, which floats over the board rather than shrinking it.
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: 13.0,
                min_height: 17.0,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}
