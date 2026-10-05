//! Shared Reactor / Megastructure metal, first established by the Chargeworks.
//! This is structural material, not a powered signal or a conveyor effect.
use crate::{ArchitectureSurfaceRole, HexSurfaceLook, surfaces::SurfaceImages};
use bevy::color::{Color, LinearRgba};

pub const ROUGHNESS: f32 = 0.42;
pub const METALLIC: f32 = 0.65;

#[must_use]
pub fn surface(role: ArchitectureSurfaceRole) -> HexSurfaceLook {
    HexSurfaceLook {
        base_color: match role {
            ArchitectureSurfaceRole::Floor => Color::srgb(0.48, 0.49, 0.48),
            ArchitectureSurfaceRole::Ceiling => Color::srgb(0.34, 0.39, 0.45),
            _ => Color::srgb(0.54, 0.59, 0.65),
        },
        emissive: LinearRgba::BLACK,
        unlit: false,
        textured: true,
    }
}

/// Original chamfered plates, inset ribs and stepped inlays at a four-metre repeat.
/// The same height samples supply the normal map: a groove catches real fixed light.
#[must_use]
pub fn panel_images(role: ArchitectureSurfaceRole) -> SurfaceImages {
    let n = crate::surfaces::SURFACE_TEXTURE_SIZE as usize;
    let mut heights = Vec::with_capacity(n * n);
    let mut albedo = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x % 256, y % 256);
            let edge = u.min(255 - u).min(v.min(255 - v));
            let corner = (u + v).min(u + 255 - v).min(255 - u + v).min(510 - u - v);
            let bevel = edge.min(corner.saturating_sub(30) / 2);
            let trim = !(36..=219).contains(&u);
            let rib = trim && (u % 12 < 4);
            let stepped = (v / 32 % 2) * 8;
            let circuit = (u.abs_diff(52 + stepped) < 2 || u.abs_diff(203 - stepped) < 2)
                && (24..232).contains(&v);
            let seam = bevel < 3 || circuit;
            let (tone, height) = if seam {
                (54u8, 0.12)
            } else if bevel < 8 {
                (128 + (bevel as u8) * 9, 0.35 + bevel as f32 * 0.07)
            } else if rib {
                (85, 0.38)
            } else if trim {
                (145, 0.62)
            } else {
                // Fine brushed striations remain subdued beneath the large plates.
                (211 + (x % 3) as u8 * 3, 0.88)
            };
            let blue = if trim { tone.saturating_add(16) } else { tone };
            albedo.extend_from_slice(&[tone, tone, blue, 255]);
            heights.push(height);
        }
    }
    let strength = if role == ArchitectureSurfaceRole::Floor {
        1.3
    } else {
        2.2
    };
    let h = |x: usize, y: usize| heights[(y % n) * n + x % n];
    let mut normal = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let nx = (h(x + n - 1, y) - h(x + 1, y)) * strength;
            let ny = (h(x, y + n - 1) - h(x, y + 1)) * strength;
            let len = (nx * nx + ny * ny + 1.0).sqrt();
            let encode = |v: f32| ((v / len * 0.5 + 0.5) * 255.0).round() as u8;
            normal.extend_from_slice(&[encode(nx), encode(ny), encode(1.0), 255]);
        }
    }
    SurfaceImages { albedo, normal }
}
