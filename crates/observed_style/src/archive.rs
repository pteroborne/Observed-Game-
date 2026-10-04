//! Shared Babel / Infinite Gallery mineral finish, established by the Archive Well.
//! Bindings, bronze gallery details and fixed wonder light remain local to the Archive.
use crate::{ArchitectureSurfaceRole, HexSurfaceLook};
use bevy::color::{Color, LinearRgba};

pub const FIXTURE_INTENSITY: f32 = 6_500_000.0;
pub const FIXTURE_BOUNCE_INTENSITY: f32 = 170_000.0;
pub const ROUGHNESS: f32 = 0.68;
#[must_use]
pub fn fixture_color() -> Color {
    Color::srgb(0.98, 0.91, 0.73)
}
#[must_use]
pub fn surface(role: ArchitectureSurfaceRole) -> HexSurfaceLook {
    HexSurfaceLook {
        base_color: match role {
            ArchitectureSurfaceRole::Floor => Color::srgb(0.55, 0.49, 0.37),
            ArchitectureSurfaceRole::Ceiling => Color::srgb(0.81, 0.79, 0.70),
            _ => Color::srgb(0.74, 0.70, 0.59),
        },
        emissive: LinearRgba::BLACK,
        unlit: false,
        textured: true,
    }
}
/// Muted bindings are architectural detail, never gameplay signals.
#[must_use]
pub fn detail_colors() -> [Color; 8] {
    [
        Color::srgb(0.17, 0.10, 0.085),
        Color::srgb(0.27, 0.17, 0.105),
        Color::srgb(0.14, 0.20, 0.18),
        Color::srgb(0.22, 0.12, 0.14),
        Color::srgb(0.38, 0.32, 0.22),
        Color::srgb(0.12, 0.16, 0.21),
        Color::srgb(0.36, 0.27, 0.14),
        fixture_color(),
    ]
}
/// Broad tessellation establishes the reading floor's scale without collision detail.
#[must_use]
pub fn floor_albedo() -> Vec<u8> {
    let n = crate::surfaces::SURFACE_TEXTURE_SIZE as usize;
    let mut rgba = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let joint = x % 64 < 2 || y % 64 < 2;
            let dark = (x / 64 + y / 64) % 2 == 0;
            let tone = if joint {
                114
            } else if dark {
                190
            } else {
                246
            };
            rgba.extend_from_slice(&[tone, tone, tone, 255]);
        }
    }
    rgba
}
