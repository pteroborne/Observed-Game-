//! Stateless steering over the compatibility spine and deck annotations.
//!
//! The facility planner chooses which cell-to-cell transition is wanted. This
//! module owns only the local, shape-aware decision that turns one selected
//! annotation into the same [`PlayerIntent`] used by human and network inputs.

use std::f32::consts::PI;

use glam::{Vec2, Vec3};
use player_input::PlayerIntent;

use crate::graph::{
    GRAPH_NODE_CAPTURE_RADIUS, GraphFollowDecision, GraphFollowState, TraversalCursor,
    TraversalGuide, TraversalNodeId,
};
use crate::guide::{DeckPath, StairSpine, climb_metric};
use crate::profile::TraversalRuntimeProfile;

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
    profile: &TraversalRuntimeProfile,
) -> FollowDecision {
    let config = profile.follower();
    let look_step = profile.controller().look_step;
    match target {
        FollowTarget::Climb {
            spine,
            approach,
            direction,
        } => follow_climb(pose, spine, approach, direction, config, look_step),
        FollowTarget::Deck {
            path,
            goal,
            handoff,
        } => follow_deck(pose, path, goal, handoff, config, look_step),
    }
}

/// Follow a leased graph cursor without mutating a character body. The only
/// output crossing into simulation is [`PlayerIntent`]; local edge progress is
/// retained by the caller-owned cursor.
///
/// The cursor is the whole point. Nothing here reads an archetype, a port
/// class, or a logical cell, so the caller may re-derive the body's logical
/// cell as often as it likes without the edge under the body changing: only
/// reaching a node advances it, and it advances only along the route to `exit`.
#[must_use]
pub fn follow_graph(
    pose: FollowerPose,
    guide: &TraversalGuide,
    cursor: &mut TraversalCursor,
    exit: TraversalNodeId,
    profile: &TraversalRuntimeProfile,
) -> GraphFollowDecision {
    let Some(mut target_id) = guide.cursor_target(*cursor) else {
        return GraphFollowDecision {
            state: GraphFollowState::InvalidCursor,
            target: None,
            intent: None,
        };
    };
    let Some(mut target) = guide
        .node(target_id)
        .map(|node| Vec3::from_array(node.position))
    else {
        return GraphFollowDecision {
            state: GraphFollowState::InvalidCursor,
            target: None,
            intent: None,
        };
    };

    while climb_metric(pose.feet).distance(climb_metric(target)) <= GRAPH_NODE_CAPTURE_RADIUS {
        if target_id == exit {
            return GraphFollowDecision {
                state: GraphFollowState::Arrived,
                target: Some(target.to_array()),
                intent: None,
            };
        }
        let Some(next) = guide.next_cursor(*cursor, exit) else {
            return GraphFollowDecision {
                state: GraphFollowState::OffGuide,
                target: Some(target.to_array()),
                intent: None,
            };
        };
        *cursor = next;
        let Some(next_target_id) = guide.cursor_target(*cursor) else {
            return GraphFollowDecision {
                state: GraphFollowState::InvalidCursor,
                target: None,
                intent: None,
            };
        };
        target_id = next_target_id;
        let Some(next_target) = guide
            .node(target_id)
            .map(|node| Vec3::from_array(node.position))
        else {
            return GraphFollowDecision {
                state: GraphFollowState::InvalidCursor,
                target: None,
                intent: None,
            };
        };
        target = next_target;
    }

    let config = profile.follower();
    GraphFollowDecision {
        state: GraphFollowState::Following,
        target: Some(target.to_array()),
        intent: Some(walk_toward(
            pose,
            target,
            config,
            profile.controller().look_step,
        )),
    }
}

fn follow_climb(
    pose: FollowerPose,
    spine: &StairSpine,
    approach: Option<&DeckPath>,
    direction: TraversalDirection,
    config: FollowerConfig,
    look_step: f32,
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
        intent: Some(walk_toward(pose, target, config, look_step)),
    }
}

fn follow_deck(
    pose: FollowerPose,
    path: &DeckPath,
    goal: Vec3,
    handoff: Option<DeckHandoff>,
    config: FollowerConfig,
    look_step: f32,
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
            intent: Some(walk_toward(pose, handoff.destination, config, look_step)),
        };
    }
    let Some(target) = path.step_toward(pose.feet, goal) else {
        return FollowDecision::unavailable();
    };
    FollowDecision {
        state: FollowState::FollowingDeck,
        target: Some(target),
        intent: Some(walk_toward(pose, target, config, look_step)),
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

fn walk_toward(
    pose: FollowerPose,
    target: Vec3,
    config: FollowerConfig,
    look_step: f32,
) -> PlayerIntent {
    let direction = target - pose.feet;
    let desired_yaw = direction.x.atan2(-direction.z);
    let applied_turn = wrap_angle(desired_yaw - pose.yaw)
        .clamp(-config.max_turn_per_tick, config.max_turn_per_tick);
    debug_assert!(look_step.is_finite() && look_step > 0.0);
    let look = applied_turn / look_step;
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
    use crate::FpsConfig;

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
        let profile = TraversalRuntimeProfile::canonical_hex();
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
                &profile,
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
            &profile,
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
        let profile = TraversalRuntimeProfile::canonical_hex();
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
            &profile,
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
            &profile,
        );
        assert_eq!(handoff.state, FollowState::PastDeckHandoff);
        assert_eq!(handoff.target, Some(Vec3::new(6.0, 0.0, 0.0)));
    }

    #[test]
    fn look_intent_scales_through_the_controller_look_step() {
        let mut controller = FpsConfig::deliberate_rapier();
        controller.look_step = 0.5;
        let profile = TraversalRuntimeProfile::from_controller(controller);
        let deck = DeckPath {
            nodes: vec![Vec3::ZERO, Vec3::X],
        };
        let decision = follow_stateless(
            FollowerPose {
                feet: Vec3::ZERO,
                yaw: 0.0,
            },
            FollowTarget::Deck {
                path: &deck,
                goal: Vec3::X,
                handoff: None,
            },
            &profile,
        );
        let intent = decision.intent.expect("deck intent");

        assert_eq!(intent.look.x, 0.08 / controller.look_step);
        assert_eq!(intent.look.x * controller.look_step, 0.08);
    }

    #[test]
    fn graph_follower_advances_edges_and_emits_only_abstract_intent() {
        use crate::graph::{
            TraversalEdge, TraversalEdgeId, TraversalMode, TraversalNode, TraversalNodeId,
        };

        let guide = TraversalGuide::try_new(
            vec![
                TraversalNode {
                    id: TraversalNodeId(0),
                    position: [0.0, 0.0, 0.0],
                },
                TraversalNode {
                    id: TraversalNodeId(1),
                    position: [1.0, 0.0, 0.0],
                },
                TraversalNode {
                    id: TraversalNodeId(2),
                    position: [2.0, 0.0, 0.0],
                },
            ],
            vec![
                TraversalEdge {
                    id: TraversalEdgeId(0),
                    from: TraversalNodeId(0),
                    to: TraversalNodeId(1),
                    mode: TraversalMode::Walk,
                },
                TraversalEdge {
                    id: TraversalEdgeId(1),
                    from: TraversalNodeId(1),
                    to: TraversalNodeId(2),
                    mode: TraversalMode::Walk,
                },
            ],
        )
        .unwrap();
        let mut cursor = guide
            .cursor_between(TraversalNodeId(0), TraversalNodeId(2))
            .unwrap();
        let decision = follow_graph(
            FollowerPose {
                feet: Vec3::new(1.0, 0.0, 0.0),
                yaw: 0.0,
            },
            &guide,
            &mut cursor,
            TraversalNodeId(2),
            &TraversalRuntimeProfile::canonical_hex(),
        );

        assert_eq!(cursor.edge, TraversalEdgeId(1));
        assert_eq!(decision.state, GraphFollowState::Following);
        assert_eq!(decision.target, Some([2.0, 0.0, 0.0]));
        assert!(decision.intent.is_some());
    }

    /// Following a compiled graph up a climb emits the **same intent** as
    /// following the spine it was compiled from.
    ///
    /// This measures the thing the plan calls contract boundary 4, and it is
    /// worth being precise about what it does and does not settle.
    ///
    /// The boundary was raised because the graph follower emits `movement 0.35`
    /// without sprint while *lateral* steering emits `1.0` with it - 1.61 m/s
    /// against 7.0. Both of those do ship. But they are not two answers to one
    /// question: a lateral hop between cells is not a graph leg. Everything the
    /// graph currently models is module-local, and module-local traversal
    /// already goes through this very function at 0.35, for climbs and decks
    /// alike. So for the three functions TR-10 actually deletes -
    /// `vertical_command`, `climb_command`, `finish_stair_command` - both sides
    /// were always the same tuning, and this asserts it rather than assuming
    /// it.
    ///
    /// What this does **not** settle: whether cell-to-cell movement should
    /// later become graph legs. That would put a `Walk` edge where 1.0/sprint
    /// is used today, and *that* is a real intent change needing its own
    /// evidence. `TraversalEdge` already carries the mode to tell them apart.
    #[test]
    fn a_compiled_climb_graph_emits_what_the_spine_it_came_from_emits() {
        let spine = spine();
        let graph = crate::graph::compile_compatibility_graph(None, Some(&spine))
            .expect("a spine compiles to a graph");
        let guide = &graph.guide;
        let profile = TraversalRuntimeProfile::canonical_hex();

        let entry = *guide.nodes().first().expect("a compiled graph has nodes");
        let exit = *guide.nodes().last().expect("a compiled graph has nodes");
        let mut cursor = guide
            .cursor_between(entry.id, exit.id)
            .expect("entry and exit are joined");

        // Sample along the climb rather than at one pose: a single point can
        // agree by luck, and the question is whether the two paths agree while
        // a body is actually moving through the shape.
        let mut compared = 0;
        for step in 0..=8u8 {
            let t = f32::from(step) / 8.0;
            let along = spine.nodes[0].lerp(spine.nodes[spine.nodes.len() - 1], t);
            let pose = FollowerPose {
                feet: along,
                yaw: 0.4,
            };

            let compatibility = follow_stateless(
                pose,
                FollowTarget::Climb {
                    spine: &spine,
                    approach: None,
                    direction: TraversalDirection::Forward,
                },
                &profile,
            );
            let mut probe = cursor;
            let graphed = follow_graph(pose, guide, &mut probe, exit.id, &profile);

            let (Some(old), Some(new)) = (compatibility.intent, graphed.intent) else {
                continue;
            };
            assert_eq!(
                old.movement, new.movement,
                "step {step}: movement diverges - this is the boundary-4 claim, and it \
                 would mean the graph is not module-local after all"
            );
            assert_eq!(
                old.sprint_held, new.sprint_held,
                "step {step}: sprint diverges"
            );
            compared += 1;
        }
        // A loop that skips every sample agrees with itself about nothing.
        assert!(
            compared >= 6,
            "only {compared} poses actually produced two intents to compare"
        );

        // And the tuning is the module-local one, not the lateral one. Stated
        // as a value so that changing `FollowerConfig` cannot quietly move
        // production speed while both sides keep agreeing with each other.
        let pose = FollowerPose {
            feet: spine.nodes[0],
            yaw: 0.0,
        };
        let decision = follow_graph(pose, guide, &mut cursor, exit.id, &profile);
        let intent = decision.intent.expect("a fresh cursor steers");
        assert_eq!(intent.movement.y, profile.follower().movement_scale);
        assert!(!intent.sprint_held);
    }
}
