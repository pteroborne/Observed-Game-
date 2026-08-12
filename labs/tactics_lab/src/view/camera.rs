//! Where the camera sits, and how the board is framed.
//!
//! Both modes frame the whole board by default rather than starting zoomed in.
//! The lab exists to make a facility legible at a glance; a camera that opens
//! inside it would reproduce the first-person problem this variant was built to
//! escape.

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
/// Padding around the framed board, as a multiple of its extent.
const FRAME_MARGIN: f32 = 1.55;

#[derive(Component)]
pub struct BoardCamera;

/// The transform and orthographic scale that frame the whole board.
#[must_use]
pub fn frame(game: &TacticsGame, mode: ViewMode, level: u8) -> (Transform, f32) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut any = false;
    for &cell in game.world.placements.keys() {
        if !draws_level(mode, cell, level) {
            continue;
        }
        if mode == ViewMode::Deck && paint(game, cell) == CellPaint::Unknown {
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
        ViewMode::Overview => (
            Quat::from_euler(EulerRot::YXZ, ISO_YAW, ISO_PITCH, 0.0),
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
    // Vertical extent of the orthographic view. Taking the larger horizontal
    // span keeps a wide board framed on a wide window.
    let scale = (extent.x.max(extent.z).max(extent.y) * FRAME_MARGIN).max(20.0);
    (transform, scale)
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
    fn the_overview_frames_more_than_one_detailed_deck() {
        let game = game();
        let (iso, iso_scale) = frame(&game, ViewMode::Overview, 0);
        let (flat, flat_scale) = frame(&game, ViewMode::Deck, 0);
        assert!(iso_scale >= flat_scale);
        assert_eq!(
            iso.rotation, flat.rotation,
            "both modes use the same isometric read"
        );
    }

    #[test]
    fn a_level_with_nothing_on_it_still_produces_a_camera() {
        let game = game();
        let empty = game.world.config.levels + 5;
        let (transform, scale) = frame(&game, ViewMode::Deck, empty);
        assert!(transform.translation.is_finite());
        assert!(scale >= 20.0);
    }
}
