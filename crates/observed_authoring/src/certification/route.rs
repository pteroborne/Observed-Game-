//! Compiling a declared module graph into the legs the shared follower takes.
//!
//! The graph is a *declared* route. It says which nodes are joined and whether
//! a joint is walked or climbed; it does not say that a capsule fits. This
//! module turns one entry-to-exit path through that graph into the exact
//! [`FollowTarget`](observed_traversal::FollowTarget) inputs the production
//! follower already consumes, so certification drives the shipped steering
//! rather than a certification-only copy of it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use glam::{Quat, Vec3};
use observed_traversal::{DeckPath, StairSpine, TraversalDirection, TraversalMode, TraversalNodeId};

use crate::contract::{ModuleTraversalContract, QuantizedPoint};
use crate::source::editor_origin_to_world;

/// One contiguous run of same-mode edges, already placed in scene space.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Leg {
    Deck {
        path: DeckPath,
        goal: Vec3,
    },
    Climb {
        spine: StairSpine,
        direction: TraversalDirection,
        terminal: Vec3,
    },
}

impl Leg {
    /// Where this leg wants the body to end up. Certification uses it as the
    /// deck arrival test and as the spawn bearing for the next leg.
    pub(crate) fn destination(&self) -> Vec3 {
        match self {
            Self::Deck { goal, .. } => *goal,
            Self::Climb { terminal, .. } => *terminal,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RouteError {
    UnboundPort,
    NoRoute,
    DegenerateNode(TraversalNodeId),
}

/// The transform placing one module instance's local guide into scene space.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GuidePlacement {
    pub offset: Vec3,
    pub rotation: Quat,
}

impl GuidePlacement {
    pub(crate) fn identity() -> Self {
        Self {
            offset: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        }
    }

    pub(crate) fn new(offset: Vec3, turn: u8) -> Self {
        Self {
            offset,
            rotation: Quat::from_rotation_y(-f32::from(turn) * std::f32::consts::TAU / 6.0),
        }
    }

    fn apply(self, point: Vec3) -> Vec3 {
        self.rotation * point + self.offset
    }
}

/// Editor integer units to the module-local world metres the guide, hulls, and
/// controller all share.
pub(crate) fn node_world(point: QuantizedPoint) -> Vec3 {
    editor_origin_to_world([f64::from(point.x), f64::from(point.y), f64::from(point.z)])
}

/// The node sequence from `entry` to `exit`, breadth-first with the graph's
/// declared tie-break: equally short continuations resolve by the lowest edge
/// ID, never by insertion or hash order.
fn node_path(
    traversal: &ModuleTraversalContract,
    entry: TraversalNodeId,
    exit: TraversalNodeId,
) -> Option<Vec<(TraversalNodeId, Option<TraversalMode>)>> {
    let mut adjacency: BTreeMap<TraversalNodeId, Vec<(TraversalNodeId, TraversalMode)>> =
        BTreeMap::new();
    let mut edges = traversal.edges.clone();
    edges.sort_by_key(|edge| edge.id);
    for edge in &edges {
        adjacency
            .entry(edge.from)
            .or_default()
            .push((edge.to, edge.mode));
        adjacency
            .entry(edge.to)
            .or_default()
            .push((edge.from, edge.mode));
    }

    let mut seen = BTreeSet::from([entry]);
    let mut previous: BTreeMap<TraversalNodeId, (TraversalNodeId, TraversalMode)> = BTreeMap::new();
    let mut queue = VecDeque::from([entry]);
    while let Some(node) = queue.pop_front() {
        if node == exit {
            break;
        }
        for &(next, mode) in adjacency.get(&node).into_iter().flatten() {
            if seen.insert(next) {
                previous.insert(next, (node, mode));
                queue.push_back(next);
            }
        }
    }
    if entry != exit && !previous.contains_key(&exit) {
        return None;
    }

    let mut reversed = vec![(exit, None)];
    let mut cursor = exit;
    while cursor != entry {
        let (before, mode) = *previous.get(&cursor)?;
        reversed.push((before, Some(mode)));
        cursor = before;
    }
    reversed.reverse();
    // Each entry now carries the mode of the edge that *leaves* it toward the
    // following node; the destination carries `None`.
    Some(reversed)
}

/// Compile the entry-to-exit route into follower legs placed in scene space.
pub(crate) fn plan_route(
    traversal: &ModuleTraversalContract,
    entry_port: &str,
    exit_port: &str,
    placement: GuidePlacement,
) -> Result<Vec<Leg>, RouteError> {
    let entry = *traversal
        .port_bindings
        .get(entry_port)
        .ok_or(RouteError::UnboundPort)?;
    let exit = *traversal
        .port_bindings
        .get(exit_port)
        .ok_or(RouteError::UnboundPort)?;
    let positions = traversal
        .nodes
        .iter()
        .map(|node| (node.id, placement.apply(node_world(node.position))))
        .collect::<BTreeMap<_, _>>();
    for (id, position) in &positions {
        if !position.is_finite() {
            return Err(RouteError::DegenerateNode(*id));
        }
    }
    let path = node_path(traversal, entry, exit).ok_or(RouteError::NoRoute)?;
    if path.len() < 2 {
        return Err(RouteError::NoRoute);
    }

    let mut legs = Vec::new();
    let mut run: Vec<Vec3> = vec![positions[&path[0].0]];
    let mut run_mode = path[0].1;
    for window in path.windows(2) {
        let (_, mode) = window[0];
        let (next_id, next_mode) = window[1];
        let point = positions[&next_id];
        run.push(point);
        if mode != next_mode && next_mode.is_some() {
            legs.push(leg_from_run(&run, run_mode));
            run = vec![point];
            run_mode = next_mode;
        }
    }
    legs.push(leg_from_run(&run, run_mode));
    Ok(legs)
}

fn leg_from_run(run: &[Vec3], mode: Option<TraversalMode>) -> Leg {
    let goal = *run.last().expect("a run always carries its endpoints");
    match mode {
        Some(TraversalMode::Climb) => {
            // A spine is authored bottom-to-top by contract, so a descending
            // run is stored reversed and walked in reverse.
            let ascending = run.last().map(|node| node.y) >= run.first().map(|node| node.y);
            let mut nodes = run.to_vec();
            let direction = if ascending {
                TraversalDirection::Forward
            } else {
                nodes.reverse();
                TraversalDirection::Reverse
            };
            Leg::Climb {
                spine: StairSpine { nodes },
                direction,
                terminal: goal,
            }
        }
        _ => Leg::Deck {
            path: DeckPath {
                nodes: run.to_vec(),
            },
            goal,
        },
    }
}

/// The legs a compatibility prototype declares, for modules that predate the
/// v3 contract: its climb in one direction, optionally approached over its
/// deck.
pub(crate) fn compatibility_climb(
    spine: &StairSpine,
    deck: &DeckPath,
    ascending: bool,
) -> Option<(Vec3, Leg, DeckPath)> {
    if spine.is_empty() {
        return None;
    }
    let first = *spine.nodes.first()?;
    let last = *spine.nodes.last()?;
    let (start, terminal, direction) = if ascending {
        (first, last, TraversalDirection::Forward)
    } else {
        (last, first, TraversalDirection::Reverse)
    };
    Some((
        start,
        Leg::Climb {
            spine: spine.clone(),
            direction,
            terminal,
        },
        deck.clone(),
    ))
}

#[cfg(test)]
mod tests {
    use observed_traversal::TraversalEdgeId;

    use super::*;
    use crate::contract::{CompiledTraversalEdge, QuantizedTraversalNode};

    fn node(id: u16, x: i32, y: i32, z: i32) -> QuantizedTraversalNode {
        QuantizedTraversalNode {
            id: TraversalNodeId(id),
            position: QuantizedPoint { x, y, z },
        }
    }

    fn edge(id: u16, from: u16, to: u16, mode: TraversalMode) -> CompiledTraversalEdge {
        CompiledTraversalEdge {
            id: TraversalEdgeId(id),
            from: TraversalNodeId(from),
            to: TraversalNodeId(to),
            mode,
        }
    }

    fn contract() -> ModuleTraversalContract {
        // west door -> deck -> climb -> deck -> east door on the level above.
        ModuleTraversalContract {
            nodes: vec![
                node(0, -96, 0, 8),
                node(1, -32, 0, 8),
                node(2, 32, 0, 136),
                node(3, 96, 0, 136),
            ],
            edges: vec![
                edge(0, 0, 1, TraversalMode::Walk),
                edge(1, 1, 2, TraversalMode::Climb),
                edge(2, 2, 3, TraversalMode::Walk),
            ],
            port_bindings: BTreeMap::from([
                ("west".to_string(), TraversalNodeId(0)),
                ("east".to_string(), TraversalNodeId(3)),
            ]),
        }
    }

    #[test]
    fn a_route_splits_into_deck_climb_deck_legs_in_both_directions() {
        let up = plan_route(&contract(), "west", "east", GuidePlacement::identity())
            .expect("route exists");
        assert_eq!(up.len(), 3);
        assert!(matches!(up[0], Leg::Deck { .. }));
        assert!(matches!(
            up[1],
            Leg::Climb {
                direction: TraversalDirection::Forward,
                ..
            }
        ));
        assert!(matches!(up[2], Leg::Deck { .. }));

        let down = plan_route(&contract(), "east", "west", GuidePlacement::identity())
            .expect("reverse route exists");
        assert_eq!(down.len(), 3);
        let Leg::Climb {
            spine, direction, ..
        } = &down[1]
        else {
            panic!("the middle leg is the climb");
        };
        assert_eq!(*direction, TraversalDirection::Reverse);
        assert!(
            spine.nodes[0].y < spine.nodes[spine.nodes.len() - 1].y,
            "a descending run still stores its spine bottom-to-top"
        );
    }

    #[test]
    fn an_unreachable_or_unbound_port_is_a_named_route_failure() {
        let mut split = contract();
        split.edges.remove(1);
        assert_eq!(
            plan_route(&split, "west", "east", GuidePlacement::identity()),
            Err(RouteError::NoRoute)
        );
        assert_eq!(
            plan_route(&contract(), "west", "nowhere", GuidePlacement::identity()),
            Err(RouteError::UnboundPort)
        );
    }

    #[test]
    fn placement_moves_the_whole_guide_with_its_module() {
        let placed = plan_route(
            &contract(),
            "west",
            "east",
            GuidePlacement::new(Vec3::Y * 8.0, 0),
        )
        .expect("route exists");
        let base =
            plan_route(&contract(), "west", "east", GuidePlacement::identity()).expect("route");
        assert_eq!(
            placed[2].destination(),
            base[2].destination() + Vec3::Y * 8.0
        );
    }
}
