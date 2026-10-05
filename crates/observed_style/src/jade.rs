//! Jade Nave's local stone, warm mineral flooring and bronze architectural finish.
use bevy::color::Color;
pub const FIXTURE_INTENSITY: f32 = 4_200_000.0;
pub const FIXTURE_BOUNCE_INTENSITY: f32 = 340_000.0;
#[must_use]
pub fn fixture_color() -> Color {
    Color::srgb(1.0, 0.85, 0.64)
}
#[must_use]
pub fn colors() -> [Color; 6] {
    [
        Color::srgb(0.66, 0.64, 0.55),
        Color::srgb(0.83, 0.82, 0.71),
        Color::srgb(0.16, 0.43, 0.33),
        Color::srgb(0.57, 0.38, 0.17),
        fixture_color(),
        Color::srgb(0.075, 0.15, 0.12),
    ]
}
#[must_use]
pub fn albedo(role: usize) -> Vec<u8> {
    let n = crate::surfaces::SURFACE_TEXTURE_SIZE as usize;
    let mut rgba = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let grain = ((x * 17 + y * 43 + (x * y) % 79) % 19) as u8;
            let wave = ((y as f32 * 0.032).sin() * 19.0 + (y as f32 * 0.081).sin() * 7.0) as i32;
            let vein = (x as i32 + y as i32 / 2 + wave).rem_euclid(83);
            let tone = match role {
                2 | 5 if vein < 3 => 245,
                2 | 5 if vein < 8 => 180 + grain,
                2 | 5 => 210 + grain,
                0 if x % 128 < 2 || y % 128 < 2 => 164,
                0 | 1 => 231 + grain,
                3 => 213 + grain,
                _ => 255,
            };
            rgba.extend_from_slice(&[tone, tone, tone, 255]);
        }
    }
    rgba
}
#[must_use]
pub fn roughness(role: usize) -> f32 {
    if matches!(role, 2 | 5) {
        0.28
    } else if role == 3 {
        0.36
    } else {
        0.66
    }
}
#[must_use]
pub fn emission(role: usize) -> f32 {
    if role == 4 { 1.1 } else { 0.0 }
}
#[must_use]
pub fn unpowered_panel_color() -> Color {
    Color::srgb(0.18, 0.15, 0.10)
}
