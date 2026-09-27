//! Candidate designs for the Observer's kinetic tool: parts, poses and meshes, with no
//! rendering and no simulation. `kinetic_tool_lab` draws them side by side.
//!
//! A design is a list of parts (a shape from the Guardians' own kit, and a finish), and
//! a pose places every part for what the tool last did, how long ago, the direction its
//! lash is armed at, and a clock. Every design shows the armed direction on the tool
//! itself, because the armed lash turns with the Observer and is otherwise invisible:
//!
//! - **Coil**, kin to the Guardians: a barrel of hex tiers that turn like the Tumbler's
//!   and snap into line when it fires, and a lidded eye at the muzzle that looks the way
//!   the lash will send things.
//! - **Plumb**, a surveyor's instrument: a banded body, a spirit vial, and a gimballed
//!   plumb bob at the muzzle that hangs along the armed direction, the lash's new down.
//! - **Lance**, a working emitter: a long hex barrel, three prongs that open to push and
//!   close to pull, and the Plumb's gimbal mounted on top as its sight.
//!
//! The gimbal is the directional measure: its outer ring turns to the armed yaw, its inner
//! ring tips to the armed pitch, and the bob hangs along the armed direction.
//!
//! The frame: the palm's grip point at the origin, the muzzle toward -Z, up +Y, metres.
use std::f32::consts::{FRAC_PI_2, FRAC_PI_3, TAU};

use bevy::math::{Quat, Vec3};
use bevy::transform::components::Transform;
pub use observed_guardian::form::Shape;

/// Seconds a push, pull or lash reads on the tool after it fires.
pub const FIRE_SECONDS: f32 = 0.35;
/// How far a push kicks the tool back into the hand, metres.
const RECOIL: f32 = 0.035;

/// One of the candidate designs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Design {
    Coil,
    Plumb,
    Lance,
}

impl Design {
    pub const ALL: [Self; 3] = [Self::Coil, Self::Plumb, Self::Lance];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Coil => "coil",
            Self::Plumb => "plumb",
            Self::Lance => "lance",
        }
    }

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Coil => "COIL - kin to the Guardians",
            Self::Plumb => "PLUMB - a surveyor's instrument",
            Self::Lance => "LANCE - a working emitter",
        }
    }
}

/// How a part is finished. The drawer maps these onto its own materials: the equipment
/// hardware for the first four, the kinetic push or pull colour for the signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Finish {
    Body,
    Trim,
    Grip,
    Glass,
    /// Lit by the tool's charge, flaring when it fires.
    Signal,
    /// An eye's white, self-lit.
    Eye,
    Pupil,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub shape: Shape,
    pub finish: Finish,
}

/// What the tool last did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Beat {
    Idle,
    Push,
    Pull,
    Lash,
}

/// Everything a pose depends on besides the clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolState {
    pub beat: Beat,
    /// Seconds since the beat began.
    pub since: f32,
    /// The pitch the lash is armed at, radians, up positive.
    pub armed_pitch: f32,
    /// The yaw the lash is armed at, radians from the way the Observer faces, right
    /// positive. With [`Self::armed_pitch`], the armed direction in the Observer's facing.
    pub armed_yaw: f32,
    /// Charge left, 0 to 1.
    pub charge: f32,
}

impl Default for ToolState {
    fn default() -> Self {
        Self {
            beat: Beat::Idle,
            since: 10.0,
            armed_pitch: 0.0,
            armed_yaw: 0.0,
            charge: 1.0,
        }
    }
}

/// Every part placed, and how brightly the signal burns.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolPose {
    pub parts: Vec<Transform>,
    /// 0 dark to 1 at full charge; a firing flares past 1.
    pub signal: f32,
}

#[must_use]
pub fn parts(design: Design) -> Vec<Part> {
    match design {
        Design::Coil => coil::parts(),
        Design::Plumb => plumb::parts(),
        Design::Lance => lance::parts(),
    }
}

#[must_use]
pub fn pose(design: Design, state: ToolState, clock: f32) -> ToolPose {
    let parts = match design {
        Design::Coil => coil::pose(state, clock),
        Design::Plumb => plumb::pose(state),
        Design::Lance => lance::pose(state),
    };
    // A push kicks the tool back into the hand, a pull tugs it forward, a lash nudges.
    let kick = match state.beat {
        Beat::Idle => 0.0,
        Beat::Push => RECOIL,
        Beat::Pull => -RECOIL * 0.6,
        Beat::Lash => RECOIL * 0.4,
    } * fading(state);
    let recoil = Transform::from_translation(Vec3::Z * kick);
    ToolPose {
        parts: parts.into_iter().map(|at| recoil * at).collect(),
        signal: 0.2 + 0.6 * state.charge.clamp(0.0, 1.0) + 0.8 * fired(state),
    }
}

/// 1 the moment the tool fires, falling to 0 over [`FIRE_SECONDS`].
fn fading(state: ToolState) -> f32 {
    (1.0 - state.since / FIRE_SECONDS).clamp(0.0, 1.0)
}

/// [`fading`], and 0 while idle.
fn fired(state: ToolState) -> f32 {
    if state.beat == Beat::Idle {
        0.0
    } else {
        fading(state)
    }
}

/// Which way the armed lash points in the tool's frame.
fn armed(state: ToolState) -> Vec3 {
    let pitch = state.armed_pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
    turned(state) * Vec3::new(0.0, pitch.sin(), -pitch.cos())
}

/// The armed yaw as a turn about the tool's up: `-Z` turned to face the armed heading.
fn turned(state: ToolState) -> Quat {
    Quat::from_rotation_y(-state.armed_yaw)
}

/// A shape that stands up +Y, laid down to run toward the muzzle from `at`.
fn forward(at: Vec3) -> Transform {
    Transform::from_translation(at).with_rotation(Quat::from_rotation_x(-FRAC_PI_2))
}

/// A shape that stands up +Y, stood along `direction` from `at`.
fn along(at: Vec3, direction: Vec3) -> Transform {
    Transform::from_translation(at)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, direction.normalize()))
}

/// The pistol grip and its pommel, the same on every design.
fn grip() -> [Part; 2] {
    [
        Part {
            shape: Shape::HexFrustum {
                bottom: 0.020,
                top: 0.024,
                height: 0.12,
            },
            finish: Finish::Grip,
        },
        Part {
            shape: Shape::HexFrustum {
                bottom: 0.028,
                top: 0.021,
                height: 0.02,
            },
            finish: Finish::Body,
        },
    ]
}

fn grip_pose() -> [Transform; 2] {
    [
        Transform::from_xyz(0.0, -0.12, 0.0),
        Transform::from_xyz(0.0, -0.14, 0.0),
    ]
}

/// Barrel axis height above the grip point.
const AXIS: f32 = 0.035;

/// A gimballed plumb bob, the lash's new down shown literally: an outer ring that turns
/// to the armed yaw, an inner ring inside it that tips to the armed pitch, and a bob hung
/// along the armed direction from their common centre. `size` scales the Plumb's own.
mod gimbal {
    use super::*;

    pub(super) fn parts(size: f32) -> [Part; 4] {
        [
            Part {
                shape: Shape::Torus {
                    major: 0.055 * size,
                    minor: 0.005 * size,
                },
                finish: Finish::Trim,
            },
            Part {
                shape: Shape::Torus {
                    major: 0.044 * size,
                    minor: 0.004 * size,
                },
                finish: Finish::Trim,
            },
            Part {
                shape: Shape::Rod {
                    radius: 0.002 * size,
                    length: 0.034 * size,
                },
                finish: Finish::Trim,
            },
            Part {
                shape: Shape::HexFrustum {
                    bottom: 0.016 * size,
                    top: 0.0,
                    height: 0.03 * size,
                },
                finish: Finish::Signal,
            },
        ]
    }

    /// The outer ring's radius, to its outside edge.
    pub(super) fn radius(size: f32) -> f32 {
        0.060 * size
    }

    pub(super) fn pose(state: ToolState, at: Vec3, size: f32) -> [Transform; 4] {
        let pitch = state.armed_pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
        let turn = turned(state);
        // A lash swings the bob out along its new down, and it settles back.
        let swing = Quat::from_axis_angle(
            turn * Vec3::X,
            0.35 * fired(state) * (state.since * 30.0).sin(),
        );
        let down = swing * armed(state);
        [
            // Both rings stand facing the armed heading; the inner one tips to the pitch.
            Transform::from_translation(at).with_rotation(turn * Quat::from_rotation_x(FRAC_PI_2)),
            Transform::from_translation(at)
                .with_rotation(turn * Quat::from_rotation_x(FRAC_PI_2 + pitch)),
            along(at, down),
            along(at + down * 0.064 * size, -down),
        ]
    }
}

mod coil {
    use super::*;

    const TIERS: usize = 4;
    const EYE_AT: Vec3 = Vec3::new(0.0, AXIS, -0.215);

    pub(super) fn parts() -> Vec<Part> {
        let mut parts = grip().to_vec();
        parts.push(Part {
            shape: Shape::Rod {
                radius: 0.012,
                length: 0.24,
            },
            finish: Finish::Signal,
        });
        for tier in 0..TIERS {
            parts.push(Part {
                shape: Shape::HexRing {
                    outer: 0.050 - tier as f32 * 0.004,
                    inner: 0.016,
                    height: 0.034,
                },
                finish: Finish::Body,
            });
        }
        parts.extend([
            Part {
                shape: Shape::Sphere { radius: 0.024 },
                finish: Finish::Eye,
            },
            Part {
                shape: Shape::Sphere { radius: 0.010 },
                finish: Finish::Pupil,
            },
            Part {
                shape: Shape::Torus {
                    major: 0.027,
                    minor: 0.004,
                },
                finish: Finish::Trim,
            },
        ]);
        parts
    }

    pub(super) fn pose(state: ToolState, clock: f32) -> Vec<Transform> {
        let mut at = grip_pose().to_vec();
        // The core starts inside the last tier, so it shows only through the gaps.
        at.push(forward(Vec3::new(0.0, AXIS, 0.025)));
        // The tiers turn against each other like the Tumbler's while it is idle, spin
        // hard on a pull, and snap into line when it pushes or lashes.
        let spin = if state.beat == Beat::Pull {
            1.0 + 6.0 * fading(state)
        } else {
            1.0
        };
        let line_up = if matches!(state.beat, Beat::Push | Beat::Lash) {
            fading(state)
        } else {
            0.0
        };
        for tier in 0..TIERS {
            let sign = if tier % 2 == 0 { 1.0 } else { -1.0 };
            let turning = sign * clock * spin * 0.8 * (1.0 + tier as f32 * 0.15);
            let lined = (turning / FRAC_PI_3).round() * FRAC_PI_3;
            let angle = turning + (lined - turning) * line_up;
            at.push(
                forward(Vec3::new(0.0, AXIS, 0.03 - tier as f32 * 0.06))
                    * Transform::from_rotation(Quat::from_rotation_y(angle.rem_euclid(TAU))),
            );
        }
        // The eye looks where the lash will send things.
        at.push(Transform::from_translation(EYE_AT));
        at.push(Transform::from_translation(EYE_AT + armed(state) * 0.018));
        at.push(
            Transform::from_translation(EYE_AT).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        );
        at
    }
}

mod plumb {
    use super::*;

    const GIMBAL_AT: Vec3 = Vec3::new(0.0, AXIS, -0.17);

    pub(super) fn parts() -> Vec<Part> {
        let mut parts = grip().to_vec();
        parts.extend([
            Part {
                shape: Shape::HexFrustum {
                    bottom: 0.042,
                    top: 0.034,
                    height: 0.21,
                },
                finish: Finish::Body,
            },
            Part {
                shape: Shape::HexRing {
                    outer: 0.047,
                    inner: 0.036,
                    height: 0.012,
                },
                finish: Finish::Trim,
            },
            Part {
                shape: Shape::HexRing {
                    outer: 0.041,
                    inner: 0.031,
                    height: 0.012,
                },
                finish: Finish::Trim,
            },
            Part {
                shape: Shape::Sphere { radius: 0.020 },
                finish: Finish::Glass,
            },
            Part {
                shape: Shape::Sphere { radius: 0.006 },
                finish: Finish::Signal,
            },
        ]);
        parts.extend(gimbal::parts(1.0));
        parts
    }

    pub(super) fn pose(state: ToolState) -> Vec<Transform> {
        let pitch = state.armed_pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
        let mut at = grip_pose().to_vec();
        at.extend([
            forward(Vec3::new(0.0, AXIS, 0.06)),
            forward(Vec3::new(0.0, AXIS, 0.035)),
            forward(Vec3::new(0.0, AXIS, -0.10)),
            Transform::from_xyz(0.0, AXIS + 0.052, -0.03),
            // The vial's bubble runs to the high end as the armed pitch climbs.
            Transform::from_xyz(0.0, AXIS + 0.060, -0.03 + 0.010 * pitch.sin()),
        ]);
        at.extend(gimbal::pose(state, GIMBAL_AT, 1.0));
        at
    }
}

mod lance {
    use super::*;

    const MUZZLE: f32 = -0.235;
    /// The Plumb's gimbal, at this share of its size, stands on the barrel as a sight.
    pub(super) const GIMBAL_SIZE: f32 = 0.55;
    /// Where the barrel's top runs under the gimbal.
    pub(super) const BARREL_TOP: f32 = AXIS + 0.0275;
    const GIMBAL_Z: f32 = -0.06;
    /// The gimbal's centre: high enough that the bob clears the barrel hung straight down.
    pub(super) const GIMBAL_AT: Vec3 = Vec3::new(0.0, BARREL_TOP + 0.067 * GIMBAL_SIZE, GIMBAL_Z);
    /// The post runs from inside the barrel to the outer ring's foot.
    const POST_FROM: f32 = AXIS + 0.018;

    pub(super) fn parts() -> Vec<Part> {
        let mut parts = grip().to_vec();
        parts.extend([
            Part {
                shape: Shape::HexFrustum {
                    bottom: 0.030,
                    top: 0.024,
                    height: 0.30,
                },
                finish: Finish::Body,
            },
            Part {
                shape: Shape::HexFrustum {
                    bottom: 0.036,
                    top: 0.031,
                    height: 0.03,
                },
                finish: Finish::Trim,
            },
        ]);
        for _ in 0..3 {
            parts.push(Part {
                shape: Shape::Rod {
                    radius: 0.006,
                    length: 0.085,
                },
                finish: Finish::Trim,
            });
        }
        parts.extend([
            Part {
                shape: Shape::HexRing {
                    outer: 0.034,
                    inner: 0.020,
                    height: 0.014,
                },
                finish: Finish::Signal,
            },
            Part {
                shape: Shape::Rod {
                    radius: 0.0035,
                    length: GIMBAL_AT.y - gimbal::radius(GIMBAL_SIZE) - POST_FROM,
                },
                finish: Finish::Trim,
            },
        ]);
        parts.extend(gimbal::parts(GIMBAL_SIZE));
        parts
    }

    pub(super) fn pose(state: ToolState) -> Vec<Transform> {
        // The prongs open to push, close to pull, and rest a little apart.
        let spread = match state.beat {
            Beat::Push | Beat::Lash => 0.15 + 0.45 * fading(state),
            Beat::Pull => 0.15 - 0.35 * fading(state),
            Beat::Idle => 0.15,
        };
        let mut at = grip_pose().to_vec();
        at.extend([
            forward(Vec3::new(0.0, AXIS, 0.065)),
            forward(Vec3::new(0.0, AXIS, 0.095)),
        ]);
        for prong in 0..3 {
            let angle = prong as f32 * TAU / 3.0 + FRAC_PI_2;
            let radial = Vec3::new(angle.cos(), angle.sin(), 0.0);
            at.push(along(
                Vec3::new(0.0, AXIS, MUZZLE) + radial * 0.026,
                Vec3::NEG_Z + radial * spread,
            ));
        }
        at.extend([
            forward(Vec3::new(0.0, AXIS, MUZZLE + 0.007)),
            Transform::from_xyz(0.0, POST_FROM, GIMBAL_Z),
        ]);
        at.extend(gimbal::pose(state, GIMBAL_AT, GIMBAL_SIZE));
        at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn states() -> Vec<ToolState> {
        let mut states = Vec::new();
        for beat in [Beat::Idle, Beat::Push, Beat::Pull, Beat::Lash] {
            for since in [0.0, 0.1, 0.3, 1.0] {
                for armed_pitch in [-FRAC_PI_2, -1.2, 0.0, 0.9, FRAC_PI_2] {
                    for armed_yaw in [-2.5, 0.0, 1.1, 3.0] {
                        states.push(ToolState {
                            beat,
                            since,
                            armed_pitch,
                            armed_yaw,
                            charge: 0.5,
                        });
                    }
                }
            }
        }
        states
    }

    #[test]
    fn every_pose_places_every_part() {
        for design in Design::ALL {
            let count = parts(design).len();
            for state in states() {
                let pose = pose(design, state, 3.7);
                assert_eq!(pose.parts.len(), count, "{design:?} {state:?}");
                assert!(
                    pose.parts
                        .iter()
                        .all(|at| at.translation.is_finite() && at.rotation.is_finite()),
                    "{design:?} {state:?}"
                );
            }
        }
    }

    /// A hand holds it: every part stays within a box a forearm's length from the palm.
    #[test]
    fn every_design_fits_in_a_hand() {
        for design in Design::ALL {
            for state in states() {
                for at in pose(design, state, 1.3).parts {
                    let p = at.translation;
                    assert!(
                        p.x.abs() < 0.08
                            && (-0.16..0.14).contains(&p.y)
                            && (-0.33..0.14).contains(&p.z),
                        "{design:?} part at {p}"
                    );
                }
            }
        }
    }

    /// The armed pitch reads on every design: pose it up and pose it down, and the
    /// indicator moves.
    #[test]
    fn every_design_shows_the_armed_pitch() {
        for design in Design::ALL {
            let up = pose(
                design,
                ToolState {
                    armed_pitch: 1.0,
                    ..ToolState::default()
                },
                0.0,
            );
            let down = pose(
                design,
                ToolState {
                    armed_pitch: -1.0,
                    ..ToolState::default()
                },
                0.0,
            );
            assert_ne!(up.parts, down.parts, "{design:?} hides the armed pitch");
        }
    }

    /// And so does the armed yaw: turned left and turned right, the indicator moves.
    #[test]
    fn every_design_shows_the_armed_yaw() {
        for design in Design::ALL {
            let [left, right] = [-1.0, 1.0].map(|armed_yaw| {
                pose(
                    design,
                    ToolState {
                        armed_yaw,
                        ..ToolState::default()
                    },
                    0.0,
                )
            });
            assert_ne!(left.parts, right.parts, "{design:?} hides the armed yaw");
        }
    }

    /// The bob hangs along the armed direction in the tool's frame: ahead is `-Z`, a
    /// positive yaw turns it right toward `+X`, a positive pitch raises it.
    #[test]
    fn the_bob_hangs_along_the_armed_direction() {
        for (armed_pitch, armed_yaw, expected) in [
            (0.0, 0.0, Vec3::NEG_Z),
            (0.0, FRAC_PI_2, Vec3::X),
            (0.0, -FRAC_PI_2, Vec3::NEG_X),
            (0.0, std::f32::consts::PI, Vec3::Z),
            (FRAC_PI_2, 1.0, Vec3::Y),
            (-FRAC_PI_2, -2.0, Vec3::NEG_Y),
        ] {
            let state = ToolState {
                armed_pitch,
                armed_yaw,
                ..ToolState::default()
            };
            let [_, _, rod, _] = gimbal::pose(state, Vec3::ZERO, 1.0);
            let hangs = rod.rotation * Vec3::Y;
            assert!(
                hangs.distance(expected) < 1e-4,
                "pitch {armed_pitch} yaw {armed_yaw}: hangs {hangs}, expected {expected}"
            );
        }
    }

    /// The Lance's sight never swings its bob into the barrel it stands on.
    #[test]
    fn the_lance_bob_clears_the_barrel() {
        for state in states() {
            let [_, _, rod, bob] = gimbal::pose(state, lance::GIMBAL_AT, lance::GIMBAL_SIZE);
            let tip = bob.translation;
            assert!(
                rod.translation.y > lance::BARREL_TOP && tip.y > lance::BARREL_TOP,
                "{state:?}: bob at {tip}"
            );
        }
    }

    #[test]
    fn firing_flares_and_fades() {
        let idle = pose(Design::Coil, ToolState::default(), 0.0).signal;
        let fired = pose(
            Design::Coil,
            ToolState {
                beat: Beat::Push,
                since: 0.0,
                ..ToolState::default()
            },
            0.0,
        )
        .signal;
        let faded = pose(
            Design::Coil,
            ToolState {
                beat: Beat::Push,
                since: FIRE_SECONDS,
                ..ToolState::default()
            },
            0.0,
        )
        .signal;
        assert!(fired > idle);
        assert!((faded - idle).abs() < 1e-5);
    }
}
