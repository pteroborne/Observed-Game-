//! Corpus certification: every module, plus the stacked assemblies its
//! declared vertical interfaces make possible.
//!
//! The expensive mistake this module exists to avoid is a Cartesian sweep. A
//! corpus of *n* towers that all open upward and downward has *n²* stacked
//! combinations, and almost all of them are the same physical handoff wearing
//! a different register or cosmetic variant. What actually decides whether a
//! body crosses the threshold is the interface: its landing, aperture,
//! clearance, port class, and guide terminal. Those are exactly the fields
//! [`InterfaceProfile::fingerprint`](crate::contract::InterfaceProfile::fingerprint)
//! hashes, so one run per fingerprint class certifies every member of it.

use std::collections::{BTreeMap, BTreeSet};

use glam::Vec3;
use observed_hex::HexFace;
use observed_traversal::{TraversalRuntimeProfile, rapier_controller::RapierTraversalScene};

use super::model::{
    CertificationFailure, CertificationFailureKind, CertificationReport, CertifiedVerticalCase,
    CorpusCertificationReport, TRAVERSAL_CERTIFICATE_VERSION,
};
use super::route::{GuidePlacement, Leg, plan_route};
use super::runner::{
    certify_compiled_module, certify_runtime_prototype, leg_start, walk_failure,
    walk_route_repeatably,
};
use super::scene::{PlacedModule, certification_scene, hulls_from_compiled};
use crate::catalog::{CompiledModule, CompiledTileCatalog};
use crate::contract::{InterfaceFingerprint, ModuleContract, ModuleInterface};
use crate::tile::TilePrototype;

/// The number of stacked cells in the representative vertical column.
const REPRESENTATIVE_COLUMN_CELLS: u8 = 3;

fn hull_set<'a>(
    catalog: &'a CompiledTileCatalog,
    module: &CompiledModule,
) -> Option<&'a [Vec<[f32; 3]>]> {
    catalog
        .hull_sets
        .iter()
        .find(|set| set.structural_hash == module.structural_hash)
        .map(|set| set.hulls.as_slice())
}

fn interfaces_on(contract: &ModuleContract, face: HexFace) -> Vec<&ModuleInterface> {
    contract
        .interfaces
        .iter()
        .filter(|interface| interface.profile.face == face)
        .collect()
}

/// The port a body should enter or leave a stacked member through, preferring
/// a lateral door so the run also exercises the doorway handoffs.
fn side_port(contract: &ModuleContract, avoid: &str) -> Option<String> {
    contract
        .interfaces
        .iter()
        .filter(|interface| interface.port != avoid)
        .min_by_key(|interface| (interface.profile.face.is_vertical(), interface.port.clone()))
        .map(|interface| interface.port.clone())
}

/// Ticks walked, plus the intent and body digests that make the run
/// identifiable. Boxed failures keep the `Result` small enough to pass by
/// value through the corpus loop.
type ColumnEvidence = (u32, [u8; 32], [u8; 32]);

/// One member of a stacked column, already resolved to hulls and a route.
struct ColumnMember<'a> {
    module: &'a CompiledModule,
    hulls: &'a [Vec<[f32; 3]>],
    entry: String,
    exit: String,
}

/// Build and walk one stacked column, bottom member first.
fn walk_column(
    members: &[ColumnMember<'_>],
    profile: &TraversalRuntimeProfile,
) -> Result<ColumnEvidence, Box<CertificationFailure>> {
    let placed = members
        .iter()
        .enumerate()
        .map(|(level, member)| {
            #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
            PlacedModule::new(hulls_from_compiled(member.hulls), level as i32, 0)
        })
        .collect::<Vec<_>>();
    #[allow(clippy::cast_possible_truncation)]
    let levels = (members.len() as u8).saturating_add(1);
    let arena = certification_scene(&placed, levels);
    let scene = RapierTraversalScene::from_arena_spec(&arena);

    let mut legs: Vec<Leg> = Vec::new();
    for (level, member) in members.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let offset = Vec3::Y * (level as f32 * observed_hex::TILE_LEVEL_HEIGHT);
        let contract = member
            .module
            .contract
            .as_ref()
            .expect("column members are contract modules");
        match plan_route(
            &contract.traversal,
            &member.entry,
            &member.exit,
            GuidePlacement::new(offset, 0),
        ) {
            Ok(member_legs) => legs.extend(member_legs),
            Err(error) => {
                let mut failure = CertificationFailure::module(
                    &member.module.id,
                    CertificationFailureKind::NoRoute,
                    format!(
                        "stacked member at level {level} has no route {:?} -> {:?}: {error:?}",
                        member.entry, member.exit
                    ),
                );
                failure.entry = Some(member.entry.clone());
                failure.exit = Some(member.exit.clone());
                failure.stage = "stacked_route".to_string();
                return Err(Box::new(failure));
            }
        }
    }
    let start = leg_start(&legs);
    match walk_route_repeatably(&scene, profile, start, &legs, None) {
        Ok(result) => Ok((result.ticks, result.intent_digest, result.body_digest)),
        Err(fault) => Err(Box::new(walk_failure(
            &members[0].module.id,
            Some(members[0].entry.clone()),
            members.last().map(|member| member.exit.clone()),
            "stacked_column",
            fault,
        ))),
    }
}

/// Every distinct vertical interface class in the catalog, with the modules
/// that can sit below and above it.
struct VerticalClass<'a> {
    fingerprint: InterfaceFingerprint,
    family: String,
    /// `(module, its Up port)` for everything that can carry a neighbour.
    lower: Vec<(&'a CompiledModule, String)>,
    /// `(module, its Down port)` for everything that can stand on one.
    upper: Vec<(&'a CompiledModule, String)>,
}

fn vertical_classes<'a>(modules: &[&'a CompiledModule]) -> Vec<VerticalClass<'a>> {
    let mut classes: BTreeMap<InterfaceFingerprint, VerticalClass<'a>> = BTreeMap::new();
    for module in modules {
        let Some(contract) = module.contract.as_ref() else {
            continue;
        };
        for (face, is_lower) in [(HexFace::Up, true), (HexFace::Down, false)] {
            for interface in interfaces_on(contract, face) {
                let fingerprint = interface.profile.fingerprint();
                let class = classes.entry(fingerprint).or_insert_with(|| VerticalClass {
                    fingerprint,
                    family: contract.assembly.family.0.clone(),
                    lower: Vec::new(),
                    upper: Vec::new(),
                });
                let slot = if is_lower {
                    &mut class.lower
                } else {
                    &mut class.upper
                };
                slot.push((*module, interface.port.clone()));
            }
        }
    }
    let mut ordered = classes.into_values().collect::<Vec<_>>();
    for class in &mut ordered {
        class.lower.sort_by(|a, b| a.0.id.cmp(&b.0.id));
        class.upper.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    }
    ordered
}

fn members_of(class: &VerticalClass<'_>) -> Vec<String> {
    let mut members = class
        .lower
        .iter()
        .chain(class.upper.iter())
        .map(|(module, _)| module.id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    members.sort();
    members
}

/// Certify a whole compiled catalog: every module, then one stacked run per
/// unique vertical interface class, then a representative three-cell column.
#[must_use]
pub fn certify_catalog(
    catalog: &CompiledTileCatalog,
    profile: &TraversalRuntimeProfile,
) -> CorpusCertificationReport {
    certify_selected(catalog, profile, |_| true)
}

/// The fast command's scope: one module ID, one family, or one archetype.
#[must_use]
pub fn certify_catalog_selection(
    catalog: &CompiledTileCatalog,
    selector: &str,
    profile: &TraversalRuntimeProfile,
) -> CorpusCertificationReport {
    certify_selected(catalog, profile, |module| {
        module.id == selector
            || module.archetype == selector
            || module
                .contract
                .as_ref()
                .is_some_and(|contract| contract.assembly.family.0 == selector)
    })
}

fn certify_selected(
    catalog: &CompiledTileCatalog,
    profile: &TraversalRuntimeProfile,
    keep: impl Fn(&CompiledModule) -> bool,
) -> CorpusCertificationReport {
    let mut report = CorpusCertificationReport {
        version: TRAVERSAL_CERTIFICATE_VERSION,
        profile_hash: profile.profile_hash(),
        ..CorpusCertificationReport::default()
    };
    let selected = catalog
        .modules
        .iter()
        .filter(|module| keep(module))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        report.failures.push(CertificationFailure::module(
            "<selection>",
            CertificationFailureKind::MissingContract,
            "no compiled module matched the selection",
        ));
        return report;
    }

    for module in &selected {
        let Some(hulls) = hull_set(catalog, module) else {
            report.failures.push(CertificationFailure::module(
                &module.id,
                CertificationFailureKind::InvalidHull,
                format!("no hull set for structural hash {}", module.structural_hash),
            ));
            continue;
        };
        report.modules.push(if module.contract.is_some() {
            certify_compiled_module(module, hulls, profile)
        } else {
            compatibility_report(module, hulls, profile)
        });
    }

    certify_vertical_assemblies(&selected, catalog, profile, &mut report);
    report.modules.sort_by(|a, b| {
        let id = |report: &CertificationReport| {
            report
                .certificate
                .as_ref()
                .map(|certificate| certificate.module_id.clone())
                .or_else(|| {
                    report
                        .failures
                        .first()
                        .map(|failure| failure.module_id.clone())
                })
                .unwrap_or_default()
        };
        id(a).cmp(&id(b))
    });
    report.vertical_cases.sort_by(|a, b| {
        (a.fingerprint, a.cells, &a.lower).cmp(&(b.fingerprint, b.cells, &b.lower))
    });
    report
}

/// Project one pre-contract compiled module back into the runtime prototype
/// the compatibility runner understands. Only the fields a walk consumes are
/// needed, so this deliberately avoids the catalog's full rotation expansion.
fn compatibility_report(
    module: &CompiledModule,
    hulls: &[Vec<[f32; 3]>],
    profile: &TraversalRuntimeProfile,
) -> CertificationReport {
    use observed_hex::{PortClass, PortSignature};
    use observed_traversal::{DeckPath, StairSpine};

    let prototype = TilePrototype {
        key: crate::manifest::TileKey {
            archetype: module.archetype.clone(),
            register: module
                .register_scope
                .first()
                .cloned()
                .unwrap_or_else(|| "generic".to_string()),
            variant: module.variant,
        },
        weight: module.weight,
        levels: module.levels,
        signature: PortSignature::try_from_ports([PortClass::Sealed; 8])
            .expect("a sealed signature is always valid"),
        hulls: hulls_from_compiled(hulls),
        lights: Vec::new(),
        spine: StairSpine {
            nodes: module
                .stair_spine
                .iter()
                .copied()
                .map(Vec3::from_array)
                .collect(),
        },
        deck: DeckPath {
            nodes: module
                .deck_path
                .iter()
                .copied()
                .map(Vec3::from_array)
                .collect(),
        },
        contract: None,
        assembly: None,
    };
    certify_runtime_prototype(&module.id, &prototype, profile)
}

fn certify_vertical_assemblies(
    selected: &[&CompiledModule],
    catalog: &CompiledTileCatalog,
    profile: &TraversalRuntimeProfile,
    report: &mut CorpusCertificationReport,
) {
    let classes = vertical_classes(selected);
    if classes.is_empty() {
        return;
    }
    let mut stackable = false;
    let mut three_cell = None;

    for class in &classes {
        let (Some((lower, lower_port)), Some((upper, upper_port))) =
            (class.lower.first(), class.upper.first())
        else {
            report.failures.push(unmatched_class(class));
            continue;
        };
        stackable = true;
        let (Some(lower_hulls), Some(upper_hulls)) =
            (hull_set(catalog, lower), hull_set(catalog, upper))
        else {
            report.failures.push(CertificationFailure::module(
                &lower.id,
                CertificationFailureKind::InvalidHull,
                "a stacked member has no hull set",
            ));
            continue;
        };
        let lower_contract = lower.contract.as_ref().expect("class members are v3");
        let upper_contract = upper.contract.as_ref().expect("class members are v3");
        let Some(lower_entry) = side_port(lower_contract, lower_port) else {
            report.failures.push(unmatched_class(class));
            continue;
        };
        let Some(upper_exit) = side_port(upper_contract, upper_port) else {
            report.failures.push(unmatched_class(class));
            continue;
        };
        let members = [
            ColumnMember {
                module: lower,
                hulls: lower_hulls,
                entry: lower_entry,
                exit: lower_port.clone(),
            },
            ColumnMember {
                module: upper,
                hulls: upper_hulls,
                entry: upper_port.clone(),
                exit: upper_exit,
            },
        ];
        match walk_column(&members, profile) {
            Ok((ticks, intent_digest, body_digest)) => {
                report.vertical_cases.push(CertifiedVerticalCase {
                    family: class.family.clone(),
                    fingerprint: class.fingerprint,
                    lower: lower.id.clone(),
                    upper: upper.id.clone(),
                    members: members_of(class),
                    cells: 2,
                    ticks,
                    intent_digest,
                    body_digest,
                });
            }
            Err(failure) => report.failures.push(*failure),
        }

        // A three-cell column needs one module that both carries and stands on
        // this exact interface, so the middle cell is a real handoff on both
        // faces rather than two unrelated pairings glued together.
        if three_cell.is_none()
            && let Some((middle, middle_up)) = class
                .lower
                .iter()
                .find(|(module, _)| class.upper.iter().any(|(other, _)| other.id == module.id))
            && let Some((_, middle_down)) = class
                .upper
                .iter()
                .find(|(module, _)| module.id == middle.id)
        {
            three_cell = Some((class, *middle, middle_up.clone(), middle_down.clone()));
        }
    }

    let Some((class, middle, middle_up, middle_down)) = three_cell else {
        if stackable {
            report.failures.push(CertificationFailure::module(
                "<corpus>",
                CertificationFailureKind::MissingVerticalRepresentative,
                "no module both carries and stands on one vertical interface, so no three-cell column can be certified",
            ));
        }
        return;
    };
    let Some(hulls) = hull_set(catalog, middle) else {
        report.failures.push(CertificationFailure::module(
            &middle.id,
            CertificationFailureKind::InvalidHull,
            "the three-cell representative has no hull set",
        ));
        return;
    };
    let contract = middle.contract.as_ref().expect("class members are v3");
    let Some(bottom_entry) = side_port(contract, &middle_up) else {
        return;
    };
    let Some(top_exit) = side_port(contract, &middle_down) else {
        return;
    };
    let column = [
        ColumnMember {
            module: middle,
            hulls,
            entry: bottom_entry,
            exit: middle_up.clone(),
        },
        ColumnMember {
            module: middle,
            hulls,
            entry: middle_down.clone(),
            exit: middle_up.clone(),
        },
        ColumnMember {
            module: middle,
            hulls,
            entry: middle_down.clone(),
            exit: top_exit,
        },
    ];
    match walk_column(&column, profile) {
        Ok((ticks, intent_digest, body_digest)) => {
            report.vertical_cases.push(CertifiedVerticalCase {
                family: class.family.clone(),
                fingerprint: class.fingerprint,
                lower: middle.id.clone(),
                upper: middle.id.clone(),
                members: members_of(class),
                cells: REPRESENTATIVE_COLUMN_CELLS,
                ticks,
                intent_digest,
                body_digest,
            })
        }
        Err(failure) => report.failures.push(*failure),
    }
}

fn unmatched_class(class: &VerticalClass<'_>) -> CertificationFailure {
    let owner = class
        .lower
        .first()
        .or_else(|| class.upper.first())
        .map(|(module, _)| module.id.as_str())
        .unwrap_or("<corpus>");
    let mut failure = CertificationFailure::module(
        owner,
        CertificationFailureKind::MissingVerticalRepresentative,
        format!(
            "vertical interface class {:?} has {} lower and {} upper members, so no stacked pair can be certified",
            hex_prefix(class.fingerprint),
            class.lower.len(),
            class.upper.len()
        ),
    );
    failure.stage = "vertical_class".to_string();
    failure
}

fn hex_prefix(fingerprint: InterfaceFingerprint) -> String {
    fingerprint.0[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
