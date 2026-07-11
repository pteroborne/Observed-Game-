//! The luminance corridor: the pure check Phase 69 promotes into the game's
//! visual audit. A capture passes when it is neither too dark to read (floor)
//! nor blown out (ceiling). Backlog #6 exists because near-black captures
//! shipped for a full arc without anything objecting; this makes that class of
//! defect a red test.

use serde::Serialize;

/// Floor: at least the brightest twentieth of the frame must be clearly
/// visible…
pub const FLOOR_P95_MIN: f32 = 0.02;
/// …and the median pixel must not be effectively black.
pub const FLOOR_P50_MIN: f32 = 0.002;
/// Ceiling: the median pixel must not be blown toward white…
pub const CEILING_P50_MAX: f32 = 0.75;
/// …and the darkest twentieth must retain some shading structure (an all-white
/// frame has none).
pub const CEILING_P05_MAX: f32 = 0.40;

/// Relative-luminance percentiles of a frame plus the corridor verdict.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct CorridorVerdict {
    pub p05: f32,
    pub p50: f32,
    pub p95: f32,
    pub floor_pass: bool,
    pub ceiling_pass: bool,
}

impl CorridorVerdict {
    pub fn pass(&self) -> bool {
        self.floor_pass && self.ceiling_pass
    }
}

/// sRGB channel (0–255) to linear intensity (0–1), the standard transfer curve.
fn srgb_to_linear(byte: u8) -> f32 {
    let c = byte as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Rec. 709 relative luminance of one sRGB pixel.
fn pixel_luminance(r: u8, g: u8, b: u8) -> f32 {
    0.2126 * srgb_to_linear(r) + 0.7152 * srgb_to_linear(g) + 0.0722 * srgb_to_linear(b)
}

/// Evaluate the corridor over an RGBA8 buffer (alpha ignored — HDR screenshots
/// store brightness metadata there). Pixels are subsampled for speed; the
/// percentile error from a stride of 4 is far below the corridor margins.
pub fn corridor(rgba: &[u8], stride: usize) -> CorridorVerdict {
    let stride = stride.max(1);
    let mut lums: Vec<f32> = rgba
        .chunks_exact(4)
        .step_by(stride)
        .map(|px| pixel_luminance(px[0], px[1], px[2]))
        .collect();
    if lums.is_empty() {
        // An empty frame is maximally suspicious: fail the floor, not the test harness.
        return CorridorVerdict {
            p05: 0.0,
            p50: 0.0,
            p95: 0.0,
            floor_pass: false,
            ceiling_pass: true,
        };
    }
    lums.sort_by(|a, b| a.total_cmp(b));
    let pct = |p: f32| lums[((lums.len() - 1) as f32 * p) as usize];
    let (p05, p50, p95) = (pct(0.05), pct(0.50), pct(0.95));
    CorridorVerdict {
        p05,
        p50,
        p95,
        floor_pass: p95 >= FLOOR_P95_MIN && p50 >= FLOOR_P50_MIN,
        ceiling_pass: p50 <= CEILING_P50_MAX && p05 <= CEILING_P05_MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(r: u8, g: u8, b: u8, n: usize) -> Vec<u8> {
        [r, g, b, 255].repeat(n)
    }

    #[test]
    fn the_archived_phase_62_dark_capture_fails_the_floor() {
        // The exact evidence PNG that shipped with Phase 62 and motivated
        // backlog #6, pinned as a fixture. If this test ever passes the floor,
        // the corridor has been loosened past the defect it exists to catch.
        let png = image::open(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/phase62_long_hallway_dark.png"
        ))
        .expect("pinned fixture decodes")
        .to_rgba8();
        let verdict = corridor(png.as_raw(), 4);
        assert!(
            !verdict.floor_pass,
            "the near-black shipped capture must fail the floor: {verdict:?}"
        );
        assert!(verdict.ceiling_pass, "dark frames don't violate the ceiling");
    }

    #[test]
    fn an_all_white_frame_fails_the_ceiling() {
        let verdict = corridor(&solid(255, 255, 255, 4096), 1);
        assert!(!verdict.ceiling_pass, "blown-out white must fail: {verdict:?}");
        assert!(verdict.floor_pass);
    }

    #[test]
    fn a_readable_mid_range_frame_passes_both() {
        // A plausible lit room: dark shadow band, mid tones, a bright band.
        let mut buf = solid(8, 8, 12, 1400);
        buf.extend(solid(70, 74, 82, 1800));
        buf.extend(solid(190, 185, 170, 900));
        let verdict = corridor(&buf, 1);
        assert!(verdict.pass(), "a readable frame passes the corridor: {verdict:?}");
    }

    #[test]
    fn an_all_black_frame_fails_the_floor() {
        let verdict = corridor(&solid(0, 0, 0, 4096), 1);
        assert!(!verdict.floor_pass);
        assert!(verdict.ceiling_pass);
    }

    #[test]
    fn percentiles_are_ordered_and_bounded() {
        let mut buf = solid(0, 0, 0, 100);
        buf.extend(solid(128, 128, 128, 100));
        buf.extend(solid(255, 255, 255, 100));
        let v = corridor(&buf, 1);
        assert!(0.0 <= v.p05 && v.p05 <= v.p50 && v.p50 <= v.p95 && v.p95 <= 1.0);
    }
}
