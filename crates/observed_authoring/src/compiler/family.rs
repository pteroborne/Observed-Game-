//! Family-first assembly coverage.
//!
//! A compiled catalog with no family identity can offer two structurally
//! incompatible modules under the same `(archetype, register, PortSignature)`
//! key, which is exactly how a column mixed an authored tower with a generated
//! one. This index groups contract-bearing modules by [`ModuleFamilyId`],
//! resolves register fallback and rotation once for the whole family, and
//! refuses a family that cannot answer every signature its siblings can for the
//! same archetype, register, and rotation — because the runtime's only way out
//! of that hole is to mix families, which is the fault itself.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use observed_hex::{HexFace, PortClass};

use crate::catalog::{CompiledModule, class_from_compiled_name, face_from_compiled_name};
use crate::contract::{
    AssemblyScope, AssemblyVariantId, InterfaceFingerprint, ModuleContract, ModuleFamilyId,
};
use crate::source::{ModuleCellRef, ModuleKind};

/// One assembly variant's exact coordinates: what shape it is, which register
/// it answers, and which turn it was expanded to.
pub type VariantKey = (String, String, u8);

/// A precise, ordered family fault. Every coordinate a compiler user needs to
/// find the offending source is named rather than summarized.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FamilyDiagnostic {
    pub code: &'static str,
    pub family: ModuleFamilyId,
    pub archetype: Option<String>,
    pub register: Option<String>,
    pub rotation: Option<u8>,
    pub signature: Option<String>,
    pub modules: Vec<String>,
    pub message: String,
}

impl fmt::Display for FamilyDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let any = "<any>";
        write!(
            f,
            "{}: family {:?} archetype {} register {} rotation ",
            self.code,
            self.family.0,
            self.archetype.as_deref().unwrap_or(any),
            self.register.as_deref().unwrap_or(any),
        )?;
        match self.rotation {
            Some(rotation) => write!(f, "{rotation}")?,
            None => write!(f, "{any}")?,
        }
        write!(f, " signature {}", self.signature.as_deref().unwrap_or(any))?;
        if !self.modules.is_empty() {
            write!(f, " modules {:?}", self.modules)?;
        }
        write!(f, ": {}", self.message)
    }
}

impl std::error::Error for FamilyDiagnostic {}

/// One member of an assembly variant.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FamilyMember {
    pub module: String,
    /// Member selection weight. It may only choose among interface-equivalent
    /// members inside one variant; the family weight chooses the family.
    pub weight: u16,
}

/// Everything one family resolves once, for the whole assembly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FamilyEntry {
    pub scope: AssemblyScope,
    pub family_weight: u16,
    /// Accepted clockwise turns, identical across every member.
    pub rotations: Vec<u8>,
    /// Registers this family answers, after `all` fallback expansion.
    pub registers: Vec<String>,
    variants: BTreeMap<VariantKey, BTreeMap<String, Vec<FamilyMember>>>,
}

impl FamilyEntry {
    /// Members that satisfy one exact signature inside one assembly variant.
    #[must_use]
    pub fn members(&self, key: &VariantKey, signature: &str) -> &[FamilyMember] {
        self.variants
            .get(key)
            .and_then(|by_signature| by_signature.get(signature))
            .map_or(&[], Vec::as_slice)
    }

    /// Every signature this family answers for one assembly variant.
    #[must_use]
    pub fn signatures(&self, key: &VariantKey) -> Vec<String> {
        self.variants
            .get(key)
            .map(|by_signature| by_signature.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub fn variant_keys(&self) -> impl Iterator<Item = &VariantKey> {
        self.variants.keys()
    }
}

/// Compiled family index for one catalog and one demanded register set.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AssemblyFamilyIndex {
    families: BTreeMap<ModuleFamilyId, FamilyEntry>,
}

impl AssemblyFamilyIndex {
    #[must_use]
    pub fn get(&self, family: &ModuleFamilyId) -> Option<&FamilyEntry> {
        self.families.get(family)
    }

    /// The family entry for an assembly variant, or `None` when that family
    /// does not accept that rotation. Rotation belongs to the variant identity,
    /// never to an individual signature member.
    #[must_use]
    pub fn variant(&self, variant: &AssemblyVariantId) -> Option<&FamilyEntry> {
        self.families
            .get(&variant.family)
            .filter(|entry| entry.rotations.contains(&variant.rotation))
    }

    pub fn families(&self) -> impl Iterator<Item = (&ModuleFamilyId, &FamilyEntry)> {
        self.families.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.families.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.families.is_empty()
    }

    /// Group every contract-bearing module, prove the family invariants, and
    /// reject anything that would let selection mix incompatible geometry.
    ///
    /// `registers` is the architecture-register set an `all`-scoped family
    /// expands into. Compatibility modules with no contract are ignored, so a
    /// version-1/2 corpus compiles to an empty index rather than an error.
    pub fn build(
        modules: &[CompiledModule],
        registers: &[&str],
    ) -> Result<Self, Box<FamilyDiagnostic>> {
        let mut grouped: BTreeMap<ModuleFamilyId, Vec<(&CompiledModule, &ModuleContract)>> =
            BTreeMap::new();
        for module in modules {
            if let Some(contract) = module.contract.as_ref() {
                grouped
                    .entry(contract.assembly.family.clone())
                    .or_default()
                    .push((module, contract));
            }
        }

        let mut families = BTreeMap::new();
        for (family, members) in &grouped {
            families.insert(family.clone(), compile_family(family, members, registers)?);
        }

        // What the solver may demand of any family answering one variant: the
        // union of what every family already answers there. A lone family is
        // trivially complete; two families for one archetype have to agree.
        let mut demand: BTreeMap<VariantKey, BTreeSet<String>> = BTreeMap::new();
        for entry in families.values() {
            for (key, by_signature) in &entry.variants {
                demand
                    .entry(key.clone())
                    .or_default()
                    .extend(by_signature.keys().cloned());
            }
        }
        for (family, entry) in &families {
            for (key, by_signature) in &entry.variants {
                let provided = by_signature.keys().cloned().collect::<BTreeSet<_>>();
                let Some(missing) = demand
                    .get(key)
                    .and_then(|wanted| wanted.difference(&provided).next())
                else {
                    continue;
                };
                let (archetype, register, rotation) = key.clone();
                return Err(Box::new(FamilyDiagnostic {
                    code: "incomplete_signature_coverage",
                    family: family.clone(),
                    archetype: Some(archetype),
                    register: Some(register),
                    rotation: Some(rotation),
                    signature: Some(missing.clone()),
                    modules: grouped[family]
                        .iter()
                        .map(|(module, _)| module.id.clone())
                        .collect(),
                    message: "another family answers this signature for the same assembly variant; \
                              filling the hole would mix families inside one assembly"
                        .to_string(),
                }));
            }
        }
        Ok(Self { families })
    }
}

/// Canonical text for one member's runtime signature: stable across processes,
/// and readable in the diagnostic that names it.
fn signature_label(module: &CompiledModule, turn: u8) -> Result<String, Box<FamilyDiagnostic>> {
    if module.kind == ModuleKind::Room {
        return Ok(format!(
            "room:{}",
            module.room_role.as_deref().unwrap_or("<none>")
        ));
    }
    let unreadable = |detail: String| {
        Box::new(FamilyDiagnostic {
            code: "unreadable_compiled_port",
            family: ModuleFamilyId(String::new()),
            archetype: Some(module.archetype.clone()),
            register: None,
            rotation: Some(turn),
            signature: None,
            modules: vec![module.id.clone()],
            message: detail,
        })
    };
    let origin = ModuleCellRef {
        q: 0,
        r: 0,
        level: 0,
    };
    let mut ports = [PortClass::Sealed; 8];
    for port in module.ports.iter().filter(|port| port.cell == origin) {
        let face = face_from_compiled_name(&port.face)
            .ok_or_else(|| unreadable(format!("unknown port face {:?}", port.face)))?;
        let class = class_from_compiled_name(&port.class)
            .ok_or_else(|| unreadable(format!("unknown port class {:?}", port.class)))?;
        let rotated = if face.is_vertical() {
            face
        } else {
            HexFace::LATERAL[(face.index() + usize::from(turn)) % 6]
        };
        ports[rotated.index()] = class;
    }
    let label = HexFace::ALL
        .into_iter()
        .filter(|face| ports[face.index()] != PortClass::Sealed)
        .map(|face| format!("{}={}", face_label(face), class_label(ports[face.index()])))
        .collect::<Vec<_>>()
        .join(",");
    Ok(if label.is_empty() {
        "sealed".to_string()
    } else {
        label
    })
}

fn face_label(face: HexFace) -> &'static str {
    match face {
        HexFace::East => "east",
        HexFace::SouthEast => "south_east",
        HexFace::SouthWest => "south_west",
        HexFace::West => "west",
        HexFace::NorthWest => "north_west",
        HexFace::NorthEast => "north_east",
        HexFace::Up => "up",
        HexFace::Down => "down",
    }
}

fn class_label(class: PortClass) -> &'static str {
    match class {
        PortClass::Sealed => "sealed",
        PortClass::Door => "door",
        PortClass::RampOpen => "ramp_open",
        PortClass::ShaftOpen => "shaft_open",
    }
}

fn vertical_fingerprint(contract: &ModuleContract, face: HexFace) -> Option<InterfaceFingerprint> {
    contract
        .interfaces
        .iter()
        .find(|interface| interface.profile.face == face)
        .map(|interface| interface.profile.fingerprint())
}

fn compile_family(
    family: &ModuleFamilyId,
    members: &[(&CompiledModule, &ModuleContract)],
    registers: &[&str],
) -> Result<FamilyEntry, Box<FamilyDiagnostic>> {
    let fault = |code: &'static str, module: Option<&CompiledModule>, message: String| {
        Box::new(FamilyDiagnostic {
            code,
            family: family.clone(),
            archetype: module.map(|module| module.archetype.clone()),
            register: None,
            rotation: None,
            signature: None,
            modules: module.map(|module| vec![module.id.clone()]).unwrap_or_default(),
            message,
        })
    };
    let (first, first_contract) = members[0];
    let scope = first_contract.assembly.scope;
    let family_weight = first_contract.assembly.family_weight;
    let mut rotations = first.rotations.clone();
    rotations.sort_unstable();
    rotations.dedup();
    for &turn in &rotations {
        AssemblyVariantId {
            family: family.clone(),
            rotation: turn,
        }
        .validate()
        .map_err(|diagnostic| fault("invalid_assembly_rotation", Some(first), diagnostic.message))?;
    }

    for (module, contract) in members {
        if contract.assembly.scope != scope {
            return Err(Box::new(FamilyDiagnostic {
                modules: vec![first.id.clone(), module.id.clone()],
                ..*fault(
                    "mixed_assembly_scope",
                    Some(module),
                    format!(
                        "one family declares one assembly scope; found {scope:?} and {:?}",
                        contract.assembly.scope
                    ),
                )
            }));
        }
        if contract.assembly.family_weight != family_weight {
            return Err(Box::new(FamilyDiagnostic {
                modules: vec![first.id.clone(), module.id.clone()],
                ..*fault(
                    "mixed_family_weight",
                    Some(module),
                    format!(
                        "family weight chooses the family once; found {family_weight} and {}",
                        contract.assembly.family_weight
                    ),
                )
            }));
        }
        let mut theirs = module.rotations.clone();
        theirs.sort_unstable();
        theirs.dedup();
        if theirs != rotations {
            return Err(Box::new(FamilyDiagnostic {
                rotation: theirs.first().copied(),
                modules: vec![first.id.clone(), module.id.clone()],
                ..*fault(
                    "mixed_assembly_rotations",
                    Some(module),
                    format!(
                        "rotation is an assembly-wide choice, expanded once per variant; \
                         found {rotations:?} and {theirs:?}"
                    ),
                )
            }));
        }
    }

    let is_generic = |module: &CompiledModule| module.register_scope.iter().any(|s| s == "all");
    let generic = members
        .iter()
        .filter(|(module, _)| is_generic(module))
        .collect::<Vec<_>>();
    if !generic.is_empty() && generic.len() != members.len() {
        let exact = members
            .iter()
            .find(|(module, _)| !is_generic(module))
            .expect("a non-generic member exists");
        return Err(Box::new(FamilyDiagnostic {
            register: exact.0.register_scope.first().cloned(),
            modules: vec![generic[0].0.id.clone(), exact.0.id.clone()],
            ..*fault(
                "mixed_register_fallback",
                Some(exact.0),
                "a family resolves register fallback once: every member is generic, or none is"
                    .to_string(),
            )
        }));
    }

    let fallback: Vec<String> = if registers.is_empty() {
        vec!["generic".to_string()]
    } else {
        registers.iter().map(|&name| name.to_string()).collect()
    };
    let mut variants: BTreeMap<VariantKey, BTreeMap<String, Vec<FamilyMember>>> = BTreeMap::new();
    let mut answered = BTreeSet::new();
    for (module, _) in members {
        let module_registers: Vec<String> = if is_generic(module) {
            fallback.clone()
        } else {
            module
                .register_scope
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        };
        for &turn in &rotations {
            let signature = signature_label(module, turn).map_err(|mut diagnostic| {
                diagnostic.family = family.clone();
                diagnostic
            })?;
            for register in &module_registers {
                answered.insert(register.clone());
                variants
                    .entry((module.archetype.clone(), register.clone(), turn))
                    .or_default()
                    .entry(signature.clone())
                    .or_default()
                    .push(FamilyMember {
                        module: module.id.clone(),
                        weight: module.weight,
                    });
            }
        }
    }
    for by_signature in variants.values_mut() {
        for bucket in by_signature.values_mut() {
            bucket.sort();
            bucket.dedup();
        }
    }

    if scope == AssemblyScope::VerticalColumn {
        let mut expected: BTreeMap<HexFace, (InterfaceFingerprint, String)> = BTreeMap::new();
        for (module, contract) in members {
            let signature = signature_label(module, rotations[0]).map_err(|mut diagnostic| {
                diagnostic.family = family.clone();
                diagnostic
            })?;
            for face in [HexFace::Up, HexFace::Down] {
                let coordinates = FamilyDiagnostic {
                    code: "",
                    family: family.clone(),
                    archetype: Some(module.archetype.clone()),
                    register: answered.iter().next().cloned(),
                    rotation: rotations.first().copied(),
                    signature: Some(signature.clone()),
                    modules: vec![module.id.clone()],
                    message: String::new(),
                };
                let Some(fingerprint) = vertical_fingerprint(contract, face) else {
                    return Err(Box::new(FamilyDiagnostic {
                        code: "missing_vertical_interface",
                        message: format!(
                            "a vertical-column member declares both vertical interfaces; \
                             {face:?} is absent"
                        ),
                        ..coordinates
                    }));
                };
                match expected.get(&face) {
                    None => {
                        expected.insert(face, (fingerprint, module.id.clone()));
                    }
                    Some((known, known_id)) if *known != fingerprint => {
                        return Err(Box::new(FamilyDiagnostic {
                            code: "mismatched_vertical_fingerprint",
                            modules: vec![known_id.clone(), module.id.clone()],
                            message: format!(
                                "every member of a vertical column presents one {face:?} interface; \
                                 landing, aperture, clearance, or guide terminal differ"
                            ),
                            ..coordinates
                        }));
                    }
                    Some(_) => {}
                }
            }
        }
    }

    Ok(FamilyEntry {
        scope,
        family_weight,
        rotations,
        registers: answered.into_iter().collect(),
        variants,
    })
}
