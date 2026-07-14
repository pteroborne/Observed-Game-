//! Pure semantic selection for the aesthetic projection.

use bevy::prelude::*;
use observed_style::{
    District, DistrictPalette, MarkerRole, SurfaceRole, Treatment, district, marker, surface,
};
use rapier_portal_lab::authoring::{ArchitectureKind, ConvexHullData};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceVisual {
    pub role: SurfaceRole,
    pub district: District,
}

pub fn visual_for_hull(kind: ArchitectureKind, hull: &ConvexHullData) -> SurfaceVisual {
    let size = hull.max - hull.min;
    match kind {
        ArchitectureKind::Gantry => SurfaceVisual {
            role: if hull.max.y <= 0.05 {
                SurfaceRole::SafeBypass
            } else if hull.points.len() == 6 || size.y < 3.0 {
                SurfaceRole::GantryDeck
            } else {
                SurfaceRole::Wall
            },
            district: District::Archive,
        },
        ArchitectureKind::Colonnade => SurfaceVisual {
            role: if hull.max.y <= 0.05 {
                SurfaceRole::Plain
            } else if size.x < 1.5 && size.z < 1.5 {
                SurfaceRole::WellshaftStone
            } else {
                SurfaceRole::Wall
            },
            district: District::Reactor,
        },
    }
}

pub fn palette_for(kind: ArchitectureKind) -> DistrictPalette {
    district(match kind {
        ArchitectureKind::Gantry => District::Archive,
        ArchitectureKind::Colonnade => District::Reactor,
    })
}

fn srgb(color: Color) -> [f32; 3] {
    let color = color.to_srgba();
    [color.red, color.green, color.blue]
}

/// Same presentation projection used by the assembled game: district atmosphere may
/// tint structural treatments, while signal treatments remain byte-for-byte semantic.
pub fn district_treatment(treatment: Treatment, palette: &DistrictPalette) -> Treatment {
    if treatment.signal {
        return treatment;
    }
    let role = srgb(treatment.base_color);
    let light = srgb(palette.light_color);
    let ambient = srgb(palette.ambient_color);
    let brightness = (palette.ambient_brightness / 75.0).clamp(0.45, 1.1);
    let mut rgb = [0.0; 3];
    for index in 0..3 {
        let palette_channel = light[index] * 0.68 + ambient[index] * 0.32;
        // Preserve the original style lab's dark structural mass. District colour is
        // a cast, not a replacement albedo; most identity comes from actual lights.
        rgb[index] = (role[index] * 0.72 + palette_channel * 0.28) * 0.45 * brightness;
    }
    Treatment {
        base_color: Color::srgb(rgb[0], rgb[1], rgb[2]),
        emissive: LinearRgba::rgb(
            treatment.emissive.red * 0.25 + palette.accent.red * 0.06,
            treatment.emissive.green * 0.25 + palette.accent.green * 0.06,
            treatment.emissive.blue * 0.25 + palette.accent.blue * 0.06,
        ),
        ..treatment
    }
}

pub fn surface_treatment(visual: SurfaceVisual) -> Treatment {
    district_treatment(surface(visual.role), &district(visual.district))
}

/// Player gaze is intentionally absent: only durable anchor state owns the indicator.
pub fn threshold_indicator(anchored: bool) -> Treatment {
    if anchored {
        marker(MarkerRole::Control)
    } else {
        district_treatment(surface(SurfaceRole::Wall), &district(District::Archive))
    }
}

pub fn standard_material(treatment: Treatment) -> StandardMaterial {
    StandardMaterial {
        base_color: treatment.base_color,
        emissive: treatment.emissive,
        unlit: treatment.signal,
        perceptual_roughness: 0.86,
        metallic: 0.08,
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_style::{SIGNAL_MIN_LUMINANCE, luminance};
    use rapier_portal_lab::authoring::authored_modules;

    #[test]
    fn authored_hulls_map_only_to_shared_semantic_roles() {
        for module in authored_modules() {
            for hull in &module.hulls {
                let visual = visual_for_hull(module.kind, hull);
                assert!(SurfaceRole::ALL.contains(&visual.role));
                assert!(District::ALL.contains(&visual.district));
            }
        }
    }

    #[test]
    fn district_tint_never_modifies_signal_treatment() {
        let signal = marker(MarkerRole::Control);
        for district_role in District::ALL {
            assert_eq!(district_treatment(signal, &district(district_role)), signal);
        }
    }

    #[test]
    fn anchor_indicator_clears_legibility_floor_and_idle_does_not_claim_anchor() {
        let anchored = threshold_indicator(true);
        let idle = threshold_indicator(false);
        assert!(anchored.signal);
        assert!(luminance(anchored.emissive) >= SIGNAL_MIN_LUMINANCE);
        assert!(!idle.signal);
        assert!(luminance(idle.emissive) < SIGNAL_MIN_LUMINANCE);
    }
}
