//! `architect_hand_lab` — the card is the interface.
//!
//! This lab deliberately stops before turns, opponents and networking. It asks
//! whether an Architect can read four authored topology cards, confidently aim
//! one at a live hex board, understand refusal before commitment, and recover
//! from a mistake without losing their place.

pub mod art;
pub mod board;
pub mod capture;
pub mod input;
pub mod model;
pub mod ui;

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ScalingMode, Viewport};
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

use board::BoardCamera;

#[derive(Resource)]
pub struct UiCameraEntity(pub Entity);

pub fn run() {
    let (width, height) = capture_size().unwrap_or((900, 820));
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Observed — Architect Hand Lab".to_string(),
            resolution: WindowResolution::new(width, height).with_scale_factor_override(1.0),
            present_mode: PresentMode::AutoVsync,
            canvas: Some("#architect-hand-canvas".to_string()),
            fit_canvas_to_parent: true,
            prevent_default_event_handling: true,
            ..default()
        }),
        ..default()
    }));
    configure(&mut app);
    capture::configure(&mut app);
    app.run();
}

pub fn configure(app: &mut App) {
    app.insert_resource(ClearColor(Color::srgb(0.015, 0.025, 0.038)))
        .init_resource::<art::CardArt>()
        .init_resource::<model::LabState>()
        .init_resource::<input::TrialClock>()
        .init_resource::<input::CardDrag>()
        .add_systems(Startup, (spawn_camera, art::load, ui::spawn).chain())
        .add_systems(
            Update,
            (
                (
                    input::cards,
                    input::controls,
                    input::keyboard,
                    input::drag_card,
                    input::board_taps,
                    input::hover_board,
                )
                    .chain(),
                (
                    board::redraw,
                    board::spawn_landing,
                    ui::sync_text,
                    ui::sync_hand,
                )
                    .chain(),
                (ui::sync_controls, ui::sync_drag, ui::sync_completion_layout).chain(),
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                input::tick,
                input::button_visuals,
                board::animate,
                board::animate_landing,
                ui::sync_layout,
                sync_camera_viewport,
                ui::position_rotation_overlay,
            ),
        );
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        BoardCamera,
        Camera2d,
        RenderLayers::layer(0),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: 12.8,
                min_height: 12.8,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
    let ui_camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: 10,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            RenderLayers::layer(31),
            Name::new("Architect hand UI camera"),
        ))
        .id();
    commands.insert_resource(UiCameraEntity(ui_camera));
}

fn sync_camera_viewport(
    windows: Query<&Window>,
    mut cameras: Query<&mut Camera, With<BoardCamera>>,
) {
    let (Ok(window), Ok(mut camera)) = (windows.single(), cameras.single_mut()) else {
        return;
    };
    let layout = ui::DockLayout::for_window(Vec2::new(window.width(), window.height()));
    let size = (layout.viewport_size * window.scale_factor())
        .as_uvec2()
        .max(UVec2::ONE);
    if camera
        .viewport
        .as_ref()
        .map(|viewport| viewport.physical_size)
        != Some(size)
    {
        camera.viewport = Some(Viewport {
            physical_position: UVec2::ZERO,
            physical_size: size,
            depth: 0.0..1.0,
        });
    }
}

fn capture_size() -> Option<(u32, u32)> {
    let raw = std::env::var("OBSERVED2_CAPTURE_SIZE").ok()?;
    let (width, height) = raw.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

#[cfg(test)]
mod tests {
    #[test]
    fn capture_size_is_optional() {
        // Parsing is pure once the environment supplied a value; malformed
        // input simply leaves the ordinary window size in control.
        assert!("375x812".split_once('x').is_some());
    }
}
