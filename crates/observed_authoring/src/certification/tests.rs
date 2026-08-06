//! A synthetic but complete v3 tower, certified end to end.
//!
//! The fixture is deliberately a whole module rather than a stub: a floor with
//! a shaft aperture, a walkable ramp at the documented 8-over-12 slope, the
//! landing that closes the shaft from above, and the two side strips a body
//! uses to leave the shaft on the level it arrived at. Everything the packet
//! must cover — both directions of every declared pair, the four handoffs,
//! fingerprint-deduplicated stacking, a three-cell column, no out-of-world
//! recovery, and deterministic digests — is proved against that geometry.

use std::collections::BTreeMap;

use glam::Vec3;
use observed_hex::{HexFace, PortClass};
use observed_traversal::{
    TraversalEdgeId, TraversalMode, TraversalNodeId, TraversalRuntimeProfile,
};

use super::route::{GuidePlacement, Leg, plan_route};
use super::*;
use crate::catalog::{CompiledHullSet, CompiledModule, CompiledPort, CompiledTileCatalog};
use crate::contract::{
    AssemblyContract, AssemblyScope, ClearanceVolume, CompiledTraversalEdge, FACE_LOCAL_SCALE,
    FaceLocalBox, FaceLocalDirection, FaceLocalPoint, GeometryEnvelope, InterfaceProfile,
    LateralFaceFrame, LogicalCell, LogicalFootprint, ModuleContract, ModuleFamilyId,
    ModuleInterface, ModuleSpatialContract, ModuleTraversalContract, QuantizedAperture,
    QuantizedBox, QuantizedPoint, QuantizedPose, QuantizedTraversalNode, VerticalFaceFrame,
};
use crate::source::{FloorPolicy, ModuleCell, ModuleCellRef, ModuleKind};
use crate::tile::{class_name, face_name};

const CELL: ModuleCellRef = ModuleCellRef {
    q: 0,
    r: 0,
    level: 0,
};
/// Face-local units in two metres: the padding every synthetic aperture and
/// clearance box uses. `FACE_LOCAL_SCALE / 128` is one editor unit.
const PAD: i64 = 2 * 16 * (FACE_LOCAL_SCALE / 128);

fn world(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// World metres to exact TrenchBroom integer units, Z-up.
fn editor(point: Vec3) -> QuantizedPoint {
    let unit = |value: f32| {
        let scaled = value * 16.0;
        assert_eq!(scaled.fract(), 0.0, "fixture coordinates must be exact");
        #[allow(clippy::cast_possible_truncation)]
        let quantized = scaled as i32;
        quantized
    };
    QuantizedPoint {
        x: unit(point.x),
        y: unit(-point.z),
        z: unit(point.y),
    }
}

fn box_hull(min: Vec3, max: Vec3) -> Vec<[f32; 3]> {
    let mut points = Vec::with_capacity(8);
    for x in [min.x, max.x] {
        for y in [min.y, max.y] {
            for z in [min.z, max.z] {
                points.push([x, y, z]);
            }
        }
    }
    points
}

/// Floor with a shaft aperture, the ramp through it, the landing that closes
/// it from above, and the two strips joining that landing to the next floor.
fn tower_hulls() -> Vec<Vec<[f32; 3]>> {
    vec![
        // Floor: hole at x in [-5, 5], z in [-8, 0].
        box_hull(world(-7.0, -0.5, 0.0), world(7.0, 0.0, 8.0)),
        box_hull(world(-7.0, -0.5, -8.0), world(-5.0, 0.0, 0.0)),
        box_hull(world(5.0, -0.5, -8.0), world(7.0, 0.0, 0.0)),
        // Ramp: rise 8 over run 12, inside the shaft's x extent.
        vec![
            [-3.0, 0.0, 6.0],
            [3.0, 0.0, 6.0],
            [-3.0, -0.6, 6.0],
            [3.0, -0.6, 6.0],
            [-3.0, 8.0, -6.0],
            [3.0, 8.0, -6.0],
            [-3.0, 7.4, -6.0],
            [3.0, 7.4, -6.0],
        ],
        // Landing at the head of the climb, closing the shaft from above.
        box_hull(world(-5.0, 7.5, -8.0), world(5.0, 8.0, -6.0)),
        // Strips beside the shaft, joining that landing to the floor above.
        box_hull(world(-5.0, 7.5, -6.0), world(-3.0, 8.0, 0.0)),
        box_hull(world(3.0, 7.5, -6.0), world(5.0, 8.0, 0.0)),
    ]
}

fn node(id: u16, position: Vec3) -> QuantizedTraversalNode {
    QuantizedTraversalNode {
        id: TraversalNodeId(id),
        position: editor(position),
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

const DOWN_TERMINAL: Vec3 = Vec3::new(0.0, 0.0, -7.0);
const UP_TERMINAL: Vec3 = Vec3::new(0.0, 8.0, -7.0);
/// The door sits beyond the foot of the ramp, not beside it. A route across
/// the middle of the cell would pass under the ramp's own underside, which is
/// below standing height for most of its run — the first thing this fixture's
/// certification run found, and a fair example of what the packet is for.
const WEST_TERMINAL: Vec3 = Vec3::new(-6.5, 0.0, 7.0);

fn traversal() -> ModuleTraversalContract {
    ModuleTraversalContract {
        nodes: vec![
            node(0, DOWN_TERMINAL),
            node(1, world(4.0, 0.0, -7.0)),
            node(2, world(4.0, 0.0, -4.0)),
            node(3, world(4.0, 0.0, 3.0)),
            node(4, world(0.0, 0.0, 6.0)),
            node(5, world(0.0, 8.0, -6.0)),
            node(6, UP_TERMINAL),
            node(7, WEST_TERMINAL),
            node(8, world(4.0, 0.0, 7.0)),
        ],
        edges: vec![
            edge(0, 0, 1, TraversalMode::Walk),
            edge(1, 1, 2, TraversalMode::Walk),
            edge(2, 2, 3, TraversalMode::Walk),
            edge(3, 3, 8, TraversalMode::Walk),
            edge(4, 8, 4, TraversalMode::Walk),
            edge(5, 4, 5, TraversalMode::Climb),
            edge(6, 5, 6, TraversalMode::Walk),
            edge(7, 8, 7, TraversalMode::Walk),
        ],
        port_bindings: BTreeMap::from([
            ("down".to_string(), TraversalNodeId(0)),
            ("up".to_string(), TraversalNodeId(6)),
            ("west".to_string(), TraversalNodeId(7)),
        ]),
    }
}

/// Project a bound node exactly the way certification's preflight does, which
/// is what a real compiler must also do: an interface's landing and terminal
/// are the guide node seen from the connection face.
fn project(face: HexFace, terminal: Vec3) -> FaceLocalPoint {
    let point = editor(terminal);
    if face.is_vertical() {
        let boundary = if face == HexFace::Up { 128 } else { 0 };
        VerticalFaceFrame::try_new(face, boundary)
            .expect("vertical frame")
            .project_point(point)
            .expect("projection")
    } else {
        LateralFaceFrame::try_new(face, 0)
            .expect("lateral frame")
            .project_point(point)
            .expect("projection")
    }
}

fn tangent() -> FaceLocalDirection {
    FaceLocalDirection {
        u: 1,
        v: 0,
        inward: 0,
    }
}

fn interface(port: &str, face: HexFace, class: PortClass, terminal: Vec3) -> ModuleInterface {
    let projected = project(face, terminal);
    ModuleInterface {
        port: port.to_string(),
        cell: CELL,
        profile: InterfaceProfile {
            face,
            class,
            landing: QuantizedPose {
                position: projected,
                tangent: tangent(),
            },
            aperture: QuantizedAperture {
                min_u: projected.u - PAD,
                min_v: projected.v - PAD,
                max_u: projected.u + PAD,
                max_v: projected.v + PAD,
            },
            clearance: FaceLocalBox {
                min: FaceLocalPoint {
                    u: projected.u - PAD,
                    v: projected.v - PAD,
                    inward: projected.inward - PAD,
                },
                max: FaceLocalPoint {
                    u: projected.u + PAD,
                    v: projected.v + PAD,
                    inward: projected.inward + PAD,
                },
            },
            guide_terminal: QuantizedPose {
                position: projected,
                tangent: tangent(),
            },
        },
    }
}

fn tower_contract() -> ModuleContract {
    ModuleContract {
        spatial: ModuleSpatialContract {
            logical_footprint: LogicalFootprint {
                cells: vec![LogicalCell {
                    cell: CELL,
                    floor: FloorPolicy::Open,
                }],
            },
            geometry_envelope: GeometryEnvelope {
                bounds: QuantizedBox {
                    min: QuantizedPoint {
                        x: -112,
                        y: -128,
                        z: -16,
                    },
                    max: QuantizedPoint {
                        x: 112,
                        y: 128,
                        z: 136,
                    },
                },
            },
            clearance_volumes: vec![ClearanceVolume {
                id: "shaft".to_string(),
                bounds: QuantizedBox {
                    min: QuantizedPoint { x: -80, y: 0, z: 0 },
                    max: QuantizedPoint {
                        x: 80,
                        y: 128,
                        z: 128,
                    },
                },
            }],
        },
        assembly: AssemblyContract {
            family: ModuleFamilyId("test/tower".to_string()),
            scope: AssemblyScope::VerticalColumn,
            family_weight: 1,
        },
        traversal: traversal(),
        interfaces: vec![
            interface("down", HexFace::Down, PortClass::ShaftOpen, DOWN_TERMINAL),
            interface("up", HexFace::Up, PortClass::ShaftOpen, UP_TERMINAL),
            interface("west", HexFace::West, PortClass::Door, WEST_TERMINAL),
        ],
    }
    .canonicalized()
}

const STRUCTURAL_HASH: &str = "aa000000000000000000000000000000000000000000000000000000000000bb";

fn tower_module(id: &str) -> CompiledModule {
    let ports = [
        ("down", HexFace::Down, PortClass::ShaftOpen),
        ("up", HexFace::Up, PortClass::ShaftOpen),
        ("west", HexFace::West, PortClass::Door),
    ]
    .into_iter()
    .map(|(name, face, class)| CompiledPort {
        cell: CELL,
        face: face_name(face).to_string(),
        class: class_name(class).to_string(),
        name: name.to_string(),
    })
    .collect();
    CompiledModule {
        id: id.to_string(),
        source_path: format!("synthetic/{id}.map"),
        source_sha256: String::new(),
        kind: ModuleKind::Cell,
        archetype: "stair_tower".to_string(),
        variant: 0,
        levels: 1,
        room_role: None,
        register_scope: vec!["all".to_string()],
        rotations: vec![0],
        weight: 1,
        footprint: vec![ModuleCell {
            q: 0,
            r: 0,
            level: 0,
            levels: 1,
            floor: FloorPolicy::Open,
        }],
        ports,
        lights: Vec::new(),
        sockets: Vec::new(),
        stair_spine: Vec::new(),
        deck_path: Vec::new(),
        structural_hash: STRUCTURAL_HASH.to_string(),
        legacy_key: None,
        contract: Some(tower_contract()),
    }
}

fn tower_catalog(ids: &[&str]) -> CompiledTileCatalog {
    CompiledTileCatalog {
        version: crate::catalog::CONTRACT_CATALOG_VERSION,
        simulation_content_hash: String::new(),
        hull_sets: vec![CompiledHullSet {
            structural_hash: STRUCTURAL_HASH.to_string(),
            hulls: tower_hulls(),
        }],
        modules: ids.iter().map(|id| tower_module(id)).collect(),
    }
}

/// Ratchet invariant 9: certification runs the match's controller, not a
/// third profile of its own.
fn profile() -> TraversalRuntimeProfile {
    TraversalRuntimeProfile::canonical_hex()
}

#[test]
fn certification_runs_the_shipped_match_controller() {
    let canonical = profile();
    let mut expected = observed_traversal::FpsConfig::deliberate_rapier();
    expected.look_step = 1.0;
    assert_eq!(
        canonical.controller(),
        expected,
        "certification must never invent a third traversal profile"
    );
    assert_ne!(
        canonical.controller(),
        observed_traversal::FpsConfig::default(),
        "the historical authoring-test default is not the match body"
    );
}

#[test]
fn the_declared_route_produces_the_four_authored_handoffs() {
    let contract = tower_contract();
    let legs = plan_route(
        &contract.traversal,
        "west",
        "up",
        GuidePlacement::identity(),
    )
    .expect("the tower joins its door to its head");
    assert_eq!(
        legs.len(),
        3,
        "doorway -> deck -> climb -> deck -> aperture"
    );
    assert!(matches!(legs[0], Leg::Deck { .. }), "doorway to deck");
    assert!(matches!(legs[1], Leg::Climb { .. }), "deck to climb");
    assert!(matches!(legs[2], Leg::Deck { .. }), "climb to deck");

    let out = plan_route(
        &contract.traversal,
        "up",
        "west",
        GuidePlacement::identity(),
    )
    .expect("and back again");
    assert!(matches!(out[2], Leg::Deck { .. }), "deck to door");
}

#[test]
fn a_complete_tower_certifies_every_supported_pair_in_both_directions() {
    let module = tower_module("test/tower.a");
    let report = certify_compiled_module(&module, &tower_hulls(), &profile());
    assert!(
        report.failures.is_empty(),
        "synthetic tower must certify: {:#?}",
        report.failures
    );
    let certificate = report.certificate.expect("a passing report carries one");
    assert_eq!(report.authority, CertificationAuthority::Contract);
    assert_eq!(certificate.profile_hash, profile().profile_hash());
    assert!(certificate.verify_hash(), "canonical ordering and identity");

    let pairs = certificate
        .directed_pairs
        .iter()
        .map(|pair| (pair.entry.as_str(), pair.exit.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        pairs,
        vec![("up", "west"), ("west", "up")],
        "both directions of every pair a lone module physically supports"
    );
    assert!(certificate.directed_pairs.iter().all(|pair| pair.ticks > 0));
    assert_eq!(
        certificate
            .interfaces
            .iter()
            .map(|interface| interface.port.as_str())
            .collect::<Vec<_>>(),
        vec!["down", "up", "west"],
        "every declared interface is fingerprinted, including the stacked ones"
    );
}

/// One authored value corrupted, and the diagnosis it must produce.
type CorruptionCase = (
    &'static str,
    CertificationFailureKind,
    fn(&mut ModuleContract),
);

/// The packet's exit criterion. Each corruption is a single authored value,
/// and each must be rejected before a body is ever simulated.
#[test]
fn corrupting_a_landing_aperture_clearance_or_guide_terminal_fails_certification() {
    let hulls = tower_hulls();
    let profile = profile();
    let cases: Vec<CorruptionCase> = vec![
        (
            "landing",
            CertificationFailureKind::LandingOutsideAperture,
            |contract| {
                contract.interfaces[2].profile.landing.position.u += 10 * PAD;
            },
        ),
        (
            "aperture",
            CertificationFailureKind::LandingOutsideAperture,
            |contract| {
                let profile = &mut contract.interfaces[2].profile;
                profile.aperture.min_u = profile.landing.position.u + 1;
                profile.aperture.max_u = profile.landing.position.u + 2;
            },
        ),
        (
            "clearance",
            CertificationFailureKind::TerminalOutsideClearance,
            |contract| {
                let profile = &mut contract.interfaces[2].profile;
                profile.clearance.min.inward = profile.guide_terminal.position.inward + 1;
                profile.clearance.max.inward = profile.guide_terminal.position.inward + 2;
            },
        ),
        (
            "guide terminal",
            CertificationFailureKind::TerminalBindingMismatch,
            |contract| {
                contract.interfaces[2].profile.guide_terminal.position.v += PAD / 4;
            },
        ),
    ];

    for (name, expected, corrupt) in cases {
        let mut module = tower_module("test/tower.a");
        let mut contract = module.contract.take().expect("fixture carries a contract");
        corrupt(&mut contract);
        module.contract = Some(contract);
        let report = certify_compiled_module(&module, &hulls, &profile);
        assert!(!report.passed(), "a corrupted {name} must not certify");
        assert!(
            report.certificate.is_none(),
            "a corrupted {name} must not produce a certificate"
        );
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.kind == expected && failure.stage == "preflight"),
            "corrupted {name} expected {expected:?}, got {:#?}",
            report.failures
        );
    }
}

#[test]
fn a_corrupt_module_is_rejected_before_a_catalog_certifies() {
    let mut catalog = tower_catalog(&["test/tower.a"]);
    let contract = catalog.modules[0]
        .contract
        .as_mut()
        .expect("fixture carries a contract");
    contract.interfaces[1].profile.aperture.max_v =
        contract.interfaces[1].profile.aperture.min_v + 1;
    let report = certify_catalog(&catalog, &profile());
    assert!(
        !report.passed(),
        "a corpus containing a corrupt module must not certify"
    );
}

#[test]
fn a_module_without_a_contract_is_not_authoritatively_certified() {
    let mut module = tower_module("test/tower.a");
    module.contract = None;
    let report = certify_compiled_module(&module, &tower_hulls(), &profile());
    assert!(!report.passed());
    assert_eq!(
        report.failures[0].kind,
        CertificationFailureKind::MissingContract
    );
}

#[test]
fn stacking_is_deduplicated_by_fingerprint_and_covers_a_three_cell_column() {
    // Three cosmetic members with identical interfaces: a Cartesian sweep
    // would build nine stacked scenes, all of them the same handoff.
    let catalog = tower_catalog(&["test/tower.a", "test/tower.b", "test/tower.c"]);
    let report = certify_catalog(&catalog, &profile());
    assert!(
        report.failures.is_empty(),
        "corpus certification failed: {:#?}",
        report.failures
    );
    assert!(report.passed());
    assert_eq!(report.tally(), (3, 0, 0), "three contract modules");

    assert_eq!(
        report.vertical_cases.len(),
        2,
        "one pairing per fingerprint class, plus the representative column"
    );
    let pairing = &report.vertical_cases[0];
    let column = &report.vertical_cases[1];
    assert_eq!(pairing.cells, 2);
    assert_eq!(column.cells, 3, "a representative three-cell column");
    assert_eq!(
        pairing.fingerprint, column.fingerprint,
        "both runs belong to the same vertical interface class"
    );
    assert_eq!(
        pairing.members,
        vec![
            "test/tower.a".to_string(),
            "test/tower.b".to_string(),
            "test/tower.c".to_string()
        ],
        "one run certifies every member folded into the class"
    );
    assert_eq!(pairing.lower, "test/tower.a");
    assert!(
        column.ticks > pairing.ticks,
        "a taller column walks further"
    );
    assert_eq!(report.profile_hash, profile().profile_hash());
}

#[test]
fn certification_is_deterministic_and_changes_with_the_profile() {
    let module = tower_module("test/tower.a");
    let hulls = tower_hulls();
    let first = certify_compiled_module(&module, &hulls, &profile())
        .certificate
        .expect("first run certifies");
    let second = certify_compiled_module(&module, &hulls, &profile())
        .certificate
        .expect("second run certifies");
    assert_eq!(first, second, "two clean runs produce the same certificate");

    let mut controller = profile().controller();
    controller.walk_speed *= 0.5;
    let other = TraversalRuntimeProfile::from_controller(controller);
    let changed = certify_compiled_module(&module, &hulls, &other);
    let changed = changed
        .certificate
        .expect("the slower profile still certifies");
    assert_ne!(
        changed.certificate_hash, first.certificate_hash,
        "a certificate names the physics that ran"
    );
}

#[test]
fn a_certificate_round_trips_through_canonical_ron() {
    let certificate =
        certify_compiled_module(&tower_module("test/tower.a"), &tower_hulls(), &profile())
            .certificate
            .expect("certifies");
    let text = certificate.to_pretty_ron().expect("serialize");
    let parsed: TraversalCertificate = ron::from_str(&text).expect("parse");
    assert_eq!(parsed, certificate);
    assert!(parsed.verify_hash());
    let unknown = text.replacen("version:", "unknown: 1,version:", 1);
    assert!(ron::from_str::<TraversalCertificate>(&unknown).is_err());
}

#[test]
fn the_selection_command_scopes_to_one_module_or_family() {
    let catalog = tower_catalog(&["test/tower.a", "test/tower.b"]);
    let one = certify_catalog_selection(&catalog, "test/tower.b", &profile());
    assert_eq!(one.modules.len(), 1);
    assert!(one.passed(), "{:#?}", one.failures);

    let family = certify_catalog_selection(&catalog, "test/tower", &profile());
    assert_eq!(family.modules.len(), 2, "the family selects both members");

    let missing = certify_catalog_selection(&catalog, "test/absent", &profile());
    assert!(!missing.passed());
}
