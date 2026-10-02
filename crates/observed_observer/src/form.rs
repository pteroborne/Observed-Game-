//! The floating eye, as parts and a pose.
//!
//! A dark globe hangs at the body's eye height inside a steel gimbal, over a faint
//! ring that says it is hovering. It turns to wherever the body is looking - the
//! globe and its lids to the full gaze, the gimbal to the heading alone - so the one
//! thing an Observer always shows is the one a game about observation most needs:
//! where it is looking. It bobs, so it is never quite still, and it blinks, each eye
//! on its own schedule.
//!
//! The pose is a pure function of the gaze, a clock and a seed, so those properties
//! are tested rather than eyeballed.
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::math::{Quat, Vec3};
use bevy::transform::components::Transform;

/// The globe's centre above the body's, metres: where the body's eye is, so an
/// Observer looks out of exactly the place its player sees from.
pub const EYE_RISE: f32 = 0.70;
/// The globe's radius.
pub const GLOBE: f32 = 0.26;
/// The iris's radius, and how flat it is.
pub const IRIS: f32 = 0.125;
const IRIS_FLAT: f32 = 0.3;
/// How far in front of the centre the iris sits: where the globe's surface is at the
/// iris's edge, so it lies on the globe and stands a little proud of it.
const IRIS_SET: f32 = 0.235;
const PUPIL: f32 = 0.055;
const PUPIL_SET: f32 = 0.262;
/// How far forward of the centre the pupil's face reaches: the frontmost thing on the
/// globe.
const PUPIL_FRONT: f32 = PUPIL_SET + PUPIL * IRIS_FLAT;
/// The lids sit just outside the globe - and outside the iris and pupil standing
/// proud of it, or a shut eye shows its pupil through its lids.
pub const LID: f32 = PUPIL_FRONT + 0.012;

// Compile-time, not tests: relationships between constants. Shut lids cover
// everything on the globe - iris and pupil both - and an open lid does not stand off
// the globe like a hood.
const _: () = assert!(LID > PUPIL_FRONT + 0.005);
const _: () = assert!(LID > IRIS_SET + IRIS * IRIS_FLAT + 0.005);
const _: () = assert!(LID < GLOBE * 1.2);
/// The gimbal ring round the globe, facing forward, and the pins it holds it by.
pub const RING: f32 = 0.31;
const RING_TUBE: f32 = 0.016;
const PIN: f32 = 0.036;
/// The hover ring under the globe.
const HALO: f32 = 0.2;
const HALO_TUBE: f32 = 0.01;
const HALO_DROP: f32 = 0.42;
/// The bob: how far, and how many times a second.
pub const BOB: f32 = 0.03;
const BOB_HZ: f32 = 0.28;
/// How far the lids draw back when open, radians from closed.
pub const LID_OPEN: f32 = 1.15;
/// How long a blink takes, seconds, and the shortest and longest gaps between.
pub const BLINK_SECONDS: f32 = 0.2;
const BLINK_EVERY: (f32, f32) = (3.5, 6.5);
/// The furthest a body's pitch is drawn: an eye looking straight up or down shows
/// nothing but its globe.
const PITCH_LIMIT: f32 = 1.2;

/// A part's shape, in its own frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Sphere {
        radius: f32,
    },
    /// A hemispherical shell, dome up, rim on the XZ plane, seen from both sides.
    Lid {
        radius: f32,
    },
    /// A ring round the Y axis.
    Torus {
        ring: f32,
        tube: f32,
    },
}

/// How a part is drawn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Look {
    Globe,
    Trim,
    Iris,
    Pupil,
    Haze,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub shape: Shape,
    /// Squashed along its own Z: the iris and pupil are discs pressed onto the globe.
    pub flat: f32,
    pub look: Look,
}

pub const GLOBE_PART: usize = 0;
pub const IRIS_PART: usize = 1;
pub const PUPIL_PART: usize = 2;
pub const UPPER_LID: usize = 3;
pub const LOWER_LID: usize = 4;
pub const RING_PART: usize = 5;
pub const HALO_PART: usize = 8;

/// Every part, in the order [`pose`] places them.
#[must_use]
pub fn parts() -> Vec<Part> {
    let part = |shape, look| Part {
        shape,
        flat: 1.0,
        look,
    };
    vec![
        part(Shape::Sphere { radius: GLOBE }, Look::Globe),
        Part {
            flat: IRIS_FLAT,
            ..part(Shape::Sphere { radius: IRIS }, Look::Iris)
        },
        Part {
            flat: IRIS_FLAT,
            ..part(Shape::Sphere { radius: PUPIL }, Look::Pupil)
        },
        part(Shape::Lid { radius: LID }, Look::Globe),
        part(Shape::Lid { radius: LID }, Look::Globe),
        part(
            Shape::Torus {
                ring: RING,
                tube: RING_TUBE,
            },
            Look::Trim,
        ),
        // The pins carry the iris's colour: they sit on the eye's axis, so they say
        // whose eye it is from behind and from the side, where the iris cannot.
        part(Shape::Sphere { radius: PIN }, Look::Iris),
        part(Shape::Sphere { radius: PIN }, Look::Iris),
        part(
            Shape::Torus {
                ring: HALO,
                tube: HALO_TUBE,
            },
            Look::Haze,
        ),
    ]
}

/// Where a body is looking: its heading and pitch, as the simulation keeps them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gaze {
    pub yaw: f32,
    pub pitch: f32,
}

impl Gaze {
    /// The gaze as a rotation of something that looks down -Z, the way the game's
    /// camera turns for the same body.
    #[must_use]
    pub fn rotation(self) -> Quat {
        Quat::from_rotation_y(-self.yaw)
            * Quat::from_rotation_x(self.pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT))
    }

    /// The direction it is looking.
    #[must_use]
    pub fn forward(self) -> Vec3 {
        self.rotation() * Vec3::NEG_Z
    }
}

fn fract(x: f32) -> f32 {
    x - x.floor()
}

/// How open the lids are, 0 shut to 1 open, at `clock` seconds for the eye `seed`.
///
/// Each eye blinks on its own period, between [`BLINK_EVERY`]'s bounds, so a room of
/// them never blinks together.
#[must_use]
pub fn openness(clock: f32, seed: u32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let seed = seed as f32;
    let (shortest, longest) = BLINK_EVERY;
    let every = shortest + (longest - shortest) * fract(seed * 0.618_034 + 0.13);
    let into = (clock + every * fract(seed * 0.414_214 + 0.37)).rem_euclid(every);
    if into < BLINK_SECONDS {
        (2.0 * into / BLINK_SECONDS - 1.0).abs()
    } else {
        1.0
    }
}

/// Every part placed in the body's frame - origin at the body's centre - for a gaze,
/// a clock in seconds, and the eye's own seed.
#[must_use]
pub fn pose(gaze: Gaze, clock: f32, seed: u32) -> Vec<Transform> {
    #[allow(clippy::cast_precision_loss)]
    let phase = fract(seed as f32 * 0.757) * TAU;
    let bob = BOB * (clock * BOB_HZ * TAU + phase).sin();
    let centre = Vec3::Y * (EYE_RISE + bob);
    let eye = gaze.rotation();
    let heading = Quat::from_rotation_y(-gaze.yaw);
    let lid = LID_OPEN * openness(clock, seed);
    let flat = |radius_scale: f32| Vec3::new(1.0, 1.0, radius_scale);
    let pin = |side: f32| Transform::from_translation(centre + heading * Vec3::X * side * RING);
    vec![
        Transform::from_translation(centre).with_rotation(eye),
        Transform::from_translation(centre + eye * Vec3::NEG_Z * IRIS_SET)
            .with_rotation(eye)
            .with_scale(flat(IRIS_FLAT)),
        Transform::from_translation(centre + eye * Vec3::NEG_Z * PUPIL_SET)
            .with_rotation(eye)
            .with_scale(flat(IRIS_FLAT)),
        // Upper lid: dome up, swung back over the top by its openness.
        Transform::from_translation(centre).with_rotation(eye * Quat::from_rotation_x(lid)),
        // Lower lid: the same shell turned over, swung back under.
        Transform::from_translation(centre)
            .with_rotation(eye * Quat::from_rotation_x(-lid) * Quat::from_rotation_z(PI)),
        // The gimbal faces forward and turns with the heading only: it is the frame
        // the globe pitches in.
        Transform::from_translation(centre)
            .with_rotation(heading * Quat::from_rotation_x(FRAC_PI_2)),
        pin(-1.0),
        pin(1.0),
        Transform::from_translation(centre - Vec3::Y * HALO_DROP),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gazes() -> Vec<Gaze> {
        let mut out = Vec::new();
        for yaw in [0.0, 0.7, 2.0, -2.6, PI] {
            for pitch in [-0.9, -0.3, 0.0, 0.4, 1.0] {
                out.push(Gaze { yaw, pitch });
            }
        }
        out
    }

    #[test]
    fn every_part_is_placed() {
        assert_eq!(pose(Gaze::default(), 0.0, 0).len(), parts().len());
    }

    #[test]
    fn the_pupil_looks_where_the_body_looks() {
        for gaze in gazes() {
            let placed = pose(gaze, 1.3, 2);
            let centre = placed[GLOBE_PART].translation;
            let looking = (placed[PUPIL_PART].translation - centre).normalize();
            assert!(
                looking.dot(gaze.forward()) > 0.999,
                "{gaze:?} looks {looking} not {}",
                gaze.forward()
            );
        }
    }

    #[test]
    fn the_eye_is_where_the_body_sees_from() {
        for clock in [0.0, 0.9, 2.2, 7.5] {
            let centre = pose(Gaze::default(), clock, 5)[GLOBE_PART].translation;
            assert!((centre.y - EYE_RISE).abs() <= BOB + 1e-5, "{centre}");
            assert!(centre.x.abs() < 1e-6 && centre.z.abs() < 1e-6);
        }
    }

    #[test]
    fn an_eye_is_never_quite_still() {
        let a = pose(Gaze::default(), 1.0, 1)[GLOBE_PART].translation;
        let b = pose(Gaze::default(), 1.5, 1)[GLOBE_PART].translation;
        assert!(a.distance(b) > 0.002, "{a} vs {b}");
    }

    #[test]
    fn the_gimbal_turns_with_the_heading_and_not_the_pitch() {
        // The ring's axis: the way its face points.
        let axis = |gaze| pose(gaze, 0.0, 0)[RING_PART].rotation * Vec3::Y;
        let level = axis(Gaze {
            yaw: 0.8,
            pitch: 0.0,
        });
        let up = axis(Gaze {
            yaw: 0.8,
            pitch: 0.9,
        });
        assert!(level.distance(up) < 1e-5, "{level} vs {up}");
        let turned = axis(Gaze {
            yaw: 1.8,
            pitch: 0.0,
        });
        assert!((level.angle_between(turned) - 1.0).abs() < 1e-3);
        // It faces the way the body is heading.
        let heading = Gaze {
            yaw: 0.8,
            pitch: 0.0,
        }
        .forward();
        assert!(level.dot(heading).abs() > 0.999, "{level} vs {heading}");
    }

    #[test]
    fn whose_eye_it_is_shows_from_every_side() {
        // From any bearing round the eye, some part in the iris's colour faces the
        // viewer: the iris from the front, a pin from the side or behind.
        let placed = pose(Gaze::default(), 0.0, 0);
        let centre = placed[GLOBE_PART].translation;
        let parts = parts();
        for step in 0..24 {
            #[allow(clippy::cast_precision_loss)]
            let bearing = std::f32::consts::TAU * step as f32 / 24.0;
            let toward_viewer = Vec3::new(bearing.sin(), 0.0, bearing.cos());
            let seen = parts.iter().zip(&placed).any(|(part, at)| {
                part.look == Look::Iris
                    && (at.translation - centre).normalize().dot(toward_viewer) > -0.05
            });
            assert!(seen, "nothing says whose eye it is from bearing {bearing}");
        }
    }

    #[test]
    fn open_lids_clear_the_iris() {
        let iris = (IRIS / GLOBE).asin();
        assert!(
            LID_OPEN > iris + 0.3,
            "lids at {LID_OPEN} cover an iris of {iris}"
        );
    }

    #[test]
    fn an_eye_blinks_now_and_then_and_is_mostly_open() {
        let samples = 60 * 120;
        for seed in 0..6 {
            let mut shut = 0;
            let mut blinks = 0;
            let mut was_open = true;
            for frame in 0..samples {
                #[allow(clippy::cast_precision_loss)]
                let open = openness(frame as f32 / 60.0, seed) > 0.5;
                shut += usize::from(!open);
                blinks += usize::from(was_open && !open);
                was_open = open;
            }
            #[allow(clippy::cast_precision_loss)]
            let fraction = shut as f32 / samples as f32;
            assert!((15..=40).contains(&blinks), "seed {seed}: {blinks} blinks");
            assert!(fraction < 0.03, "seed {seed}: shut {fraction}");
        }
    }

    #[test]
    fn a_room_of_eyes_does_not_blink_together() {
        let shut = |clock: f32, seed| openness(clock, seed) < 0.5;
        let mut together = 0;
        for frame in 0..60 * 120 {
            #[allow(clippy::cast_precision_loss)]
            let clock = frame as f32 / 60.0;
            if (0..4).filter(|&seed| shut(clock, seed)).count() >= 3 {
                together += 1;
            }
        }
        assert!(
            together < 10,
            "{together} frames with three eyes shut at once"
        );
    }

    #[test]
    fn a_blink_closes_the_lids() {
        let seed = 3;
        let shut = (0..60 * 20)
            .map(|frame| {
                #[allow(clippy::cast_precision_loss)]
                let clock = frame as f32 / 60.0;
                clock
            })
            .min_by(|a, b| openness(*a, seed).total_cmp(&openness(*b, seed)))
            .expect("frames");
        assert!(openness(shut, seed) < 0.1);
        let placed = pose(Gaze::default(), shut, seed);
        // Shut, both rims sit near the equator: the lids meet over the iris.
        let upper = placed[UPPER_LID].rotation * Vec3::NEG_Z;
        let lower = placed[LOWER_LID].rotation * Vec3::NEG_Z;
        assert!(
            upper.y.abs() < 0.2 && lower.y.abs() < 0.2,
            "{upper} {lower}"
        );
    }
}
