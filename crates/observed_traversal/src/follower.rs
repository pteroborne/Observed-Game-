//! Stateless steering over the compatibility spine and deck annotations.
//!
//! The facility planner chooses which cell-to-cell transition is wanted. This
//! module owns only the local, shape-aware decision that turns one selected
//! annotation into the same [`PlayerIntent`] used by human and network inputs.

use std::f32::consts::PI;

use glam::{Vec2, Vec3};
use player_input::PlayerIntent;

use crate::guide::{DeckPath, StairSpine};

/// The exact tuning of the pre-extraction hex bot follower.
///
/// `max_turn_per_tick` is currently an applied yaw delta because the hex match
/// deliberately sets its controller `look_step` to `1.0`. The canonical
/// profile work will make that relationship explicit; this compatibility move
/// intentionally does not change it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FollowerConfig {
    pub max_turn_per_tick: f32,
    pub climb_capture_radius: f32,
    pub movement_scale: f32,
}

impl Default for FollowerConfig {
    fn default() -> Self {
        Self {
            max_turn_per_tick: 0.08,
            climb_capture_radius: 1.6,
            movement_scale: 0.35,
        }
    }
}

/// Direction through a spine, whose nodes are authored bottom-to-top.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraversalDirection {
    Forward,
    Reverse,
}

impl TraversalDirection {
    fn is_forward(self) -> bool {
        self == Self::Forward
    }
}

/// The physical pose the local follower reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FollowerPose {
    pub feet: Vec3,
    pub yaw: f32,
}

/// A deck exit where the follower stops consulting the current tile after it
/// crosses the aperture plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeckHandoff {
    pub threshold: Vec3,
    pub outward: Vec2,
    pub destination: Vec3,
}

/// One compatibility annotation selected by the match's facility route.
#[derive(Clone, Copy, Debug)]
pub enum FollowTarget<'a> {
    Climb {
        spine: &'a StairSpine,
        approach: Option<&'a DeckPath>,
        direction: TraversalDirection,
    },
    Deck {
        path: &'a DeckPath,
        goal: Vec3,
        handoff: Option<DeckHandoff>,
    },
}

/// Observable phase of one stateless local decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FollowState {
    Unavailable,
    ApproachingClimb,
    FollowingClimb,
    PassedClimbTerminal,
    FollowingDeck,
    PastDeckHandoff,
}

/// The selected local target and the exact abstract intent emitted for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FollowDecision {
    pub state: FollowState,
    pub target: Option<Vec3>,
    pub intent: Option<PlayerIntent>,
}

impl FollowDecision {
    #[must_use]
    pub fn is_on_unfinished_climb(self) -> bool {
        self.state == FollowState::FollowingClimb
    }

    fn unavailable() -> Self {
        Self {
            state: FollowState::Unavailable,
            target: None,
            intent: None,
        }
    }
}

/// Follow one selected compatibility spine or deck without retaining state.
///
/// Match code remains responsible for choosing the current/next logical cell
/// and for recovery. This function owns the geometry-dependent local policy:
/// approach a climb over its deck, follow its nearest segment, recognize a
/// passed terminal, walk deck corners, and hand off after an aperture.
#[must_use]
pub fn follow_stateless(
    pose: FollowerPose,
    target: FollowTarget<'_>,
    config: &FollowerConfig,
) -> FollowDecision {
    match target {
        FollowTarget::Climb {
            spine,
            approach,
            direction,
        } => follow_climb(pose, spine, approach, direction, config),
        FollowTarget::Deck {
            path,
            goal,
            handoff,
        } => follow_deck(pose, path, goal, handoff, config),
    }
}

fn follow_climb(
    pose: FollowerPose,
    spine: &StairSpine,
    approach: Option<&DeckPath>,
    direction: TraversalDirection,
    config: &FollowerConfig,
) -> FollowDecision {
    if spine.is_empty() {
        return FollowDecision::unavailable();
    }
    let forward = direction.is_forward();
    let passed_terminal = has_passed_climb_terminal(spine, pose.feet, forward);
    let off_climb = spine
        .distance(pose.feet)
        .is_some_and(|distance| distance > config.climb_capture_radius);
    let entry = if forward {
        spine.nodes.first().copied()
    } else {
        spine.nodes.last().copied()
    };
    let target = if off_climb {
        approach
            .filter(|deck| !deck.is_empty())
            .and_then(|deck| entry.and_then(|entry| deck.step_toward(pose.feet, entry)))
            .or_else(|| spine.target(pose.feet, forward))
    } else {
        spine.target(pose.feet, forward)
    };
    let Some(target) = target else {
        return FollowDecision::unavailable();
    };
    let state = if passed_terminal {
        FollowState::PassedClimbTerminal
    } else if off_climb {
        FollowState::ApproachingClimb
    } else {
        FollowState::FollowingClimb
    };
    FollowDecision {
        state,
        target: Some(target),
        intent: Some(walk_toward(pose, target, config)),
    }
}

fn follow_deck(
    pose: FollowerPose,
    path: &DeckPath,
    goal: Vec3,
    handoff: Option<DeckHandoff>,
    config: &FollowerConfig,
) -> FollowDecision {
    if let Some(handoff) = handoff
        && Vec2::new(
            pose.feet.x - handoff.threshold.x,
            pose.feet.z - handoff.threshold.z,
        )
        .dot(handoff.outward)
            > 0.0
    {
        return FollowDecision {
            state: FollowState::PastDeckHandoff,
            target: Some(handoff.destination),
            intent: Some(walk_toward(pose, handoff.destination, config)),
        };
    }
    let Some(target) = path.step_toward(pose.feet, goal) else {
        return FollowDecision::unavailable();
    };
    FollowDecision {
        state: FollowState::FollowingDeck,
        target: Some(target),
        intent: Some(walk_toward(pose, target, config)),
    }
}

/// Whether a body has reached or moved beyond the terminal end of a climb.
///
/// The radius check recognizes arrival. The segment parameter makes that
/// recognition directional and therefore sticky without stored follower state:
/// after walking off the terminal node onto the deck, the nearest point on the
/// final segment remains its clamped endpoint.
fn has_passed_climb_terminal(spine: &StairSpine, point: Vec3, forward: bool) -> bool {
    if forward {
        spine.has_arrived(point)
            || spine
                .locate(point)
                .is_some_and(|(index, t)| index + 2 == spine.nodes.len() && t >= 1.0 - f32::EPSILON)
    } else {
        spine.has_descended(point)
            || spine
                .locate(point)
                .is_some_and(|(index, t)| index == 0 && t <= f32::EPSILON)
    }
}

fn walk_toward(pose: FollowerPose, target: Vec3, config: &FollowerConfig) -> PlayerIntent {
    let direction = target - pose.feet;
    let desired_yaw = direction.x.atan2(-direction.z);
    let look = wrap_angle(desired_yaw - pose.yaw)
        .clamp(-config.max_turn_per_tick, config.max_turn_per_tick);
    PlayerIntent {
        movement: Vec2::Y * config.movement_scale,
        look: Vec2::new(look, 0.0),
        ..PlayerIntent::default()
    }
}

fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(PI * 2.0) - PI
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spine() -> StairSpine {
        StairSpine {
            nodes: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 1.0),
                Vec3::new(0.0, 2.0, 2.0),
            ],
        }
    }

    #[test]
    fn passing_a_climb_endpoint_keeps_the_handoff_committed() {
        let spine = spine();
        let config = FollowerConfig::default();
        for (feet, direction) in [
            (Vec3::new(0.0, 2.0, 2.7), TraversalDirection::Forward),
            (Vec3::new(0.0, 0.0, -0.7), TraversalDirection::Reverse),
        ] {
            let decision = follow_stateless(
                FollowerPose { feet, yaw: 0.0 },
                FollowTarget::Climb {
                    spine: &spine,
                    approach: None,
                    direction,
                },
                &config,
            );
            assert_eq!(decision.state, FollowState::PassedClimbTerminal);
            assert!(!decision.is_on_unfinished_climb());
            assert!(decision.intent.is_some());
        }

        let decision = follow_stateless(
            FollowerPose {
                feet: Vec3::new(0.0, 1.0, 1.0),
                yaw: 0.0,
            },
            FollowTarget::Climb {
                spine: &spine,
                approach: None,
                direction: TraversalDirection::Forward,
            },
            &config,
        );
        assert_eq!(decision.state, FollowState::FollowingClimb);
        assert!(decision.is_on_unfinished_climb());
    }

    #[test]
    fn climb_approach_and_deck_handoff_emit_the_compatibility_intent() {
        let spine = spine();
        let deck = DeckPath {
            nodes: vec![
                Vec3::new(-3.0, 0.0, 0.0),
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 0.0),
            ],
        };
        let config = FollowerConfig::default();
        let approach = follow_stateless(
            FollowerPose {
                feet: Vec3::new(-3.0, 0.0, 0.0),
                yaw: 0.0,
            },
            FollowTarget::Climb {
                spine: &spine,
                approach: Some(&deck),
                direction: TraversalDirection::Forward,
            },
            &config,
        );
        assert_eq!(approach.state, FollowState::ApproachingClimb);
        assert_eq!(approach.target, Some(Vec3::new(-1.0, 0.0, 0.0)));
        assert_eq!(
            approach.intent.expect("approach intent").movement,
            Vec2::Y * 0.35
        );

        let handoff = follow_stateless(
            FollowerPose {
                feet: Vec3::new(3.1, 0.0, 0.0),
                yaw: 0.0,
            },
            FollowTarget::Deck {
                path: &deck,
                goal: Vec3::new(3.0, 0.0, 0.0),
                handoff: Some(DeckHandoff {
                    threshold: Vec3::new(3.0, 0.0, 0.0),
                    outward: Vec2::X,
                    destination: Vec3::new(6.0, 0.0, 0.0),
                }),
            },
            &config,
        );
        assert_eq!(handoff.state, FollowState::PastDeckHandoff);
        assert_eq!(handoff.target, Some(Vec3::new(6.0, 0.0, 0.0)));
    }
}
