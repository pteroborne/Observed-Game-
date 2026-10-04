//! The Chargeworks' original industrial finish and fixed work-light palette.
use crate::{ArchitectureSurfaceRole, HexSurfaceLook};
use bevy::color::{Color, LinearRgba};

pub const FIXTURE_INTENSITY: f32 = 7_500_000.0;
pub const FIXTURE_BOUNCE_INTENSITY: f32 = 170_000.0;
pub const ROUGHNESS: f32 = 0.58;

#[must_use]
pub fn fixture_color() -> Color {
    Color::srgb(0.79, 0.92, 1.0)
}
#[must_use]
pub fn charge_color() -> Color {
    Color::srgba(0.08, 0.78, 0.89, 0.42)
}
#[must_use]
pub fn warning_color() -> Color {
    Color::srgb(0.96, 0.59, 0.16)
}
#[must_use]
pub fn belt_color() -> Color {
    Color::srgb(0.12, 0.17, 0.19)
}

#[must_use]
pub fn surface(role: ArchitectureSurfaceRole) -> HexSurfaceLook {
    HexSurfaceLook {
        base_color: match role {
            ArchitectureSurfaceRole::Floor => Color::srgb(0.46, 0.51, 0.52),
            ArchitectureSurfaceRole::Ceiling => Color::srgb(0.30, 0.37, 0.40),
            _ => Color::srgb(0.38, 0.45, 0.49),
        },
        emissive: LinearRgba::BLACK,
        unlit: false,
        textured: true,
    }
}

/// Large folded panels, fine seams and fastening marks at a four-metre repeat.
#[must_use]
pub fn panel_albedo() -> Vec<u8> {
    let n = crate::surfaces::SURFACE_TEXTURE_SIZE as usize;
    let mut rgba = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let seam = x % 128 < 3 || y % 128 < 3;
            let bolt = (x % 128 < 11 && x % 128 > 7) && (y % 128 < 11 && y % 128 > 7);
            let tone = if seam {
                135
            } else if bolt {
                175
            } else {
                242
            };
            rgba.extend_from_slice(&[tone, tone, tone, 255]);
        }
    }
    rgba
}
