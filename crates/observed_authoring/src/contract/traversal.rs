use std::collections::{BTreeMap, BTreeSet};

use observed_traversal::{TraversalEdgeId, TraversalMode, TraversalNodeId};
use serde::{Deserialize, Serialize};

use super::{ContractDiagnostic, DiagnosticLocation, QuantizedPoint};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuantizedTraversalNode {
    pub id: TraversalNodeId,
    pub position: QuantizedPoint,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledTraversalEdge {
    pub id: TraversalEdgeId,
    pub from: TraversalNodeId,
    pub to: TraversalNodeId,
    pub mode: TraversalMode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleTraversalContract {
    pub nodes: Vec<QuantizedTraversalNode>,
    pub edges: Vec<CompiledTraversalEdge>,
    pub port_bindings: BTreeMap<String, TraversalNodeId>,
}

impl ModuleTraversalContract {
    pub fn validate(&self) -> Result<(), ContractDiagnostic> {
        let mut node_ids = BTreeSet::new();
        for node in &self.nodes {
            if !node_ids.insert(node.id) {
                return Err(ContractDiagnostic {
                    code: "duplicate_traversal_node".to_string(),
                    location: DiagnosticLocation::GuideNode(node.id),
                    message: format!("traversal node {:?} is duplicated", node.id),
                });
            }
        }
        let mut edge_ids = BTreeSet::new();
        for edge in &self.edges {
            if !edge_ids.insert(edge.id) {
                return Err(ContractDiagnostic::whole(
                    "duplicate_traversal_edge",
                    format!("traversal edge {:?} is duplicated", edge.id),
                ));
            }
            for endpoint in [edge.from, edge.to] {
                if !node_ids.contains(&endpoint) {
                    return Err(ContractDiagnostic {
                        code: "missing_traversal_endpoint".to_string(),
                        location: DiagnosticLocation::GuideNode(endpoint),
                        message: format!("edge {:?} references missing node {endpoint:?}", edge.id),
                    });
                }
            }
        }
        for (port, node) in &self.port_bindings {
            if port.trim().is_empty() || !node_ids.contains(node) {
                return Err(ContractDiagnostic {
                    code: "invalid_port_binding".to_string(),
                    location: DiagnosticLocation::GuideNode(*node),
                    message: format!("port binding {port:?} references missing node {node:?}"),
                });
            }
        }
        Ok(())
    }
}
