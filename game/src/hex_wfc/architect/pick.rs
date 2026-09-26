//! The board's geometry: how a floor is framed at the isometric pitch, which ray a pixel
//! is, and which cell that ray meets on the floor in view.
//!
//! The board is looked at the way `architect_lab` and the survivor map look at the
//! facility (`observed_style::iso`), orthographically, so a pixel is a ray parallel to the
//! camera's view and picking is that ray meeting the floor's deck: exact at any angle,
//! with nothing drawn in front of the floor in view.

use bevy::prelude::*;
use observed_facility::hex_wfc::HexWfcConfig;
use observed_hex::{FLOOR_SLAB_TOP, HexCoord, HexFace, TILE_LEVEL_HEIGHT, hex_origin};
use observed_style::iso::{ISO_PITCH, detent_yaw};

/// Metres between neighbouring cell centres, and between rows.
const COLUMN: f32 = 14.0;
const ROW: f32 = 12.0;
/// A cell's circumradius: the board frames whole hexes, not their centres.
pub(super) const CELL_RADIUS: f32 = 8.1;
/// The smallest stretch of floor the board shows, in metres: about eight cells by six,
/// so a team that has seen three cells still sees them in their surroundings.
pub(super) const MIN_VIEW: Vec2 = Vec2::new(8.0 * COLUMN, 6.0 * ROW);

/// Screen space the board must leave clear: the side panel, the hand and the top bar,
/// in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::hex_wfc) struct Margins {
    pub left: f32,
    pub bottom: f32,
    pub top: f32,
}

/// What the board's camera looks at and how many metres a pixel spans.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::hex_wfc) struct Framing {
    /// The point that appears at the centre of the board's free area.
    pub focus: Vec3,
    pub metres_per_pixel: f32,
}

/// Where the board's scene is built: the building at its true plan, moved far from the
/// facility (and from the prisons below it) so the world's lamps and torches, which light
/// whatever is near them on every layer, never reach it. Picking and framing work in the
/// facility's own coordinates; only what is drawn is moved.
pub(super) const BOARD_ORIGIN: Vec3 = Vec3::new(20_000.0, 0.0, 0.0);

/// The board camera's orientation: the isometric pitch, from the first detent.
#[must_use]
pub(super) fn rotation() -> Quat {
    Quat::from_euler(EulerRot::YXZ, detent_yaw(0), ISO_PITCH, 0.0)
}

/// The height of a floor's deck, where the board's picking plane lies.
#[must_use]
pub(super) fn deck(level: u8) -> f32 {
    f32::from(level) * TILE_LEVEL_HEIGHT + FLOOR_SLAB_TOP
}

/// Frame `cells` on the floor at `level` inside the free area of a `window`, showing at
/// least [`MIN_VIEW`] of floor. `None` when there are no cells.
#[must_use]
pub(super) fn frame(
    cells: impl IntoIterator<Item = HexCoord>,
    level: u8,
    window: Vec2,
    margins: Margins,
) -> Option<Framing> {
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    let mut any = false;
    for cell in cells {
        let at = hex_origin(cell);
        min = min.min(Vec2::new(at[0], at[2]) - Vec2::splat(CELL_RADIUS));
        max = max.max(Vec2::new(at[0], at[2]) + Vec2::splat(CELL_RADIUS));
        any = true;
    }
    if !any {
        return None;
    }
    let grow = (MIN_VIEW - (max - min)).max(Vec2::ZERO) * 0.5;
    let (min, max) = (min - grow, max + grow);
    let floor = deck(level);
    let low = Vec3::new(min.x, floor, min.y);
    let high = Vec3::new(max.x, floor + 3.0, max.y);
    let focus = (low + high) * 0.5;
    let inverse = rotation().inverse();
    let mut extent = Vec2::ZERO;
    for i in 0..8u8 {
        let corner = Vec3::new(
            if i & 1 == 0 { low.x } else { high.x },
            if i & 2 == 0 { low.y } else { high.y },
            if i & 4 == 0 { low.z } else { high.z },
        );
        extent = extent.max((inverse * (corner - focus)).truncate().abs());
    }
    let room = free_area(window, margins);
    let metres_per_pixel = (extent.x * 2.0 / room.x).max(extent.y * 2.0 / room.y) * 1.1;
    Some(Framing {
        focus,
        metres_per_pixel,
    })
}

fn free_area(window: Vec2, margins: Margins) -> Vec2 {
    Vec2::new(
        (window.x - margins.left).max(1.0),
        (window.y - margins.bottom - margins.top).max(1.0),
    )
}

/// Where the camera stands for `framing`: back along its view from the focus, and moved
/// so the focus lands in the middle of the free area rather than of the window.
#[must_use]
pub(super) fn camera(framing: Framing, margins: Margins) -> Transform {
    let rotation = rotation();
    let right = rotation * Vec3::X;
    let up = rotation * Vec3::Y;
    let shift = right * (-margins.left * 0.5) + up * ((margins.top - margins.bottom) * 0.5);
    Transform::from_translation(
        framing.focus + rotation * Vec3::Z * 1_500.0 + shift * framing.metres_per_pixel,
    )
    .with_rotation(rotation)
}

/// The ray a pixel of the window is, for a camera at `at` framing at `metres_per_pixel`.
#[must_use]
pub(super) fn ray(at: Transform, metres_per_pixel: f32, window: Vec2, pixel: Vec2) -> Ray3d {
    let offset = (pixel - window * 0.5) * metres_per_pixel;
    let origin = at.translation + at.rotation * Vec3::new(offset.x, -offset.y, 0.0);
    Ray3d::new(origin, at.forward())
}

/// Where a ray meets a floor's deck, as a point in plan.
#[must_use]
pub(super) fn on_deck(ray: Ray3d, level: u8) -> Option<Vec2> {
    let distance = ray.intersect_plane(Vec3::Y * deck(level), InfinitePlane3d::new(Vec3::Y))?;
    let hit = ray.get_point(distance);
    Some(Vec2::new(hit.x, hit.z))
}

/// Ease `from` toward `to`, closing `rate` of the gap per second, so the board glides as
/// the team maps more, or up and down the climb, rather than jumping.
#[must_use]
pub(super) fn ease(from: Framing, to: Framing, rate: f32, seconds: f32) -> Framing {
    let t = (rate * seconds).clamp(0.0, 1.0);
    Framing {
        focus: from.focus.lerp(to.focus, t),
        metres_per_pixel: from.metres_per_pixel + (to.metres_per_pixel - from.metres_per_pixel) * t,
    }
}

/// The cell whose hexagon contains a plan point, on `level`, if it is on the lattice.
#[must_use]
pub(super) fn cell_at(config: HexWfcConfig, point: Vec2, level: u8) -> Option<HexCoord> {
    let (q, r) = axial(point);
    let inside = |value: f32, count: u16| value >= 0.0 && value < f32::from(count);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (inside(q, config.cols) && inside(r, config.rows)).then_some(HexCoord {
        q: q as u16,
        r: r as u16,
        level,
    })
}

/// The cell whose hexagon contains a point, on `level`, whatever lattice it is on.
#[must_use]
pub(super) fn cell_at_level(point: Vec3, level: u8) -> Option<HexCoord> {
    let (q, r) = axial(Vec2::new(point.x, point.z));
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (q >= 0.0 && r >= 0.0).then_some(HexCoord {
        q: q as u16,
        r: r as u16,
        level,
    })
}

/// Axial coordinates of the hexagon containing a plan point, by cube rounding.
fn axial(point: Vec2) -> (f32, f32) {
    let r = point.y / ROW;
    let q = (point.x - r * COLUMN * 0.5) / COLUMN;
    let s = -q - r;
    let (mut rq, mut rr, rs) = (q.round(), r.round(), s.round());
    let (dq, dr, ds) = ((rq - q).abs(), (rr - r).abs(), (rs - s).abs());
    if dq > dr && dq > ds {
        rq = -rr - rs;
    } else if dr > ds {
        rr = -rq - rs;
    }
    (rq, rr)
}

/// The plan direction of a lateral face, as an angle in radians from world +X toward +Z:
/// on a card seen from above, from screen right clockwise.
#[must_use]
pub(super) fn face_angle(face: HexFace) -> f32 {
    let (dq, dr, _) = face.delta();
    #[allow(clippy::cast_precision_loss)]
    let (x, z) = (
        dq as f32 * COLUMN + dr as f32 * COLUMN * 0.5,
        dr as f32 * ROW,
    );
    z.atan2(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> HexWfcConfig {
        HexWfcConfig::arc_default()
    }

    const WINDOW: Vec2 = Vec2::new(1280.0, 800.0);
    const MARGINS: Margins = Margins {
        left: 320.0,
        bottom: 220.0,
        top: 56.0,
    };

    #[test]
    fn every_centre_picks_its_own_cell_and_so_does_a_point_near_it() {
        let config = config();
        for q in 0..config.cols {
            for r in 0..config.rows {
                let cell = HexCoord { q, r, level: 3 };
                let at = hex_origin(cell);
                let centre = Vec2::new(at[0], at[2]);
                assert_eq!(cell_at(config, centre, 3), Some(cell));
                assert_eq!(
                    cell_at(config, centre + Vec2::new(4.0, -3.0), 3),
                    Some(cell)
                );
            }
        }
    }

    #[test]
    fn a_point_just_across_a_face_picks_the_neighbour() {
        let config = config();
        let cell = HexCoord {
            q: 5,
            r: 5,
            level: 0,
        };
        let at = hex_origin(cell);
        for face in HexFace::LATERAL {
            let next = config.grid().neighbor(cell, face).expect("inside");
            let there = hex_origin(next);
            let across = Vec2::new(at[0], at[2]).lerp(Vec2::new(there[0], there[2]), 0.56);
            assert_eq!(cell_at(config, across, 0), Some(next), "{face:?}");
        }
    }

    #[test]
    fn off_the_lattice_is_nothing() {
        assert_eq!(cell_at(config(), Vec2::new(-40.0, -40.0), 0), None);
    }

    /// The pixel a point appears at, for a camera at `at`: the inverse of [`ray`].
    fn pixel_of(at: Transform, metres_per_pixel: f32, point: Vec3) -> Vec2 {
        let local = at.rotation.inverse() * (point - at.translation);
        Vec2::new(local.x, -local.y) / metres_per_pixel + WINDOW * 0.5
    }

    #[test]
    fn the_known_cells_land_in_the_free_area_and_pick_back_exactly() {
        let config = config();
        let cells: Vec<HexCoord> = (3..9)
            .flat_map(|q| (2..6).map(move |r| HexCoord { q, r, level: 2 }))
            .collect();
        let framing = frame(cells.iter().copied(), 2, WINDOW, MARGINS).expect("cells");
        let at = camera(framing, MARGINS);
        for &cell in &cells {
            let origin = hex_origin(cell);
            let point = Vec3::new(origin[0], deck(2), origin[2]);
            let pixel = pixel_of(at, framing.metres_per_pixel, point);
            assert!(
                pixel.x > MARGINS.left && pixel.x < WINDOW.x,
                "{cell:?} at {pixel}"
            );
            assert!(
                pixel.y > MARGINS.top && pixel.y < WINDOW.y - MARGINS.bottom,
                "{cell:?} at {pixel}"
            );
            let back = on_deck(ray(at, framing.metres_per_pixel, WINDOW, pixel), 2)
                .and_then(|plan| cell_at(config, plan, 2));
            assert_eq!(back, Some(cell));
        }
    }

    #[test]
    fn a_few_cells_are_framed_in_their_surroundings() {
        let one = [HexCoord {
            q: 3,
            r: 3,
            level: 0,
        }];
        let many: Vec<HexCoord> = (0..20)
            .flat_map(|q| (0..14).map(move |r| HexCoord { q, r, level: 0 }))
            .collect();
        let close = frame(one, 0, WINDOW, MARGINS).expect("a cell");
        let wide = frame(many, 0, WINDOW, MARGINS).expect("cells");
        assert!(close.metres_per_pixel < wide.metres_per_pixel);
        assert_eq!(frame(std::iter::empty(), 0, WINDOW, MARGINS), None);
    }

    #[test]
    fn easing_closes_the_gap_and_stops_there() {
        let a = Framing {
            focus: Vec3::ZERO,
            metres_per_pixel: 1.0,
        };
        let b = Framing {
            focus: Vec3::new(10.0, 0.0, 0.0),
            metres_per_pixel: 0.5,
        };
        let halfway = ease(a, b, 5.0, 0.1);
        assert!((halfway.focus.x - 5.0).abs() < 1e-4);
        assert_eq!(ease(a, b, 5.0, 10.0), b);
    }

    #[test]
    fn opposite_faces_point_opposite_ways() {
        for face in HexFace::LATERAL {
            let a = face_angle(face);
            let b = face_angle(face.opposite());
            let turn = (a - b).rem_euclid(std::f32::consts::TAU);
            assert!((turn - std::f32::consts::PI).abs() < 1e-4, "{face:?}");
        }
    }
}
