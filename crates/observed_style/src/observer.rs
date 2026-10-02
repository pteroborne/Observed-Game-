//! Observers: the facility's explorers, drawn as floating eyes.
//!
//! An Observer is built like the major Guardian (`crate::guardian`): dark hardware,
//! with its signal in one part. The signal is whose eye it is - yours, a teammate's,
//! a rival's ([`MarkerRole::You`], [`MarkerRole::Teammate`], [`MarkerRole::Rival`]) -
//! and it is carried by the iris, the one thing about an eye you read from across a
//! room. Everything else is the silhouette's: a sphere where a Guardian is a pyramid,
//! dark glossy enamel where a minor is pale.
//!
//! Rules, each tested:
//!
//! * **The globe is dark and never emits,** darker than a minor Guardian's ivory: an
//!   Observer is a silhouette against the moonlit facility, its iris the light.
//! * **The iris is signal-tier,** for every role an Observer can wear.
//! * **An Observer's eye is never a Guardian's:** no iris shares the threat colour.
//! * **The trim is not the Guardian's brass:** steel, cooler than it is warm.
//! * **The hover haze is haze,** well under the signal floor, and never outshines the
//!   iris it belongs to.
use bevy::color::{Color, LinearRgba};

use crate::guardian::Finish;
use crate::{MarkerRole, marker};

/// A part of an Observer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Part {
    /// The globe and its lids: deep blue-black enamel, glossy enough to carry the moon.
    Globe,
    /// The gimbal that holds the globe: brushed steel.
    Trim,
    /// The iris, in whose colour the Observer is, and the gimbal pins that say it
    /// from behind.
    Iris(MarkerRole),
    /// The pupil.
    Pupil,
}

/// The roles an Observer's iris can carry.
pub const ROLES: [MarkerRole; 3] = [MarkerRole::You, MarkerRole::Teammate, MarkerRole::Rival];

/// The finish of a part.
#[must_use]
pub fn finish(part: Part) -> Finish {
    match part {
        Part::Globe => Finish {
            base_color: Color::srgb(0.035, 0.045, 0.065),
            emissive: LinearRgba::BLACK,
            metallic: 0.25,
            roughness: 0.16,
        },
        Part::Trim => Finish {
            base_color: Color::srgb(0.50, 0.54, 0.60),
            emissive: LinearRgba::BLACK,
            metallic: 1.0,
            roughness: 0.32,
        },
        Part::Iris(role) => Finish {
            base_color: marker(role).base_color,
            emissive: marker(role).emissive,
            metallic: 0.0,
            roughness: 0.25,
        },
        Part::Pupil => Finish {
            base_color: Color::srgb(0.005, 0.005, 0.008),
            emissive: LinearRgba::BLACK,
            metallic: 0.0,
            roughness: 0.08,
        },
    }
}

/// How much of the iris's colour the hover ring under the globe gives off.
pub const HAZE_SCALE: f32 = 0.03;

/// The faint ring the globe floats over, in its iris's colour: it says the thing is
/// hovering, and whose it is, without competing with the eye.
#[must_use]
pub fn haze(role: MarkerRole) -> LinearRgba {
    marker(role).emissive * HAZE_SCALE
}

#[cfg(test)]
mod tests {
    use bevy::color::LinearRgba;

    use super::{Part, ROLES, finish, haze};
    use crate::kinetic::{Role, treatment};
    use crate::{MarkerRole, SIGNAL_MIN_LUMINANCE, luminance, marker};

    /// A colour's chromaticity: what is left when its brightness is divided out.
    fn hue(c: LinearRgba) -> [f32; 3] {
        let sum = (c.red + c.green + c.blue).max(f32::EPSILON);
        [c.red / sum, c.green / sum, c.blue / sum]
    }

    #[test]
    fn the_globe_is_dark_and_darker_than_a_minor() {
        let globe = finish(Part::Globe);
        assert_eq!(globe.emissive, LinearRgba::BLACK);
        let minor = luminance(LinearRgba::from(treatment(Role::GuardianShell).base_color));
        let observer = luminance(LinearRgba::from(globe.base_color));
        assert!(
            observer < minor * 0.1,
            "observer {observer} vs minor {minor}"
        );
    }

    #[test]
    fn every_iris_is_signal_tier() {
        for role in ROLES {
            let iris = luminance(finish(Part::Iris(role)).emissive);
            assert!(iris >= SIGNAL_MIN_LUMINANCE, "{role:?} iris at {iris}");
        }
    }

    #[test]
    fn an_observers_eye_is_never_a_guardians() {
        let threat = hue(marker(MarkerRole::Collapse).emissive);
        for role in ROLES {
            let iris = hue(finish(Part::Iris(role)).emissive);
            let apart: f32 = iris.iter().zip(threat).map(|(a, b)| (a - b).abs()).sum();
            assert!(apart > 0.25, "{role:?} reads as the threat ({apart})");
        }
    }

    #[test]
    fn the_trim_is_steel_not_brass() {
        let steel = LinearRgba::from(finish(Part::Trim).base_color);
        assert!(steel.blue > steel.red, "trim {steel:?} is warm");
        let brass =
            LinearRgba::from(crate::guardian::finish(crate::guardian::Part::Trim).base_color);
        assert!(brass.red > brass.blue, "the Guardian's brass went cold");
    }

    #[test]
    fn the_haze_is_haze_under_the_iris() {
        for role in ROLES {
            let haze = luminance(haze(role));
            assert!(haze < SIGNAL_MIN_LUMINANCE * 0.1, "{role:?} haze {haze}");
            assert!(haze > 0.02, "{role:?} haze invisible at {haze}");
            assert!(haze < luminance(finish(Part::Iris(role)).emissive) * 0.1);
        }
    }
}
