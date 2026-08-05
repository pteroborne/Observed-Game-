//! Canonical module-local traversal graph data.
//!
//! A graph declares a route through one resolved authored module. It does not
//! choose a facility route, move a body, or imply that the declared route is
//! physically traversable; the certification runner proves that separately.

use std::collections::BTreeSet;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::follower::TraversalDirection;
use player_input::PlayerIntent;

const GUIDE_HASH_DOMAIN: &[u8] = b"observed2.traversal-guide";
const GUIDE_HASH_VERSION: u16 = 1;

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
}
