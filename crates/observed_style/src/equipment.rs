//! Hand equipment: what an Observer carries and puts down.
//!
//! The teleport plate and the anchor lantern are signal devices, and each carries its
//! signal in one place: the plate in its state ring and lens, the lantern in its guide
//! core and anchor trim. Everything else is hardware (dark metal, a rubbered grip, the
//! lantern's glass), and hardware never glows. Before this module a carried plate was
//! drawn entirely in the teammate signal, so the whole disc bloomed and was the
//! brightest thing on screen, brighter than anything it was meant to point at.
//!
//! Three rules, each tested:
//!
//! * **Hardware is dark and never emits.** Every [`Hardware`] finish is darker than
//!   the dimmest marker's base colour, so no part of a device reads as its signal.
//! * **A carried signal sits inside the first-person budget.** A part held half a
//!   metre from the eye is scaled by [`HELD_SIGNAL_SCALE`], which keeps it off the
//!   white-out end of the bloom; the lantern's guide core is the exception, because it
//!   is what the player reads the exit from, and stays signal-tier.
//! * **The link column is haze, not a beacon.** A linked plate's light column is the
//!   faint vertical read that the link is live from across a room; it stays well under
//!   the signal floor, so it never competes with the plate's own ring.
use bevy::color::{Color, LinearRgba};

use crate::{MarkerRole, marker};

/// A part of a device that is not its signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Hardware {
    /// The body: plate deck and rim, lantern caps and cage.
    Body,
    /// Bright machined edges, studs and finials that catch the light.
    Trim,
    /// The lantern's grip.
    Grip,
    /// The lantern's glass chamber.
    Glass,
}

/// A physically based finish for one [`Hardware`] part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Finish {
    /// Albedo; for [`Hardware::Glass`], alpha is its opacity.
    pub base_color: Color,
    pub metallic: f32,
    pub roughness: f32,
}

/// What a signal part is scaled by while a hand holds it.
pub const HELD_SIGNAL_SCALE: f32 = 0.35;
/// What a linked plate's light column is scaled by, from the plate's own signal.
pub const LINK_COLUMN_SCALE: f32 = 0.05;

/// The finish of a hardware part.
#[must_use]
pub fn finish(part: Hardware) -> Finish {
    match part {
        // Blued gunmetal: dark enough that the signal parts own the silhouette, and
        // metallic enough to pick up the moon and the district key along its edges.
        Hardware::Body => Finish {
            base_color: Color::srgb(0.11, 0.12, 0.14),
            metallic: 0.85,
            roughness: 0.38,
        },
        Hardware::Trim => Finish {
            base_color: Color::srgb(0.30, 0.31, 0.33),
            metallic: 1.0,
            roughness: 0.22,
        },
        Hardware::Grip => Finish {
            base_color: Color::srgb(0.05, 0.05, 0.055),
            metallic: 0.0,
            roughness: 0.85,
        },
        Hardware::Glass => Finish {
            base_color: Color::srgba(0.62, 0.72, 0.80, 0.08),
            metallic: 0.0,
            roughness: 0.05,
        },
    }
}

/// A signal part's emission while a hand holds it.
#[must_use]
pub fn held(role: MarkerRole) -> LinearRgba {
    marker(role).emissive * HELD_SIGNAL_SCALE
}

/// A linked plate's light column, from the colour of the plate it rises from.
#[must_use]
pub fn link_column(role: MarkerRole) -> LinearRgba {
    marker(role).emissive * LINK_COLUMN_SCALE
}

#[cfg(test)]
mod tests {
    use bevy::color::{Alpha, LinearRgba};

    use super::{Hardware, finish, held, link_column};
    use crate::{MarkerRole, SIGNAL_MIN_LUMINANCE, luminance, marker};

    const HARDWARE: [Hardware; 4] = [
        Hardware::Body,
        Hardware::Trim,
        Hardware::Grip,
        Hardware::Glass,
    ];
    const DEVICE_SIGNALS: [MarkerRole; 4] = [
        MarkerRole::NextRoom,
        MarkerRole::Control,
        MarkerRole::Teammate,
        MarkerRole::Rival,
    ];

    #[test]
    fn hardware_is_darker_than_every_signal_it_carries() {
        let dimmest = DEVICE_SIGNALS
            .map(|role| luminance(LinearRgba::from(marker(role).base_color)))
            .into_iter()
            .fold(f32::MAX, f32::min);
        for part in HARDWARE {
            // What a part contributes: a nearly clear glass is mostly what is behind it.
            let color = finish(part).base_color;
            let albedo = luminance(LinearRgba::from(color)) * color.alpha();
            assert!(albedo < dimmest * 0.5, "{part:?} at {albedo} vs {dimmest}");
        }
    }

    #[test]
    fn a_held_signal_is_dimmed_but_still_lit() {
        for role in DEVICE_SIGNALS {
            let full = luminance(marker(role).emissive);
            let carried = luminance(held(role));
            assert!(carried < full * 0.5, "{role:?}");
            assert!(carried > 0.2, "{role:?} is dark in the hand at {carried}");
        }
    }

    #[test]
    fn the_link_column_is_haze_under_the_signal_floor() {
        for role in DEVICE_SIGNALS {
            let column = luminance(link_column(role));
            assert!(column < SIGNAL_MIN_LUMINANCE * 0.2, "{role:?} at {column}");
            assert!(column > 0.02, "{role:?} column invisible at {column}");
        }
    }

    #[test]
    fn the_glass_is_nearly_clear() {
        let alpha = finish(Hardware::Glass).base_color.alpha();
        assert!((0.05..0.3).contains(&alpha), "glass opacity {alpha}");
    }
}
