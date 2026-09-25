//! How a regular octahedron rolls: face to face, pivoting over the edge of the face it
//! stands on, so that something with no legs still walks.
//!
//! An octahedron's dihedral angle is `acos(-1/3)`, about 109.47 degrees, so each step
//! turns it through the supplement, about 70.53 degrees, about the leading ground
//! edge. After the step the next face lies flat on the floor.
use bevy::math::{Quat, Vec3};

/// Centre to vertex, metres.
pub const VERTEX: f32 = 1.2;

/// One step's turn, radians: the supplement of the dihedral angle.
#[must_use]
pub fn step_angle() -> f32 {
    std::f32::consts::PI - (-1.0_f32 / 3.0).acos()
}

/// The six vertices in the body frame.
#[must_use]
pub fn body_vertices() -> [Vec3; 6] {
    [
        Vec3::X * VERTEX,
        -Vec3::X * VERTEX,
        Vec3::Y * VERTEX,
        -Vec3::Y * VERTEX,
        Vec3::Z * VERTEX,
        -Vec3::Z * VERTEX,
    ]
}

/// A body at rest: its orientation and where its centre is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rest {
    pub rotation: Quat,
    pub centre: Vec3,
}

impl Rest {
    /// Standing on a face, centred over `at` on the floor.
    #[must_use]
    pub fn on_a_face(at: Vec3) -> Self {
        let down = Vec3::new(-1.0, -1.0, -1.0).normalize();
        let rotation = Quat::from_rotation_arc(down, -Vec3::Y);
        Self {
            rotation,
            centre: Vec3::new(at.x, VERTEX / 3.0_f32.sqrt(), at.z),
        }
    }

    #[must_use]
    pub fn vertices(&self) -> [Vec3; 6] {
        body_vertices().map(|v| self.centre + self.rotation * v)
    }
}

/// One step: the ground edge it pivots over and the turn about it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Roll {
    pub from: Rest,
    pub pivot: Vec3,
    pub axis: Vec3,
}

impl Roll {
    /// Roll `from` toward `direction` (horizontal) over the ground edge that leads.
    #[must_use]
    pub fn toward(from: Rest, direction: Vec3) -> Self {
        let mut ground = from.vertices();
        ground.sort_by(|a, b| a.y.total_cmp(&b.y));
        let ground = [ground[0], ground[1], ground[2]];
        let (a, b) = [(0, 1), (1, 2), (0, 2)]
            .into_iter()
            .map(|(i, j)| (ground[i], ground[j]))
            .max_by(|x, y| {
                ((x.0 + x.1) * 0.5)
                    .dot(direction)
                    .total_cmp(&((y.0 + y.1) * 0.5).dot(direction))
            })
            .expect("a face has three edges");
        let pivot = (a + b) * 0.5;
        let edge = (b - a).normalize();
        // Turn the way that carries the centre toward `direction`.
        let axis = if edge.cross(from.centre - pivot).dot(direction) > 0.0 {
            edge
        } else {
            -edge
        };
        Self { from, pivot, axis }
    }

    /// Where the body is `u` of the way through the step (0 at rest, 1 landed).
    #[must_use]
    pub fn at(&self, u: f32) -> Rest {
        let turn = Quat::from_axis_angle(self.axis, step_angle() * u);
        Rest {
            rotation: turn * self.from.rotation,
            centre: self.pivot + turn * (self.from.centre - self.pivot),
        }
    }
}

/// The resting poses of a walk: `steps` rolls toward `direction` and back again, which
/// ends exactly where it began.
#[must_use]
pub fn out_and_back(start: Rest, direction: Vec3, steps: usize) -> Vec<Roll> {
    let mut rolls = Vec::with_capacity(steps * 2);
    let mut at = start;
    for i in 0..steps * 2 {
        let way = if i < steps { direction } else { -direction };
        let roll = Roll::toward(at, way);
        at = roll.at(1.0);
        rolls.push(roll);
    }
    rolls
}

/// Stood on a vertex, perfectly balanced: the pose no unfrozen body could hold. The
/// vertex is the one lowest in `from`, so tipping onto it is the shortest move.
#[must_use]
pub fn on_a_vertex(from: Rest) -> Rest {
    let lowest = body_vertices()
        .into_iter()
        .min_by(|a, b| (from.rotation * *a).y.total_cmp(&(from.rotation * *b).y))
        .expect("six vertices");
    let tip = Quat::from_rotation_arc((from.rotation * lowest).normalize(), -Vec3::Y);
    Rest {
        rotation: tip * from.rotation,
        centre: Vec3::new(from.centre.x, VERTEX, from.centre.z),
    }
}

/// Between two rests, turning and keeping the lowest vertex on the floor.
#[must_use]
pub fn tip_between(from: Rest, to: Rest, u: f32) -> Rest {
    let rotation = from.rotation.slerp(to.rotation, u);
    let lowest = body_vertices()
        .into_iter()
        .map(|v| (rotation * v).y)
        .fold(f32::MAX, f32::min);
    let plan = from.centre.lerp(to.centre, u);
    Rest {
        rotation,
        centre: Vec3::new(plan.x, -lowest, plan.z),
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec3;

    use super::{Rest, Roll, VERTEX, on_a_vertex, out_and_back, tip_between};

    fn lowest(rest: &Rest) -> f32 {
        rest.vertices()
            .into_iter()
            .map(|v| v.y)
            .fold(f32::MAX, f32::min)
    }

    fn on_floor(rest: &Rest) -> usize {
        rest.vertices()
            .into_iter()
            .filter(|v| v.y.abs() < 1e-4)
            .count()
    }

    #[test]
    fn it_starts_and_lands_flat_on_a_face() {
        let start = Rest::on_a_face(Vec3::ZERO);
        assert_eq!(on_floor(&start), 3);
        let landed = Roll::toward(start, Vec3::X).at(1.0);
        assert_eq!(on_floor(&landed), 3, "{:?}", landed.vertices());
    }

    #[test]
    fn it_never_sinks_or_lifts_off_mid_roll() {
        let mut at = Rest::on_a_face(Vec3::ZERO);
        for _ in 0..6 {
            let roll = Roll::toward(at, Vec3::X);
            for step in 0..=20 {
                #[allow(clippy::cast_precision_loss)]
                let low = lowest(&roll.at(step as f32 / 20.0));
                assert!(low.abs() < 1e-4, "lowest vertex at {low}");
            }
            at = roll.at(1.0);
        }
    }

    #[test]
    fn it_goes_the_way_it_is_sent_and_comes_back() {
        let start = Rest::on_a_face(Vec3::ZERO);
        let walk = out_and_back(start, Vec3::X, 3);
        for roll in &walk[..3] {
            assert!(roll.at(1.0).centre.x > roll.from.centre.x + 0.3);
        }
        let end = walk.last().expect("steps").at(1.0);
        assert!(end.centre.distance(start.centre) < 1e-3, "{end:?}");
    }

    #[test]
    fn frozen_it_stands_on_one_point() {
        let start = Roll::toward(Rest::on_a_face(Vec3::ZERO), Vec3::X).at(1.0);
        let balanced = on_a_vertex(start);
        assert_eq!(on_floor(&balanced), 1);
        assert!((balanced.centre.y - VERTEX).abs() < 1e-5);
        for step in 0..=10 {
            #[allow(clippy::cast_precision_loss)]
            let between = tip_between(start, balanced, step as f32 / 10.0);
            assert!(lowest(&between).abs() < 1e-4);
        }
    }
}
