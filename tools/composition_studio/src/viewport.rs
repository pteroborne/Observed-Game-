//! Orthographic isometric viewport camera, framing, zoom, and pan controls.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;

use crate::{LabMenuState, StudioState};

/// Component tag for the studio camera.
#[derive(Component)]
pub struct StudioCamera;

/// Re-exported so the studio and the game read a facility the same way. The
/// definitions live in `observed_style::iso`; keeping a second copy here is how
/// two surfaces end up teaching two different facilities.
pub use observed_style::iso::{AZIMUTH_DETENTS, ISO_PITCH, detent_bearing, detent_yaw};

pub const WINDOW_WIDTH: f32 = 1600.0;
pub const WINDOW_HEIGHT: f32 = 1000.0;

/// Fit the facility to the window, with a little air.
///
/// `iso_observer_lab` opens at 0.34 because it always solves the 28x20x10
/// production lattice, where a fitted layer is 544 cells wide and a doorway is
/// sub-pixel. The studio defaults to the compact config so a tuning edit
/// re-solves in milliseconds, and at that size fitting is exactly right —
/// opening cropped just makes the first action "zoom out".
pub const DEFAULT_ZOOM: f32 = 1.0;
pub const MIN_ZOOM: f32 = 0.04;
pub const MAX_ZOOM: f32 = 2.0;

/// Frame the drawn cells in an orthographic isometric view, returning
/// (transform, world-units-per-pixel, far plane).
#[must_use]
pub fn frame_camera(min: Vec3, max: Vec3) -> (Transform, f32, f32) {
    frame_camera_at(min, max, 0)
}

/// [`frame_camera`] from a given azimuth detent.
#[must_use]
pub fn frame_camera_at(min: Vec3, max: Vec3, detent: usize) -> (Transform, f32, f32) {
    let framing = observed_style::iso::frame(min, max, detent, WINDOW_WIDTH, WINDOW_HEIGHT);
    (
        Transform::from_translation(framing.translation).with_rotation(framing.rotation),
        framing.units_per_pixel,
        framing.far,
    )
}

/// Zoom and pan are applied to the camera every frame.
pub fn sync_camera(
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    menu_state: Res<LabMenuState>,
    mut state: ResMut<StudioState>,
    mut camera: Query<(&mut Projection, &mut Transform), With<StudioCamera>>,
) {
    // When menu is open or modal is active, mouse dragging and zoom are ignored unless intended.
    if !menu_state.is_open {
        if scroll.delta.y.abs() > f32::EPSILON {
            state.zoom = (state.zoom * 1.14_f32.powf(-scroll.delta.y)).clamp(MIN_ZOOM, MAX_ZOOM);
        }
        // Left drag is reserved for selecting/painting hexes (Slice 3), pan is right/middle mouse.
        if mouse.pressed(MouseButton::Right) || mouse.pressed(MouseButton::Middle) {
            let scale = state.base_frame.1 * state.zoom;
            state.pan += Vec2::new(-motion.delta.x, motion.delta.y) * scale;
        }
    }

    let Ok((mut projection, mut transform)) = camera.single_mut() else {
        return;
    };
    let (base, scale, far) = state.base_frame;
    let offset = base.rotation * Vec3::new(state.pan.x, state.pan.y, 0.0);
    *transform =
        Transform::from_translation(base.translation + offset).with_rotation(base.rotation);
    *projection = Projection::Orthographic(OrthographicProjection {
        scale: scale * state.zoom,
        near: 0.1,
        far,
        ..OrthographicProjection::default_3d()
    });
}

/// Inset the 3D camera so the facility renders beside the panel, not under it.
///
/// This is what makes the panel *docked* rather than an overlay: with the
/// viewport inset, no part of the layout is ever hidden behind chrome, and the
/// framing maths sees the real drawable area instead of the whole window.
/// Keep the layout mode in step with the window.
///
/// Runs before the chrome and the camera read it, so a resize never leaves the
/// panel and the viewport disagreeing about which edge the panel is on - which
/// would show up as the facility drawn underneath the controls.
pub fn sync_layout_mode(windows: Query<&Window>, mut state: ResMut<StudioState>) {
    let Ok(window) = windows.single() else {
        return;
    };
    let wanted = crate::LayoutMode::for_window_width(window.width());
    if state.layout != wanted {
        state.layout = wanted;
    }
}

pub fn sync_camera_viewport(
    windows: Query<&Window>,
    state: Res<StudioState>,
    mut camera: Query<&mut Camera, With<StudioCamera>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok(mut camera) = camera.single_mut() else {
        return;
    };

    let scale = window.scale_factor();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let left = (state.viewport_origin() * scale) as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let bottom = (state.viewport_bottom_inset(window.height()) * scale) as u32;
    let full = window.physical_size();
    // A window smaller than the panel would ask for a zero-sized viewport,
    // which Bevy treats as invalid; fall back to the whole window. Both axes
    // are checked because the panel docks left in a wide layout and along the
    // foot in a compact one.
    let viewport = (full.x > left + 32 && full.y > bottom + 32).then(|| bevy::camera::Viewport {
        physical_position: UVec2::new(left, 0),
        physical_size: UVec2::new(full.x - left, full.y - bottom),
        ..default()
    });
    // Compared field-wise rather than by equality — `Viewport` is not `PartialEq`
    // — and only assigned on a real change, so the camera is not marked dirty
    // every frame.
    let current = camera
        .viewport
        .as_ref()
        .map(|view| (view.physical_position, view.physical_size));
    let wanted = viewport
        .as_ref()
        .map(|view| (view.physical_position, view.physical_size));
    if current != wanted {
        camera.viewport = viewport;
    }
}
