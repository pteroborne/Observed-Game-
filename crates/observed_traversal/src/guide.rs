//! Module-local traversal annotations and their pure path math.

use glam::{Quat, Vec2, Vec3};

/// How far apart two non-adjacent stretches of one spine must stay, in metres.
/// A shade over the 0.76 m player capsule, so a body standing anywhere on the
/// climb is unambiguously on one stretch of it.
pub const STAIR_SPINE_MIN_SEPARATION: f32 = 0.9;

/// How much more a metre of height counts than a metre of floor when deciding
/// which stretch of a climb a body is on. See [`StairSpine::locate`].
const CLIMB_VERTICAL_WEIGHT: f32 = 3.0;

/// How close to a spine's terminal node counts as having got there, in the
/// weighted metric. Shorter than the flat stub at either end, so it can only
/// trigger on deck, and comfortably longer than the residual a walker leaves
/// when it stops steering at its target.
const ARRIVAL_RADIUS: f32 = 0.6;

/// The walkable line through a tile's vertical circulation, in tile-local world
/// metres, ordered from the bottom entry to the top exit.
///
/// This is the contract that lets vertical circulation be more than one shape.
/// Before it existed, the objective bot climbed by hardcoded rise thresholds and
/// named tread points calibrated against the single generated switchback — so
/// authoring a second stair tower would have produced geometry no bot could
/// walk, and even *fixing* the switchback desynchronised the steering from the
/// surface underneath it. A tower now ships the line through itself alongside
/// its brushes, and the two cannot drift because both are derived from the same
/// constants.
///
/// The nodes are a polyline, not a set of targets: a follower walks segment to
/// segment. The first node sits on the lower deck at the foot of the climb and
/// the last on the deck above, so the ends join the flat circulation either
/// side without a special case.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StairSpine {
    pub nodes: Vec<Vec3>,
}

impl StairSpine {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.len() < 2
    }

    /// The spine rotated by `turn` sixths about the tile centre, matching how
    /// hulls and lights rotate.
    #[must_use]
    pub fn rotated(&self, turn: u8) -> Self {
        let rotation = Quat::from_rotation_y(-f32::from(turn) * std::f32::consts::TAU / 6.0);
        Self {
            nodes: self.nodes.iter().map(|&node| rotation * node).collect(),
        }
    }

    /// The index of the segment `point` is nearest, and how far along it the
    /// body has got (0 at the segment's start, 1 at its end).
    ///
    /// Nearest *segment*, deliberately, not nearest node. Selecting a target by
    /// proximity to a waypoint is not monotonic along a path — walking away
    /// from a landing onto the next flight grows the distance back past any
    /// threshold, so the target flips and the body spins on the spot. Distance
    /// to a segment falls and then rises exactly once as you walk it, so the
    /// choice advances with the body and never oscillates. That property is
    /// what [`Self::self_crossing`] exists to protect.
    ///
    /// Nearness is measured with height counting for
    /// [`CLIMB_VERTICAL_WEIGHT`] times as much as lateral distance, because a
    /// body stands *on* a surface: a metre sideways is a step, a metre up is a
    /// wall. Measured plainly, a body on the deck of a switchback comes out
    /// nearest to the flight passing overhead — 2.8 m away through the ceiling
    /// versus 3.8 m along the floor — so it would be steered into the underside
    /// of its own staircase. That is not a hypothetical; it stalled all four
    /// soak bots on the first run of this code.
    #[must_use]
    pub fn locate(&self, point: Vec3) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32, f32)> = None;
        for index in 0..self.nodes.len().saturating_sub(1) {
            let (distance, t) = segment_distance(
                climb_metric(self.nodes[index]),
                climb_metric(self.nodes[index + 1]),
                climb_metric(point),
            );
            if best.is_none_or(|(_, best_distance, _)| distance < best_distance) {
                best = Some((index, distance, t));
            }
        }
        best.map(|(index, _, t)| (index, t))
    }

    /// The point a body at `point` should walk toward, climbing when `up`.
    ///
    /// Always the far end of the segment the body is on, so the target sits on
    /// the surface underfoot rather than across a void.
    #[must_use]
    pub fn target(&self, point: Vec3, up: bool) -> Option<Vec3> {
        let (index, _) = self.locate(point)?;
        Some(if up {
            self.nodes[index + 1]
        } else {
            self.nodes[index]
        })
    }

    /// Whether a body at `point` has finished climbing: it has reached the last
    /// node, which by contract stands on the deck above.
    ///
    /// A follower needs this as an explicit end, not an inference. The last
    /// stretch of a spine lies flat on the deck it arrives at, so "am I still
    /// on the climb?" cannot be answered by height — a body standing exactly on
    /// the exit node is at deck level and on the line, and without this test it
    /// gets steered toward the point it is already standing on, forever.
    #[must_use]
    pub fn has_arrived(&self, point: Vec3) -> bool {
        self.nodes
            .last()
            .is_some_and(|node| climb_metric(*node).distance(climb_metric(point)) <= ARRIVAL_RADIUS)
    }

    /// The mirror of [`Self::has_arrived`] for a body coming down: it has
    /// reached the first node, on the deck the climb rises from.
    #[must_use]
    pub fn has_descended(&self, point: Vec3) -> bool {
        self.nodes
            .first()
            .is_some_and(|node| climb_metric(*node).distance(climb_metric(point)) <= ARRIVAL_RADIUS)
    }

    /// How far `point` is from the line, or `None` for an empty spine.
    #[must_use]
    pub fn distance(&self, point: Vec3) -> Option<f32> {
        let (index, t) = self.locate(point)?;
        Some(
            self.nodes[index]
                .lerp(self.nodes[index + 1], t)
                .distance(point),
        )
    }

    /// The closest approach between two stretches of the spine that are not
    /// neighbours, if that is nearer than [`STAIR_SPINE_MIN_SEPARATION`].
    #[must_use]
    pub fn self_crossing(&self) -> Option<(usize, usize, f32)> {
        let count = self.nodes.len().saturating_sub(1);
        for first in 0..count {
            for second in first + 2..count {
                let separation = segment_separation(
                    climb_metric(self.nodes[first]),
                    climb_metric(self.nodes[first + 1]),
                    climb_metric(self.nodes[second]),
                    climb_metric(self.nodes[second + 1]),
                );
                if separation < STAIR_SPINE_MIN_SEPARATION {
                    return Some((first, second, separation));
                }
            }
        }
        None
    }
}

/// Distance from `point` to segment `a`..`b`, measured in plan only.
fn plan_segment_distance(a: Vec3, b: Vec3, point: Vec3) -> f32 {
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    segment_distance(flat(a), flat(b), flat(point)).0
}

/// Rescale so height dominates lateral distance when judging which stretch of a
/// climb a body is standing on. See [`StairSpine::locate`].
fn climb_metric(point: Vec3) -> Vec3 {
    Vec3::new(point.x, point.y * CLIMB_VERTICAL_WEIGHT, point.z)
}

/// Distance from `point` to segment `a`..`b`, with the parameter along it.
fn segment_distance(a: Vec3, b: Vec3, point: Vec3) -> (f32, f32) {
    let span = b - a;
    let length_squared = span.length_squared();
    let t = if length_squared < 1.0e-6 {
        0.0
    } else {
        ((point - a).dot(span) / length_squared).clamp(0.0, 1.0)
    };
    ((a + span * t).distance(point), t)
}

/// Closest approach between two segments, sampled densely enough that a
/// crossing cannot slip between the samples at the tolerances involved.
fn segment_separation(a0: Vec3, a1: Vec3, b0: Vec3, b1: Vec3) -> f32 {
    const SAMPLES: usize = 32;
    let mut closest = f32::MAX;
    for step in 0..=SAMPLES {
        let point = a0.lerp(a1, step as f32 / SAMPLES as f32);
        closest = closest.min(segment_distance(b0, b1, point).0);
    }
    closest
}

/// The walkable line around a tile's floor, in tile-local world metres.
///
/// A stair tower's deck is not a disc: the stairwell is a hole in it, and the
/// flights overhang parts of what is left. A body that steers straight at a
/// door on the far side walks into the void or into the underside of a flight.
/// That is bug backlog #19, and it is not fixable by tuning, because there is
/// no line the tower can be assumed to have — every tower shape puts its hole
/// somewhere else.
///
/// So the tower says where its floor goes. The nodes are an ordered open path,
/// not a closed ring: the switchback's deck is a C, with the stairwell cutting
/// the west side, and pretending otherwise would route bodies through a wall.
/// Followers step one node at a time toward the node nearest their goal, so
/// consecutive nodes must be joined by walkable floor — that is the whole
/// contract.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeckPath {
    pub nodes: Vec<Vec3>,
}

impl DeckPath {
    /// How close a follower may come to a waypoint before it commits to the
    /// next leg.
    ///
    /// The follower is deliberately stateless, so a capsule negotiating a
    /// corner can otherwise keep locating itself on the leg it just walked and
    /// target the shared endpoint forever. A quarter metre is smaller than the
    /// production capsule radius and only cuts the inside of a turn by that
    /// amount; authored deck paths must already leave a whole body clear.
    const WAYPOINT_CAPTURE_RADIUS: f32 = 0.25;

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.len() < 2
    }

    #[must_use]
    pub fn rotated(&self, turn: u8) -> Self {
        let rotation = Quat::from_rotation_y(-f32::from(turn) * std::f32::consts::TAU / 6.0);
        Self {
            nodes: self.nodes.iter().map(|&node| rotation * node).collect(),
        }
    }

    /// The index of the segment nearest `point` in plan — height ignored, since
    /// a deck is one storey and a body on it stands a capsule above the nodes.
    ///
    /// Nearest segment rather than nearest node, for the same reason
    /// [`StairSpine::locate`] does it: a node-based choice flips between two
    /// opposite targets as the body crosses a boundary between them, and the
    /// body orbits that boundary instead of crossing it. Segment distance falls
    /// and rises once as you walk a leg, so the choice advances with the body.
    #[must_use]
    fn locate(&self, point: Vec3) -> Option<usize> {
        (0..self.nodes.len().saturating_sub(1)).min_by(|&a, &b| {
            let to = |index: usize| {
                plan_segment_distance(self.nodes[index], self.nodes[index + 1], point)
            };
            to(a).total_cmp(&to(b))
        })
    }

    /// The next point a body at `from` should walk toward to reach `goal` over
    /// the floor: along the leg it is on, then leg by leg, then `goal` itself.
    ///
    /// One leg at a time, deliberately. Handing over a distant node lets the
    /// body cut the corner, and the corners are where the stairwell is.
    #[must_use]
    pub fn step_toward(&self, from: Vec3, goal: Vec3) -> Option<Vec3> {
        let here = self.locate(from)?;
        let there = self.locate(goal)?;
        Some(match here.cmp(&there) {
            std::cmp::Ordering::Less => {
                let waypoint = here + 1;
                if plan_distance(from, self.nodes[waypoint]) <= Self::WAYPOINT_CAPTURE_RADIUS {
                    if waypoint >= there {
                        goal
                    } else {
                        self.nodes[waypoint + 1]
                    }
                } else {
                    self.nodes[waypoint]
                }
            }
            std::cmp::Ordering::Greater => {
                let waypoint = here;
                if plan_distance(from, self.nodes[waypoint]) <= Self::WAYPOINT_CAPTURE_RADIUS {
                    if waypoint.saturating_sub(1) <= there {
                        goal
                    } else {
                        self.nodes[waypoint - 1]
                    }
                } else {
                    self.nodes[waypoint]
                }
            }
            std::cmp::Ordering::Equal => goal,
        })
    }
}

fn plan_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deck_paths_capture_a_reached_corner_in_both_directions() {
        let path = DeckPath {
            nodes: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 2.0),
                Vec3::new(3.0, 0.0, 2.0),
            ],
        };

        assert_eq!(
            path.step_toward(Vec3::new(0.85, 0.0, 0.05), Vec3::new(3.0, 0.0, 2.0)),
            Some(Vec3::new(1.0, 0.0, 2.0))
        );
        assert_eq!(
            path.step_toward(Vec3::new(1.15, 0.0, 1.95), Vec3::ZERO),
            Some(Vec3::new(1.0, 0.0, 0.0))
        );
    }
}
