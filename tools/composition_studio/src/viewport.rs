//! Orthographic isometric viewport camera, framing, zoom, and pan controls.

use std::f32::consts::FRAC_PI_4;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;

use crate::{LabMenuState, StudioState};

/// Component tag for the studio camera.
#[derive(Component)]
pub struct StudioCamera;

pub const ISO_PITCH: f32 = -0.615_479_7;

/// The six view bearings, in 60-degree steps **anchored at the historical
/// 45-degree default**.
///
/// Detents matter because the cutaway drops the walls facing the camera: orbit
/// freely and walls pop in and out at arbitrary angles. Six steps match the six
/// hex faces, so each detent has one unambiguous set of three near walls.
///
/// Anchoring at `FRAC_PI_4` rather than at 0 or 30 degrees is deliberate —
/// detent 0 is exactly the camera every earlier capture was framed with, so
/// adding this feature does not silently invalidate the evidence already in
/// `docs/evidence/composition_studio/`.
pub const AZIMUTH_DETENTS: usize = 6;

/// Yaw for `detent`, wrapping.
#[must_use]
pub fn detent_yaw(detent: usize) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let step = std::f32::consts::TAU / AZIMUTH_DETENTS as f32;
    #[allow(clippy::cast_precision_loss)]
    let index = (detent % AZIMUTH_DETENTS) as f32;
    FRAC_PI_4 + step * index
}

/// The plan-space direction from the scene toward the camera at `detent`.
///
/// This is what the cutaway tests hull azimuths against: a perimeter hull whose
/// centroid points this way is between the viewer and the interior.
#[must_use]
pub fn detent_bearing(detent: usize) -> Vec2 {
    let yaw = detent_yaw(detent);
    // The camera sits along `rotation * +Z`; in plan that is (sin yaw, cos yaw).
    Vec2::new(yaw.sin(), yaw.cos())
}
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
    let rotation = Quat::from_euler(EulerRot::YXZ, detent_yaw(detent), ISO_PITCH, 0.0);
    let centre = (min + max) * 0.5;
    let inverse = rotation.inverse();
    let mut extent = Vec2::ZERO;
    for i in 0..8u8 {
        let corner = Vec3::new(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
        );
        extent = extent.max((inverse * (corner - centre)).truncate().abs());
    }
    let scale = (extent.x * 2.0 / WINDOW_WIDTH)
        .max(extent.y * 2.0 / WINDOW_HEIGHT)
        .max(f32::MIN_POSITIVE)
        * 1.08;
    let diagonal = (max - min).length().max(1.0);
    let transform =
        Transform::from_translation(centre + rotation * Vec3::Z * diagonal).with_rotation(rotation);
    (transform, scale, diagonal * 2.0)
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
    let full = window.physical_size();
    // A window narrower than the panel would ask for a zero-width viewport,
    // which Bevy treats as invalid; fall back to the whole window.
    let viewport = (full.x > left + 32).then(|| bevy::camera::Viewport {
        physical_position: UVec2::new(left, 0),
        physical_size: UVec2::new(full.x - left, full.y),
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
