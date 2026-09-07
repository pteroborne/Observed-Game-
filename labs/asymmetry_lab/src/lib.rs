//! `asymmetry_lab` — two seats, two screens, one match.
//!
//! The architect sees the whole lattice and holds tiles. The operator moves the
//! squad and sees only what the squad has looked at, remembered as it was when
//! last seen. Neither can do the other's job, and the thing being tested is
//! whether that dependency is a pleasure or a chore.
//!
//! The rules are `observed_mechanics`; this crate is only the two windows onto
//! them.

pub mod seat;
pub mod view;

use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

use observed_mechanics::spec::ModeSpec;

use crate::seat::{Seat, Session};

fn opening_spec() -> ModeSpec {
    ModeSpec::presets()
        .into_iter()
        .find(|spec| spec.name == "Architect")
        .unwrap_or_else(ModeSpec::plant)
}

pub fn run() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Observed - Asymmetry Lab".to_string(),
            resolution: WindowResolution::new(900, 1000).with_scale_factor_override(1.0),
            present_mode: PresentMode::AutoVsync,
            canvas: Some("#asymmetry-canvas".to_string()),
            fit_canvas_to_parent: true,
            prevent_default_event_handling: true,
            ..default()
        }),
        ..default()
    }));
    assert!(
        app.is_plugin_added::<bevy::ui_render::UiRenderPlugin>(),
        "asymmetry_lab requires Bevy's UiRenderPlugin"
    );
    configure(&mut app);
    app.run();
}

pub fn configure(app: &mut App) {
    app.insert_resource(ClearColor(Color::srgb(0.02, 0.026, 0.036)))
        .insert_resource(Session::new(Seat::from_environment(), opening_spec()))
        .add_systems(Startup, (spawn_camera, view::art::load, view::hud::spawn))
        .add_systems(
            Update,
            (
                view::input::board_taps,
                view::input::hand_taps,
                view::input::controls,
                view::board::redraw,
                view::hud::sync,
            )
                .chain(),
        )
        .add_systems(Update, (view::animate::pulse, view::animate::glide));
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: 13.0,
                min_height: 19.0,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}
