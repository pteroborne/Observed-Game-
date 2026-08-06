//! Quantized interface fingerprints and the module-local guide they terminate
//! on.
//!
//! `seam_auditor::FaceSignature` already captured a lateral boundary's class,
//! floor height, and headroom. That is not enough to certify a handoff, and it
//! covered lateral faces only. An [`InterfaceProfile`] carries the same
//! physical facts in the frozen normalized connection frame — landing,
//! aperture, clearance, and guide terminal — for vertical faces as well, in
//! exact integer authoring units.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use observed_traversal::{TraversalEdgeId, TraversalMode, TraversalNodeId};

use super::spatial::{
    self, CLEARANCE_HEIGHT_UNITS, UNITS_PER_LEVEL, VERTICAL_CLEARANCE_DEPTH_UNITS,
    VERTICAL_CLEARANCE_HALF_UNITS, quantize_world,
};
use crate::contract::{
    CompiledTraversalEdge, ContractDiagnostic, DiagnosticLocation, FACE_LOCAL_SCALE, FaceLocalBox,
    FaceLocalDirection, FaceLocalPoint, InterfaceProfile, LateralFaceFrame, ModuleInterface,
    ModuleTraversalContract, QuantizedAperture, QuantizedPoint, QuantizedPose,
    QuantizedTraversalNode, VerticalFaceFrame,
};
use crate::source::{AuthoredModule, ModulePort};

/// Face-local units per editor unit of height. Exact by construction: the
/// frozen frames scale one level to [`FACE_LOCAL_SCALE`].
pub const VERTICAL_LOCAL_SCALE: i64 = FACE_LOCAL_SCALE / UNITS_PER_LEVEL as i64;
/// Canonical clearance depth through an interface, in the connection frame.
/// A constant rather than a projected box: an axis-aligned editor volume is
/// not rotation invariant, and a fingerprint that changes with the face a
/// module happens to be authored on is not a compatibility key.
pub const INTERFACE_CLEARANCE_DEPTH_LOCAL: i64 = FACE_LOCAL_SCALE / 8;

/// Travel through an interface, expressed in the normalized connection frame.
const THROUGH: FaceLocalDirection = FaceLocalDirection {
    u: 0,
    v: 0,
    inward: 1,
};

enum FaceFrame {
    Lateral(LateralFaceFrame),
    Vertical(VerticalFaceFrame),
}

impl FaceFrame {
    fn project(&self, point: QuantizedPoint) -> Result<FaceLocalPoint, ContractDiagnostic> {
        match self {
            Self::Lateral(frame) => frame.project_point(point),
            Self::Vertical(frame) => frame.project_point(point),
        }
    }
}

/// The compiled traversal graph plus the port bindings that terminate it.
pub struct CompiledGuide {
    pub contract: ModuleTraversalContract,
}

fn distance_squared(a: QuantizedPoint, b: QuantizedPoint) -> i64 {
    let dx = i64::from(a.x) - i64::from(b.x);
    let dy = i64::from(a.y) - i64::from(b.y);
    let dz = i64::from(a.z) - i64::from(b.z);
    dx * dx + dy * dy + dz * dz
}

/// The nearest already-placed node, ties broken by the lowest node ID so
/// authoring order can never change the compiled graph.
fn nearest(
    nodes: &[QuantizedTraversalNode],
    candidates: &[TraversalNodeId],
    to: QuantizedPoint,
) -> Option<TraversalNodeId> {
    candidates
        .iter()
        .copied()
        .filter_map(|id| {
            nodes
                .iter()
                .find(|node| node.id == id)
                .map(|node| (distance_squared(node.position, to), id))
        })
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, id)| id)
}

/// Highest authored surface under the module's own base cell centre, used as
/// the interior anchor when a module declares neither deck nor climb.
fn centre_anchor(module: &AuthoredModule) -> Result<QuantizedPoint, ContractDiagnostic> {
    let base = module
        .footprint
        .first()
        .ok_or_else(|| ContractDiagnostic::whole("empty_logical_footprint", "no authored cells"))?;
    let cell = crate::source::ModuleCellRef {
        q: base.q,
        r: base.r,
        level: base.level,
    };
    let origin = spatial::cell_plan_origin(cell);
    let level = spatial::cell_level_z(cell);
    let hulls = spatial::quantized_hulls(module)?;
    let top = hulls
        .iter()
        .filter(|hull| {
            hull.extent.max.z >= level
                && hull.extent.max.z <= level + UNITS_PER_LEVEL / 2
                && hull.covers_plan_point(origin)
        })
        .map(|hull| hull.extent.max.z)
        .max()
        .unwrap_or(level);
    Ok(QuantizedPoint {
        x: origin.0,
        y: origin.1,
        z: top,
    })
}

/// Compile the authored deck/climb annotations and every declared port into
/// one connected module-local graph. Node and edge IDs are assigned in a fixed
/// order — deck, climb, interior anchor, then ports by name — so the same
/// source always compiles to the same graph.
pub fn compile_guide(module: &AuthoredModule) -> Result<CompiledGuide, ContractDiagnostic> {
    let mut nodes: Vec<QuantizedTraversalNode> = Vec::new();
    let mut edges: Vec<CompiledTraversalEdge> = Vec::new();
    let push_node = |nodes: &mut Vec<QuantizedTraversalNode>, position: QuantizedPoint| {
        let id = TraversalNodeId(u16::try_from(nodes.len()).unwrap_or(u16::MAX));
        nodes.push(QuantizedTraversalNode { id, position });
        id
    };

    let mut deck = Vec::new();
    for node in &module.prototype.deck.nodes {
        let position = quantize_world(*node)?;
        deck.push(push_node(&mut nodes, position));
    }
    let mut spine = Vec::new();
    for node in &module.prototype.spine.nodes {
        let position = quantize_world(*node)?;
        spine.push(push_node(&mut nodes, position));
    }
    let mut interior: Vec<TraversalNodeId> = deck.iter().chain(&spine).copied().collect();
    if interior.is_empty() {
        let anchor = centre_anchor(module)?;
        interior.push(push_node(&mut nodes, anchor));
    }

    let push_edge = |edges: &mut Vec<CompiledTraversalEdge>, from, to, mode| {
        if from == to {
            return;
        }
        let id = TraversalEdgeId(u16::try_from(edges.len()).unwrap_or(u16::MAX));
        edges.push(CompiledTraversalEdge { id, from, to, mode });
    };
    for pair in deck.windows(2) {
        push_edge(&mut edges, pair[0], pair[1], TraversalMode::Walk);
    }
    for pair in spine.windows(2) {
        push_edge(&mut edges, pair[0], pair[1], TraversalMode::Climb);
    }
    if let (Some(&foot), false) = (spine.first(), deck.is_empty()) {
        let position = nodes
            .iter()
            .find(|node| node.id == foot)
            .expect("spine node exists")
            .position;
        if let Some(landing) = nearest(&nodes, &deck, position) {
            push_edge(&mut edges, landing, foot, TraversalMode::Walk);
        }
    }

    let mut port_bindings = BTreeMap::new();
    let mut ports = module.ports.iter().collect::<Vec<_>>();
    ports.sort_by(|a, b| a.name.cmp(&b.name));
    for port in ports {
        if port.name.trim().is_empty() {
            return Err(ContractDiagnostic {
                code: "unnamed_contract_port".to_string(),
                location: DiagnosticLocation::Face(port.cell, port.face),
                message: "a version-3 module names every port so it can be bound".to_string(),
            });
        }
        let anchor = spatial::port_geometry(module, port)?.anchor;
        let attach = nearest(&nodes, &interior, anchor).ok_or_else(|| {
            ContractDiagnostic::whole("empty_traversal_guide", "no interior guide node to bind to")
        })?;
        // The terminal sits on the port's own axis, at the height the guide
        // actually reaches there. A vertical handoff's terminal is therefore
        // the top or foot of the authored climb, not the shared level plane —
        // which is exactly the difference two tower families must not hide.
        let reach = nodes
            .iter()
            .find(|node| node.id == attach)
            .expect("interior node exists")
            .position
            .z;
        let terminal = push_node(
            &mut nodes,
            QuantizedPoint {
                x: anchor.x,
                y: anchor.y,
                z: reach,
            },
        );
        let climbing = port.face.is_vertical() && spine.contains(&attach);
        push_edge(
            &mut edges,
            terminal,
            attach,
            if climbing {
                TraversalMode::Climb
            } else {
                TraversalMode::Walk
            },
        );
        if port_bindings.insert(port.name.clone(), terminal).is_some() {
            return Err(ContractDiagnostic::whole(
                "duplicate_contract_port",
                format!("port name {:?} is declared twice", port.name),
            ));
        }
    }

    let contract = ModuleTraversalContract {
        nodes,
        edges,
        port_bindings,
    };
    assert_connected(&contract)?;
    Ok(CompiledGuide { contract })
}

fn assert_connected(contract: &ModuleTraversalContract) -> Result<(), ContractDiagnostic> {
    let Some(start) = contract.nodes.first().map(|node| node.id) else {
        return Err(ContractDiagnostic::whole(
            "empty_traversal_guide",
            "a module contract needs at least one guide node",
        ));
    };
    let mut adjacency: BTreeMap<TraversalNodeId, Vec<TraversalNodeId>> = BTreeMap::new();
    for edge in &contract.edges {
        adjacency.entry(edge.from).or_default().push(edge.to);
        adjacency.entry(edge.to).or_default().push(edge.from);
    }
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(node) = queue.pop_front() {
        for &next in adjacency.get(&node).into_iter().flatten() {
            if seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    if seen.len() == contract.nodes.len() {
        return Ok(());
    }
    let orphan = contract
        .nodes
        .iter()
        .find(|node| !seen.contains(&node.id))
        .expect("a missing node exists");
    Err(ContractDiagnostic {
        code: "disconnected_traversal_guide".to_string(),
        location: DiagnosticLocation::GuideNode(orphan.id),
        message: format!(
            "guide node {:?} is unreachable from the compiled graph",
            orphan.id
        ),
    })
}

fn pose(position: FaceLocalPoint) -> QuantizedPose {
    QuantizedPose {
        position,
        tangent: THROUGH,
    }
}

/// Compile one interface profile per declared port, in the frozen connection
/// frame, and bind it to its guide terminal.
pub fn compile_interfaces(
    module: &AuthoredModule,
    guide: &ModuleTraversalContract,
) -> Result<Vec<ModuleInterface>, ContractDiagnostic> {
    let mut interfaces = Vec::with_capacity(module.ports.len());
    for port in &module.ports {
        interfaces.push(compile_interface(module, guide, port)?);
    }
    Ok(interfaces)
}

fn compile_interface(
    module: &AuthoredModule,
    guide: &ModuleTraversalContract,
    port: &ModulePort,
) -> Result<ModuleInterface, ContractDiagnostic> {
    let geometry = spatial::port_geometry(module, port)?;
    let origin = geometry.cell_origin;
    let local = |point: QuantizedPoint| QuantizedPoint {
        x: point.x - origin.0,
        y: point.y - origin.1,
        z: point.z,
    };
    let frame = if port.face.is_lateral() {
        FaceFrame::Lateral(LateralFaceFrame::try_new(port.face, geometry.frame_z)?)
    } else {
        FaceFrame::Vertical(VerticalFaceFrame::try_new(port.face, geometry.frame_z)?)
    };

    let landing = frame.project(local(geometry.landing))?;
    let aperture = if port.face.is_lateral() {
        if geometry.headroom < CLEARANCE_HEIGHT_UNITS {
            return Err(ContractDiagnostic {
                code: "insufficient_interface_headroom".to_string(),
                location: DiagnosticLocation::Face(port.cell, port.face),
                message: format!(
                    "port {:?} authors {} units of headroom; {CLEARANCE_HEIGHT_UNITS} are required",
                    port.name, geometry.headroom
                ),
            });
        }
        let mid = spatial::face_midpoint_units(port.face);
        let step = spatial::face_tangent_step(port.face);
        let mut corners = Vec::with_capacity(4);
        for (sign, height) in [
            (1, 0),
            (-1, 0),
            (1, geometry.headroom),
            (-1, geometry.headroom),
        ] {
            corners.push(frame.project(QuantizedPoint {
                x: mid.0 + sign * step.0,
                y: mid.1 + sign * step.1,
                z: geometry.landing.z + height,
            })?);
        }
        aperture_of(&corners)
    } else {
        let mut corners = Vec::with_capacity(4);
        for (dx, dy) in [
            (
                -VERTICAL_CLEARANCE_HALF_UNITS,
                -VERTICAL_CLEARANCE_HALF_UNITS,
            ),
            (
                VERTICAL_CLEARANCE_HALF_UNITS,
                -VERTICAL_CLEARANCE_HALF_UNITS,
            ),
            (
                -VERTICAL_CLEARANCE_HALF_UNITS,
                VERTICAL_CLEARANCE_HALF_UNITS,
            ),
            (VERTICAL_CLEARANCE_HALF_UNITS, VERTICAL_CLEARANCE_HALF_UNITS),
        ] {
            corners.push(frame.project(QuantizedPoint {
                x: dx,
                y: dy,
                z: geometry.frame_z,
            })?);
        }
        aperture_of(&corners)
    };

    let clearance = if port.face.is_lateral() {
        FaceLocalBox {
            min: FaceLocalPoint {
                u: aperture.min_u,
                v: aperture.min_v,
                inward: 0,
            },
            max: FaceLocalPoint {
                u: aperture.max_u,
                v: aperture.min_v + i64::from(CLEARANCE_HEIGHT_UNITS) * VERTICAL_LOCAL_SCALE,
                inward: INTERFACE_CLEARANCE_DEPTH_LOCAL,
            },
        }
    } else {
        let depth = i64::from(VERTICAL_CLEARANCE_DEPTH_UNITS) * VERTICAL_LOCAL_SCALE;
        FaceLocalBox {
            min: FaceLocalPoint {
                u: aperture.min_u,
                v: aperture.min_v,
                inward: -depth,
            },
            max: FaceLocalPoint {
                u: aperture.max_u,
                v: aperture.max_v,
                inward: depth,
            },
        }
    };

    let terminal_id = guide
        .port_bindings
        .get(&port.name)
        .copied()
        .ok_or_else(|| ContractDiagnostic {
            code: "interface_unbound".to_string(),
            location: DiagnosticLocation::Face(port.cell, port.face),
            message: format!("port {:?} has no compiled guide terminal", port.name),
        })?;
    let terminal = guide
        .nodes
        .iter()
        .find(|node| node.id == terminal_id)
        .ok_or_else(|| ContractDiagnostic {
            code: "invalid_port_binding".to_string(),
            location: DiagnosticLocation::GuideNode(terminal_id),
            message: format!("port {:?} binds a node the guide does not own", port.name),
        })?;

    Ok(ModuleInterface {
        port: port.name.clone(),
        cell: port.cell,
        profile: InterfaceProfile {
            face: port.face,
            class: port.class,
            landing: pose(landing),
            aperture,
            clearance,
            guide_terminal: pose(frame.project(local(terminal.position))?),
        },
    })
}

fn aperture_of(corners: &[FaceLocalPoint]) -> QuantizedAperture {
    QuantizedAperture {
        min_u: corners.iter().map(|point| point.u).min().unwrap_or(0),
        min_v: corners.iter().map(|point| point.v).min().unwrap_or(0),
        max_u: corners.iter().map(|point| point.u).max().unwrap_or(0),
        max_v: corners.iter().map(|point| point.v).max().unwrap_or(0),
    }
}
