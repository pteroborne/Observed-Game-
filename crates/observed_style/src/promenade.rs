//! Local Sky wonder finishes: chalk stone, brushed titanium and warm guide lights.
use bevy::color::Color;
pub const FIXTURE_INTENSITY: f32 = 2_800_000.0;
pub const FIXTURE_BOUNCE_INTENSITY: f32 = 240_000.0;
#[must_use]
pub fn fixture_color() -> Color {
    Color::srgb(1.0, 0.91, 0.77)
}
#[must_use]
pub fn colors() -> [Color; 6] {
    [
        Color::srgb(0.78, 0.80, 0.78),
        Color::srgb(0.89, 0.90, 0.86),
        Color::srgb(0.48, 0.55, 0.57),
        Color::srgb(0.26, 0.33, 0.35),
        fixture_color(),
        Color::srgb(0.57, 0.61, 0.60),
    ]
}
#[must_use]
pub fn albedo(role: usize) -> Vec<u8> {
    let n = crate::surfaces::SURFACE_TEXTURE_SIZE as usize;
    let mut rgba = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let grain = ((x * 31 + y * 17 + (x * y) % 53) % 13) as u8;
            let tone = match role {
                0 if x % 128 < 2 => 182,
                0 | 1 | 5 => 238 + grain,
                2 | 3 => 222 + (y % 7) as u8 * 3,
                _ => 255,
            };
            rgba.extend_from_slice(&[tone, tone, tone, 255]);
        }
    }
    rgba
}
#[must_use]
pub fn roughness(role: usize) -> f32 {
    if matches!(role, 2 | 3) { 0.38 } else { 0.72 }
}
#[must_use]
pub fn emission(role: usize) -> f32 {
    if role == 4 { 1.0 } else { 0.0 }
}
#[must_use]
pub fn unpowered_panel_color() -> Color {
    Color::srgb(0.23, 0.24, 0.22)
}
