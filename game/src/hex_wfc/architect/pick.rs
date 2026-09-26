//! The board's geometry: which cell is under the cursor, and how a floor is framed.
//!
//! The board is a top-down orthographic view, screen right along world +X and screen
//! down along world +Z, so a pixel maps to the floor plane by a scale and an offset.

use bevy::prelude::*;
use observed_facility::hex_wfc::HexWfcConfig;
use observed_hex::{HexCoord, HexFace, hex_origin};

/// Metres between neighbouring cell centres, and between rows.
const COLUMN: f32 = 14.0;
const ROW: f32 = 12.0;
/// A cell's circumradius: the board frames whole hexes, not their centres.
pub(super) const CELL_RADIUS: f32 = 8.1;

/// Screen space the board must leave clear: the side panel and the hand, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::hex_wfc) struct Margins {
    pub left: f32,
    pub bottom: f32,
    pub top: f32,
}

/// Where the board's camera looks and how many metres a pixel spans.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::hex_wfc) struct Framing {
    pub centre: Vec2,
    pub metres_per_pixel: f32,
}

/// The smallest stretch of floor the board shows, in metres: about twelve cells by
/// eight, so a team that has seen three cells still sees them in their surroundings.
pub(super) const MIN_VIEW: Vec2 = Vec2::new(12.0 * COLUMN, 8.0 * ROW);

/// Frame the whole of a floor of `config`: what the board shows before anything is known.
#[must_use]
pub(super) fn frame_floor(config: HexWfcConfig, window: Vec2, margins: Margins) -> Framing {
    let corners = [
        (0, 0),
        (config.cols - 1, 0),
        (0, config.rows - 1),
        (config.cols - 1, config.rows - 1),
    ];
    frame_cells(
        corners.map(|(q, r)| HexCoord { q, r, level: 0 }),
        Vec2::ZERO,
        window,
        margins,
    )
    .expect("a lattice has corners")
}

/// Frame `cells` inside a `window` of pixels, clear of `margins`, showing at least
/// `min_view` metres of floor. `None` when there are no cells.
#[must_use]
pub(super) fn frame_cells(
    cells: impl IntoIterator<Item = HexCoord>,
    min_view: Vec2,
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
    let grow = (min_view - (max - min)).max(Vec2::ZERO) * 0.5;
    let (min, max) = (min - grow, max + grow);
    let room = Vec2::new(
        (window.x - margins.left).max(1.0),
        (window.y - margins.bottom - margins.top).max(1.0),
    );
    let size = max - min;
    let metres_per_pixel = (size.x / room.x).max(size.y / room.y) * 1.06;
    // The free area's centre sits right of and above the window's; shift the camera the
    // other way so the floor lands in it.
    let shift = Vec2::new(-margins.left * 0.5, (margins.bottom - margins.top) * 0.5);
    Some(Framing {
        centre: (min + max) * 0.5 + shift * metres_per_pixel,
        metres_per_pixel,
    })
}

/// Ease `from` toward `to`, closing `rate` of the gap per second, so the board glides as
/// the team maps more rather than jumping.
#[must_use]
pub(super) fn ease(from: Framing, to: Framing, rate: f32, seconds: f32) -> Framing {
    let t = (rate * seconds).clamp(0.0, 1.0);
    Framing {
        centre: from.centre.lerp(to.centre, t),
        metres_per_pixel: from.metres_per_pixel + (to.metres_per_pixel - from.metres_per_pixel) * t,
    }
}

/// The floor point under a pixel.
#[must_use]
pub(super) fn floor_point(framing: Framing, window: Vec2, pixel: Vec2) -> Vec2 {
    framing.centre + (pixel - window * 0.5) * framing.metres_per_pixel
}

/// The cell whose hexagon contains a floor point, on `level`, if it is on the lattice.
#[must_use]
pub(super) fn cell_at(config: HexWfcConfig, point: Vec2, level: u8) -> Option<HexCoord> {
    // Axial coordinates from the plan, then cube rounding: the nearest centre is the
    // containing hexagon.
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
    let inside = |value: f32, count: u16| value >= 0.0 && value < f32::from(count);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (inside(rq, config.cols) && inside(rr, config.rows)).then_some(HexCoord {
        q: rq as u16,
        r: rr as u16,
        level,
    })
}

/// The plan direction of a lateral face, as a screen angle in radians: 0 along screen
/// right, increasing clockwise, as screen down is world +Z.
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

    #[test]
    fn the_framed_floor_fills_the_free_area_and_its_centre_lands_in_it() {
        let config = config();
        let window = Vec2::new(1280.0, 800.0);
        let margins = Margins {
            left: 320.0,
            bottom: 220.0,
            top: 40.0,
        };
        let framing = frame_floor(config, window, margins);
        // Every cell centre lands inside the free area.
        for (q, r) in [
            (0, 0),
            (config.cols - 1, config.rows - 1),
            (0, config.rows - 1),
        ] {
            let at = hex_origin(HexCoord { q, r, level: 0 });
            let pixel = (Vec2::new(at[0], at[2]) - framing.centre) / framing.metres_per_pixel
                + window * 0.5;
            assert!(pixel.x > margins.left && pixel.x < window.x, "{pixel}");
            assert!(
                pixel.y > margins.top && pixel.y < window.y - margins.bottom,
                "{pixel}"
            );
        }
        // And picking round-trips through the same framing.
        let cell = HexCoord {
            q: 9,
            r: 7,
            level: 0,
        };
        let at = hex_origin(cell);
        let pixel =
            (Vec2::new(at[0], at[2]) - framing.centre) / framing.metres_per_pixel + window * 0.5;
        assert_eq!(
            cell_at(config, floor_point(framing, window, pixel), 0),
            Some(cell)
        );
    }

    #[test]
    fn a_few_known_cells_are_framed_in_their_surroundings_and_many_fill_the_view() {
        let window = Vec2::new(1280.0, 800.0);
        let margins = Margins {
            left: 320.0,
            bottom: 220.0,
            top: 40.0,
        };
        let one = [HexCoord {
            q: 3,
            r: 3,
            level: 0,
        }];
        let close = frame_cells(one, MIN_VIEW, window, margins).expect("a cell");
        let whole = frame_floor(config(), window, margins);
        assert!(
            close.metres_per_pixel < whole.metres_per_pixel,
            "closer than the lattice"
        );
        // At least the minimum view fits the free area.
        let room = Vec2::new(
            window.x - margins.left,
            window.y - margins.bottom - margins.top,
        );
        assert!(room.x * close.metres_per_pixel >= MIN_VIEW.x);
        assert!(room.y * close.metres_per_pixel >= MIN_VIEW.y);
        assert_eq!(
            frame_cells(std::iter::empty(), MIN_VIEW, window, margins),
            None
        );
    }

    #[test]
    fn easing_closes_the_gap_and_stops_there() {
        let a = Framing {
            centre: Vec2::ZERO,
            metres_per_pixel: 1.0,
        };
        let b = Framing {
            centre: Vec2::new(10.0, 0.0),
            metres_per_pixel: 0.5,
        };
        let halfway = ease(a, b, 5.0, 0.1);
        assert!((halfway.centre.x - 5.0).abs() < 1e-4);
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
