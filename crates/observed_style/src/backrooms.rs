//! Ordinary Backrooms finishes and fluorescent construction. Colour remains
//! style-owned; authored neutral PBR maps supply only material character.
use crate::{ArchitectureSurfaceRole, HexSurfaceLook};
use bevy::color::{Color, LinearRgba};

#[must_use]
pub fn surface(role: ArchitectureSurfaceRole) -> HexSurfaceLook {
    let (rgb, glow) = match role {
        ArchitectureSurfaceRole::Floor => ([0.53, 0.46, 0.27], 0.004),
        ArchitectureSurfaceRole::Wall => ([0.78, 0.71, 0.43], 0.012),
        ArchitectureSurfaceRole::Ceiling => ([0.82, 0.80, 0.67], 0.035),
        ArchitectureSurfaceRole::PracticalFixture => ([0.98, 0.96, 0.87], 1.8),
    };
    HexSurfaceLook {
        base_color: Color::srgb(rgb[0], rgb[1], rgb[2]),
        emissive: LinearRgba::rgb(rgb[0] * glow, rgb[1] * glow, rgb[2] * glow),
        unlit: false,
        textured: role != ArchitectureSurfaceRole::PracticalFixture,
    }
}

#[must_use]
pub const fn roughness(role: ArchitectureSurfaceRole) -> f32 {
    match role {
        ArchitectureSurfaceRole::Floor => 1.0,
        ArchitectureSurfaceRole::Wall => 1.0,
        ArchitectureSurfaceRole::Ceiling => 1.0,
        ArchitectureSurfaceRole::PracticalFixture => 0.42,
    }
}

#[must_use]
pub const fn texture_metres(role: ArchitectureSurfaceRole) -> f32 {
    if matches!(role, ArchitectureSurfaceRole::Ceiling) {
        6.0
    } else {
        4.0
    }
}

#[must_use]
pub fn trim() -> HexSurfaceLook {
    HexSurfaceLook {
        base_color: Color::srgb(0.53, 0.49, 0.35),
        emissive: LinearRgba::BLACK,
        unlit: false,
        textured: false,
    }
}

pub const TRIM_ROUGHNESS: f32 = 0.68;

/// Physical soundscape gains relative to the existing ambience preference.
pub const HVAC_GAIN: f32 = 0.22;
pub const BUZZ_GAIN: f32 = 0.18;
pub const MACHINERY_GAIN: f32 = 0.12;
pub const CREAK_GAIN: f32 = 0.16;
