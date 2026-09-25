//! The Guardians' forms, as parts and poses, with no rendering in them.
//!
//! Each form is a list of [`Part`]s (a shape and a look) and a pose function that places
//! every part for a [`State`], the seconds spent in it, and the lab's clock. The pose
//! is a pure function, so the properties that decide between the forms can be tested:
//! a frozen Guardian is perfectly still, a hunting one never is, and each fits the
//! facility's doorway.
use std::f32::consts::{FRAC_PI_3, PI, TAU};

use bevy::math::{Quat, Vec3};
use bevy::transform::components::Transform;

use crate::roll::{self, Rest, Roll, VERTEX};

/// A candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Form {
    /// A stepped pyramid of hexagonal tiers that turn like a lock's tumblers while it
    /// hunts and snap into line when it is seen. Its rank is its tier count.
    Tumbler { tiers: u8 },
    /// An inverted pyramid hanging point-down, ringed by orbits, sweeping its beam as
    /// it turns. Seen, its rings fall flat and it sets its point on the floor.
    Plumb,
    /// An octahedral cage that walks by tumbling face to face, its eye level inside.
    /// Seen, it stands on one point, impossibly balanced.
    Roller,
}

impl Form {
    pub const MAJORS: [Self; 3] = [Self::Tumbler { tiers: 4 }, Self::Plumb, Self::Roller];

    #[must_use]
    pub fn name(self) -> String {
        match self {
            Self::Tumbler { tiers } => format!("tumbler_{tiers}"),
            Self::Plumb => "plumb".into(),
            Self::Roller => "roller".into(),
        }
    }
}

/// What the Guardian is doing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Hunting,
    FrozenBySight,
    FrozenByAnchor,
    /// It has reached someone nobody was watching.
    Catch,
}

impl State {
    pub const ALL: [Self; 4] = [
        Self::Hunting,
        Self::FrozenBySight,
        Self::FrozenByAnchor,
        Self::Catch,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Hunting => "hunting",
            Self::FrozenBySight => "frozen_by_sight",
            Self::FrozenByAnchor => "frozen_by_anchor",
            Self::Catch => "catch",
        }
    }

    #[must_use]
    pub const fn frozen(self) -> bool {
        matches!(self, Self::FrozenBySight | Self::FrozenByAnchor)
    }
}

/// A part's shape, in its own frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// Hexagonal frustum standing on its base at the origin.
    HexFrustum {
        bottom: f32,
        top: f32,
        height: f32,
    },
    /// Flat hexagonal ring standing on the origin.
    HexRing {
        outer: f32,
        inner: f32,
        height: f32,
    },
    Sphere {
        radius: f32,
    },
    /// Ring about the Y axis.
    Torus {
        major: f32,
        minor: f32,
    },
    /// A triangular slab, the triangle given in the part's frame, thickened toward
    /// `inward`.
    Panel {
        corners: [Vec3; 3],
        inward: Vec3,
        thickness: f32,
    },
    /// A rod from the origin up the Y axis.
    Rod {
        radius: f32,
        length: f32,
    },
    /// A cone of light from the origin along +Z, fading as it goes.
    Beam {
        length: f32,
        radius: f32,
    },
    /// A flat hexagon of light on the floor.
    Stamp {
        radius: f32,
    },
}

/// How a part is drawn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Look {
    Shell,
    Trim,
    /// The core showing through the seams: live, dark when frozen, flaring in a catch.
    Seam,
    Eye,
    Pupil,
    Beam,
    /// The anchor lantern's purple clamp.
    Clamp,
    Stamp,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub shape: Shape,
    pub look: Look,
}

/// Every part placed, `None` where a part is not shown in this state.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub parts: Vec<Option<Transform>>,
    /// The seams are dark.
    pub seams_dark: bool,
    /// How far into a catch's flare, 0 to 1.
    pub flare: f32,
}

/// Where the Guardian stands and what it is looking at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage {
    pub at: Vec3,
    pub toward: Vec3,
}

/// How long a catch plays before it repeats, seconds.
pub const CATCH_SECONDS: f32 = 1.8;
/// The Roller's step, seconds: it lands on its next face at each whole multiple of
/// this, which is when its fall is heard.
pub const ROLLER_STEP_SECONDS: f32 = 1.0;

fn ease_out(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    1.0 - (1.0 - u).powi(3)
}

/// Past its target and back, like a latch dropping home.
fn ease_out_back(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    let c = 2.2;
    1.0 + (c + 1.0) * (u - 1.0).powi(3) + c * (u - 1.0).powi(2)
}

fn yaw_to(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    d.x.atan2(d.z)
}

/// The nearest angle to `angle` that is a multiple of `step`.
fn snapped(angle: f32, step: f32) -> f32 {
    (angle / step).round() * step
}

/// The nearest angle to `from` that equals `target` modulo a full turn.
fn nearest_turn(from: f32, target: f32) -> f32 {
    target + ((from - target) / TAU).round() * TAU
}

#[must_use]
pub fn parts(form: Form) -> Vec<Part> {
    match form {
        Form::Tumbler { tiers } => tumbler::parts(tiers),
        Form::Plumb => plumb::parts(),
        Form::Roller => roller::parts(),
    }
}

#[must_use]
pub fn pose(form: Form, state: State, t: f32, clock: f32, stage: Stage) -> Pose {
    match form {
        Form::Tumbler { tiers } => tumbler::pose(tiers, state, t, clock, stage),
        Form::Plumb => plumb::pose(state, t, clock, stage),
        Form::Roller => roller::pose(state, t, clock, stage),
    }
}

/// The eye and pupil, looking along +Z of whatever frame they are placed in.
fn eye_parts(radius: f32) -> [Part; 2] {
    [
        Part {
            shape: Shape::Sphere { radius },
            look: Look::Eye,
        },
        Part {
            shape: Shape::Sphere { radius: 1.0 },
            look: Look::Pupil,
        },
    ]
}

/// Where the eye and its vertical slit pupil go, for an eye of `radius` at `centre`
/// looking along `facing`.
fn eye_transforms(centre: Vec3, facing: Quat, radius: f32) -> [Transform; 2] {
    [
        Transform::from_translation(centre).with_rotation(facing),
        Transform::from_translation(centre + facing * Vec3::Z * radius * 0.93)
            .with_rotation(facing)
            .with_scale(Vec3::new(radius * 0.16, radius * 0.62, radius * 0.12)),
    ]
}

fn beam_part() -> Part {
    Part {
        shape: Shape::Beam {
            length: 9.0,
            radius: 1.3,
        },
        look: Look::Beam,
    }
}

fn stamp_part() -> Part {
    Part {
        shape: Shape::Stamp { radius: 1.0 },
        look: Look::Stamp,
    }
}

/// The stamp a catch leaves on the floor: spreading, then gone.
fn stamp(at: Vec3, t: f32) -> Option<Transform> {
    let u = (t / 1.1).clamp(0.0, 1.0);
    (t < 1.1).then(|| {
        Transform::from_translation(Vec3::new(at.x, 0.012, at.z)).with_scale(Vec3::new(
            0.4 + 2.6 * ease_out(u),
            1.0,
            0.4 + 2.6 * ease_out(u),
        ))
    })
}

mod tumbler {
    use super::*;

    const TIER_HEIGHT: f32 = 0.5;
    const GAP: f32 = 0.12;
    const HOVER: f32 = 0.18;
    const SETTLED: f32 = 0.05;
    const CROWN_HEIGHT: f32 = 0.55;
    const EYE: f32 = 0.2;

    fn radius(tiers: u8, k: u8) -> f32 {
        0.75 + f32::from(tiers - 1 - k) * 0.25
    }

    fn base(k: u8) -> f32 {
        f32::from(k) * (TIER_HEIGHT + GAP)
    }

    /// Parts, in order: per tier its body and its brass rim; then the core, the crown,
    /// the eye and pupil, the beam, the clamp and the stamp.
    pub(super) fn parts(tiers: u8) -> Vec<Part> {
        let mut parts = Vec::new();
        for k in 0..tiers {
            let r = radius(tiers, k);
            parts.push(Part {
                shape: Shape::HexFrustum {
                    bottom: r,
                    top: r - 0.1,
                    height: TIER_HEIGHT,
                },
                look: Look::Shell,
            });
            parts.push(Part {
                shape: Shape::HexRing {
                    outer: r - 0.08,
                    inner: r - 0.24,
                    height: 0.035,
                },
                look: Look::Trim,
            });
        }
        parts.push(Part {
            shape: Shape::Rod {
                radius: 0.5,
                length: base(tiers) + 0.1,
            },
            look: Look::Seam,
        });
        parts.push(Part {
            shape: Shape::HexFrustum {
                bottom: 0.62,
                top: 0.26,
                height: CROWN_HEIGHT,
            },
            look: Look::Shell,
        });
        parts.extend(eye_parts(EYE));
        parts.push(beam_part());
        let r0 = radius(tiers, 0);
        parts.push(Part {
            shape: Shape::HexRing {
                outer: r0 + 0.16,
                inner: r0 + 0.02,
                height: 0.16,
            },
            look: Look::Clamp,
        });
        parts.push(stamp_part());
        parts
    }

    fn tier_yaw(k: u8, clock: f32) -> f32 {
        let turn = if k.is_multiple_of(2) { 1.0 } else { -1.0 };
        turn * (0.25 + 0.2 * f32::from(k)) * clock + f32::from(k) * 0.4
    }

    pub(super) fn pose(tiers: u8, state: State, t: f32, clock: f32, stage: Stage) -> Pose {
        let since = clock - t;
        let hunt_hover = |c: f32| HOVER + 0.035 * (1.4 * c).sin();
        let face = yaw_to(stage.at, stage.toward);
        let (hover, yaws, crown_yaw, lift, flare): (f32, Vec<f32>, f32, f32, f32) = match state {
            State::Hunting => (
                hunt_hover(clock),
                (0..tiers).map(|k| tier_yaw(k, clock)).collect(),
                face + 0.35 * (0.6 * clock).sin(),
                0.0,
                0.0,
            ),
            State::FrozenBySight | State::FrozenByAnchor => {
                let snap = ease_out_back(t / 0.14);
                let settle = ease_out(t / 0.25);
                let from = hunt_hover(since);
                (
                    from + (SETTLED - from) * settle,
                    (0..tiers)
                        .map(|k| {
                            let yaw = tier_yaw(k, since);
                            yaw + (snapped(yaw, FRAC_PI_3) - yaw) * snap
                        })
                        .collect(),
                    {
                        let from = face + 0.35 * (0.6 * since).sin();
                        from + (face - from) * ease_out(t / 0.2)
                    },
                    0.0,
                    0.0,
                )
            }
            State::Catch => {
                let open = ease_out(t / 0.45);
                (
                    hunt_hover(since),
                    (0..tiers)
                        .map(|k| tier_yaw(k, since) + tier_yaw(k, t * 3.0) - tier_yaw(k, 0.0))
                        .collect(),
                    face,
                    open,
                    (1.0 - ((t - 0.9) / 0.9).clamp(0.0, 1.0)) * open,
                )
            }
        };
        let at = stage.at;
        let mut parts = Vec::new();
        for k in 0..tiers {
            let y = hover + base(k) + f32::from(k) * 0.3 * lift;
            let turn = Quat::from_rotation_y(yaws[usize::from(k)]);
            parts.push(Some(
                Transform::from_translation(at + Vec3::Y * y).with_rotation(turn),
            ));
            parts.push(Some(
                Transform::from_translation(at + Vec3::Y * (y + TIER_HEIGHT - 0.01))
                    .with_rotation(turn),
            ));
        }
        let crown_y = hover + base(tiers) + f32::from(tiers) * 0.3 * lift + 0.2 * lift;
        parts.push(Some(
            Transform::from_translation(at + Vec3::Y * hover).with_scale(Vec3::new(
                1.0,
                1.0 + 0.4 * lift,
                1.0,
            )),
        ));
        let crown = Quat::from_rotation_y(crown_yaw);
        parts.push(Some(
            Transform::from_translation(at + Vec3::Y * crown_y).with_rotation(crown),
        ));
        // The eye sits in the crown's front face, looking out and slightly down.
        let eye_centre = at + Vec3::Y * (crown_y + 0.24) + crown * (Vec3::Z * 0.44);
        let look = crown * Quat::from_rotation_x(0.12);
        parts.extend(eye_transforms(eye_centre, look, EYE).map(Some));
        parts.push((state == State::Hunting).then(|| {
            Transform::from_translation(eye_centre)
                .with_rotation(look * Quat::from_rotation_x(0.05))
        }));
        parts.push((state == State::FrozenByAnchor).then(|| {
            let drop = 1.0 - ease_out(t / 0.3);
            Transform::from_translation(at + Vec3::Y * (hover + 0.16 + drop * 1.6))
        }));
        parts.push((state == State::Catch).then(|| stamp(at, t)).flatten());
        Pose {
            parts,
            seams_dark: state.frozen() && t > 0.1,
            flare,
        }
    }
}

mod plumb {
    use super::*;

    /// Height of the body, point to rim.
    const HEIGHT: f32 = 2.4;
    const RIM: f32 = 1.05;
    const HOVER: f32 = 0.35;
    const RINGS: [(f32, f32, f32); 3] = [(1.3, 0.55, 0.7), (1.5, -0.75, -0.45), (1.7, 0.35, 0.3)];
    const RING_HEIGHT: f32 = 1.6;
    const EYE: f32 = 0.2;
    const EYE_HEIGHT: f32 = 1.8;

    fn corner(i: usize) -> Vec3 {
        #[allow(clippy::cast_precision_loss)]
        let angle = PI / 6.0 + FRAC_PI_3 * i as f32;
        // Corners at 30 degrees either side of +Z put a face square to it, as the
        // hexagon meshes do.
        Vec3::new(angle.sin() * RIM, HEIGHT, angle.cos() * RIM)
    }

    /// Where the front face is, `y` up from the point.
    fn face_depth(y: f32) -> f32 {
        RIM * (PI / 6.0).cos() * y / HEIGHT
    }

    /// Parts: six panels, the cap and its rim, the core, the crown, eye and pupil,
    /// three rings, the beam, the clamp and the stamp.
    pub(super) fn parts() -> Vec<Part> {
        let mut parts: Vec<Part> = (0..6)
            .map(|i| {
                let (a, b) = (corner(i), corner((i + 1) % 6));
                let centroid = (a + b) / 3.0;
                // Drawn a little shy of its edges, so the core shows along the seams.
                let shrink = |p: Vec3| centroid + (p - centroid) * 0.95;
                Part {
                    shape: Shape::Panel {
                        corners: [shrink(Vec3::ZERO), shrink(a), shrink(b)],
                        inward: -Vec3::new(centroid.x, 0.0, centroid.z).normalize(),
                        thickness: 0.07,
                    },
                    look: Look::Shell,
                }
            })
            .collect();
        parts.push(Part {
            shape: Shape::HexFrustum {
                bottom: RIM + 0.02,
                top: RIM - 0.06,
                height: 0.16,
            },
            look: Look::Shell,
        });
        parts.push(Part {
            shape: Shape::HexRing {
                outer: RIM - 0.04,
                inner: RIM - 0.2,
                height: 0.03,
            },
            look: Look::Trim,
        });
        parts.push(Part {
            shape: Shape::Sphere { radius: 0.42 },
            look: Look::Seam,
        });
        parts.push(Part {
            shape: Shape::HexFrustum {
                bottom: 0.42,
                top: 0.16,
                height: 0.34,
            },
            look: Look::Trim,
        });
        parts.extend(eye_parts(EYE));
        for (major, _, _) in RINGS {
            parts.push(Part {
                shape: Shape::Torus {
                    major,
                    minor: 0.032,
                },
                look: Look::Trim,
            });
        }
        parts.push(beam_part());
        parts.push(Part {
            shape: Shape::Torus {
                major: face_depth(1.1) / (PI / 6.0).cos() + 0.06,
                minor: 0.075,
            },
            look: Look::Clamp,
        });
        parts.push(stamp_part());
        parts
    }

    pub(super) fn pose(state: State, t: f32, clock: f32, stage: Stage) -> Pose {
        let since = clock - t;
        let face = yaw_to(stage.at, stage.toward);
        let hunt_yaw = |c: f32| face + 0.5 * c;
        let hunt_sway = |c: f32| {
            Quat::from_rotation_x(0.06 * (1.1 * c).sin())
                * Quat::from_rotation_z(0.05 * (0.9 * c).sin())
        };
        let hunt_hover = |c: f32| HOVER + 0.04 * (1.3 * c).sin();
        // Frozen: the rings fall flat, it turns its eye on whoever froze it, and it sets
        // its point on the floor.
        let settle = if state.frozen() {
            ease_out(t / 0.3)
        } else {
            0.0
        };
        let (yaw, sway, hover) = match state {
            State::Hunting => (hunt_yaw(clock), hunt_sway(clock), hunt_hover(clock)),
            State::FrozenBySight | State::FrozenByAnchor => {
                let from = hunt_yaw(since);
                let to = nearest_turn(from, face);
                (
                    from + (to - from) * settle,
                    hunt_sway(since).slerp(Quat::IDENTITY, settle),
                    hunt_hover(since) * (1.0 - settle),
                )
            }
            State::Catch => (hunt_yaw(since) + 2.5 * t, Quat::IDENTITY, hunt_hover(since)),
        };
        let open = if state == State::Catch {
            ease_out(t / 0.5)
        } else {
            0.0
        };
        let body = Transform::from_translation(stage.at + Vec3::Y * hover)
            .with_rotation(Quat::from_rotation_y(yaw) * sway);
        let mut parts = Vec::new();
        for i in 0..6 {
            // Each panel hinges on its top edge, its point swinging out.
            let (a, b) = (corner(i), corner((i + 1) % 6));
            let hinge = (a + b) * 0.5;
            let turn = Quat::from_axis_angle((b - a).normalize(), -open);
            let local = Transform::from_translation(hinge - turn * hinge).with_rotation(turn);
            parts.push(Some(body * local));
        }
        parts.push(Some(body * Transform::from_xyz(0.0, HEIGHT, 0.0)));
        parts.push(Some(body * Transform::from_xyz(0.0, HEIGHT + 0.15, 0.0)));
        parts.push(Some(
            body * Transform::from_xyz(0.0, 1.45, 0.0).with_scale(Vec3::splat(1.0 + 0.5 * open)),
        ));
        parts.push(Some(body * Transform::from_xyz(0.0, HEIGHT + 0.16, 0.0)));
        let eye_local = Vec3::new(0.0, EYE_HEIGHT, face_depth(EYE_HEIGHT) + 0.02);
        let eye_centre = body.transform_point(eye_local);
        let look = body.rotation * Quat::from_rotation_x(0.1);
        parts.extend(eye_transforms(eye_centre, look, EYE).map(Some));
        for (k, (_, tilt, rate)) in RINGS.into_iter().enumerate() {
            let hunt = |c: f32| {
                Quat::from_rotation_y(rate * c)
                    * Quat::from_rotation_x(tilt + 0.15 * (0.7 * c).sin())
            };
            let ring = match state {
                State::Hunting => hunt(clock),
                State::FrozenBySight | State::FrozenByAnchor => {
                    let from = hunt(since);
                    let flat = Quat::from_rotation_y(snapped(rate * since, FRAC_PI_3));
                    from.slerp(flat, ease_out_back(t / 0.2))
                }
                State::Catch => hunt(since + 3.0 * t),
            };
            #[allow(clippy::cast_precision_loss)]
            let stack = if state.frozen() {
                settle * (k as f32 * 0.07 - 0.07)
            } else {
                0.0
            };
            parts.push(Some(
                Transform::from_translation(stage.at + Vec3::Y * (hover + RING_HEIGHT + stack))
                    .with_rotation(ring)
                    .with_scale(Vec3::splat(1.0 + 0.3 * open)),
            ));
        }
        parts.push((state == State::Hunting).then(|| {
            Transform::from_translation(eye_centre)
                .with_rotation(look * Quat::from_rotation_x(0.06))
        }));
        parts.push((state == State::FrozenByAnchor).then(|| {
            let drop = 1.0 - ease_out(t / 0.3);
            Transform::from_translation(stage.at + Vec3::Y * (1.1 + drop * 1.6))
        }));
        parts.push(
            (state == State::Catch)
                .then(|| stamp(stage.at, t))
                .flatten(),
        );
        Pose {
            parts,
            seams_dark: state.frozen() && t > 0.1,
            flare: if state == State::Catch {
                (1.0 - ((t - 0.9) / 0.9).clamp(0.0, 1.0)) * open
            } else {
                0.0
            },
        }
    }
}

mod roller {
    use super::*;

    const STEP_SECONDS: f32 = ROLLER_STEP_SECONDS;
    const STEPS: usize = 3;
    const EYE: f32 = 0.42;

    /// The eight faces, each as its three vertices in the body frame.
    fn faces() -> Vec<[Vec3; 3]> {
        let mut faces = Vec::new();
        for sx in [1.0, -1.0] {
            for sy in [1.0, -1.0] {
                for sz in [1.0, -1.0] {
                    faces.push([
                        Vec3::X * sx * VERTEX,
                        Vec3::Y * sy * VERTEX,
                        Vec3::Z * sz * VERTEX,
                    ]);
                }
            }
        }
        faces
    }

    fn edges() -> Vec<(Vec3, Vec3)> {
        let v = roll::body_vertices();
        let mut edges = Vec::new();
        for i in 0..6 {
            for j in (i + 1)..6 {
                // Adjacent unless opposite.
                if v[i].dot(v[j]).abs() < 1e-3 {
                    edges.push((v[i], v[j]));
                }
            }
        }
        edges
    }

    /// Parts: eight panels, twelve struts, the eye and pupil, the beam, the clamp and
    /// the stamp.
    pub(super) fn parts() -> Vec<Part> {
        let mut parts: Vec<Part> = faces()
            .into_iter()
            .map(|[a, b, c]| {
                let centroid = (a + b + c) / 3.0;
                // Open windows at the edges, so the level eye inside reads through the
                // cage as it tumbles.
                let shrink = |p: Vec3| centroid + (p - centroid) * 0.62;
                Part {
                    shape: Shape::Panel {
                        corners: [shrink(a), shrink(b), shrink(c)],
                        inward: -centroid.normalize(),
                        thickness: 0.08,
                    },
                    look: Look::Shell,
                }
            })
            .collect();
        for (a, b) in edges() {
            parts.push(Part {
                shape: Shape::Rod {
                    radius: 0.04,
                    length: a.distance(b),
                },
                look: Look::Trim,
            });
        }
        parts.extend(eye_parts(EYE));
        parts.push(beam_part());
        parts.push(Part {
            shape: Shape::Torus {
                major: 0.95,
                minor: 0.075,
            },
            look: Look::Clamp,
        });
        parts.push(stamp_part());
        parts
    }

    fn walk(stage: Stage) -> Vec<Roll> {
        let along = Vec3::X;
        // Centred on the stage: start half a walk back.
        let start = Rest::on_a_face(stage.at);
        let probe = roll::out_and_back(start, along, STEPS);
        let far = probe[STEPS - 1].at(1.0).centre.x - start.centre.x;
        roll::out_and_back(Rest::on_a_face(stage.at - along * far * 0.5), along, STEPS)
    }

    /// Where a hunting body is at `clock`: in a roll, lifting slowly and falling fast.
    fn hunting(walk: &[Roll], clock: f32) -> Rest {
        let steps = walk.len();
        let cycle = clock.rem_euclid(STEP_SECONDS * steps as f32);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let index = ((cycle / STEP_SECONDS) as usize).min(steps - 1);
        let u = cycle / STEP_SECONDS - index as f32;
        // A pause on the face, then the tip, then the fall.
        let u = ((u - 0.25) / 0.75).clamp(0.0, 1.0);
        walk[index].at(u * u)
    }

    /// The rest a hunting body was nearest when something happened to it at `clock`.
    fn landed(walk: &[Roll], clock: f32) -> Rest {
        let steps = walk.len();
        let cycle = clock.rem_euclid(STEP_SECONDS * steps as f32);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let index = ((cycle / STEP_SECONDS) as usize).min(steps - 1);
        let u = cycle / STEP_SECONDS - index as f32;
        if u < 0.6 {
            walk[index].from
        } else {
            walk[index].at(1.0)
        }
    }

    pub(super) fn pose(state: State, t: f32, clock: f32, stage: Stage) -> Pose {
        let walk = walk(stage);
        let since = clock - t;
        let (body, lift, flare) = match state {
            State::Hunting => (hunting(&walk, clock), 0.0, 0.0),
            State::FrozenBySight | State::FrozenByAnchor => {
                let from = hunting(&walk, since);
                let to = roll::on_a_vertex(landed(&walk, since));
                (roll::tip_between(from, to, ease_out(t / 0.35)), 0.0, 0.0)
            }
            State::Catch => {
                let from = hunting(&walk, since);
                let to = roll::on_a_vertex(landed(&walk, since));
                let open = ease_out((t - 0.2) / 0.45);
                (
                    roll::tip_between(from, to, ease_out(t / 0.2)),
                    open,
                    (1.0 - ((t - 0.9) / 0.9).clamp(0.0, 1.0)) * open,
                )
            }
        };
        let frame = Transform::from_translation(body.centre).with_rotation(body.rotation);
        // In a catch the half above the equator lifts off and turns.
        let up = body.rotation.inverse() * Vec3::Y;
        let opened = |local: Vec3| -> Transform {
            if local.dot(up) > 1e-3 {
                Transform::from_translation(Vec3::Y * 0.6 * lift)
                    * Transform::from_translation(body.centre)
                    * Transform::from_rotation(Quat::from_rotation_y(FRAC_PI_3 * 0.75 * lift))
                    * Transform::from_translation(-body.centre)
            } else {
                Transform::IDENTITY
            }
        };
        let mut parts = Vec::new();
        for [a, b, c] in faces() {
            parts.push(Some(opened((a + b + c) / 3.0) * frame));
        }
        for (a, b) in edges() {
            let local = Transform::from_translation(a)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, (b - a).normalize()));
            parts.push(Some(opened((a + b) * 0.5) * frame * local));
        }
        // The eye stays level and keeps its target, whatever the cage is doing, and
        // while it hunts it scans, so it is never still even between rolls.
        let scan = if state == State::Hunting {
            0.3 * (0.8 * clock).sin()
        } else {
            0.0
        };
        let look = Quat::from_rotation_y(yaw_to(body.centre, stage.toward) + scan);
        parts.extend(eye_transforms(body.centre, look, EYE).map(Some));
        parts.push((state == State::Hunting).then(|| {
            Transform::from_translation(body.centre + look * Vec3::Z * EYE)
                .with_rotation(look * Quat::from_rotation_x(0.08))
        }));
        parts.push((state == State::FrozenByAnchor).then(|| {
            let drop = 1.0 - ease_out(t / 0.3);
            Transform::from_translation(Vec3::new(
                body.centre.x,
                VERTEX + drop * 1.6,
                body.centre.z,
            ))
        }));
        parts.push(
            (state == State::Catch)
                .then(|| stamp(body.centre, t))
                .flatten(),
        );
        Pose {
            parts,
            seams_dark: state.frozen() && t > 0.1,
            flare,
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec3;

    use super::{Form, Pose, Stage, State, parts, pose};

    const STAGE: Stage = Stage {
        at: Vec3::ZERO,
        toward: Vec3::new(1.0, 1.6, 7.0),
    };

    fn forms() -> Vec<Form> {
        vec![
            Form::Tumbler { tiers: 3 },
            Form::Tumbler { tiers: 4 },
            Form::Tumbler { tiers: 5 },
            Form::Plumb,
            Form::Roller,
        ]
    }

    #[test]
    fn every_pose_places_every_part() {
        for form in forms() {
            let count = parts(form).len();
            for state in State::ALL {
                assert_eq!(
                    pose(form, state, 0.5, 3.0, STAGE).parts.len(),
                    count,
                    "{form:?}"
                );
            }
        }
    }

    fn moved(a: &Pose, b: &Pose) -> f32 {
        a.parts
            .iter()
            .zip(&b.parts)
            .filter_map(|(a, b)| Some((a.as_ref()?, b.as_ref()?)))
            .map(|(a, b)| {
                // Quaternion components rather than `angle_between`, whose acos cannot
                // tell identical rotations apart from ones 0.0007 rad away.
                let (qa, qb) = (
                    bevy::math::Vec4::from(a.rotation),
                    bevy::math::Vec4::from(b.rotation),
                );
                a.translation.distance(b.translation) + (qa - qb).length().min((qa + qb).length())
            })
            .fold(0.0, f32::max)
    }

    /// Frozen means perfectly still, whether sight or an anchor holds it.
    #[test]
    fn a_frozen_guardian_does_not_move() {
        for form in forms() {
            for state in [State::FrozenBySight, State::FrozenByAnchor] {
                let a = pose(form, state, 1.0, 5.0, STAGE);
                let b = pose(form, state, 4.0, 8.0, STAGE);
                assert!(
                    moved(&a, &b) < 1e-4,
                    "{form:?} {state:?} moved {}",
                    moved(&a, &b)
                );
            }
        }
    }

    /// And a hunting one never is: something always turns.
    #[test]
    fn a_hunting_guardian_is_always_moving() {
        for form in forms() {
            for step in 0..40 {
                #[allow(clippy::cast_precision_loss)]
                let clock = step as f32 * 0.37;
                let a = pose(form, State::Hunting, clock, clock, STAGE);
                let b = pose(form, State::Hunting, clock + 0.05, clock + 0.05, STAGE);
                assert!(moved(&a, &b) > 1e-3, "{form:?} still at {clock}");
            }
        }
    }

    /// The seams go dark once frozen, and only then.
    #[test]
    fn frozen_seams_are_dark() {
        for form in forms() {
            assert!(!pose(form, State::Hunting, 1.0, 1.0, STAGE).seams_dark);
            assert!(pose(form, State::FrozenBySight, 1.0, 1.0, STAGE).seams_dark);
        }
    }

    /// Every part of every form has its origin inside the facility's doorway (4.5 m wide,
    /// 4 m clear) while it hunts or is frozen: origins, not full extents. A catch happens
    /// where its victim stands, and may rise past it.
    #[test]
    fn every_form_fits_a_doorway() {
        for form in forms() {
            for state in [State::Hunting, State::FrozenBySight, State::FrozenByAnchor] {
                for step in 0..30 {
                    #[allow(clippy::cast_precision_loss)]
                    let t = step as f32 * 0.07;
                    let posed = pose(form, state, t, 2.0 + t, STAGE);
                    for (part, at) in parts(form).iter().zip(&posed.parts) {
                        let Some(at) = at else { continue };
                        if matches!(
                            part.look,
                            super::Look::Beam | super::Look::Stamp | super::Look::Clamp
                        ) {
                            continue;
                        }
                        let p = at.translation;
                        assert!(p.y < 4.0, "{form:?} {state:?} part at {p}");
                        assert!(
                            (p - STAGE.at).with_y(0.0).length() < 2.25,
                            "{form:?} {state:?} part at {p}"
                        );
                    }
                }
            }
        }
    }
}
