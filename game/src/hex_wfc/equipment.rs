//! What hand equipment shares: procedural meshes (flat-shaded hexagonal prisms and
//! rings, and a light tube that fades as it rises), the slow spin of a live part, and
//! where a hand holds a device in front of the eye.
//!
//! Hexagonal because the facility is: a plate lying on the floor lines up with the
//! cell it lies in, and the lantern's cage repeats the same six sides in the hand.
//! Every hexagon here has its corners where the lattice's are, in the same order
//! (`observed_hex::CORNERS`, from -30 degrees in steps of 60). Every face is
//! flat-shaded, since a smoothed six-sided prism reads as a badly tessellated
//! cylinder.
use std::f32::consts::{FRAC_PI_3, FRAC_PI_6};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::sim::{EYE_OFFSET, HexWfcRuntime};

/// A part that turns slowly about a local axis: a live plate's inner ring, the
/// lantern's gyro. Presentation only, keyed to wall-clock time.
#[derive(Component)]
pub(super) struct Spin {
    pub(super) axis: Vec3,
    /// Radians per second.
    pub(super) rate: f32,
    pub(super) rest: Quat,
}

pub(super) fn spin(
    time: Res<Time>,
    settings: Res<crate::settings::Settings>,
    mut parts: Query<(&Spin, &mut Transform)>,
) {
    let t = if settings.reduced_hand_motion {
        0.0
    } else {
        time.elapsed_secs()
    };
    for (spin, mut transform) in &mut parts {
        transform.rotation = Quat::from_axis_angle(spin.axis, spin.rate * t) * spin.rest;
    }
}

/// How the local player's hands move with their body: a step's rise and fall and a
/// small side-to-side, in eye space, from how fast the body is actually travelling.
/// Presentation only: read from the simulation's positions, never written back.
#[derive(Resource, Default)]
pub(super) struct HeldSway {
    offset: Vec3,
    phase: f32,
    last: Option<Vec3>,
    speed: f32,
}

pub(super) fn setup(mut commands: Commands) {
    commands.init_resource::<HeldSway>();
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<HeldSway>();
}

/// Metres of travel per step cycle.
const STRIDE: f32 = 1.6;
/// Rise and side travel of a hand at a full walk, metres.
const BOB: Vec2 = Vec2::new(0.006, 0.009);
/// The walking speed at which the sway is full.
const WALK: f32 = 4.0;

pub(super) fn sway(
    time: Res<Time>,
    runtime: Res<HexWfcRuntime>,
    settings: Res<crate::settings::Settings>,
    mut sway: ResMut<HeldSway>,
) {
    if settings.reduced_hand_motion {
        *sway = HeldSway {
            last: Some(runtime.local().position),
            ..Default::default()
        };
        return;
    }
    let dt = time.delta_secs();
    let at = runtime.local().position;
    // Capped, so a teleport is not a thousand steps.
    let moved = sway
        .last
        .map_or(0.0, |last| Vec2::new(at.x - last.x, at.z - last.z).length())
        .min(0.5);
    sway.last = Some(at);
    if dt <= 0.0 {
        return;
    }
    // Eased, so a fixed-step position that advances on some frames and not others
    // does not stutter the hands.
    let speed = (moved / dt).min(WALK * 2.0);
    sway.speed += (speed - sway.speed) * (dt * 10.0).min(1.0);
    sway.phase += moved / STRIDE * std::f32::consts::TAU;
    let amount = (sway.speed / WALK).min(1.0);
    let breath = (time.elapsed_secs() * 1.3).sin() * 0.0015;
    sway.offset = Vec3::new(
        sway.phase.sin() * BOB.x * amount,
        -(sway.phase * 2.0).cos().abs() * BOB.y * amount + breath,
        0.0,
    );
}

/// Where a hand holds a device.
pub(super) struct Hand {
    /// Eye space: x right, y up, -z ahead.
    pub(super) offset: Vec3,
    /// Roll about the view axis, then tip about the eye's x axis, radians.
    pub(super) roll: f32,
    pub(super) tip: f32,
    pub(super) scale: f32,
}

/// Where `hand` holds its device for `player`, displaced by `sway` (the local player's
/// hands sway; anyone else's are passed [`Vec3::ZERO`]).
pub(super) fn held_transform(
    player: &observed_match::hex_wfc::HexPlayerState,
    sway: Vec3,
    hand: &Hand,
) -> Transform {
    let eye = Quat::from_rotation_y(-player.yaw) * Quat::from_rotation_x(player.pitch);
    Transform::from_translation(player.position + Vec3::Y * EYE_OFFSET + eye * (hand.offset + sway))
        .with_rotation(eye * Quat::from_rotation_z(hand.roll) * Quat::from_rotation_x(hand.tip))
        .with_scale(Vec3::splat(hand.scale))
}

/// The sway for `player`'s hands: the local player's, or none.
pub(super) fn sway_for(
    runtime: &HexWfcRuntime,
    sway: &HeldSway,
    player: &observed_match::hex_wfc::HexPlayerState,
) -> Vec3 {
    if player.id == runtime.local_player {
        sway.offset
    } else {
        Vec3::ZERO
    }
}

/// Corner `i` of a hexagon of circumradius `radius`, in plan.
fn corner(i: usize, radius: f32) -> Vec2 {
    #[allow(clippy::cast_precision_loss)]
    let angle = -FRAC_PI_6 + FRAC_PI_3 * (i % 6) as f32;
    Vec2::new(angle.cos(), angle.sin()) * radius
}

fn at(plan: Vec2, y: f32) -> Vec3 {
    Vec3::new(plan.x, y, plan.y)
}

#[derive(Default)]
struct Builder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Builder {
    /// A flat polygon, wound so that its normal is `normal`.
    fn face(&mut self, points: &[Vec3], normal: Vec3, color: impl Fn(Vec3) -> [f32; 4]) {
        let base = u32::try_from(self.positions.len()).expect("small mesh");
        let wound = (points[1] - points[0])
            .cross(points[2] - points[0])
            .dot(normal)
            >= 0.0;
        for &p in points {
            self.positions.push(p.to_array());
            self.normals.push(normal.to_array());
            self.colors.push(color(p));
        }
        let n = u32::try_from(points.len()).expect("small polygon");
        for k in 1..n - 1 {
            if wound {
                self.indices.extend([base, base + k, base + k + 1]);
            } else {
                self.indices.extend([base, base + k + 1, base + k]);
            }
        }
    }

    /// A side quad between two rings of corners, facing away from the axis (or toward
    /// it when `inward`).
    fn side(&mut self, lower: [Vec3; 2], upper: [Vec3; 2], inward: bool) {
        let quad = [lower[0], lower[1], upper[1], upper[0]];
        let mid = (lower[0] + lower[1]) * 0.5;
        let across = (lower[1] - lower[0]).cross(upper[0] - lower[0]).normalize();
        let out = Vec3::new(mid.x, 0.0, mid.z);
        let normal = if across.dot(out) >= 0.0 {
            across
        } else {
            -across
        };
        let normal = if inward { -normal } else { normal };
        self.face(&quad, normal, |_| [1.0; 4]);
    }

    fn mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// A hexagonal prism from `y0` to `y1`, circumradius `bottom` below and `top` above: a
/// frustum when they differ, which is how an edge is chamfered.
#[must_use]
pub fn hex_prism(bottom: f32, top: f32, y0: f32, y1: f32) -> Mesh {
    let mut b = Builder::default();
    let low: Vec<Vec3> = (0..6).map(|i| at(corner(i, bottom), y0)).collect();
    let high: Vec<Vec3> = (0..6).map(|i| at(corner(i, top), y1)).collect();
    b.face(&high, Vec3::Y, |_| [1.0; 4]);
    b.face(&low, -Vec3::Y, |_| [1.0; 4]);
    for i in 0..6 {
        let j = (i + 1) % 6;
        b.side([low[i], low[j]], [high[i], high[j]], false);
    }
    b.mesh()
}

/// A flat hexagonal ring from `y0` to `y1`, between circumradii `inner` and `outer`.
#[must_use]
pub fn hex_ring(outer: f32, inner: f32, y0: f32, y1: f32) -> Mesh {
    let mut b = Builder::default();
    for i in 0..6 {
        let j = (i + 1) % 6;
        let (oi, oj, ii, ij) = (
            corner(i, outer),
            corner(j, outer),
            corner(i, inner),
            corner(j, inner),
        );
        b.face(
            &[at(ii, y1), at(oi, y1), at(oj, y1), at(ij, y1)],
            Vec3::Y,
            |_| [1.0; 4],
        );
        b.face(
            &[at(ii, y0), at(oi, y0), at(oj, y0), at(ij, y0)],
            -Vec3::Y,
            |_| [1.0; 4],
        );
        b.side([at(oi, y0), at(oj, y0)], [at(oi, y1), at(oj, y1)], false);
        b.side([at(ii, y0), at(ij, y0)], [at(ii, y1), at(ij, y1)], true);
    }
    b.mesh()
}

/// An open hexagonal tube of light, `height` tall, whose vertex colour fades from
/// white at its foot to black at its top: drawn additively, it rises out of a plate
/// and dissolves. Both sides of every wall face out, so it reads from inside too.
#[must_use]
pub fn light_tube(radius: f32, height: f32) -> Mesh {
    let mut b = Builder::default();
    let fade = |p: Vec3| {
        let t = 1.0 - (p.y / height).clamp(0.0, 1.0);
        let t = t * t;
        [t, t, t, 1.0]
    };
    for i in 0..6 {
        let j = (i + 1) % 6;
        let quad = [
            at(corner(i, radius), 0.0),
            at(corner(j, radius), 0.0),
            at(corner(j, radius), height),
            at(corner(i, radius), height),
        ];
        let mid = (corner(i, radius) + corner(j, radius)).normalize();
        let normal = Vec3::new(mid.x, 0.0, mid.y);
        b.face(&quad, normal, fade);
        b.face(&quad, -normal, fade);
    }
    b.mesh()
}

#[cfg(test)]
mod tests {
    use bevy::mesh::{Indices, VertexAttributeValues};
    use bevy::prelude::*;

    use super::{corner, hex_prism, hex_ring, light_tube};

    fn triangles(mesh: &Mesh) -> Vec<(Vec3, Vec3)> {
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions")
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("normals")
        };
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("indices")
        };
        indices
            .chunks(3)
            .map(|t| {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(positions[i as usize]));
                ((b - a).cross(c - a), Vec3::from(normals[t[0] as usize]))
            })
            .collect()
    }

    /// Every triangle's winding agrees with its normal, so nothing is back-face culled
    /// from the side it should be seen from.
    #[test]
    fn every_face_is_wound_toward_its_normal() {
        for mesh in [
            hex_prism(0.8, 0.74, 0.0, 0.05),
            hex_ring(0.66, 0.58, 0.0, 0.055),
            light_tube(0.6, 1.6),
        ] {
            let tris = triangles(&mesh);
            assert!(!tris.is_empty());
            for (area, normal) in tris {
                assert!(area.normalize().dot(normal) > 0.99, "{area} vs {normal}");
            }
        }
    }

    /// A prism's side normals point away from its axis, and its caps up and down.
    #[test]
    fn a_prism_faces_outward() {
        let mesh = hex_prism(0.5, 0.4, -0.1, 0.1);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions")
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("normals")
        };
        for (p, n) in positions.iter().zip(normals) {
            let (p, n) = (Vec3::from(*p), Vec3::from(*n));
            assert!(p.dot(n) > 0.0, "{p} has normal {n}");
        }
    }

    /// A held device stays inside the body's own radius in plan, at any heading and
    /// across the pitches play uses, so standing against a wall never pushes it through.
    #[test]
    fn held_devices_stay_inside_the_body() {
        use observed_core::{PlayerId, TeamId};
        let radius = observed_traversal::FpsConfig::default()
            .radius
            .min(observed_traversal::FpsConfig::deliberate_rapier().radius);
        for (name, hand, [lo, hi]) in [
            (
                "lantern",
                &crate::hex_wfc::lantern::torch::HAND,
                crate::hex_wfc::lantern::torch::REACH,
            ),
            (
                "pad",
                &crate::hex_wfc::pad::HAND,
                crate::hex_wfc::pad::REACH,
            ),
        ] {
            let corners: Vec<Vec3> = (0..8)
                .map(|i| {
                    Vec3::new(
                        if i & 1 == 0 { lo.x } else { hi.x },
                        if i & 2 == 0 { lo.y } else { hi.y },
                        if i & 4 == 0 { lo.z } else { hi.z },
                    )
                })
                .collect();
            for yaw in [0.0_f32, 1.0, 2.5, -2.0] {
                for step in -9..=9 {
                    #[allow(clippy::cast_precision_loss)]
                    let pitch = step as f32 * 0.1;
                    let player = observed_match::hex_wfc::HexPlayerState {
                        id: PlayerId(0),
                        team: TeamId(0),
                        cell: observed_facility::hex_wfc::HexCoord {
                            q: 0,
                            r: 0,
                            level: 0,
                        },
                        position: Vec3::new(3.0, 1.0, -2.0),
                        yaw,
                        pitch,
                        escaped: false,
                    };
                    let held = super::held_transform(&player, Vec3::ZERO, hand);
                    for &corner in &corners {
                        let p = held.transform_point(corner) - player.position;
                        let plan = Vec2::new(p.x, p.z).length();
                        assert!(
                            plan < radius,
                            "{name} reaches {plan} m out at yaw {yaw} pitch {pitch}"
                        );
                    }
                }
            }
        }
    }

    /// The hexagons line up with the lattice's, corner for corner, so a plate's flats
    /// run parallel to the walls of the cell it lies in.
    #[test]
    fn hexagons_share_the_lattices_corners() {
        for (i, &(x, z)) in observed_hex::CORNERS.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let lattice = Vec2::new(x as f32, z as f32).normalize();
            assert!(corner(i, 1.0).dot(lattice) > 0.999, "corner {i}");
        }
    }
}
