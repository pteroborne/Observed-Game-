//! Module-local graph legs: the runtime half of the traversal contract.
//!
//! The facility planner is asked exactly one question — *which external port
//! comes next* — and this module answers the other one: *which local edge
//! crosses the selected physical module to reach it*. Everything shape-specific
//! that survives here is confined to the named legacy adapters, so a module that
//! ships its own graph is executed without any of it.

use std::collections::BTreeMap;

use glam::Vec3;
use observed_facility::hex_wfc::{HexArchetype, HexCoord, HexFace};
use observed_hex::{face_edge, hex_origin};
use observed_traversal::{
    FollowerPose, GraphFollowDecision, GraphFollowState, TraversalGuide, TraversalGuideBuilder,
    TraversalMode, TraversalNodeId, compile_compatibility_graph, follow_graph,
};

use crate::hex_wfc::{
    HexModuleInstanceId, HexModuleRevision, HexTraversalCursor, HexTraversalLease, ProjectedPort,
    ProjectedTraversalGraph,
};

use super::super::{FLOOR_SLAB_TOP, HexWfcMatch};

/// How far above the floor slab an adapter puts its walkable nodes. Bodies walk
/// on the slab, and a port binding compares a doorway's height against a node's.
const ADAPTER_FLOOR: f32 = FLOOR_SLAB_TOP;

/// The vertical slack a lateral port may bind across. One storey is eight
/// metres, so half a metre keeps a doorway on the floor it was cut into while
/// tolerating authored slab and tread thickness.
const PORT_BIND_MAX_RISE: f32 = 0.5;

/// The single external transition a facility route asks for next.
///
/// This is the entire question the planner answers for local traversal. It
/// carries no geometry, no archetype, and no steering: the port identifies
/// itself by face, and the module says where that face lands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ExternalTransition {
    pub from: HexCoord,
    pub to: HexCoord,
    pub face: HexFace,
}

impl ExternalTransition {
    /// Name the external port that leaves `from` toward the planner's `to`.
    ///
    /// `None` when the two cells do not share a face, which a well-formed route
    /// never produces; callers keep their compatibility path for that case
    /// rather than inventing a port.
    pub(super) fn between(game: &HexWfcMatch, from: HexCoord, to: HexCoord) -> Option<Self> {
        let face = if to.level > from.level {
            HexFace::Up
        } else if to.level < from.level {
            HexFace::Down
        } else {
            HexFace::LATERAL
                .into_iter()
                .find(|&face| game.facility.config.grid().neighbor(from, face) == Some(to))?
        };
        Some(Self { from, to, face })
    }

    fn port(self) -> ProjectedPort {
        ProjectedPort {
            cell: self.from,
            face: self.face,
        }
    }
}

/// One resolved module's graph in facility world space, with its external ports
/// bound to graph terminals.
///
/// Derived, never retained: a cursor stores only stable identities, and this
/// value is rebuilt from the same projection on every tick that needs it. That
/// is what makes a lease safe to hold across a relayout — the revision is
/// compared, not a cached pointer.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ResolvedModuleGraph {
    pub instance: HexModuleInstanceId,
    pub revision: HexModuleRevision,
    pub graph: ProjectedTraversalGraph,
}

impl ResolvedModuleGraph {
    /// The graph for the module anchored at `cell`.
    ///
    /// Three sources, in descending order of authority:
    ///
    /// 1. a projected graph the module actually ships;
    /// 2. the compatibility spine/deck annotations, compiled;
    /// 3. a direct adapter over the cell's own open faces.
    ///
    /// Only (3) reads an archetype, and it reads it to build data rather than to
    /// steer. Retiring it is a matter of authoring graphs, not of editing this
    /// file — which is the whole point of the packet.
    pub(super) fn resolve(game: &HexWfcMatch, cell: HexCoord) -> Option<Self> {
        if let Some(guide) = game.geometry.guides.get(&cell) {
            let instance = guide.instance;
            let revision = guide.revision.clone();
            if let Some(graph) = &guide.graph {
                return Some(Self {
                    instance,
                    revision,
                    graph: graph.clone(),
                });
            }
            let compiled = compile_compatibility_graph(guide.deck.as_ref(), guide.climb.as_ref())?;
            let mut port_bindings = BTreeMap::new();
            if let Some(top) = compiled.climb_top() {
                port_bindings.insert(
                    ProjectedPort {
                        cell,
                        face: HexFace::Up,
                    },
                    top,
                );
            }
            if let Some(bottom) = compiled.climb_bottom() {
                port_bindings.insert(
                    ProjectedPort {
                        cell,
                        face: HexFace::Down,
                    },
                    bottom,
                );
            }
            bind_lateral_ports(game, cell, &compiled.guide, &mut port_bindings);
            return Some(Self {
                instance,
                revision,
                graph: ProjectedTraversalGraph {
                    guide: compiled.guide,
                    port_bindings,
                },
            });
        }
        legacy_cell_adapter(game, cell)
    }

    fn exit_for(&self, transition: ExternalTransition) -> Option<TraversalNodeId> {
        self.graph.port_bindings.get(&transition.port()).copied()
    }
}

/// Bind every open lateral face of `cell` to the graph node nearest its doorway.
///
/// Nearest in plan, on the doorway's own storey. A deck path is authored as the
/// walkable line around a floor, so the node closest to an aperture is the point
/// a body should stand at to leave through it; nothing here needs to know which
/// shape put it there.
fn bind_lateral_ports(
    game: &HexWfcMatch,
    cell: HexCoord,
    guide: &TraversalGuide,
    bindings: &mut BTreeMap<ProjectedPort, TraversalNodeId>,
) {
    let Some(placement) = game.facility.placements.get(&cell) else {
        return;
    };
    for face in HexFace::LATERAL {
        if !placement.is_open(face) {
            continue;
        }
        if let Some(node) = guide.nearest_node_in_plan(doorway(cell, face), PORT_BIND_MAX_RISE) {
            bindings.insert(ProjectedPort { cell, face }, node);
        }
    }
}

/// The world-space midpoint of one lateral aperture, on the floor slab.
fn doorway(cell: HexCoord, face: HexFace) -> Vec3 {
    let origin = Vec3::from_array(hex_origin(cell));
    let [a, b] = face_edge(face);
    Vec3::new(
        origin.x + (a.0 + b.0) as f32 * 0.5,
        origin.y + ADAPTER_FLOOR,
        origin.z + (a.1 + b.1) as f32 * 0.5,
    )
}

/// A direct graph for a legacy cell that carries no authored annotation at all.
///
/// Flat cells become a hub: every open doorway joined to the cell centre by a
/// walk edge, which is exactly the route the compatibility lateral steering
/// describes and nothing more. A ramp additionally joins its low and high
/// doorways with a climb edge, since a ramp's whole purpose is that transition.
///
/// This exists so migration can be incremental: a facility of half-authored
/// modules still presents one uniform graph to the runtime. It is deliberately
/// the *only* place left in leg execution that mentions [`HexArchetype`].
fn legacy_cell_adapter(game: &HexWfcMatch, cell: HexCoord) -> Option<ResolvedModuleGraph> {
    let placement = game.facility.placements.get(&cell)?;
    let revision = HexModuleRevision::single(cell, game.facility.cell_revision(cell)?);
    let origin = Vec3::from_array(hex_origin(cell));

    let mut builder = TraversalGuideBuilder::new();
    let hub = builder.node(Vec3::new(origin.x, origin.y + ADAPTER_FLOOR, origin.z));
    let mut bindings = BTreeMap::new();
    let mut doors = Vec::new();
    for face in HexFace::LATERAL {
        if !placement.is_open(face) {
            continue;
        }
        let node = builder.node(doorway(cell, face));
        builder.connect(hub, node, TraversalMode::Walk);
        bindings.insert(ProjectedPort { cell, face }, node);
        doors.push((face, node));
    }

    // A ramp rises between the face it opens on and the face opposite. That is
    // the same rule the compatibility `ramp_walk_dir` applied, moved out of
    // steering and into data, where a second ramp shape can replace it by
    // shipping a graph instead of by editing a follower.
    if matches!(
        placement.archetype,
        HexArchetype::RampUp | HexArchetype::RampHead
    ) && let Some(&(low, high)) = ramp_faces(placement.archetype, &doors).as_ref()
    {
        builder.connect(low, high, TraversalMode::Climb);
        bindings.insert(
            ProjectedPort {
                cell,
                face: HexFace::Up,
            },
            high,
        );
        bindings.insert(
            ProjectedPort {
                cell,
                face: HexFace::Down,
            },
            low,
        );
    }

    let guide = builder.build().ok()?;
    // A sealed cell yields the hub and nothing else. That is not a route, and
    // presenting it as one would let a bot lease a leg it can never finish.
    if guide.nodes().len() < 2 {
        return None;
    }
    Some(ResolvedModuleGraph {
        instance: HexModuleInstanceId { source_cell: cell },
        revision,
        graph: ProjectedTraversalGraph {
            guide,
            port_bindings: bindings,
        },
    })
}

/// The low and high doorway nodes of a legacy ramp.
///
/// The rise face is chosen exactly as the compatibility `ramp_walk_dir` chose
/// it: from the first open lateral face in `HexFace` order, opposite for a
/// `RampUp` and the face itself for a `RampHead`.
fn ramp_faces(
    archetype: HexArchetype,
    doors: &[(HexFace, TraversalNodeId)],
) -> Option<(TraversalNodeId, TraversalNodeId)> {
    let &(open, _) = doors.first()?;
    let rise = match archetype {
        HexArchetype::RampUp => open.opposite(),
        _ => open,
    };
    let node_at = |face: HexFace| {
        doors
            .iter()
            .find(|(candidate, _)| *candidate == face)
            .map(|&(_, node)| node)
    };
    Some((node_at(rise.opposite())?, node_at(rise)?))
}

/// Whether the module at `cell` presents a traversal graph.
///
/// The single switch that decides whether a leg is executed as a graph. Any
/// annotated module qualifies, because [`ResolvedModuleGraph::resolve`] compiles
/// the compatibility spine and deck into the same graph when the module ships no
/// authored one — a guide is only recorded where a climb or a deck exists, so
/// the compile cannot fail for anything this admits.
///
/// It is deliberately a property of the content rather than a flag: authoring a
/// graph replaces what a module presents here without changing this line.
pub(super) fn ships_a_graph(game: &HexWfcMatch, cell: HexCoord) -> bool {
    game.geometry.guides.contains_key(&cell)
}

/// The follower pose of one player's feet.
pub(super) fn pose(game: &HexWfcMatch, position: Vec3, yaw: f32) -> FollowerPose {
    let half_height = game
        .content
        .traversal_profile()
        .requirements()
        .capsule_half_height;
    FollowerPose {
        feet: Vec3::new(position.x, position.y - half_height, position.z),
        yaw,
    }
}

/// Acquire the local leg that crosses `transition.from` toward its next port.
///
/// Returns `None` when the module does not bind that port, or when the body
/// cannot reach the binding over the module's own graph. Both cases leave the
/// caller free to fall back; neither invents a route.
pub(super) fn acquire(
    game: &HexWfcMatch,
    feet: Vec3,
    transition: ExternalTransition,
) -> Option<HexTraversalCursor> {
    let module = ResolvedModuleGraph::resolve(game, transition.from)?;
    let exit = module.exit_for(transition)?;
    let entry = module.graph.guide.nearest_node(feet)?;
    let local = module.graph.guide.cursor_between(entry, exit)?;
    Some(HexTraversalCursor {
        lease: HexTraversalLease {
            instance: module.instance,
            revision: module.revision,
            entry,
            exit,
        },
        local,
    })
}

/// Advance one leased leg by one tick.
///
/// Re-resolves the module from stable identity and compares the exact revision
/// the lease was taken against. A bounded relayout that replaces this module
/// invalidates the leg; one that replaces a module elsewhere does not.
pub(super) fn follow(
    game: &HexWfcMatch,
    cursor: &mut HexTraversalCursor,
    pose: FollowerPose,
) -> GraphFollowDecision {
    let invalid = GraphFollowDecision {
        state: GraphFollowState::InvalidCursor,
        target: None,
        intent: None,
    };
    let Some(module) = ResolvedModuleGraph::resolve(game, cursor.lease.instance.source_cell) else {
        return invalid;
    };
    if module.revision != cursor.lease.revision {
        return invalid;
    }
    follow_graph(
        pose,
        &module.graph.guide,
        &mut cursor.local,
        cursor.lease.exit,
        game.content.traversal_profile(),
    )
}
