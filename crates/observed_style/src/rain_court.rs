//! Local Zen wonder palette: dry cedar, opaque rice paper and wet garden stone.
use bevy::color::Color;
pub const FIXTURE_INTENSITY: f32 = 3_800_000.0;
pub const FIXTURE_BOUNCE_INTENSITY: f32 = 110_000.0;
#[must_use]
pub fn fixture_color() -> Color {
    Color::srgb(1.0, 0.83, 0.57)
}
/// Timber, paper, rock, paving, moss, tatami, lantern housing.
#[must_use]
pub fn colors() -> [Color; 7] {
    [
        Color::srgb(0.18, 0.12, 0.078),
        Color::srgb(0.80, 0.75, 0.61),
        Color::srgb(0.30, 0.32, 0.29),
        Color::srgb(0.65, 0.66, 0.59),
        Color::srgb(0.22, 0.29, 0.14),
        Color::srgb(0.48, 0.43, 0.29),
        Color::srgb(0.12, 0.10, 0.08),
    ]
}
#[must_use]
pub fn weather_colors() -> [Color; 3] {
    [
        Color::srgb(0.65, 0.73, 0.72),
        Color::srgb(0.58, 0.67, 0.64),
        Color::srgb(0.82, 0.90, 0.85),
    ]
}
#[must_use]
pub fn unpowered_panel_color() -> Color {
    Color::linear_rgb(0.018, 0.025, 0.024)
}

/// Fine grain and broad joints remain subdued beside the observation signals.
#[must_use]
pub fn albedo(index: usize) -> Vec<u8> {
    let n = crate::surfaces::SURFACE_TEXTURE_SIZE as usize;
    let mut rgba = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let hash = (x * 37 + y * 71 + (x * y) % 97) % 23;
            let tone = match index {
                0 => {
                    if x % 64 < 2 {
                        100
                    } else {
                        185 + (y + x / 7) % 31
                    }
                }
                1 => 220 + hash / 3,
                3 => {
                    if x % 128 < 2 || y % 128 < 2 {
                        145
                    } else {
                        204 + hash
                    }
                }
                4 => 145 + hash * 3,
                5 => {
                    if x % 96 < 3 || y % 96 < 3 {
                        160
                    } else {
                        208 + (x + y) % 2 * 20
                    }
                }
                _ => 188 + hash,
            };
            rgba.extend_from_slice(&[tone as u8, tone as u8, tone as u8, 255]);
        }
    }
    rgba
}
