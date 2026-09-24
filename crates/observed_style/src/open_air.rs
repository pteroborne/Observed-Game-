//! Open air seen from inside it: the first-person counterpart of the cutaway's sky.
//!
//! The Architect's cutaway already says what air is — a cool well, deepest straight
//! down, hazing outward ([`crate::architect`]). An Observer standing at a deck edge
//! has to read the same space from the other side, so the two share their colours
//! rather than approximating each other: the horizon here *is* the cutaway's haze,
//! and the view straight down *is* its deep.
//!
//! Three rules, each tested:
//!
//! * **Down is darkest.** A drop must look like a drop. The sky below the horizon
//!   darkens toward the nadir, so the eye reads depth before it reads anything else.
//! * **Distance dissolves into the horizon, not into black.** Interior district fog
//!   is tuned for a corridor (a few tens of metres) and fades to near-black. Out here
//!   the fog colour is the horizon, so a far tower sinks into sky and the silhouette
//!   survives — which is the whole point of seeing across air.
//! * **Atmosphere never outshines a signal.** Every sky role stays under
//!   [`crate::ATMOSPHERE_MAX_LUMINANCE`]; the fall edge and the summit beacon stay
//!   signal-tier. The Legibility Contract binds in open air exactly as it does inside.
use bevy::color::{Color, LinearRgba, Mix};

use crate::architect::{Role as CutawayRole, color as cutaway};
use crate::{DISTRICT_MIN_AMBIENT_BRIGHTNESS, DistrictPalette};

/// Where open-air fog begins: a deck and its neighbour stay crisp.
pub const OPEN_AIR_FOG_START: f32 = 36.0;
/// Where open-air fog has fully become the horizon: roughly twenty cells.
pub const OPEN_AIR_FOG_END: f32 = 300.0;

/// A named part of the open-air atmosphere. None of these is a gameplay signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SkyRole {
    /// Straight up: night, faintly violet so it does not read as the drop.
    Zenith,
    /// The horizon band, and the colour distance fades into.
    Horizon,
    /// Straight down: the darkest thing in the frame.
    Nadir,
    /// The cloud sea far below every deck.
    Cloud,
    /// The sawn underside of a floating deck: stone that never sees the key light.
    Underside,
    /// The sheer exterior face of a storey stack.
    SheerFace,
    /// A storey face seen from far off, where the moon is all that lights it: drawn
    /// unlit, shaded by the moon's angle, and brighter than `SheerFace` for it.
    MoonlitFace,
    /// A roof seen from far off, likewise.
    MoonlitRoof,
}

impl SkyRole {
    pub const ALL: [Self; 8] = [
        Self::Zenith,
        Self::Horizon,
        Self::Nadir,
        Self::Cloud,
        Self::Underside,
        Self::SheerFace,
        Self::MoonlitFace,
        Self::MoonlitRoof,
    ];
}

#[must_use]
pub fn sky(role: SkyRole) -> Color {
    match role {
        SkyRole::Zenith => Color::srgb(0.030, 0.030, 0.068),
        SkyRole::Horizon => cutaway(CutawayRole::SkyHaze),
        SkyRole::Nadir => cutaway(CutawayRole::SkyDeep),
        SkyRole::Cloud => Color::srgb(0.26, 0.33, 0.40),
        SkyRole::Underside => Color::srgb(0.085, 0.088, 0.092),
        SkyRole::SheerFace => Color::srgb(0.16, 0.165, 0.17),
        SkyRole::MoonlitFace => Color::srgb(0.28, 0.29, 0.31),
        SkyRole::MoonlitRoof => Color::srgb(0.2, 0.21, 0.23),
    }
}

/// The sky colour seen along a view direction whose vertical component is `up`
/// (`-1` straight down, `0` level, `1` straight up).
///
/// The horizon band is deliberately narrow above and wide below: the eye spends most
/// of its time looking level or down off an edge, and that is where depth has to read.
#[must_use]
pub fn sky_along(up: f32) -> LinearRgba {
    let up = up.clamp(-1.0, 1.0);
    let horizon = sky(SkyRole::Horizon);
    let blended = if up >= 0.0 {
        horizon.mix(&sky(SkyRole::Zenith), smoothstep(0.0, 0.45, up))
    } else {
        horizon.mix(&sky(SkyRole::Nadir), smoothstep(0.0, 0.7, -up))
    };
    blended.to_linear()
}

/// A district palette taken outdoors: the same key and fill, but fog that reaches
/// across air and fades into the horizon instead of into the dark.
#[must_use]
pub fn open_air(mut palette: DistrictPalette) -> DistrictPalette {
    palette.fog_color = sky(SkyRole::Horizon);
    palette.fog_start = OPEN_AIR_FOG_START;
    palette.fog_end = OPEN_AIR_FOG_END;
    // Out here the fill comes from the sky, not from the room: low, and the moon's cool
    // neutral. Not the horizon's hue - where nothing else lights a surface, a tinted
    // fill tints everything, and the first captures in the facility came out teal.
    palette.ambient_color = moon();
    palette.ambient_brightness = DISTRICT_MIN_AMBIENT_BRIGHTNESS;
    palette
}

/// The moon: the one directional key over open air, cool and low so sheer faces
/// rake into light and shadow rather than all reading as the same flat grey.
#[must_use]
pub fn moon() -> Color {
    Color::srgb(0.72, 0.80, 1.0)
}

/// The exposure survey overlay: a debug view of what the renderer was told, so an
/// invisible rule is never the only explanation for how a vista looks.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SurveyRole {
    /// A face that borders open air.
    Sheer,
    /// The drop under a hanging cell, down to what would catch a fall.
    Drop,
    /// A walkway's axis.
    Span,
}

impl SurveyRole {
    pub const ALL: [Self; 3] = [Self::Sheer, Self::Drop, Self::Span];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sheer => "face open to air (sheer)",
            Self::Drop => "drop to what would catch a fall",
            Self::Span => "walkway axis (air on both flanks)",
        }
    }
}

#[must_use]
pub fn survey(role: SurveyRole) -> Color {
    match role {
        SurveyRole::Sheer => Color::srgb(0.45, 0.86, 1.0),
        SurveyRole::Drop => Color::srgb(0.78, 0.52, 1.0),
        SurveyRole::Span => Color::srgb(0.96, 0.95, 0.88),
    }
}

/// Side of the square cloud texture, texels.
pub const CLOUD_TEXTURE_SIZE: u32 = 128;

/// Tiling fractal value noise as soft white wisps in the alpha channel, RGBA8,
/// [`CLOUD_TEXTURE_SIZE`] square. Deterministic: a fixed lattice hash, no random source.
///
/// Shared by the cutaway's cloud layer and the first-person cloud sea, so the two
/// views of the same air are the same weather.
#[must_use]
pub fn cloud_rgba() -> Vec<u8> {
    const SIZE: u32 = CLOUD_TEXTURE_SIZE;
    let hash = |x: u32, y: u32, period: u32| {
        let (x, y) = (x % period, y % period);
        let mut h = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263) ^ 0x5bd1_e995;
        h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
        #[allow(clippy::cast_precision_loss)]
        let value = (h ^ (h >> 16)) as f32 / u32::MAX as f32;
        value
    };
    let noise = |x: f32, y: f32, period: u32| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (x0, y0) = (x.floor() as u32, y.floor() as u32);
        let (fx, fy) = (x.fract(), y.fract());
        let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let a = hash(x0, y0, period) + (hash(x0 + 1, y0, period) - hash(x0, y0, period)) * sx;
        let b = hash(x0, y0 + 1, period)
            + (hash(x0 + 1, y0 + 1, period) - hash(x0, y0 + 1, period)) * sx;
        a + (b - a) * sy
    };
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let mut value = 0.0;
            let mut amplitude = 0.5;
            for octave in 0..4u32 {
                let period = 4 << octave;
                #[allow(clippy::cast_precision_loss)]
                let scale = period as f32 / SIZE as f32;
                #[allow(clippy::cast_precision_loss)]
                let sample = noise(x as f32 * scale, y as f32 * scale, period);
                value += sample * amplitude;
                amplitude *= 0.5;
            }
            let alpha = smoothstep(0.42, 0.82, value);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            data.extend_from_slice(&[255, 255, 255, (alpha * 255.0) as u8]);
        }
    }
    data
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ATMOSPHERE_MAX_LUMINANCE, MarkerRole, SIGNAL_MIN_LUMINANCE, SurfaceRole, luminance, marker,
        surface,
    };

    #[test]
    fn clouds_tile_and_are_neither_empty_nor_overcast() {
        let rgba = cloud_rgba();
        let size = CLOUD_TEXTURE_SIZE as usize;
        assert_eq!(rgba.len(), size * size * 4);
        let alpha: Vec<u8> = rgba.chunks(4).map(|texel| texel[3]).collect();
        let clear = alpha.iter().filter(|&&a| a == 0).count();
        let thick = alpha.iter().filter(|&&a| a > 128).count();
        assert!(clear > size * size / 5, "{clear}");
        assert!(thick > size * size / 50, "{thick}");
        // Tiling: the texture's last column continues into its first without a seam.
        let seam = (0..size)
            .map(|row| i32::from(alpha[row * size]) - i32::from(alpha[row * size + size - 1]))
            .map(i32::abs)
            .max()
            .unwrap_or(0);
        assert!(seam < 48, "{seam}");
    }

    #[test]
    fn open_air_shares_the_cutaways_sky() {
        assert_eq!(sky(SkyRole::Horizon), cutaway(CutawayRole::SkyHaze));
        assert_eq!(sky(SkyRole::Nadir), cutaway(CutawayRole::SkyDeep));
    }

    #[test]
    fn straight_down_is_the_darkest_direction() {
        let nadir = luminance(sky_along(-1.0));
        for step in -9..=10 {
            #[allow(clippy::cast_precision_loss)]
            let up = step as f32 / 10.0;
            assert!(
                luminance(sky_along(up)) >= nadir,
                "looking along {up} is darker than straight down"
            );
        }
        assert!(nadir < luminance(sky_along(0.0)) * 0.5);
    }

    #[test]
    fn the_sky_darkens_monotonically_into_the_drop() {
        let mut previous = luminance(sky_along(0.0));
        for step in 1..=20 {
            #[allow(clippy::cast_precision_loss)]
            let here = luminance(sky_along(-(step as f32) / 20.0));
            assert!(here <= previous + 1e-6, "{here} after {previous}");
            previous = here;
        }
    }

    #[test]
    fn atmosphere_stays_under_the_neon_noir_ceiling() {
        for role in SkyRole::ALL {
            let lum = luminance(sky(role).to_linear());
            assert!(lum <= ATMOSPHERE_MAX_LUMINANCE, "{role:?}: {lum}");
        }
    }

    #[test]
    fn the_fall_edge_and_the_summit_outshine_the_sky() {
        let brightest_sky = SkyRole::ALL
            .into_iter()
            .map(|role| luminance(sky(role).to_linear()))
            .fold(0.0, f32::max);
        for signal in [surface(SurfaceRole::GantryEdge), marker(MarkerRole::Exit)] {
            assert!(signal.signal);
            let lum = luminance(signal.emissive);
            assert!(lum >= SIGNAL_MIN_LUMINANCE, "{lum}");
            assert!(lum > brightest_sky * 20.0, "{lum} against {brightest_sky}");
        }
    }

    #[test]
    fn distance_fades_into_the_horizon_and_reaches_across_cells() {
        let palette = open_air(crate::architecture(
            observed_content::ArchitectureRegister::Monolith,
        ));
        assert_eq!(palette.fog_color, sky(SkyRole::Horizon));
        // A neighbouring deck, one 14 m cell away, must not be in fog at all.
        assert!(palette.fog_start > 2.0 * 14.0);
        assert!(palette.fog_end > 10.0 * 14.0);
        assert!(palette.ambient_brightness >= DISTRICT_MIN_AMBIENT_BRIGHTNESS);
    }
}
