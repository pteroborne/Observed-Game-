//! Where the camera sits, and how the board is framed.
//!
//! Low-wall deck and operations-map framing, plus the pure viewport ownership test
//! used by every pointer system.

use bevy::prelude::*;
use observed_hex::hex_origin;

use crate::sim::TacticsGame;

use super::ViewMode;
use super::board::{cell_origin, draws_level};
use super::{CellPaint, paint};

/// True isometric pitch: `atan(1 / sqrt(2))`, the angle at which the three axes
/// of a cube project to equal screen lengths.
const ISO_PITCH: f32 = -0.615_479_7;
const ISO_YAW: f32 = std::f32::consts::FRAC_PI_4;
/// Padding around projected geometry. Framing in camera space avoids the large
/// empty wedge produced by fitting an isometric view to world X/Z extents.
const FRAME_MARGIN: f32 = 1.50;

#[derive(Component)]
pub struct BoardCamera;

/// The transform and orthographic scale that frame the whole board.
#[must_use]
pub fn frame(
    game: &TacticsGame,
    mode: ViewMode,
    level: u8,
    viewport_size: Vec2,
) -> (Transform, f32) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut any = false;
    for &cell in game.world.placements.keys() {
        if !draws_level(mode, cell, level) {
            continue;
        }
        if paint(game, cell) == CellPaint::Unknown {
            continue;
        }
        let point = cell_origin(mode, cell);
        let padding = if mode == ViewMode::Deck {
            Vec3::new(12.0, 8.0, 12.0)
        } else {
            Vec3::new(8.0, 4.0, 8.0)
        };
        min = min.min(point - padding);
        max = max.max(point + padding);
        any = true;
    }
    if !any {
        // An empty level still needs a camera looking somewhere sensible.
        let [x, _, z] = hex_origin(game.world.config.spawn());
        min = Vec3::new(x, 0.0, z);
        max = min;
    }
    let centre = (min + max) * 0.5;
    let extent = (max - min).max(Vec3::splat(1.0));

    let (rotation, distance) = match mode {
        // The operations map is a plan, so distances and connections are not
        // distorted by perspective or by stacking several decks together.
        ViewMode::Overview => (
            Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
            extent.length() + 120.0,
        ),
        // The detailed deck keeps the same projection as the strategic stack;
        // switching modes should change information density, not orientation.
        ViewMode::Deck => (
            Quat::from_euler(EulerRot::YXZ, ISO_YAW, ISO_PITCH, 0.0),
            extent.length() + 120.0,
        ),
    };
    let transform = Transform::from_translation(centre + rotation * Vec3::new(0.0, 0.0, distance))
        .with_rotation(rotation);
    let inverse = rotation.inverse();
    let mut projected = Vec2::ZERO;
    for corner in 0..8u8 {
        let point = Vec3::new(
            if corner & 1 == 0 { min.x } else { max.x },
            if corner & 2 == 0 { min.y } else { max.y },
            if corner & 4 == 0 { min.z } else { max.z },
        );
        projected = projected.max((inverse * (point - centre)).truncate().abs());
    }
    let aspect = (viewport_size.x / viewport_size.y.max(1.0)).max(0.1);
    let scale = ((projected.y * 2.0).max(projected.x * 2.0 / aspect) * FRAME_MARGIN).max(20.0);
    (transform, scale)
}

/// True only while a logical cursor position belongs to the camera viewport.
/// UI and camera input use this one boundary so wheel, drag, hover and click
/// cannot disagree about which surface owns the pointer.
#[must_use]
pub fn cursor_in_viewport(window: &Window, camera: &Camera, cursor: Vec2) -> bool {
    viewport_cursor_offset(window, camera, cursor).is_some()
}

/// Cursor displacement from the viewport centre in logical pixels, with +Y
/// pointing up in camera space. Used to keep wheel zoom anchored under the
/// pointer instead of pulling the inspected cell toward screen centre.
#[must_use]
pub fn viewport_cursor_offset(window: &Window, camera: &Camera, cursor: Vec2) -> Option<Vec2> {
    let Some(viewport) = camera.viewport.as_ref() else {
        let size = Vec2::new(window.width(), window.height());
        return (cursor.cmpge(Vec2::ZERO).all() && cursor.cmplt(size).all())
            .then_some(Vec2::new(cursor.x - size.x * 0.5, size.y * 0.5 - cursor.y));
    };
    let scale = window.scale_factor();
    let position = viewport.physical_position.as_vec2() / scale;
    let size = viewport.physical_size.as_vec2() / scale;
    (cursor.cmpge(position).all() && cursor.cmplt(position + size).all()).then_some(Vec2::new(
        cursor.x - (position.x + size.x * 0.5),
        position.y + size.y * 0.5 - cursor.y,
    ))
}

/// Apply a framing to the board camera.
pub fn apply(
    transform: &mut Transform,
    projection: &mut Projection,
    framing: (Transform, f32),
    zoom: f32,
    pan: Vec2,
) {
    let (base, scale) = framing;
    let offset = base.rotation * Vec3::new(pan.x, pan.y, 0.0);
    *transform =
        Transform::from_translation(base.translation + offset).with_rotation(base.rotation);
    if let Projection::Orthographic(ortho) = projection {
        ortho.scaling_mode = bevy::camera::ScalingMode::FixedVertical {
            viewport_height: scale * zoom,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::MatchSettings;

    fn game() -> TacticsGame {
        TacticsGame::new(MatchSettings::standard()).expect("solves")
    }

    /// The flat view frames one level; the isometric view frames the stack. If
    /// they framed the same volume, one of them would be wrong about what it is
    /// showing.
    #[test]
    fn the_map_and_low_wall_view_frame_the_same_deck_with_distinct_projections() {
        let game = game();
        let viewport = Vec2::new(1220.0, 1000.0);
        let (iso, iso_scale) = frame(&game, ViewMode::Overview, 0, viewport);
        let (flat, flat_scale) = frame(&game, ViewMode::Deck, 0, viewport);
        assert!(iso_scale.is_finite() && flat_scale.is_finite());
        assert!(iso_scale > 0.0 && flat_scale > 0.0);
        assert_ne!(
            iso.rotation, flat.rotation,
            "map and low-wall deck have distinct jobs"
        );
    }

    #[test]
    fn a_level_with_nothing_on_it_still_produces_a_camera() {
        let game = game();
        let empty = game.world.config.levels + 5;
        let (transform, scale) = frame(&game, ViewMode::Deck, empty, Vec2::new(1220.0, 1000.0));
        assert!(transform.translation.is_finite());
        assert!(scale >= 20.0);
    }

    #[test]
    fn the_camera_viewport_exclusively_owns_map_pointer_input() {
        let window = Window {
            resolution: bevy::window::WindowResolution::new(1000, 800),
            ..default()
        };
        let camera = Camera {
            viewport: Some(bevy::camera::Viewport {
                physical_position: UVec2::ZERO,
                physical_size: UVec2::new(620, 800),
                ..default()
            }),
            ..default()
        };
        assert!(cursor_in_viewport(
            &window,
            &camera,
            Vec2::new(619.0, 400.0)
        ));
        assert_eq!(
            viewport_cursor_offset(&window, &camera, Vec2::new(310.0, 400.0)),
            Some(Vec2::ZERO)
        );
        assert!(!cursor_in_viewport(
            &window,
            &camera,
            Vec2::new(621.0, 400.0)
        ));
    }
}
