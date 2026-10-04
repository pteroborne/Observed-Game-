//! Reservoir finishes. Ceramic is architectural, water is atmosphere; neither
//! treatment implies a gameplay effect. The raised causeways are the dry route.

use bevy::color::{Color, LinearRgba};

use crate::{ArchitectureSurfaceRole, HexSurfaceLook};

/// Metres above the cell origin. The bed is at 0.5 m, dry deck at 0.75 m.
pub const WATER_HEIGHT: f32 = 0.65;
pub const CERAMIC_ROUGHNESS: f32 = 0.48;
pub const WATER_REFLECTANCE: f32 = 0.18;
pub const WATER_RIPPLE: f32 = 0.0015;

/// Fixed fluorescent downlights carry the bath's illumination. These replace
/// the district's moving key, so every bay is lit before an Observer enters it.
pub const FIXTURE_INTENSITY: f32 = 6_000_000.0;
pub const FIXTURE_RANGE: f32 = 16.0;
pub const FIXTURE_RADIUS: f32 = 0.25;
pub const FIXTURE_INNER_ANGLE: f32 = 0.95;
pub const FIXTURE_OUTER_ANGLE: f32 = 1.2;
/// A small, fixed bounce fill under each fixture approximates diffuse return
/// onto the ceiling. It stays shadowless and outside the moving shadow budget.
pub const FIXTURE_BOUNCE_INTENSITY: f32 = 120_000.0;
pub const FIXTURE_BOUNCE_RANGE: f32 = 10.0;

#[must_use]
pub fn fixture_color() -> Color {
    Color::srgb(0.95, 0.94, 0.83)
}

#[must_use]
pub fn water_tint() -> Color {
    Color::srgb(0.18, 0.45, 0.43)
}

/// Cool mineral ceramic: pale surfaces catch the district's practical lights.
#[must_use]
pub fn surface(role: ArchitectureSurfaceRole) -> HexSurfaceLook {
    let base_color = match role {
        ArchitectureSurfaceRole::Floor => Color::srgb(0.72, 0.77, 0.72),
        ArchitectureSurfaceRole::Ceiling => Color::srgb(0.80, 0.79, 0.71),
        _ => Color::srgb(0.86, 0.84, 0.74),
    };
    HexSurfaceLook {
        base_color,
        emissive: LinearRgba::BLACK,
        unlit: false,
        textured: true,
    }
}

/// Quarter-metre glazed tiles on the renderer's four-metre UV repeat. A narrow,
/// recessed grout joint carries scale without adding collision or mesh entities.
#[must_use]
pub fn ceramic_albedo() -> Vec<u8> {
    let n = crate::surfaces::SURFACE_TEXTURE_SIZE as usize;
    let mut rgba = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let joint = x % 32 <= 1 || y % 32 <= 1;
            let tone = if joint { 150 } else { 244 };
            rgba.extend_from_slice(&[tone, tone, tone, 255]);
        }
    }
    rgba
}
