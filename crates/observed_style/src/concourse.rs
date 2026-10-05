//! Switching Concourse's local Lumen ceramic, canopy metal and platform markings.
use bevy::color::Color;
pub const FIXTURE_INTENSITY: f32 = 5_200_000.0;
pub const FIXTURE_BOUNCE_INTENSITY: f32 = 260_000.0;
#[must_use]
pub fn fixture_color() -> Color {
    Color::srgb(0.92, 0.97, 1.0)
}
#[must_use]
pub fn colors() -> [Color; 6] {
    [
        Color::srgb(0.81, 0.83, 0.82),
        Color::srgb(0.92, 0.94, 0.93),
        Color::srgb(0.09, 0.14, 0.17),
        Color::srgb(0.98, 0.63, 0.19),
        fixture_color(),
        Color::srgb(0.26, 0.34, 0.37),
    ]
}
#[must_use]
pub fn albedo(role: usize) -> Vec<u8> {
    let n = crate::surfaces::SURFACE_TEXTURE_SIZE as usize;
    let mut rgba = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let tone = match role {
                0 if x % 64 < 2 || y % 64 < 2 => 155,
                0 => {
                    if (x / 64 + y / 64) % 2 == 0 {
                        235
                    } else {
                        249
                    }
                }
                1 if x % 128 < 2 || y % 128 < 2 => 192,
                1 => 245 + ((x * 17 + y * 13) % 7) as u8,
                2 => {
                    if y % 24 < 3 {
                        128
                    } else {
                        230
                    }
                }
                _ => 255,
            };
            rgba.extend_from_slice(&[tone, tone, tone, 255]);
        }
    }
    rgba
}
#[must_use]
pub fn roughness(role: usize) -> f32 {
    if role == 0 {
        0.30
    } else if role == 5 {
        0.48
    } else {
        0.62
    }
}
#[must_use]
pub fn emission(role: usize) -> f32 {
    if role == 4 { 1.1 } else { 0.0 }
}

#[must_use]
pub fn unpowered_panel_color() -> Color {
    Color::srgb(0.18, 0.22, 0.24)
}
