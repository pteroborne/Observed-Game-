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

/// The direction toward the moon, unit length, in world axes (`x` east, `y` up, `z`
/// south): low in the west-south-west, about thirty degrees up.
///
/// One direction for everything the moon touches: the disc in the sky, the shading
/// baked into the far-field skin, and a lab's moonlight. If they disagreed, the light
/// on the buildings would come from somewhere the moon is not.
#[must_use]
pub fn toward_moon() -> [f32; 3] {
    let (x, y, z) = (-1.0_f32, 0.62_f32, 0.45_f32);
    let length = (x * x + y * y + z * z).sqrt();
    [x / length, y / length, z / length]
}

/// The moon's apparent diameter, radians. Nearly twenty degrees, forty times the real
/// one: the megastructure stands high enough that the moon fills the western sky.
pub const MOON_ANGULAR_DIAMETER: f32 = 0.34;

/// The moon's surface: a cool, bright grey. An HDR colour, above one so it blooms,
/// and kept below the signal floor so that no gameplay cue ever has to compete with
/// it (`the_moon_never_outshines_a_signal`).
#[must_use]
pub fn moon_disc() -> LinearRgba {
    LinearRgba::rgb(1.35, 1.40, 1.55)
}

/// The glow around the moon, at its brightest just outside the limb.
#[must_use]
pub fn moon_halo() -> LinearRgba {
    LinearRgba::rgb(0.07, 0.085, 0.12)
}

/// Side of the square moon texture, texels.
pub const MOON_TEXTURE_SIZE: u32 = 256;

/// The moon's face as RGBA8, [`MOON_TEXTURE_SIZE`] square: a disc darkened toward
/// its limb, with dark maria and brighter highlands from low-frequency noise, and
/// transparent outside the disc. Deterministic.
#[must_use]
pub fn moon_rgba() -> Vec<u8> {
    const SIZE: u32 = MOON_TEXTURE_SIZE;
    let hash = |x: u32, y: u32| {
        let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77) ^ 0x27D4_EB2F;
        h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
        #[allow(clippy::cast_precision_loss)]
        let value = (h ^ (h >> 12)) as f32 / u32::MAX as f32;
        value
    };
    let noise = |x: f32, y: f32| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (x0, y0) = (x.floor() as u32, y.floor() as u32);
        let (fx, fy) = (x.fract(), y.fract());
        let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let a = hash(x0, y0) + (hash(x0 + 1, y0) - hash(x0, y0)) * sx;
        let b = hash(x0, y0 + 1) + (hash(x0 + 1, y0 + 1) - hash(x0, y0 + 1)) * sx;
        a + (b - a) * sy
    };
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    #[allow(clippy::cast_precision_loss)]
    let half = SIZE as f32 * 0.5;
    for y in 0..SIZE {
        for x in 0..SIZE {
            #[allow(clippy::cast_precision_loss)]
            let (u, v) = (
                (x as f32 + 0.5 - half) / half,
                (y as f32 + 0.5 - half) / half,
            );
            let r = (u * u + v * v).sqrt();
            // The limb: a thin antialiased edge, and darkening toward it.
            let alpha = 1.0 - smoothstep(0.97, 1.0, r);
            let limb = (1.0 - r * r).max(0.0).sqrt().powf(0.35);
            // Five octaves: broad maria, then highland texture, then fine grain.
            let (px, py) = ((u + 1.0) * 2.2, (v + 1.0) * 2.2);
            let mut maria = 0.0;
            let (mut amplitude, mut frequency) = (0.5, 1.0);
            for _ in 0..5 {
                maria += noise(px * frequency + 7.1, py * frequency + 3.7) * amplitude;
                amplitude *= 0.5;
                frequency *= 2.1;
            }
            let tone = limb * (0.66 + 0.34 * smoothstep(0.28, 0.72, maria));
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let (grey, a) = ((tone * 255.0) as u8, (alpha * 255.0) as u8);
            data.extend_from_slice(&[grey, grey, grey, a]);
        }
    }
    data
}

/// A soft radial glow as RGBA8, [`MOON_TEXTURE_SIZE`] square, for a halo drawn larger
/// than the disc behind it.
#[must_use]
pub fn halo_rgba() -> Vec<u8> {
    const SIZE: u32 = MOON_TEXTURE_SIZE;
    #[allow(clippy::cast_precision_loss)]
    let half = SIZE as f32 * 0.5;
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            #[allow(clippy::cast_precision_loss)]
            let (u, v) = (
                (x as f32 + 0.5 - half) / half,
                (y as f32 + 0.5 - half) / half,
            );
            let r = (u * u + v * v).sqrt();
            let glow = (1.0 - smoothstep(0.0, 1.0, r)).powi(3);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let a = (glow * 255.0) as u8;
            data.extend_from_slice(&[255, 255, 255, a]);
        }
    }
    data
}

/// One star: where it is, how big it looks (radians) and how bright (linear).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Star {
    pub direction: [f32; 3],
    pub size: f32,
    pub brightness: f32,
}

/// A deterministic star field: `count` stars above the horizon haze, denser toward
/// the zenith, none behind the moon. The thin air of a high megastructure shows
/// stars low down too, so the field fades out near the horizon rather than above it.
#[must_use]
pub fn stars(count: usize) -> Vec<Star> {
    let moon = toward_moon();
    let mut state: u64 = 0x005E_ED0F_5747;
    let mut next = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        #[allow(clippy::cast_precision_loss)]
        let unit = (z ^ (z >> 31)) as f32 / u64::MAX as f32;
        unit
    };
    let mut field = Vec::with_capacity(count);
    while field.len() < count {
        // Uniform over the upper hemisphere, then thinned toward the horizon.
        let up = next();
        let angle = next() * std::f32::consts::TAU;
        let across = (1.0 - up * up).max(0.0).sqrt();
        let direction = [across * angle.cos(), up, across * angle.sin()];
        let keep = smoothstep(0.03, 0.3, up);
        let behind_moon = direction[0] * moon[0] + direction[1] * moon[1] + direction[2] * moon[2]
            > (MOON_ANGULAR_DIAMETER * 0.9).cos();
        let roll = next();
        let magnitude = next();
        if behind_moon || roll > keep {
            continue;
        }
        // Many faint, few bright.
        let brightness = 0.25 + 1.4 * magnitude.powi(6);
        field.push(Star {
            direction,
            size: 0.0022 + 0.0024 * magnitude.powi(4),
            brightness,
        });
    }
    field
}

/// What hangs in a floor's sky and lights its open edges: the climb's weather.
///
/// One a floor, by its district, so each circle of the ascent is a different hour under a
/// different sky, and climbing crossfades from one to the next. Every colour obeys the
/// rules above: down is darkest, every sky role stays atmosphere, and the light's disc is
/// the brightest thing in the sky without ever reaching a signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyMood {
    /// Straight up.
    pub zenith: Color,
    /// The horizon band, and what open-air fog fades into.
    pub horizon: Color,
    /// Straight down: the darkest thing in the frame.
    pub nadir: Color,
    /// The cloud sea below the decks.
    pub cloud: Color,
    /// The light from the sky's disc, and its strength in lux.
    pub light: Color,
    pub lux: f32,
    /// Unit direction toward the disc and so the light.
    pub toward: [f32; 3],
    /// The disc's face (HDR) and the glow around it.
    pub disc: LinearRgba,
    pub glow: LinearRgba,
    /// How much the disc is the moon's cratered face rather than a sun's plain one: one
    /// for the moon, none for a sun, between only while the sky crossfades.
    pub moon: f32,
    /// How bright the star field shows, from none to the night's full field.
    pub stars: f32,
}

/// The direction `(x, y, z)` made unit length.
fn unit(x: f32, y: f32, z: f32) -> [f32; 3] {
    let length = (x * x + y * y + z * z).sqrt();
    [x / length, y / length, z / length]
}

/// The sky over a floor of `register`.
#[must_use]
pub fn sky_mood(register: observed_content::ArchitectureRegister) -> SkyMood {
    use observed_content::ArchitectureRegister as R;
    match register {
        // The Backrooms: dreamcore. A pastel dusk that should not be there, seen through
        // a window that should not be either - pink haze, a pale lilac zenith, a soft sun.
        R::LiminalGrid => SkyMood {
            zenith: Color::srgb(0.20, 0.19, 0.32),
            horizon: Color::srgb(0.40, 0.29, 0.36),
            nadir: Color::srgb(0.10, 0.08, 0.13),
            cloud: Color::srgb(0.38, 0.31, 0.38),
            light: Color::srgb(1.0, 0.86, 0.90),
            lux: 4_000.0,
            toward: unit(0.6, 0.55, -0.6),
            disc: LinearRgba::rgb(1.45, 1.25, 1.30),
            glow: LinearRgba::rgb(0.12, 0.08, 0.10),
            moon: 0.0,
            stars: 0.0,
        },
        // The Library: the blue hour over endless shelving, a low amber lamp of a sun.
        R::InfiniteGallery => SkyMood {
            zenith: Color::srgb(0.035, 0.045, 0.11),
            horizon: Color::srgb(0.30, 0.20, 0.15),
            nadir: Color::srgb(0.02, 0.02, 0.04),
            cloud: Color::srgb(0.24, 0.19, 0.20),
            light: Color::srgb(1.0, 0.72, 0.45),
            lux: 4_500.0,
            toward: unit(-0.9, 0.18, -0.4),
            disc: LinearRgba::rgb(1.6, 1.05, 0.55),
            glow: LinearRgba::rgb(0.14, 0.08, 0.04),
            moon: 0.0,
            stars: 0.35,
        },
        // Lumen: an overlit white-out, the sky as one flat diffuser and the light from
        // nearly straight overhead.
        R::OverlitGrid => SkyMood {
            zenith: Color::srgb(0.33, 0.35, 0.37),
            horizon: Color::srgb(0.34, 0.35, 0.36),
            nadir: Color::srgb(0.16, 0.17, 0.19),
            cloud: Color::srgb(0.33, 0.34, 0.35),
            light: Color::srgb(0.94, 0.97, 1.0),
            lux: 9_000.0,
            toward: unit(0.15, 1.0, 0.2),
            disc: LinearRgba::rgb(1.4, 1.45, 1.5),
            glow: LinearRgba::rgb(0.10, 0.10, 0.11),
            moon: 0.0,
            stars: 0.0,
        },
        // Zen: sunset. The sun on the horizon behind the screens, and every shadow long.
        R::ShadowScreen => SkyMood {
            zenith: Color::srgb(0.12, 0.07, 0.18),
            horizon: Color::srgb(0.46, 0.22, 0.12),
            nadir: Color::srgb(0.07, 0.03, 0.05),
            cloud: Color::srgb(0.44, 0.22, 0.18),
            light: Color::srgb(1.0, 0.58, 0.30),
            lux: 12_000.0,
            toward: unit(-1.0, 0.12, 0.35),
            disc: LinearRgba::rgb(1.85, 0.95, 0.38),
            glow: LinearRgba::rgb(0.20, 0.08, 0.03),
            moon: 0.0,
            stars: 0.0,
        },
        // The Monument: a clear, alien day high above a ring of cloud.
        R::FacetMonument => SkyMood {
            zenith: Color::srgb(0.06, 0.14, 0.34),
            horizon: Color::srgb(0.24, 0.34, 0.44),
            nadir: Color::srgb(0.05, 0.07, 0.11),
            cloud: Color::srgb(0.30, 0.33, 0.37),
            light: Color::srgb(1.0, 0.97, 0.92),
            lux: 14_000.0,
            toward: unit(-0.45, 0.8, 0.35),
            disc: LinearRgba::rgb(1.75, 1.72, 1.62),
            glow: LinearRgba::rgb(0.12, 0.13, 0.14),
            moon: 0.0,
            stars: 0.0,
        },
        // The Sky: golden hour over the cloud sea, the tops of towers and the walkways
        // between them lit warm from low in the west.
        R::Thinning => SkyMood {
            zenith: Color::srgb(0.12, 0.13, 0.26),
            horizon: Color::srgb(0.46, 0.30, 0.20),
            nadir: Color::srgb(0.14, 0.08, 0.08),
            cloud: Color::srgb(0.46, 0.29, 0.23),
            light: Color::srgb(1.0, 0.74, 0.50),
            lux: 12_000.0,
            toward: unit(-1.0, 0.22, 0.4),
            disc: LinearRgba::rgb(1.85, 1.20, 0.62),
            glow: LinearRgba::rgb(0.18, 0.10, 0.05),
            moon: 0.0,
            stars: 0.0,
        },
        // The Reactor, and anywhere off the climb: the night this facility has always had,
        // the moon filling the western sky above a field of stars.
        R::Megastructure | R::Monolith | R::Institutional | R::Wellshaft => night(),
    }
}

/// The night: [`sky`], [`moon`] and [`toward_moon`], as one mood.
#[must_use]
pub fn night() -> SkyMood {
    SkyMood {
        zenith: sky(SkyRole::Zenith),
        horizon: sky(SkyRole::Horizon),
        nadir: sky(SkyRole::Nadir),
        cloud: sky(SkyRole::Cloud),
        light: moon(),
        lux: 8_000.0,
        toward: toward_moon(),
        disc: moon_disc(),
        glow: moon_halo(),
        moon: 1.0,
        stars: 1.0,
    }
}

impl SkyMood {
    /// The sky colour along a view direction whose vertical component is `up`: as
    /// [`sky_along`], under this mood.
    #[must_use]
    pub fn along(&self, up: f32) -> LinearRgba {
        let up = up.clamp(-1.0, 1.0);
        let blended = if up >= 0.0 {
            self.horizon.mix(&self.zenith, smoothstep(0.0, 0.45, up))
        } else {
            self.horizon.mix(&self.nadir, smoothstep(0.0, 0.7, -up))
        };
        blended.to_linear()
    }

    /// `self` a fraction `t` of the way to `other`: the crossfade as the eye climbs.
    #[must_use]
    pub fn toward_mood(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        let mix = |a: Color, b: Color| a.mix(&b, t);
        let [ax, ay, az] = self.toward;
        let [bx, by, bz] = other.toward;
        Self {
            zenith: mix(self.zenith, other.zenith),
            horizon: mix(self.horizon, other.horizon),
            nadir: mix(self.nadir, other.nadir),
            cloud: mix(self.cloud, other.cloud),
            light: mix(self.light, other.light),
            lux: lerp(self.lux, other.lux),
            toward: unit(lerp(ax, bx), lerp(ay, by), lerp(az, bz)),
            disc: self.disc.mix(&other.disc, t),
            glow: self.glow.mix(&other.glow, t),
            moon: lerp(self.moon, other.moon),
            stars: lerp(self.stars, other.stars),
        }
    }
}

/// A district palette taken outdoors under `mood`: as [`open_air`], with the fog fading
/// into this sky's horizon and the fill from its light.
#[must_use]
pub fn open_air_under(mut palette: DistrictPalette, mood: &SkyMood) -> DistrictPalette {
    palette = open_air(palette);
    palette.fog_color = mood.horizon;
    palette.ambient_color = mood.light;
    palette
}

/// A sun's face as RGBA8, [`MOON_TEXTURE_SIZE`] square: a plain bright disc with a soft
/// limb, transparent outside it. Drawn additively, tinted by the mood's disc colour.
#[must_use]
pub fn sun_rgba() -> Vec<u8> {
    const SIZE: u32 = MOON_TEXTURE_SIZE;
    #[allow(clippy::cast_precision_loss)]
    let half = SIZE as f32 * 0.5;
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            #[allow(clippy::cast_precision_loss)]
            let (u, v) = (
                (x as f32 + 0.5 - half) / half,
                (y as f32 + 0.5 - half) / half,
            );
            let r = (u * u + v * v).sqrt();
            let face = 1.0 - smoothstep(0.42, 0.5, r);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let a = (face * 255.0) as u8;
            data.extend_from_slice(&[a, a, a, a]);
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

    /// Every floor's sky keeps the open-air rules: atmosphere never outshines a signal,
    /// down is darkest, and the disc is the brightest thing in its sky without being one.
    #[test]
    fn every_floors_sky_keeps_the_open_air_rules() {
        for register in observed_content::ArchitectureRegister::ALL {
            let mood = sky_mood(register);
            let roles = [mood.zenith, mood.horizon, mood.nadir, mood.cloud];
            for color in roles {
                let lit = luminance(color.to_linear());
                assert!(
                    lit <= ATMOSPHERE_MAX_LUMINANCE,
                    "{register:?}: {color:?} at {lit}"
                );
            }
            let nadir = luminance(mood.along(-1.0));
            for step in -9..=10 {
                #[allow(clippy::cast_precision_loss)]
                let up = step as f32 / 10.0;
                assert!(
                    luminance(mood.along(up)) >= nadir - 1e-6,
                    "{register:?}: looking along {up} is darker than straight down"
                );
            }
            let disc = luminance(mood.disc);
            assert!(disc > 1.0, "{register:?}: a disc too dim to bloom");
            assert!(
                disc < SIGNAL_MIN_LUMINANCE,
                "{register:?}: a disc as bright as a signal"
            );
            assert!(luminance(mood.glow) < SIGNAL_MIN_LUMINANCE);
            for color in roles {
                assert!(disc > luminance(color.to_linear()) * 10.0, "{register:?}");
            }
            let [x, y, z] = mood.toward;
            assert!(((x * x + y * y + z * z).sqrt() - 1.0).abs() < 1e-5);
            assert!(
                y > 0.1,
                "{register:?}: the light comes from below the horizon haze"
            );
            assert!((0.0..=1.0).contains(&mood.moon) && (0.0..=1.0).contains(&mood.stars));
        }
    }

    /// The Reactor keeps the facility's night; every other circle of the climb is a
    /// different hour, and Zen's sun is low enough for long shadows.
    #[test]
    fn the_climb_changes_the_hour_and_the_reactor_keeps_the_night() {
        use observed_content::ArchitectureRegister as R;
        assert_eq!(sky_mood(R::Megastructure), night());
        let mut climb: Vec<SkyMood> = Vec::new();
        for register in R::CLIMB {
            let mood = sky_mood(register);
            if !climb.contains(&mood) {
                climb.push(mood);
            }
        }
        assert_eq!(
            climb.len(),
            7,
            "one sky for each of the climb's seven districts"
        );
        assert!(
            sky_mood(R::ShadowScreen).toward[1] < 0.2,
            "Zen's sun is on the horizon"
        );
        assert!(
            luminance(sky_mood(R::LiminalGrid).along(0.0)) > luminance(night().along(0.0)),
            "the Backrooms' dreamcore haze is lighter than the night"
        );
    }

    /// A crossfade starts on the sky it leaves and ends on the one it reaches.
    #[test]
    fn a_crossfade_runs_from_one_sky_to_the_next() {
        use observed_content::ArchitectureRegister as R;
        let (from, to) = (sky_mood(R::ShadowScreen), sky_mood(R::Megastructure));
        let start = from.toward_mood(&to, 0.0);
        let end = from.toward_mood(&to, 1.0);
        assert_eq!(
            (start.horizon, start.lux, start.moon),
            (from.horizon, from.lux, 0.0)
        );
        assert_eq!((end.horizon, end.lux, end.moon), (to.horizon, to.lux, 1.0));
        let half = from.toward_mood(&to, 0.5);
        assert!(half.lux < from.lux && half.lux > to.lux);
    }

    #[test]
    fn a_sun_is_a_plain_disc() {
        let face = sun_rgba();
        let size = MOON_TEXTURE_SIZE as usize;
        let alpha = |x: usize, y: usize| face[(y * size + x) * 4 + 3];
        assert_eq!(alpha(0, 0), 0, "transparent outside the disc");
        assert_eq!(alpha(size / 2, size / 2), 255, "opaque at its centre");
    }

    #[test]
    fn the_moon_never_outshines_a_signal() {
        let brightest = luminance(moon_disc()).max(luminance(moon_halo()));
        assert!(brightest < SIGNAL_MIN_LUMINANCE, "{brightest}");
        // But it is the brightest thing in the sky, and bright enough to bloom.
        assert!(luminance(moon_disc()) > 1.0);
        for role in SkyRole::ALL {
            assert!(luminance(moon_disc()) > luminance(sky(role).to_linear()) * 10.0);
        }
    }

    #[test]
    fn the_moon_is_where_the_light_comes_from_and_up_in_the_sky() {
        let [x, y, z] = toward_moon();
        assert!(((x * x + y * y + z * z).sqrt() - 1.0).abs() < 1e-5);
        assert!(y > 0.3, "the moon is above the horizon haze");
        let face = moon_rgba();
        let size = MOON_TEXTURE_SIZE as usize;
        let alpha = |x: usize, y: usize| face[(y * size + x) * 4 + 3];
        assert_eq!(alpha(0, 0), 0, "transparent outside the disc");
        assert_eq!(alpha(size / 2, size / 2), 255, "opaque at its centre");
    }

    #[test]
    fn stars_fill_the_sky_but_never_the_moon_or_the_drop() {
        let field = stars(1_500);
        assert_eq!(field.len(), 1_500);
        assert_eq!(field, stars(1_500), "deterministic");
        let moon = toward_moon();
        for star in &field {
            let [x, y, z] = star.direction;
            assert!(y > 0.0, "a star below the horizon, in the drop");
            let toward = x * moon[0] + y * moon[1] + z * moon[2];
            assert!(
                toward < (MOON_ANGULAR_DIAMETER * 0.5).cos(),
                "a star on the moon"
            );
            assert!(star.brightness < SIGNAL_MIN_LUMINANCE);
        }
        let high = field.iter().filter(|star| star.direction[1] > 0.5).count();
        assert!(high * 2 > field.len(), "denser toward the zenith: {high}");
    }

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
