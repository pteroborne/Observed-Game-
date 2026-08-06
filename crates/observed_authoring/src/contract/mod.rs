//! Version-3 authored module contract and version-4 compiled DTOs.
//!
//! Existing v1/v2 sources and compiled catalog v3 omit this value. The
//! contract is deliberately complete when present: partial spatial, family,
//! interface, or traversal metadata is never treated as a valid v3 module.

mod family;
mod interface;
mod spatial;
mod traversal;

use serde::{Deserialize, Serialize};

pub use family::{
    AssemblyContract, AssemblyScope, AssemblyVariantId, ModuleFamilyId, RuntimeAssembly,
};
pub use interface::{
    FACE_LOCAL_SCALE, FaceLocalBox, FaceLocalDirection, FaceLocalPoint, InterfaceFingerprint,
    InterfaceProfile, LateralFaceFrame, ModuleInterface, QuantizedAperture, QuantizedDirection,
    QuantizedPose, VerticalFaceFrame,
};
use observed_traversal::TraversalGuide;
pub use spatial::{
    ClearanceVolume, GeometryEnvelope, LogicalCell, LogicalFootprint, ModuleSpatialContract,
    QuantizedBox, QuantizedPoint,
};
pub use traversal::{CompiledTraversalEdge, ModuleTraversalContract, QuantizedTraversalNode};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticLocation {
    Whole,
    Cell(crate::source::ModuleCellRef),
    Face(crate::source::ModuleCellRef, observed_hex::HexFace),
    GuideNode(observed_traversal::TraversalNodeId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractDiagnostic {
    pub code: String,
    pub location: DiagnosticLocation,
    pub message: String,
}

impl ContractDiagnostic {
    pub(crate) fn whole(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            location: DiagnosticLocation::Whole,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleContract {
    pub spatial: ModuleSpatialContract,
    pub assembly: AssemblyContract,
    pub traversal: ModuleTraversalContract,
    pub interfaces: Vec<ModuleInterface>,
}

impl ModuleContract {
    #[must_use]
    pub fn canonicalized(mut self) -> Self {
        self.spatial
            .logical_footprint
            .cells
            .sort_by_key(|cell| cell.cell);
        self.spatial
            .clearance_volumes
            .sort_by(|a, b| a.id.cmp(&b.id));
        self.traversal.nodes.sort_by_key(|node| node.id);
        self.traversal.edges.sort_by_key(|edge| edge.id);
        self.interfaces.sort_by(|a, b| a.port.cmp(&b.port));
        self
    }

    pub fn validate(&self) -> Result<(), ContractDiagnostic> {
        self.spatial.validate()?;
        self.assembly.validate()?;
        self.traversal.validate()?;
        let logical_cells = self
            .spatial
            .logical_footprint
            .cells
            .iter()
            .map(|cell| cell.cell)
            .collect::<std::collections::BTreeSet<_>>();
        let mut interface_ports = std::collections::BTreeSet::new();
        for interface in &self.interfaces {
            interface.validate()?;
            if !logical_cells.contains(&interface.cell) {
                return Err(ContractDiagnostic {
                    code: "interface_cell_outside_footprint".to_string(),
                    location: DiagnosticLocation::Face(interface.cell, interface.profile.face),
                    message: format!(
                        "interface port {:?} belongs to a cell outside the logical footprint",
                        interface.port
                    ),
                });
            }
            if !interface_ports.insert(interface.port.as_str()) {
                return Err(ContractDiagnostic::whole(
                    "duplicate_interface_port",
                    format!("interface port {:?} is duplicated", interface.port),
                ));
            }
            if !self.traversal.port_bindings.contains_key(&interface.port) {
                return Err(ContractDiagnostic::whole(
                    "interface_unbound",
                    format!(
                        "interface port {:?} has no traversal binding",
                        interface.port
                    ),
                ));
            }
        }
        let binding_ports = self
            .traversal
            .port_bindings
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        if binding_ports != interface_ports {
            return Err(ContractDiagnostic::whole(
                "interface_binding_set_mismatch",
                format!(
                    "interface ports {interface_ports:?} do not match traversal bindings {binding_ports:?}"
                ),
            ));
        }
        Ok(())
    }
}

/// Validated in-memory form carried by expanded runtime prototypes.
///
/// It is intentionally not serialized: catalog v4 serializes [`ModuleContract`]
/// in integer authoring units and expansion produces this value once.
///
/// **This value is module-local and unrotated.** The prototype that carries it
/// has already had its clockwise turn baked into hulls, lights, spine, deck,
/// ports, and sockets; the contract has not. A consumer that projects
/// interfaces, clearance volumes, or graph nodes into facility space must apply
/// `TilePrototype::assembly`'s `variant.rotation` itself. TR-9 needs the family
/// identity, which is rotation-invariant, and deliberately did not invent a
/// rotation for the quantized boxes and face-local frames — an axis-aligned
/// authoring box has no exact 60-degree image, so rotating it here would have
/// silently widened every clearance volume. TR-10 owns that projection along
/// with the migration that produces the first rotated contracted module.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeModuleContract {
    pub spatial: ModuleSpatialContract,
    pub assembly: AssemblyContract,
    pub interfaces: Vec<ModuleInterface>,
    pub guide: TraversalGuide,
    pub port_bindings: std::collections::BTreeMap<String, observed_traversal::TraversalNodeId>,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use observed_hex::{HexFace, PortClass};
    use observed_traversal::{TraversalEdgeId, TraversalMode, TraversalNodeId};

    use super::*;
    use crate::source::{FloorPolicy, ModuleCellRef};

    fn point(x: i32, y: i32, z: i32) -> QuantizedPoint {
        QuantizedPoint { x, y, z }
    }

    fn local_point(u: i64, v: i64, inward: i64) -> FaceLocalPoint {
        FaceLocalPoint { u, v, inward }
    }

    fn contract() -> ModuleContract {
        let cell = ModuleCellRef {
            q: 0,
            r: 0,
            level: 0,
        };
        ModuleContract {
            spatial: ModuleSpatialContract {
                logical_footprint: LogicalFootprint {
                    cells: vec![LogicalCell {
                        cell,
                        floor: FloorPolicy::Solid,
                    }],
                },
                geometry_envelope: GeometryEnvelope {
                    bounds: QuantizedBox {
                        min: point(-112, -128, 0),
                        max: point(112, 128, 128),
                    },
                },
                clearance_volumes: vec![ClearanceVolume {
                    id: "door_clearance".to_string(),
                    bounds: QuantizedBox {
                        min: point(96, -24, 0),
                        max: point(112, 24, 40),
                    },
                }],
            },
            assembly: AssemblyContract {
                family: ModuleFamilyId("test/flat".to_string()),
                scope: AssemblyScope::Cell,
                family_weight: 1,
            },
            traversal: ModuleTraversalContract {
                nodes: vec![
                    QuantizedTraversalNode {
                        id: TraversalNodeId(0),
                        position: point(-96, 0, 8),
                    },
                    QuantizedTraversalNode {
                        id: TraversalNodeId(1),
                        position: point(96, 0, 8),
                    },
                ],
                edges: vec![CompiledTraversalEdge {
                    id: TraversalEdgeId(0),
                    from: TraversalNodeId(0),
                    to: TraversalNodeId(1),
                    mode: TraversalMode::Walk,
                }],
                port_bindings: BTreeMap::from([
                    ("west".to_string(), TraversalNodeId(0)),
                    ("east".to_string(), TraversalNodeId(1)),
                ]),
            },
            interfaces: [
                ("west", HexFace::West, TraversalNodeId(0)),
                ("east", HexFace::East, TraversalNodeId(1)),
            ]
            .into_iter()
            .map(|(port, face, node)| ModuleInterface {
                port: port.to_string(),
                cell,
                profile: InterfaceProfile {
                    face,
                    class: PortClass::Door,
                    landing: QuantizedPose {
                        position: local_point(0, 0, 8),
                        tangent: FaceLocalDirection {
                            u: 1,
                            v: 0,
                            inward: 0,
                        },
                    },
                    aperture: QuantizedAperture {
                        min_u: -24,
                        min_v: 0,
                        max_u: 24,
                        max_v: 40,
                    },
                    clearance: FaceLocalBox {
                        min: local_point(-24, 0, 0),
                        max: local_point(24, 32, 40),
                    },
                    guide_terminal: QuantizedPose {
                        position: local_point(i64::from(node.0), 0, 8),
                        tangent: FaceLocalDirection {
                            u: 1,
                            v: 0,
                            inward: 0,
                        },
                    },
                },
            })
            .collect(),
        }
    }

    #[test]
    fn complete_v4_contract_round_trips_and_rejects_unknown_fields() {
        let contract = contract().canonicalized();
        contract.validate().expect("synthetic contract validates");
        let text = ron::to_string(&contract).expect("serialize");
        assert_eq!(ron::from_str::<ModuleContract>(&text).unwrap(), contract);
        let unknown = text.replacen("spatial:", "unknown:(),spatial:", 1);
        assert!(ron::from_str::<ModuleContract>(&unknown).is_err());
    }

    #[test]
    fn canonicalization_removes_source_entity_order_from_serialization() {
        let canonical = contract().canonicalized();
        let mut shuffled = contract();
        shuffled.spatial.logical_footprint.cells.reverse();
        shuffled.spatial.clearance_volumes.reverse();
        shuffled.traversal.nodes.reverse();
        shuffled.traversal.edges.reverse();
        shuffled.interfaces.reverse();
        assert_eq!(
            ron::to_string(&canonical).unwrap(),
            ron::to_string(&shuffled.canonicalized()).unwrap()
        );
    }
}
