//! Major Guardians: the facility's wardens, which freeze when seen.
//!
//! A major Guardian is built like the hand equipment (`crate::equipment`): dark
//! hardware, with its signal in a few parts. The signal is the threat colour
//! ([`MarkerRole::Collapse`]) in its eye and in the core that shows through its seams.
//! Being seen, it goes still and its seams go dark; anchored, it wears the anchor's
//! purple. Colour only supports those states. Each form also says them in shape and
//! motion, which is the design's rule for every critical state.
//!
//! Minor Guardians keep the ivory of `crate::kinetic`: pale, small and many, where a
//! major is dark, large and one. The two classes differ in silhouette first; the
//! palette makes the difference obvious at a glance as well.
//!
//! Rules, each tested:
//!
//! * **The shell is dark and never emits,** and is darker than the minor's ivory.
//! * **The eye is signal-tier:** a major Guardian must punch through fog and bloom.
//! * **The search beam is haze,** well under the signal floor: you see it cross a
//!   doorway before you see the Guardian, and it never outshines the eye.
//! * **A frozen seam is dark:** at most a tenth of the live one.
use bevy::color::{Color, LinearRgba};

use crate::{MarkerRole, marker};

/// A part of a major Guardian.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Part {
    /// The body: oxidised bronze, dark enough to be a silhouette against the moonlit
    /// facility.
    Shell,
    /// Machined brass edges, bars and rings, which catch the moon.
    Trim,
    /// The eye's white.
    Eye,
    /// The pupil.
    Pupil,
}

/// A physically based finish.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Finish {
    pub base_color: Color,
    pub emissive: LinearRgba,
    pub metallic: f32,
    pub roughness: f32,
}

/// The finish of a part.
#[must_use]
pub fn finish(part: Part) -> Finish {
    match part {
        Part::Shell => Finish {
            base_color: Color::srgb(0.16, 0.14, 0.11),
            emissive: LinearRgba::BLACK,
            metallic: 0.8,
            roughness: 0.46,
        },
        Part::Trim => Finish {
            base_color: Color::srgb(0.58, 0.46, 0.26),
            emissive: LinearRgba::BLACK,
            metallic: 1.0,
            roughness: 0.28,
        },
        // The iris and the ring round it carry the threat; the white is only a white.
        Part::Eye => Finish {
            base_color: marker(MarkerRole::Collapse).base_color,
            emissive: marker(MarkerRole::Collapse).emissive,
            metallic: 0.0,
            roughness: 0.2,
        },
        Part::Pupil => Finish {
            base_color: Color::srgb(0.01, 0.01, 0.012),
            emissive: LinearRgba::BLACK,
            metallic: 0.0,
            roughness: 0.1,
        },
    }
}

/// How much of the threat colour shows through a live Guardian's seams.
pub const SEAM_SCALE: f32 = 0.55;
/// How much still shows once it is frozen: the hum has stopped.
pub const FROZEN_SEAM_SCALE: f32 = 0.04;
/// The search beam, from the eye's own colour.
pub const BEAM_SCALE: f32 = 0.035;
/// The flare of a catch.
pub const CATCH_SCALE: f32 = 1.6;

/// The core glowing through the seams, live or frozen.
#[must_use]
pub fn seam(frozen: bool) -> LinearRgba {
    marker(MarkerRole::Collapse).emissive
        * if frozen {
            FROZEN_SEAM_SCALE
        } else {
            SEAM_SCALE
        }
}

/// The haze of the search beam.
#[must_use]
pub fn beam() -> LinearRgba {
    marker(MarkerRole::Collapse).emissive * BEAM_SCALE
}

/// The flare of a catch: the seams, the eye and the stamp on the floor.
#[must_use]
pub fn catch_flare() -> LinearRgba {
    marker(MarkerRole::Collapse).emissive * CATCH_SCALE
}

/// The clamp an anchor lantern puts on a Guardian: the anchor's own purple.
#[must_use]
pub fn anchor_clamp() -> LinearRgba {
    marker(MarkerRole::Control).emissive
}

#[cfg(test)]
mod tests {
    use bevy::color::LinearRgba;

    use super::{Part, beam, finish, seam};
    use crate::kinetic::{Role, treatment};
    use crate::{MarkerRole, SIGNAL_MIN_LUMINANCE, luminance, marker};

    #[test]
    fn the_shell_is_dark_and_darker_than_a_minor() {
        let shell = finish(Part::Shell);
        assert_eq!(shell.emissive, LinearRgba::BLACK);
        let minor = luminance(LinearRgba::from(treatment(Role::GuardianShell).base_color));
        let major = luminance(LinearRgba::from(shell.base_color));
        assert!(major < minor * 0.25, "major {major} vs minor {minor}");
    }

    #[test]
    fn the_eye_is_signal_tier() {
        assert!(luminance(finish(Part::Eye).emissive) >= SIGNAL_MIN_LUMINANCE);
    }

    #[test]
    fn the_beam_is_haze_under_the_eye() {
        let beam = luminance(beam());
        assert!(beam < SIGNAL_MIN_LUMINANCE * 0.1, "beam {beam}");
        assert!(beam > 0.02, "beam invisible at {beam}");
        assert!(beam < luminance(finish(Part::Eye).emissive) * 0.1);
    }

    #[test]
    fn a_frozen_seam_is_dark() {
        let (live, frozen) = (luminance(seam(false)), luminance(seam(true)));
        assert!(frozen <= live * 0.1, "{frozen} vs {live}");
        assert!(live > luminance(marker(MarkerRole::Collapse).emissive) * 0.3);
    }
}
