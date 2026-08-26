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
    /// The foot of this module's own climb, where it stands on this module's
    /// deck. `None` when the module has no climb.
    ///
    /// Named rather than derived because it is what a *descent* into this module
    /// ends at, and the alternatives are all shape readings - the lowest node,
    /// or the one a level under the top terminal. See [`descent`].
    pub climb_foot: Option<TraversalNodeId>,
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
                // An authored graph names its own terminals; nothing in the
                // corpus ships one yet, so there is no climb foot to report and
                // a descent into such a module falls back rather than guessing.
                return Some(Self {
                    instance,
                    revision,
                    graph: graph.clone(),
                    climb_foot: None,
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
            let climb_foot = compiled.climb_bottom();
            return Some(Self {
                instance,
                revision,
                graph: ProjectedTraversalGraph {
                    guide: compiled.guide,
                    port_bindings,
                },
                climb_foot,
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
        // The legacy adapter builds a hub and doorways, never a climb.
        climb_foot: None,
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

/// Whether a graph can serve this crossing at all, from either end.
///
/// The gate used to ask only about the cell being left, which is right for
/// every lateral step and for every climb *out* of a module - the graph that
/// describes the crossing is the one the body is standing in.
///
/// A descent is the exception, and a `RampHead` is the sharp case. It is the
/// empty upper half of a two-level prefab: `placement_tile_archetype` returns
/// `None` for it, so it projects no tile, ships no guide, and answers `false`
/// here. The whole leg path was therefore skipped for a body standing on one
/// and the caller fell through to `finish_vertical_crossing`, whose heading is
/// the last metre of a crossing rather than a way down a ramp. The body pushed
/// at the slope and never descended - `FORMERLY_FAILING_SEED` caught exactly
/// that, which is what it is pinned for.
///
/// The mass it needs to walk down belongs to the cell below in both cases, so
/// the question is whether *that* module ships a graph. [`descent`] then leases
/// it. This is the same correction as the shaft head one storey further out:
/// going down is described by the module you are going down *into*.
pub(super) fn serves_the_crossing(game: &HexWfcMatch, transition: ExternalTransition) -> bool {
    ships_a_graph(game, transition.from)
        || (transition.face == HexFace::Down && ships_a_graph(game, transition.to))
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
    within(game, feet, transition).or_else(|| descent(game, feet, transition))
}

/// The ordinary case: a leg that crosses the module the body is standing in.
fn within(
    game: &HexWfcMatch,
    feet: Vec3,
    transition: ExternalTransition,
) -> Option<HexTraversalCursor> {
    let module = ResolvedModuleGraph::resolve(game, transition.from)?;
    let exit = module.exit_for(transition)?;
    lease_between(module, feet, exit)
}

/// Going down a shaft, which is a leg in the module *below*.
///
/// A climb rises out of the module that owns it: a tower's flight starts on its
/// own deck and tops out on the deck above, so going **up** is a leg across the
/// module the body is already in, and the port binding for `Up` is that flight's
/// far terminal. Every part of the leg system is shaped by that, and it is only
/// true in one direction.
///
/// Going **down** is the same flight walked the other way, and it belongs to the
/// cell below. The body stands on the upper cell's floor at the head of the
/// lower cell's climb - `ring_deck` puts a node exactly there, because that is
/// "where the tower below sets a body down" - and to descend it walks that climb
/// in reverse to its foot, arriving on the lower deck. Nothing in the upper
/// module describes any of it. If the upper module is a shaft head it does not
/// even have a climb: it is capped, so it ships no spine at all.
///
/// So `within` returns `None` for every descent out of a shaft head, and before
/// this the caller fell through to `finish_vertical_crossing`, which is
/// ramp-shaped - it takes the cell's first open lateral face and steers along
/// it. On a ramp that is the last metre of the crossing. On a tower it is a
/// heading into a wall, and the body holds it at full throttle until the match
/// runs out of ticks.
///
/// **Why now.** `forced_route_edges` is a monotone staircase - east, south-east,
/// or up, and never down - so the shipped facility's guaranteed route never
/// descended a shaft, and a weighted route that wanted to could usually go round.
/// A routed corridor skeleton claims climbs as `ShaftOpen` pairs and its BFS
/// walks them in both directions, so descending becomes an ordinary move on a
/// one-cell-wide corridor with nothing to go round by. Measured on the bot soak:
/// seven of twenty-eight layouts stalled with `route_corridors` on, six of them
/// in a shaft head. The defect is older than the routing that found it.
///
/// **The other repair this needed and did not have.** A shaft head declares a
/// `Down` port that no graph terminal answers for, and binding it to the nearest
/// deck node is the obvious companion fix. It was written, and measured, and it
/// changes nothing: on its own the soak still stalled seven of twenty-eight,
/// because walking a body to the lip of a hole is not descending. It is deleted
/// rather than kept beside this, since a second code path that fixes nothing is
/// a second code path to explain.
fn descent(
    game: &HexWfcMatch,
    feet: Vec3,
    transition: ExternalTransition,
) -> Option<HexTraversalCursor> {
    (transition.face == HexFace::Down).then_some(())?;
    let below = ResolvedModuleGraph::resolve(game, transition.to)?;
    // The entry is left to `nearest_node`, not named as the climb's top
    // terminal: the body may have drifted a node along the upper deck before
    // this was asked for, and where it actually stands is the honest answer.
    let foot = below.climb_foot?;
    lease_between(below, feet, foot)
}

fn lease_between(
    module: ResolvedModuleGraph,
    feet: Vec3,
    exit: TraversalNodeId,
) -> Option<HexTraversalCursor> {
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
