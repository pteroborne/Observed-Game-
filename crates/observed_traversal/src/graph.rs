//! Canonical module-local traversal graph data.
//!
//! A graph declares a route through one resolved authored module. It does not
//! choose a facility route, move a body, or imply that the declared route is
//! physically traversable; the certification runner proves that separately.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use glam::Vec3;
use sha2::{Digest, Sha256};

use crate::follower::TraversalDirection;
use crate::guide::{ARRIVAL_RADIUS, DeckPath, StairSpine, climb_metric};
use player_input::PlayerIntent;

const GUIDE_HASH_DOMAIN: &[u8] = b"observed2.traversal-guide";
const GUIDE_HASH_VERSION: u16 = 1;

/// How close a body must come to a graph node, in the climb-weighted metric,
/// before its cursor commits to the next edge.
///
/// Deliberately the same value and the same metric as the compatibility spine's
/// [`ARRIVAL_RADIUS`]: a graph compiled from a spine has to recognize the
/// authored terminal at exactly the distance the spine did, and a body standing
/// on the deck above a node must not capture the node under its feet.
///
/// A graph constant rather than a [`crate::FollowerConfig`] field because
/// `FollowerConfig` participates in the frozen runtime-profile identity.
pub const GRAPH_NODE_CAPTURE_RADIUS: f32 = ARRIVAL_RADIUS;

/// The largest rise a compiled deck join may span, in metres.
///
/// A [`StairSpine`] ends on the deck it arrives at, which for the top of a
/// climb is the *next* module's floor. Joining that terminal to this module's
/// deck path would declare a walkable edge through a ceiling, so a join must be
/// a step rather than a storey.
const DECK_JOIN_MAX_RISE: f32 = 0.5;

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct TraversalNodeId(pub u16);

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct TraversalEdgeId(pub u16);

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(transparent)]
pub struct TraversalGuideHash(pub [u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct TraversalNode {
    pub id: TraversalNodeId,
    /// Module-local world metres, Y-up.
    pub position: [f32; 3],
}

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum TraversalMode {
    Walk,
    Climb,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct TraversalEdge {
    pub id: TraversalEdgeId,
    /// Endpoints establish canonical storage order, not one-way travel. A
    /// cursor's [`TraversalDirection`] selects which endpoint is the target.
    pub from: TraversalNodeId,
    pub to: TraversalNodeId,
    pub mode: TraversalMode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TraversalGuide {
    nodes: Vec<TraversalNode>,
    edges: Vec<TraversalEdge>,
    guide_hash: TraversalGuideHash,
}

/// Derived progress through one graph edge. Runtime owners pair this with a
/// resolved module lease; it is never serialized into match snapshots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraversalCursor {
    pub edge: TraversalEdgeId,
    pub direction: TraversalDirection,
}

/// Graph followers resolve equally valid continuations by the lowest
/// [`TraversalEdgeId`]. This makes branching independent of insertion order.
pub const GRAPH_TIE_BREAK_RULE: &str = "lowest_edge_id";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphFollowState {
    Following,
    Arrived,
    OffGuide,
    InvalidCursor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphFollowDecision {
    pub state: GraphFollowState,
    pub target: Option<[f32; 3]>,
    pub intent: Option<PlayerIntent>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraversalGuideError {
    DuplicateNode(TraversalNodeId),
    DuplicateEdge(TraversalEdgeId),
    NonFiniteNode(TraversalNodeId),
    MissingEndpoint {
        edge: TraversalEdgeId,
        node: TraversalNodeId,
    },
}

impl fmt::Display for TraversalGuideError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for TraversalGuideError {}

impl TraversalGuide {
    pub fn try_new(
        mut nodes: Vec<TraversalNode>,
        mut edges: Vec<TraversalEdge>,
    ) -> Result<Self, TraversalGuideError> {
        nodes.sort_by_key(|node| node.id);
        edges.sort_by_key(|edge| edge.id);

        for pair in nodes.windows(2) {
            if pair[0].id == pair[1].id {
                return Err(TraversalGuideError::DuplicateNode(pair[0].id));
            }
        }
        for node in &mut nodes {
            if node.position.iter().any(|value| !value.is_finite()) {
                return Err(TraversalGuideError::NonFiniteNode(node.id));
            }
            for value in &mut node.position {
                if *value == 0.0 {
                    *value = 0.0;
                }
            }
        }
        for pair in edges.windows(2) {
            if pair[0].id == pair[1].id {
                return Err(TraversalGuideError::DuplicateEdge(pair[0].id));
            }
        }

        let node_ids = nodes.iter().map(|node| node.id).collect::<BTreeSet<_>>();
        for edge in &edges {
            for endpoint in [edge.from, edge.to] {
                if !node_ids.contains(&endpoint) {
                    return Err(TraversalGuideError::MissingEndpoint {
                        edge: edge.id,
                        node: endpoint,
                    });
                }
            }
        }

        let guide_hash = hash_guide(&nodes, &edges);
        Ok(Self {
            nodes,
            edges,
            guide_hash,
        })
    }

    #[must_use]
    pub fn nodes(&self) -> &[TraversalNode] {
        &self.nodes
    }

    #[must_use]
    pub fn edges(&self) -> &[TraversalEdge] {
        &self.edges
    }

    #[must_use]
    pub fn guide_hash(&self) -> TraversalGuideHash {
        self.guide_hash
    }

    /// Return one node by stable domain identity.
    #[must_use]
    pub fn node(&self, id: TraversalNodeId) -> Option<&TraversalNode> {
        self.nodes
            .binary_search_by_key(&id, |node| node.id)
            .ok()
            .map(|index| &self.nodes[index])
    }

    /// Return one edge by stable domain identity.
    #[must_use]
    pub fn edge(&self, id: TraversalEdgeId) -> Option<&TraversalEdge> {
        self.edges
            .binary_search_by_key(&id, |edge| edge.id)
            .ok()
            .map(|index| &self.edges[index])
    }

    /// The node nearest `point` in the climb-weighted metric, or `None` for an
    /// empty graph. Ties fall to the lowest node identity, never to storage
    /// order, so binding a port is independent of how the graph was built.
    #[must_use]
    pub fn nearest_node(&self, point: Vec3) -> Option<TraversalNodeId> {
        let weighted = climb_metric(point);
        self.nodes
            .iter()
            .min_by(|a, b| {
                let to = |node: &TraversalNode| {
                    climb_metric(Vec3::from_array(node.position)).distance(weighted)
                };
                to(a).total_cmp(&to(b)).then(a.id.cmp(&b.id))
            })
            .map(|node| node.id)
    }

    /// The node nearest `point` ignoring height, restricted to nodes whose own
    /// height is within `max_rise` of it.
    ///
    /// Doorways are authored on a floor plan while a body's feet sit on a slab,
    /// so a lateral port binds by plan position. The rise filter keeps a port on
    /// one storey from binding to the node stacked above or below it.
    #[must_use]
    pub fn nearest_node_in_plan(&self, point: Vec3, max_rise: f32) -> Option<TraversalNodeId> {
        self.nodes
            .iter()
            .filter(|node| (node.position[1] - point.y).abs() <= max_rise)
            .min_by(|a, b| {
                let to = |node: &TraversalNode| plan_distance(node.position, point);
                to(a).total_cmp(&to(b)).then(a.id.cmp(&b.id))
            })
            .map(|node| node.id)
    }

    /// Translate a module-local guide into facility world space without
    /// changing any stable node/edge identities.
    #[must_use]
    pub fn translated(&self, offset: [f32; 3]) -> Self {
        let nodes = self
            .nodes
            .iter()
            .map(|node| TraversalNode {
                id: node.id,
                position: [
                    node.position[0] + offset[0],
                    node.position[1] + offset[1],
                    node.position[2] + offset[2],
                ],
            })
            .collect();
        Self::try_new(nodes, self.edges.clone()).expect("translation preserves graph validity")
    }

    /// Select the first reversible edge on a shortest route. Equal-length
    /// continuations use the lowest edge identity, never insertion order.
    #[must_use]
    pub fn cursor_between(
        &self,
        entry: TraversalNodeId,
        exit: TraversalNodeId,
    ) -> Option<TraversalCursor> {
        (entry != exit).then_some(())?;
        self.next_cursor_from(entry, exit)
    }

    /// The node this cursor is currently walking toward.
    #[must_use]
    pub fn cursor_target(&self, cursor: TraversalCursor) -> Option<TraversalNodeId> {
        let edge = self.edge(cursor.edge)?;
        Some(match cursor.direction {
            TraversalDirection::Forward => edge.to,
            TraversalDirection::Reverse => edge.from,
        })
    }

    /// Advance from a reached edge target toward `exit`. `None` means either
    /// that the cursor has arrived or that no continuation reaches the exit;
    /// callers can distinguish those cases with [`Self::cursor_target`].
    #[must_use]
    pub fn next_cursor(
        &self,
        cursor: TraversalCursor,
        exit: TraversalNodeId,
    ) -> Option<TraversalCursor> {
        let at = self.cursor_target(cursor)?;
        (at != exit).then(|| self.next_cursor_from(at, exit))?
    }

    fn next_cursor_from(
        &self,
        from: TraversalNodeId,
        exit: TraversalNodeId,
    ) -> Option<TraversalCursor> {
        self.node(from)?;
        self.node(exit)?;
        let distances = self.distances_from(exit);
        let current_distance = *distances.get(&from)?;
        self.edges
            .iter()
            .filter_map(|edge| {
                let (next, direction) = if edge.from == from {
                    (edge.to, TraversalDirection::Forward)
                } else if edge.to == from {
                    (edge.from, TraversalDirection::Reverse)
                } else {
                    return None;
                };
                (distances.get(&next).copied() == Some(current_distance - 1)).then_some(
                    TraversalCursor {
                        edge: edge.id,
                        direction,
                    },
                )
            })
            .min_by_key(|cursor| cursor.edge)
    }

    fn distances_from(&self, origin: TraversalNodeId) -> BTreeMap<TraversalNodeId, usize> {
        let mut distances = BTreeMap::from([(origin, 0)]);
        let mut pending = VecDeque::from([origin]);
        while let Some(node) = pending.pop_front() {
            let distance = distances[&node];
            for edge in &self.edges {
                let neighbor = if edge.from == node {
                    Some(edge.to)
                } else if edge.to == node {
                    Some(edge.from)
                } else {
                    None
                };
                if let Some(neighbor) = neighbor
                    && let std::collections::btree_map::Entry::Vacant(entry) =
                        distances.entry(neighbor)
                {
                    entry.insert(distance + 1);
                    pending.push_back(neighbor);
                }
            }
        }
        distances
    }
}

fn plan_distance(node: [f32; 3], point: Vec3) -> f32 {
    glam::Vec2::new(node[0] - point.x, node[2] - point.z).length()
}

/// Deterministic assembly of one module-local graph.
///
/// Identities are handed out in insertion order, so a compiler that visits the
/// same authored annotations in the same order produces the same node and edge
/// numbering — and therefore the same [`TraversalGuideHash`] — on every run and
/// every platform. Nothing here knows what an authored shape means; naming the
/// parts is the caller's job.
#[derive(Clone, Debug, Default)]
pub struct TraversalGuideBuilder {
    nodes: Vec<TraversalNode>,
    edges: Vec<TraversalEdge>,
}

impl TraversalGuideBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one node, reusing an existing node at exactly the same position.
    ///
    /// Exact rather than tolerant equality on purpose: a builder must not
    /// silently weld two authored positions that the author kept apart. Callers
    /// that mean "attach to whatever is closest" say so with
    /// [`TraversalGuide::nearest_node`] and an explicit [`Self::connect`].
    pub fn node(&mut self, position: Vec3) -> TraversalNodeId {
        let position = position.to_array();
        if let Some(existing) = self.nodes.iter().find(|node| node.position == position) {
            return existing.id;
        }
        let id = TraversalNodeId(
            u16::try_from(self.nodes.len()).expect("module graphs stay far below 65536 nodes"),
        );
        self.nodes.push(TraversalNode { id, position });
        id
    }

    /// Add a chain of nodes joined end to end, returning their identities in
    /// authored order.
    pub fn polyline(&mut self, points: &[Vec3], mode: TraversalMode) -> Vec<TraversalNodeId> {
        let ids = points
            .iter()
            .map(|&point| self.node(point))
            .collect::<Vec<_>>();
        for pair in ids.windows(2) {
            self.connect(pair[0], pair[1], mode);
        }
        ids
    }

    /// Join two nodes. A self-loop or a repeat of an existing pairing is
    /// ignored, so a compiler may connect defensively without producing a graph
    /// whose hash depends on how many times it asked.
    pub fn connect(
        &mut self,
        from: TraversalNodeId,
        to: TraversalNodeId,
        mode: TraversalMode,
    ) -> Option<TraversalEdgeId> {
        if from == to
            || self.edges.iter().any(|edge| {
                (edge.from, edge.to) == (from, to) || (edge.from, edge.to) == (to, from)
            })
        {
            return None;
        }
        let id = TraversalEdgeId(
            u16::try_from(self.edges.len()).expect("module graphs stay far below 65536 edges"),
        );
        self.edges.push(TraversalEdge { id, from, to, mode });
        Some(id)
    }

    pub fn build(self) -> Result<TraversalGuide, TraversalGuideError> {
        TraversalGuide::try_new(self.nodes, self.edges)
    }
}

/// A graph compiled from the v1/v2 spine and deck annotations, with the
/// terminals a port binder needs named rather than inferred.
///
/// This is the migration adapter: a module that has not yet been authored with
/// a graph still presents one, so runtime consumers need exactly one code path.
#[derive(Clone, Debug, PartialEq)]
pub struct CompatibilityGraph {
    pub guide: TraversalGuide,
    /// Deck-path nodes in authored order; empty when the module has no deck.
    pub deck: Vec<TraversalNodeId>,
    /// Spine nodes bottom to top; empty when the module has no climb.
    pub climb: Vec<TraversalNodeId>,
}

impl CompatibilityGraph {
    /// The climb terminal standing on the deck this module rises from.
    #[must_use]
    pub fn climb_bottom(&self) -> Option<TraversalNodeId> {
        self.climb.first().copied()
    }

    /// The climb terminal standing on the deck above.
    #[must_use]
    pub fn climb_top(&self) -> Option<TraversalNodeId> {
        self.climb.last().copied()
    }
}

/// Compile the compatibility annotations of one resolved module into a graph.
///
/// The deck becomes a chain of [`TraversalMode::Walk`] edges and the spine a
/// chain of [`TraversalMode::Climb`] edges, in that order, so the numbering is
/// stable. A climb terminal that stands on this module's own deck is welded to
/// the nearest deck node; a terminal a storey away is left free, because it
/// belongs to the module above and only a port binding may cross that seam.
///
/// Returns `None` when neither annotation describes a route.
#[must_use]
pub fn compile_compatibility_graph(
    deck: Option<&DeckPath>,
    spine: Option<&StairSpine>,
) -> Option<CompatibilityGraph> {
    let deck = deck.filter(|deck| !deck.is_empty());
    let spine = spine.filter(|spine| !spine.is_empty());
    if deck.is_none() && spine.is_none() {
        return None;
    }

    let mut builder = TraversalGuideBuilder::new();
    let deck_ids = deck.map_or_else(Vec::new, |deck| {
        builder.polyline(&deck.nodes, TraversalMode::Walk)
    });
    let climb_ids = spine.map_or_else(Vec::new, |spine| {
        builder.polyline(&spine.nodes, TraversalMode::Climb)
    });

    if let (Some(deck), Some(spine)) = (deck, spine) {
        let terminals = [
            (climb_ids.first(), spine.nodes.first()),
            (climb_ids.last(), spine.nodes.last()),
        ];
        for (terminal, point) in terminals {
            let (Some(&terminal), Some(&point)) = (terminal, point) else {
                continue;
            };
            let nearest = deck
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| (node.y - point.y).abs() <= DECK_JOIN_MAX_RISE)
                .min_by(|(_, a), (_, b)| {
                    let plan =
                        |node: &Vec3| glam::Vec2::new(node.x - point.x, node.z - point.z).length();
                    plan(a).total_cmp(&plan(b))
                })
                .map(|(index, _)| deck_ids[index]);
            if let Some(nearest) = nearest {
                builder.connect(terminal, nearest, TraversalMode::Walk);
            }
        }
    }

    let guide = builder
        .build()
        .expect("polylines and joins reference only their own nodes");
    Some(CompatibilityGraph {
        guide,
        deck: deck_ids,
        climb: climb_ids,
    })
}

fn hash_guide(nodes: &[TraversalNode], edges: &[TraversalEdge]) -> TraversalGuideHash {
    let mut hash = Sha256::new();
    hash.update(GUIDE_HASH_DOMAIN);
    hash.update(GUIDE_HASH_VERSION.to_le_bytes());
    hash.update((nodes.len() as u64).to_le_bytes());
    for node in nodes {
        hash.update(node.id.0.to_le_bytes());
        for value in node.position {
            hash.update(value.to_bits().to_le_bytes());
        }
    }
    hash.update((edges.len() as u64).to_le_bytes());
    for edge in edges {
        hash.update(edge.id.0.to_le_bytes());
        hash.update(edge.from.0.to_le_bytes());
        hash.update(edge.to.0.to_le_bytes());
        hash.update([match edge.mode {
            TraversalMode::Walk => 0,
            TraversalMode::Climb => 1,
        }]);
    }
    TraversalGuideHash(hash.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: u16, position: [f32; 3]) -> TraversalNode {
        TraversalNode {
            id: TraversalNodeId(id),
            position,
        }
    }

    fn edge(id: u16, from: u16, to: u16, mode: TraversalMode) -> TraversalEdge {
        TraversalEdge {
            id: TraversalEdgeId(id),
            from: TraversalNodeId(from),
            to: TraversalNodeId(to),
            mode,
        }
    }

    #[test]
    fn canonical_order_and_hash_ignore_input_order_and_negative_zero() {
        let a = TraversalGuide::try_new(
            vec![node(1, [-0.0, 1.0, 2.0]), node(0, [0.0, 0.0, 0.0])],
            vec![
                edge(1, 1, 0, TraversalMode::Climb),
                edge(0, 0, 1, TraversalMode::Walk),
            ],
        )
        .unwrap();
        let b = TraversalGuide::try_new(
            vec![node(0, [0.0, 0.0, 0.0]), node(1, [0.0, 1.0, 2.0])],
            vec![
                edge(0, 0, 1, TraversalMode::Walk),
                edge(1, 1, 0, TraversalMode::Climb),
            ],
        )
        .unwrap();

        assert_eq!(a, b);
        assert_eq!(a.guide_hash(), b.guide_hash());
        assert_eq!(a.nodes()[0].id, TraversalNodeId(0));
        assert_eq!(a.edges()[0].id, TraversalEdgeId(0));
    }

    #[test]
    fn graph_validation_rejects_ambiguous_or_invalid_identity() {
        assert_eq!(
            TraversalGuide::try_new(vec![node(0, [0.0; 3]), node(0, [1.0; 3])], vec![]),
            Err(TraversalGuideError::DuplicateNode(TraversalNodeId(0)))
        );
        assert_eq!(
            TraversalGuide::try_new(vec![node(0, [f32::NAN, 0.0, 0.0])], vec![]),
            Err(TraversalGuideError::NonFiniteNode(TraversalNodeId(0)))
        );
        assert_eq!(
            TraversalGuide::try_new(
                vec![node(0, [0.0; 3])],
                vec![edge(0, 0, 1, TraversalMode::Walk)]
            ),
            Err(TraversalGuideError::MissingEndpoint {
                edge: TraversalEdgeId(0),
                node: TraversalNodeId(1)
            })
        );
    }

    #[test]
    fn every_graph_field_changes_the_hash() {
        let base = TraversalGuide::try_new(
            vec![node(0, [0.0; 3]), node(1, [1.0, 0.0, 0.0])],
            vec![edge(0, 0, 1, TraversalMode::Walk)],
        )
        .unwrap()
        .guide_hash();
        for changed in [
            TraversalGuide::try_new(
                vec![node(0, [0.0; 3]), node(1, [2.0, 0.0, 0.0])],
                vec![edge(0, 0, 1, TraversalMode::Walk)],
            )
            .unwrap(),
            TraversalGuide::try_new(
                vec![node(0, [0.0; 3]), node(1, [1.0, 0.0, 0.0])],
                vec![edge(1, 0, 1, TraversalMode::Walk)],
            )
            .unwrap(),
            TraversalGuide::try_new(
                vec![node(0, [0.0; 3]), node(1, [1.0, 0.0, 0.0])],
                vec![edge(0, 1, 0, TraversalMode::Walk)],
            )
            .unwrap(),
            TraversalGuide::try_new(
                vec![node(0, [0.0; 3]), node(1, [1.0, 0.0, 0.0])],
                vec![edge(0, 0, 1, TraversalMode::Climb)],
            )
            .unwrap(),
        ] {
            assert_ne!(changed.guide_hash(), base);
        }
    }

    #[test]
    fn reversible_paths_choose_the_lowest_edge_id_at_a_branch() {
        let guide = TraversalGuide::try_new(
            vec![
                node(0, [0.0, 0.0, 0.0]),
                node(1, [1.0, 0.0, 0.0]),
                node(2, [0.0, 0.0, 1.0]),
                node(3, [1.0, 0.0, 1.0]),
            ],
            vec![
                edge(9, 0, 1, TraversalMode::Walk),
                edge(7, 0, 2, TraversalMode::Walk),
                edge(10, 1, 3, TraversalMode::Walk),
                edge(11, 2, 3, TraversalMode::Walk),
            ],
        )
        .unwrap();

        let forward = guide
            .cursor_between(TraversalNodeId(0), TraversalNodeId(3))
            .unwrap();
        assert_eq!(forward.edge, TraversalEdgeId(7));
        assert_eq!(forward.direction, TraversalDirection::Forward);

        let reverse = guide
            .cursor_between(TraversalNodeId(3), TraversalNodeId(0))
            .unwrap();
        assert_eq!(reverse.edge, TraversalEdgeId(10));
        assert_eq!(reverse.direction, TraversalDirection::Reverse);
        let reverse = guide.next_cursor(reverse, TraversalNodeId(0)).unwrap();
        assert_eq!(reverse.edge, TraversalEdgeId(9));
        assert_eq!(reverse.direction, TraversalDirection::Reverse);
    }

    #[test]
    fn compiled_compatibility_annotations_keep_their_modes_and_weld_only_the_reachable_terminal() {
        let deck = DeckPath {
            nodes: vec![
                Vec3::new(-3.0, 0.0, 0.0),
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(-0.2, 0.0, 0.0),
            ],
        };
        let spine = StairSpine {
            nodes: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 2.0, 2.0),
                Vec3::new(0.0, 4.0, 4.0),
            ],
        };
        let compiled = compile_compatibility_graph(Some(&deck), Some(&spine)).expect("compiles");

        assert_eq!(compiled.deck.len(), 3);
        assert_eq!(compiled.climb.len(), 3);
        assert_eq!(compiled.climb_bottom(), Some(TraversalNodeId(3)));
        assert_eq!(compiled.climb_top(), Some(TraversalNodeId(5)));

        let modes = compiled
            .guide
            .edges()
            .iter()
            .map(|edge| (edge.from, edge.to, edge.mode))
            .collect::<Vec<_>>();
        assert_eq!(
            modes,
            vec![
                (TraversalNodeId(0), TraversalNodeId(1), TraversalMode::Walk),
                (TraversalNodeId(1), TraversalNodeId(2), TraversalMode::Walk),
                (TraversalNodeId(3), TraversalNodeId(4), TraversalMode::Climb),
                (TraversalNodeId(4), TraversalNodeId(5), TraversalMode::Climb),
                // Only the foot of the climb stands on this module's deck. The
                // head arrives four metres up, on the module above.
                (TraversalNodeId(3), TraversalNodeId(2), TraversalMode::Walk),
            ]
        );

        // The whole point of welding: one route runs from the far end of the
        // deck to the head of the climb without any caller knowing the shape.
        let mut cursor = compiled
            .guide
            .cursor_between(TraversalNodeId(0), TraversalNodeId(5))
            .expect("deck reaches the climb head");
        let mut visited = vec![compiled.guide.cursor_target(cursor).expect("target")];
        while let Some(next) = compiled.guide.next_cursor(cursor, TraversalNodeId(5)) {
            cursor = next;
            visited.push(compiled.guide.cursor_target(cursor).expect("target"));
        }
        assert_eq!(
            visited,
            vec![
                TraversalNodeId(1),
                TraversalNodeId(2),
                TraversalNodeId(3),
                TraversalNodeId(4),
                TraversalNodeId(5),
            ]
        );
    }

    #[test]
    fn compiling_one_annotation_alone_still_produces_a_route() {
        assert_eq!(compile_compatibility_graph(None, None), None);
        assert_eq!(
            compile_compatibility_graph(Some(&DeckPath::default()), Some(&StairSpine::default())),
            None
        );

        let flat = compile_compatibility_graph(
            Some(&DeckPath {
                nodes: vec![Vec3::ZERO, Vec3::X, Vec3::new(1.0, 0.0, 1.0)],
            }),
            None,
        )
        .expect("a deck alone is a route");
        assert!(flat.climb.is_empty());
        assert!(
            flat.guide
                .edges()
                .iter()
                .all(|edge| edge.mode == TraversalMode::Walk)
        );

        let climb_only = compile_compatibility_graph(
            None,
            Some(&StairSpine {
                nodes: vec![Vec3::ZERO, Vec3::new(0.0, 2.0, 2.0)],
            }),
        )
        .expect("a spine alone is a route");
        assert!(climb_only.deck.is_empty());
        assert_eq!(climb_only.guide.edges()[0].mode, TraversalMode::Climb);
    }

    #[test]
    fn builders_are_order_deterministic_and_ignore_redundant_requests() {
        let mut builder = TraversalGuideBuilder::new();
        let a = builder.node(Vec3::ZERO);
        let b = builder.node(Vec3::X);
        assert_eq!(builder.node(Vec3::ZERO), a, "exact positions are reused");
        assert_eq!(builder.connect(a, a, TraversalMode::Walk), None);
        assert_eq!(
            builder.connect(a, b, TraversalMode::Walk),
            Some(TraversalEdgeId(0))
        );
        assert_eq!(
            builder.connect(b, a, TraversalMode::Climb),
            None,
            "an existing pairing is not duplicated with a second mode"
        );
        let guide = builder.build().expect("valid");
        assert_eq!(guide.nodes().len(), 2);
        assert_eq!(guide.edges().len(), 1);
    }

    #[test]
    fn ports_bind_by_proximity_without_crossing_a_storey() {
        let guide = TraversalGuide::try_new(
            vec![
                node(0, [0.0, 0.0, 0.0]),
                node(1, [4.0, 0.0, 0.0]),
                node(2, [0.5, 8.0, 0.0]),
            ],
            vec![edge(0, 0, 1, TraversalMode::Walk)],
        )
        .unwrap();

        assert_eq!(
            guide.nearest_node(Vec3::new(0.4, 8.0, 0.0)),
            Some(TraversalNodeId(2))
        );
        // In plan the upper node is the closest thing to this doorway; the rise
        // filter is what stops a ground-floor port binding to the floor above.
        assert_eq!(
            guide.nearest_node_in_plan(Vec3::new(0.6, 0.0, 0.0), 0.5),
            Some(TraversalNodeId(0))
        );
        assert_eq!(
            guide.nearest_node_in_plan(Vec3::new(0.6, 0.0, 0.0), 16.0),
            Some(TraversalNodeId(2))
        );
    }

    #[test]
    fn translation_preserves_ids_and_changes_world_positions() {
        let guide = TraversalGuide::try_new(
            vec![node(0, [0.0; 3]), node(1, [1.0, 2.0, 3.0])],
            vec![edge(0, 0, 1, TraversalMode::Climb)],
        )
        .unwrap();
        let translated = guide.translated([10.0, 20.0, 30.0]);

        assert_eq!(translated.nodes()[0].position, [10.0, 20.0, 30.0]);
        assert_eq!(translated.nodes()[1].position, [11.0, 22.0, 33.0]);
        assert_eq!(translated.edges(), guide.edges());
        assert_ne!(translated.guide_hash(), guide.guide_hash());
    }
}
