//! Driving the production follower and Rapier controller over declared routes.
//!
//! Certification is not a second physics model and not a second steering rule.
//! It builds the scene from the same compiled hulls a match projects, asks
//! [`follow_stateless`] for the same `PlayerIntent` a bot would emit, and hands
//! that intent to the same Rapier character controller the game steps. The one
//! thing it adds is a verdict: did the declared route actually carry a body,
//! deterministically, without leaving the world.

use glam::Vec3;
use observed_hex::HexFace;
use observed_traversal::rapier_controller::{RapierTraversalScene, step_character_with_settings};
use observed_traversal::{
    DeckPath, FIXED_DT, FollowState, FollowTarget, FollowerPose, FpsBody, TraversalDirection,
    TraversalEdge, TraversalEdgeId, TraversalGuide, TraversalMode, TraversalNode, TraversalNodeId,
    TraversalRuntimeProfile, follow_stateless,
};
use sha2::{Digest, Sha256};

use super::model::{
    CertificationAuthority, CertificationFailure, CertificationFailureKind, CertificationReport,
    CertifiedInterface, CertifiedPortPair, FailureTracePoint, TraversalCertificate,
};
use super::preflight::preflight;
use super::route::{GuidePlacement, Leg, RouteError, node_world, plan_route};
use super::scene::{PlacedModule, certification_scene, hull_digest, hulls_from_compiled};
use crate::catalog::CompiledModule;
use crate::contract::ModuleContract;
use crate::tile::TilePrototype;

/// How long one declared leg may take before the route counts as a stall.
/// Six thousand ticks is 100 s of simulated walking: far longer than any
/// authored module leg, and short enough that a corpus run terminates.
const MAX_TICKS_PER_LEG: u32 = 6_000;
/// Trace stride, in ticks. Dense enough to show where a body wandered, sparse
/// enough that a failing certificate stays readable.
const TRACE_EVERY_TICKS: u32 = 30;
const TRACE_LIMIT: usize = 64;

/// What one completed route run produced.
pub(crate) struct WalkResult {
    pub ticks: u32,
    pub intent_digest: [u8; 32],
    pub body_digest: [u8; 32],
}

/// Why one route run stopped early, with the evidence an author needs.
pub(crate) struct WalkFault {
    pub kind: CertificationFailureKind,
    pub message: String,
    pub tick: u32,
    pub last_feet: Vec3,
    pub last_target: Option<Vec3>,
    pub trace: Vec<FailureTracePoint>,
}

/// How close to a deck leg's destination counts as arrival.
///
/// Derived from the runtime profile's capsule rather than chosen: a body whose
/// centre is within its own radius of the declared node is standing on it, and
/// the follower stops steering once it is there.
fn deck_arrival_radius(profile: &TraversalRuntimeProfile) -> f32 {
    profile.requirements().capsule_radius + 0.2
}

fn hash_vec3(hash: &mut Sha256, value: Vec3) {
    for component in [value.x, value.y, value.z] {
        hash.update(component.to_bits().to_le_bytes());
    }
}

/// Run one ordered list of legs from `start` and report what happened.
pub(crate) fn walk_route(
    scene: &RapierTraversalScene,
    profile: &TraversalRuntimeProfile,
    start: Vec3,
    legs: &[Leg],
    approach: Option<&DeckPath>,
) -> Result<WalkResult, WalkFault> {
    let controller = profile.controller();
    let rapier = profile.rapier();
    let half_height = controller.half_height;

    let mut intents = Sha256::new();
    intents.update(b"observed2.certification-intents");
    let mut body = FpsBody::spawned(start + Vec3::Y * half_height, 0.0);
    // Spawn facing the first declared target so the run exercises the route
    // rather than the follower's recovery from an arbitrary initial bearing.
    if let Some(leg) = legs.first() {
        let feet = body.position - Vec3::Y * half_height;
        if let Some(target) = follow_stateless(
            FollowerPose { feet, yaw: 0.0 },
            follow_target(leg, approach),
            profile,
        )
        .target
        {
            let to = target - feet;
            if to.length_squared() > 1.0e-6 {
                body.yaw = to.x.atan2(-to.z);
            }
        }
    }

    let mut trace = Vec::new();
    let mut total_ticks = 0_u32;
    let arrival = deck_arrival_radius(profile);

    for (index, leg) in legs.iter().enumerate() {
        let mut leg_ticks = 0_u32;
        let mut last_target = None;
        loop {
            let feet = body.position - Vec3::Y * half_height;
            if !feet.is_finite() {
                return Err(fault(
                    CertificationFailureKind::NonFiniteBody,
                    format!("leg {index} produced a non-finite body"),
                    total_ticks,
                    feet,
                    last_target,
                    trace,
                ));
            }
            let decision = follow_stateless(
                FollowerPose {
                    feet,
                    yaw: body.yaw,
                },
                follow_target(leg, approach),
                profile,
            );
            last_target = decision.target;
            if arrived(leg, feet, decision.state, arrival) {
                break;
            }
            let Some(intent) = decision.intent else {
                return Err(fault(
                    CertificationFailureKind::FollowUnavailable,
                    format!("leg {index} has no follower intent at tick {leg_ticks}"),
                    total_ticks,
                    feet,
                    last_target,
                    trace,
                ));
            };
            intents.update(intent.movement.x.to_bits().to_le_bytes());
            intents.update(intent.movement.y.to_bits().to_le_bytes());
            intents.update(intent.look.x.to_bits().to_le_bytes());
            intents.update(intent.look.y.to_bits().to_le_bytes());
            intents.update([
                u8::from(intent.jump_pressed),
                u8::from(intent.sprint_held),
                u8::from(intent.interact_pressed),
                u8::from(intent.interact_held),
                u8::from(intent.climb_pressed),
            ]);

            let step =
                step_character_with_settings(scene, &mut body, intent, &controller, rapier, FIXED_DT);
            if step.recovered {
                return Err(fault(
                    CertificationFailureKind::Recovered,
                    format!("leg {index} left the scene and the controller recovered it"),
                    total_ticks,
                    feet,
                    last_target,
                    trace,
                ));
            }
            leg_ticks += 1;
            total_ticks += 1;
            if total_ticks.is_multiple_of(TRACE_EVERY_TICKS) && trace.len() < TRACE_LIMIT {
                trace.push(FailureTracePoint {
                    tick: total_ticks,
                    feet: (body.position - Vec3::Y * half_height).to_array(),
                    target: last_target.map(|target| target.to_array()),
                });
            }
            if leg_ticks >= MAX_TICKS_PER_LEG {
                return Err(fault(
                    CertificationFailureKind::TimedOut,
                    format!("leg {index} did not reach its destination in {MAX_TICKS_PER_LEG} ticks"),
                    total_ticks,
                    body.position - Vec3::Y * half_height,
                    last_target,
                    trace,
                ));
            }
        }
    }

    let mut bodies = Sha256::new();
    bodies.update(b"observed2.certification-body");
    hash_vec3(&mut bodies, body.position);
    hash_vec3(&mut bodies, body.velocity);
    bodies.update(body.yaw.to_bits().to_le_bytes());
    bodies.update(body.pitch.to_bits().to_le_bytes());
    bodies.update([u8::from(body.grounded)]);

    Ok(WalkResult {
        ticks: total_ticks,
        intent_digest: intents.finalize().into(),
        body_digest: bodies.finalize().into(),
    })
}

fn follow_target<'a>(leg: &'a Leg, approach: Option<&'a DeckPath>) -> FollowTarget<'a> {
    match leg {
        Leg::Deck { path, goal } => FollowTarget::Deck {
            path,
            goal: *goal,
            handoff: None,
        },
        Leg::Climb {
            spine, direction, ..
        } => FollowTarget::Climb {
            spine,
            approach,
            direction: *direction,
        },
    }
}

fn arrived(leg: &Leg, feet: Vec3, state: FollowState, arrival: f32) -> bool {
    match leg {
        Leg::Deck { .. } => feet.distance(leg.destination()) <= arrival,
        Leg::Climb { .. } => state == FollowState::PassedClimbTerminal,
    }
}

fn fault(
    kind: CertificationFailureKind,
    message: String,
    tick: u32,
    feet: Vec3,
    target: Option<Vec3>,
    trace: Vec<FailureTracePoint>,
) -> WalkFault {
    WalkFault {
        kind,
        message,
        tick,
        last_feet: feet,
        last_target: target,
        trace,
    }
}

pub(crate) fn walk_failure(
    module_id: &str,
    entry: Option<String>,
    exit: Option<String>,
    stage: &str,
    fault: WalkFault,
) -> CertificationFailure {
    CertificationFailure {
        module_id: module_id.to_owned(),
        entry,
        exit,
        edge: None,
        stage: stage.to_owned(),
        tick: fault.tick,
        kind: fault.kind,
        message: fault.message,
        last_feet: Some(fault.last_feet.to_array()),
        last_target: fault.last_target.map(|target| target.to_array()),
        trace: fault.trace,
    }
}

/// Run one route twice from clean state and require identical digests.
///
/// Determinism is a certificate field, not a hope: the profile hash names the
/// physics, and a nonrepeatable run means the certificate cannot identify what
/// it proved.
pub(crate) fn walk_route_repeatably(
    scene: &RapierTraversalScene,
    profile: &TraversalRuntimeProfile,
    start: Vec3,
    legs: &[Leg],
    approach: Option<&DeckPath>,
) -> Result<WalkResult, WalkFault> {
    let first = walk_route(scene, profile, start, legs, approach)?;
    let second = walk_route(scene, profile, start, legs, approach)?;
    if first.ticks != second.ticks
        || first.intent_digest != second.intent_digest
        || first.body_digest != second.body_digest
    {
        return Err(WalkFault {
            kind: CertificationFailureKind::Nonrepeatable,
            message: "two identical runs produced different intent/body digests".to_string(),
            tick: first.ticks,
            last_feet: start,
            last_target: None,
            trace: Vec::new(),
        });
    }
    Ok(first)
}

fn parse_structural_hash(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(bytes)
}

/// The declared guide as an `observed_traversal` graph, so a certificate can
/// carry the same guide identity the runtime consumes.
pub(crate) fn contract_guide(contract: &ModuleContract) -> Option<TraversalGuide> {
    let nodes = contract
        .traversal
        .nodes
        .iter()
        .map(|node| TraversalNode {
            id: node.id,
            position: node_world(node.position).to_array(),
        })
        .collect::<Vec<_>>();
    let edges = contract
        .traversal
        .edges
        .iter()
        .map(|edge| TraversalEdge {
            id: edge.id,
            from: edge.from,
            to: edge.to,
            mode: edge.mode,
        })
        .collect::<Vec<_>>();
    TraversalGuide::try_new(nodes, edges).ok()
}

/// A compatibility graph over the spine and deck polylines of a pre-contract
/// module, used only to give its certificate a guide identity.
fn compatibility_guide(prototype: &TilePrototype) -> Option<TraversalGuide> {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut push_run = |points: &[Vec3], mode: TraversalMode| {
        if points.len() < 2 {
            return;
        }
        #[allow(clippy::cast_possible_truncation)]
        let base = nodes.len() as u16;
        for (index, point) in points.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let id = TraversalNodeId(base + index as u16);
            nodes.push(TraversalNode {
                id,
                position: point.to_array(),
            });
            if index > 0 {
                #[allow(clippy::cast_possible_truncation)]
                let edge = TraversalEdgeId(edges.len() as u16);
                edges.push(TraversalEdge {
                    id: edge,
                    from: TraversalNodeId(base + index as u16 - 1),
                    to: id,
                    mode,
                });
            }
        }
    };
    push_run(&prototype.deck.nodes, TraversalMode::Walk);
    push_run(&prototype.spine.nodes, TraversalMode::Climb);
    if nodes.is_empty() {
        return None;
    }
    TraversalGuide::try_new(nodes, edges).ok()
}

/// Certify one complete v3 module contract against its compiled hulls.
///
/// Every declared port pair is exercised in both directions; the exact-integer
/// preflight runs first, so a corrupted landing, aperture, clearance, or guide
/// terminal fails before any body is simulated.
#[must_use]
pub fn certify_compiled_module(
    module: &CompiledModule,
    hulls: &[Vec<[f32; 3]>],
    profile: &TraversalRuntimeProfile,
) -> CertificationReport {
    let Some(contract) = module.contract.as_ref() else {
        return CertificationReport {
            authority: CertificationAuthority::Contract,
            certificate: None,
            failures: vec![CertificationFailure::module(
                &module.id,
                CertificationFailureKind::MissingContract,
                "module declares no v3 contract, so it cannot be authoritatively certified",
            )],
        };
    };
    let failures = preflight(&module.id, contract);
    if !failures.is_empty() {
        return CertificationReport {
            authority: CertificationAuthority::Contract,
            certificate: None,
            failures,
        };
    }
    let Some(structural_hash) = parse_structural_hash(&module.structural_hash) else {
        return CertificationReport {
            authority: CertificationAuthority::Contract,
            certificate: None,
            failures: vec![CertificationFailure::module(
                &module.id,
                CertificationFailureKind::InvalidStructuralHash,
                format!("structural hash {:?} is not 32 hex bytes", module.structural_hash),
            )],
        };
    };
    let Some(guide) = contract_guide(contract) else {
        return CertificationReport {
            authority: CertificationAuthority::Contract,
            certificate: None,
            failures: vec![CertificationFailure::module(
                &module.id,
                CertificationFailureKind::InvalidGuide,
                "declared traversal graph is not a valid guide",
            )],
        };
    };

    let placed = PlacedModule::new(hulls_from_compiled(hulls), 0, 0);
    if placed.hulls.is_empty() {
        return CertificationReport {
            authority: CertificationAuthority::Contract,
            certificate: None,
            failures: vec![CertificationFailure::module(
                &module.id,
                CertificationFailureKind::InvalidHull,
                "module has no structural hulls to build a scene from",
            )],
        };
    }
    let arena = certification_scene(std::slice::from_ref(&placed), module.levels);
    let scene = RapierTraversalScene::from_arena_spec(&arena);

    let mut interfaces = Vec::new();
    let mut ports = Vec::new();
    for interface in &contract.interfaces {
        interfaces.push(CertifiedInterface {
            port: interface.port.clone(),
            fingerprint: interface.profile.fingerprint(),
        });
        ports.push((interface.port.clone(), interface.profile.face));
    }

    let mut pairs = Vec::new();
    let mut failures = Vec::new();
    for (entry, entry_face) in &ports {
        for (exit, exit_face) in &ports {
            if entry == exit {
                continue;
            }
            // A `Down` interface is only ever physically supported by the
            // member underneath it. Walking it in an isolated scene would
            // certify a body standing on nothing, so the stacked assembly
            // runs own those directions instead.
            if *entry_face == HexFace::Down || *exit_face == HexFace::Down {
                continue;
            }
            let legs = match plan_route(
                &contract.traversal,
                entry,
                exit,
                GuidePlacement::identity(),
            ) {
                Ok(legs) => legs,
                Err(error) => {
                    failures.push(route_failure(&module.id, entry, exit, error));
                    continue;
                }
            };
            let start = leg_start(&legs);
            match walk_route_repeatably(&scene, profile, start, &legs, None) {
                Ok(result) => pairs.push(CertifiedPortPair {
                    entry: entry.clone(),
                    exit: exit.clone(),
                    ticks: result.ticks,
                    intent_digest: result.intent_digest,
                    body_digest: result.body_digest,
                }),
                Err(fault) => failures.push(walk_failure(
                    &module.id,
                    Some(entry.clone()),
                    Some(exit.clone()),
                    "directed_pair",
                    fault,
                )),
            }
        }
    }

    if !failures.is_empty() {
        return CertificationReport {
            authority: CertificationAuthority::Contract,
            certificate: None,
            failures,
        };
    }
    CertificationReport {
        authority: CertificationAuthority::Contract,
        certificate: Some(TraversalCertificate::build(
            module.id.clone(),
            contract.assembly.family.0.clone(),
            structural_hash,
            guide.guide_hash(),
            profile.profile_hash(),
            interfaces,
            pairs,
        )),
        failures: Vec::new(),
    }
}

/// The first declared node of the first leg: where a body enters the route.
pub(crate) fn leg_start(legs: &[Leg]) -> Vec3 {
    match legs.first() {
        Some(Leg::Deck { path, .. }) => path.nodes.first().copied().unwrap_or(Vec3::ZERO),
        Some(Leg::Climb {
            spine, direction, ..
        }) => {
            let node = if *direction == TraversalDirection::Forward {
                spine.nodes.first()
            } else {
                spine.nodes.last()
            };
            node.copied().unwrap_or(Vec3::ZERO)
        }
        None => Vec3::ZERO,
    }
}

fn route_failure(
    module_id: &str,
    entry: &str,
    exit: &str,
    error: RouteError,
) -> CertificationFailure {
    let (kind, message) = match error {
        RouteError::UnboundPort => (
            CertificationFailureKind::TerminalBindingMismatch,
            format!("port pair {entry:?} -> {exit:?} names an unbound port"),
        ),
        RouteError::NoRoute => (
            CertificationFailureKind::NoRoute,
            format!("the declared graph joins no route from {entry:?} to {exit:?}"),
        ),
        RouteError::DegenerateNode(node) => (
            CertificationFailureKind::InvalidGuide,
            format!("guide node {node:?} is not finite in world space"),
        ),
    };
    let mut failure = CertificationFailure::module(module_id, kind, message);
    failure.entry = Some(entry.to_owned());
    failure.exit = Some(exit.to_owned());
    failure.stage = "route".to_owned();
    failure
}

/// Certify what a pre-contract module can still prove: that a body walks its
/// declared climb in both directions, and its declared deck end to end.
///
/// This is compatibility evidence, not the authoritative contract certificate.
/// It cannot detect a corrupted landing, aperture, or clearance, because a v1
/// or v2 module never declared one.
#[must_use]
pub fn certify_runtime_prototype(
    id: &str,
    prototype: &TilePrototype,
    profile: &TraversalRuntimeProfile,
) -> CertificationReport {
    let spine_declared = !prototype.spine.is_empty();
    let deck_declared = !prototype.deck.is_empty();
    if !spine_declared && !deck_declared {
        return CertificationReport {
            authority: CertificationAuthority::NotApplicable,
            certificate: None,
            failures: Vec::new(),
        };
    }
    if prototype.hulls.is_empty() {
        return CertificationReport {
            authority: CertificationAuthority::Compatibility,
            certificate: None,
            failures: vec![CertificationFailure::module(
                id,
                CertificationFailureKind::InvalidHull,
                "module has no structural hulls to build a scene from",
            )],
        };
    }
    let placed = PlacedModule::new(prototype.hulls.clone(), 0, 0);
    let arena = certification_scene(std::slice::from_ref(&placed), prototype.levels);
    let scene = RapierTraversalScene::from_arena_spec(&arena);
    let Some(guide) = compatibility_guide(prototype) else {
        return CertificationReport {
            authority: CertificationAuthority::Compatibility,
            certificate: None,
            failures: vec![CertificationFailure::module(
                id,
                CertificationFailureKind::InvalidGuide,
                "declared spine/deck polylines are not a valid guide",
            )],
        };
    };

    let mut pairs = Vec::new();
    let mut failures = Vec::new();
    let mut cases: Vec<(&str, &str, Vec<Leg>, Vec3)> = Vec::new();
    if spine_declared {
        for (entry, exit, ascending) in [
            ("climb_bottom", "climb_top", true),
            ("climb_top", "climb_bottom", false),
        ] {
            if let Some((start, leg, _)) =
                super::route::compatibility_climb(&prototype.spine, &prototype.deck, ascending)
            {
                cases.push((entry, exit, vec![leg], start));
            }
        }
    }
    if deck_declared {
        let nodes = &prototype.deck.nodes;
        let (first, last) = (nodes[0], nodes[nodes.len() - 1]);
        cases.push((
            "deck_start",
            "deck_end",
            vec![Leg::Deck {
                path: prototype.deck.clone(),
                goal: last,
            }],
            first,
        ));
        cases.push((
            "deck_end",
            "deck_start",
            vec![Leg::Deck {
                path: prototype.deck.clone(),
                goal: first,
            }],
            last,
        ));
    }

    let approach = deck_declared.then(|| prototype.deck.clone());
    for (entry, exit, legs, start) in cases {
        match walk_route_repeatably(&scene, profile, start, &legs, approach.as_ref()) {
            Ok(result) => pairs.push(CertifiedPortPair {
                entry: entry.to_string(),
                exit: exit.to_string(),
                ticks: result.ticks,
                intent_digest: result.intent_digest,
                body_digest: result.body_digest,
            }),
            Err(fault) => failures.push(walk_failure(
                id,
                Some(entry.to_string()),
                Some(exit.to_string()),
                "compatibility_leg",
                fault,
            )),
        }
    }

    if !failures.is_empty() {
        return CertificationReport {
            authority: CertificationAuthority::Compatibility,
            certificate: None,
            failures,
        };
    }
    CertificationReport {
        authority: CertificationAuthority::Compatibility,
        certificate: Some(TraversalCertificate::build(
            id.to_owned(),
            prototype.key.archetype.clone(),
            hull_digest(&prototype.hulls),
            guide.guide_hash(),
            profile.profile_hash(),
            Vec::new(),
            pairs,
        )),
        failures: Vec::new(),
    }
}
