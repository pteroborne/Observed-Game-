//! Deterministic source discovery and compiled authoring catalog.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use glam::{Quat, Vec3};
use observed_hex::{HexFace, PortClass, PortSignature};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::compiler::{AssemblyFamilyIndex, FamilyDiagnostic};
use crate::contract::{ContractDiagnostic, ModuleContract, RuntimeModuleContract};
use crate::manifest::{Manifest, ManifestEntry, PortDecl, TileKey};
use crate::source::{
    AuthoredModule, ModuleCell, ModuleCellRef, ModuleKind, RoomSocketKind, RotationPolicy,
    SourceError, parse_authored_module,
};
use crate::tile::{DeckPath, StairSpine, TileLight, TileLightKind, TilePrototype};

pub const COMPILED_CATALOG_VERSION: u16 = 3;
/// First catalog version that requires complete module contracts.
pub const CONTRACT_CATALOG_VERSION: u16 = 4;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CompiledHullSet {
    pub structural_hash: String,
    pub hulls: Vec<Vec<[f32; 3]>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompiledPort {
    pub cell: ModuleCellRef,
    pub face: String,
    pub class: String,
    pub name: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CompiledLight {
    pub kind: TileLightKind,
    pub position: [f32; 3],
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CompiledSocket {
    pub id: String,
    pub kind: RoomSocketKind,
    pub cell: ModuleCellRef,
    pub position: [f32; 3],
    pub yaw_degrees: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CompiledModule {
    pub id: String,
    pub source_path: String,
    pub source_sha256: String,
    pub kind: ModuleKind,
    pub archetype: String,
    pub variant: u16,
    pub levels: u8,
    pub room_role: Option<String>,
    pub register_scope: Vec<String>,
    /// Clockwise 60-degree turns accepted for this source. Structural geometry
    /// is stored once and consumers apply these deterministic transforms.
    pub rotations: Vec<u8>,
    pub weight: u16,
    pub footprint: Vec<ModuleCell>,
    pub ports: Vec<CompiledPort>,
    pub lights: Vec<CompiledLight>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sockets: Vec<CompiledSocket>,
    /// Climb nodes in tile-local metres, bottom to top.
    ///
    /// Skipped when empty, which matters more than it looks: the catalog's
    /// canonical serialization *is* the simulation content hash, and that hash
    /// gates LAN compatibility. Writing `stair_spine: []` into every module
    /// would have moved it for every client without a single module's geometry
    /// changing. It moves when a module actually declares a climb, and not
    /// before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stair_spine: Vec<[f32; 3]>,
    /// Walkable floor path in tile-local metres. Skipped when empty, for the
    /// same content-hash reason as `stair_spine`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deck_path: Vec<[f32; 3]>,
    pub structural_hash: String,
    /// Present while the old runtime manifest remains the compatibility seam.
    pub legacy_key: Option<TileKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<ModuleContract>,
}

/// One exact external connection on a compiled whole-room module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoomPrototypePort {
    pub cell: ModuleCellRef,
    pub face: HexFace,
    pub class: PortClass,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomPrototypeSocket {
    pub id: String,
    pub kind: RoomSocketKind,
    pub cell: ModuleCellRef,
    pub position: Vec3,
    pub yaw_degrees: f32,
}

/// Runtime geometry for one whole-room module. Hull points and footprint cells
/// are already rotated into one accepted orientation around the room anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomPrototype {
    pub id: String,
    pub room_role: String,
    pub key: TileKey,
    pub weight: u16,
    pub footprint: Vec<ModuleCellRef>,
    pub ports: Vec<RoomPrototypePort>,
    pub sockets: Vec<RoomPrototypeSocket>,
    pub hulls: Vec<Vec<Vec3>>,
    pub lights: Vec<TileLight>,
    pub contract: Option<RuntimeModuleContract>,
}

/// Strict v2 modules projected into the runtime forms consumed by WFC geometry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeAuthoringCatalog {
    pub cells: Vec<TilePrototype>,
    pub rooms: Vec<RoomPrototype>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct CompiledTileCatalog {
    pub version: u16,
    /// SHA-256 over the canonical catalog with this field empty.
    pub simulation_content_hash: String,
    pub hull_sets: Vec<CompiledHullSet>,
    pub modules: Vec<CompiledModule>,
}

impl CompiledTileCatalog {
    pub fn to_pretty_ron(&self) -> Result<String, CatalogError> {
        let config = ron::ser::PrettyConfig::default().new_line("\n".to_owned());
        ron::ser::to_string_pretty(self, config)
            .map_err(|error| CatalogError::Serialize(error.to_string()))
    }

    pub fn from_ron(text: &str) -> Result<Self, CatalogError> {
        let catalog: Self =
            ron::from_str(text).map_err(|error| CatalogError::Serialize(error.to_string()))?;
        if !matches!(
            catalog.version,
            COMPILED_CATALOG_VERSION | CONTRACT_CATALOG_VERSION
        ) {
            return Err(CatalogError::UnsupportedVersion(catalog.version));
        }
        if catalog.version == COMPILED_CATALOG_VERSION
            && let Some(module) = catalog
                .modules
                .iter()
                .find(|module| module.contract.is_some())
        {
            return Err(CatalogError::UnexpectedContractInV3(module.id.clone()));
        }
        if catalog.version == CONTRACT_CATALOG_VERSION
            && catalog
                .modules
                .iter()
                .any(|module| module.contract.is_none())
        {
            return Err(CatalogError::IncompleteContractCatalog);
        }
        if catalog.version == CONTRACT_CATALOG_VERSION {
            for module in &catalog.modules {
                let contract = module.contract.as_ref().expect("v4 completeness checked");
                if contract.clone().canonicalized() != *contract {
                    return Err(CatalogError::InvalidContract {
                        module: module.id.clone(),
                        diagnostic: ContractDiagnostic::whole(
                            "noncanonical_contract_order",
                            "v4 contract vectors are not in canonical order",
                        ),
                    });
                }
                contract
                    .validate()
                    .map_err(|diagnostic| CatalogError::InvalidContract {
                        module: module.id.clone(),
                        diagnostic,
                    })?;
                validate_compiled_contract(module, contract)?;
            }
            catalog.family_index(&catalog.declared_registers())?;
        }
        Ok(catalog)
    }

    /// Every architecture register the contracted modules name explicitly.
    /// A family that declares `all` answers whatever the runtime demands; one
    /// that names exact registers has to cover this set.
    #[must_use]
    pub fn declared_registers(&self) -> Vec<String> {
        let mut registers = self
            .modules
            .iter()
            .filter(|module| module.contract.is_some())
            .flat_map(|module| module.register_scope.iter())
            .filter(|scope| scope.as_str() != "all")
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if registers.is_empty() {
            registers.push("generic".to_string());
        }
        registers
    }

    /// Compile the family index and prove its coverage. Only contract-bearing
    /// modules participate; a pure compatibility catalog yields an empty index
    /// rather than an error.
    pub fn family_index(&self, registers: &[String]) -> Result<AssemblyFamilyIndex, CatalogError> {
        let borrowed = registers.iter().map(String::as_str).collect::<Vec<_>>();
        AssemblyFamilyIndex::build(&self.modules, &borrowed).map_err(CatalogError::InvalidFamily)
    }

    pub fn verify_hash(&self) -> Result<(), CatalogError> {
        let mut unhashed = self.clone();
        let expected = unhashed.simulation_content_hash.clone();
        unhashed.simulation_content_hash.clear();
        let canonical = unhashed.to_pretty_ron()?;
        let actual = sha256(canonical.as_bytes());
        if expected == actual {
            Ok(())
        } else {
            Err(CatalogError::HashMismatch { expected, actual })
        }
    }

    /// Expand strict modules into deterministic runtime variants. Legacy v1
    /// modules remain supplied by `manifest.ron`, avoiding duplicate entries
    /// while the committed corpus is migrated incrementally.
    pub fn runtime_catalog(
        &self,
        architecture_registers: &[&str],
    ) -> Result<RuntimeAuthoringCatalog, CatalogError> {
        if !matches!(
            self.version,
            COMPILED_CATALOG_VERSION | CONTRACT_CATALOG_VERSION
        ) {
            return Err(CatalogError::UnsupportedVersion(self.version));
        }
        if self.version == COMPILED_CATALOG_VERSION
            && let Some(module) = self.modules.iter().find(|module| module.contract.is_some())
        {
            return Err(CatalogError::UnexpectedContractInV3(module.id.clone()));
        }
        if self.version == CONTRACT_CATALOG_VERSION {
            return Err(CatalogError::ContractRuntimeUnavailable);
        }
        self.verify_hash()?;
        let hull_sets = self
            .hull_sets
            .iter()
            .map(|set| (set.structural_hash.as_str(), &set.hulls))
            .collect::<BTreeMap<_, _>>();
        let mut runtime = RuntimeAuthoringCatalog::default();
        for module in self
            .modules
            .iter()
            .filter(|module| module.legacy_key.is_none())
        {
            let hulls = hull_sets
                .get(module.structural_hash.as_str())
                .ok_or_else(|| CatalogError::MissingHullSet {
                    module: module.id.clone(),
                    structural_hash: module.structural_hash.clone(),
                })?;
            let registers = expanded_registers(&module.register_scope, architecture_registers);
            for &turn in &module.rotations {
                if turn >= 6 {
                    return Err(CatalogError::InvalidRotation {
                        module: module.id.clone(),
                        turn,
                    });
                }
                let rotated_hulls = rotate_hulls(hulls, turn);
                let rotated_lights = rotate_lights(&module.lights, turn);
                let rotated_spine = StairSpine {
                    nodes: module
                        .stair_spine
                        .iter()
                        .copied()
                        .map(Vec3::from_array)
                        .collect(),
                }
                .rotated(turn);
                let rotated_deck = DeckPath {
                    nodes: module
                        .deck_path
                        .iter()
                        .copied()
                        .map(Vec3::from_array)
                        .collect(),
                }
                .rotated(turn);
                let rotated_ports = module
                    .ports
                    .iter()
                    .map(|port| runtime_port(port, turn))
                    .collect::<Result<Vec<_>, _>>()?;
                let rotated_sockets = module
                    .sockets
                    .iter()
                    .map(|socket| runtime_socket(socket, turn))
                    .collect::<Vec<_>>();
                for register in &registers {
                    let key = TileKey {
                        archetype: module.archetype.clone(),
                        register: register.clone(),
                        variant: module
                            .variant
                            .checked_mul(6)
                            .and_then(|base| base.checked_add(u16::from(turn)))
                            .ok_or_else(|| CatalogError::VariantOverflow {
                                module: module.id.clone(),
                            })?,
                    };
                    match module.kind {
                        ModuleKind::Cell => {
                            let mut ports = [PortClass::Sealed; 8];
                            for port in &rotated_ports {
                                if port.cell
                                    == (ModuleCellRef {
                                        q: 0,
                                        r: 0,
                                        level: 0,
                                    })
                                {
                                    ports[port.face.index()] = port.class;
                                }
                            }
                            let signature = PortSignature::try_from_ports(ports).map_err(|_| {
                                CatalogError::InvalidRuntimePort {
                                    module: module.id.clone(),
                                }
                            })?;
                            runtime.cells.push(TilePrototype {
                                key,
                                weight: module.weight,
                                levels: module.levels,
                                signature,
                                hulls: rotated_hulls.clone(),
                                lights: rotated_lights.clone(),
                                spine: rotated_spine.clone(),
                                deck: rotated_deck.clone(),
                                contract: None,
                            });
                        }
                        ModuleKind::Room => runtime.rooms.push(RoomPrototype {
                            id: module.id.clone(),
                            room_role: module
                                .room_role
                                .clone()
                                .expect("compiled room modules have a role"),
                            key,
                            weight: module.weight,
                            footprint: expanded_rotated_footprint(&module.footprint, turn),
                            ports: rotated_ports.clone(),
                            sockets: rotated_sockets.clone(),
                            hulls: rotated_hulls.clone(),
                            lights: rotated_lights.clone(),
                            contract: None,
                        }),
                    }
                }
            }
        }
        runtime.cells.sort_by(|a, b| a.key.cmp(&b.key));
        runtime
            .rooms
            .sort_by(|a, b| (&a.room_role, &a.key, &a.id).cmp(&(&b.room_role, &b.key, &b.id)));
        Ok(runtime)
    }
}

fn runtime_socket(socket: &CompiledSocket, turn: u8) -> RoomPrototypeSocket {
    let rotation = Quat::from_rotation_y(-f32::from(turn) * std::f32::consts::TAU / 6.0);
    RoomPrototypeSocket {
        id: socket.id.clone(),
        kind: socket.kind,
        cell: rotate_cell(socket.cell, turn),
        position: rotation * Vec3::from_array(socket.position),
        yaw_degrees: socket.yaw_degrees - f32::from(turn) * 60.0,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogAudit {
    pub sources: usize,
    pub strict_sources: usize,
    pub legacy_sources: usize,
    pub hull_sets: usize,
    pub shared_hull_references: usize,
    pub compatibility_manifest_entries: usize,
    pub content_hash: String,
}

#[derive(Debug)]
pub enum CatalogError {
    Io(String),
    Source {
        path: PathBuf,
        error: SourceError,
    },
    DuplicateId {
        id: String,
        paths: Vec<PathBuf>,
    },
    Serialize(String),
    UnsupportedVersion(u16),
    UnexpectedContractInV3(String),
    IncompleteContractCatalog,
    /// One build mixed contracted and uncontracted sources. Catalog v3 refuses
    /// the contracts and v4 refuses their absence, so the corpus has to move as
    /// a whole rather than half-migrate into an unloadable artifact.
    MixedContractCorpus {
        contracted: String,
        compatibility: String,
    },
    ContractRuntimeUnavailable,
    InvalidContract {
        module: String,
        diagnostic: ContractDiagnostic,
    },
    InvalidFamily(Box<FamilyDiagnostic>),
    HashMismatch {
        expected: String,
        actual: String,
    },
    MissingHullSet {
        module: String,
        structural_hash: String,
    },
    InvalidRotation {
        module: String,
        turn: u8,
    },
    InvalidRuntimePort {
        module: String,
    },
    VariantOverflow {
        module: String,
    },
}

fn expanded_registers(scope: &[String], architecture_registers: &[&str]) -> Vec<String> {
    if scope.iter().any(|r| r == "all") {
        if architecture_registers.is_empty() {
            vec!["generic".to_string()]
        } else {
            architecture_registers
                .iter()
                .map(|&r| r.to_string())
                .collect()
        }
    } else if !scope.is_empty() {
        scope
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    } else {
        vec!["generic".to_string()]
    }
}

fn rotate_cell(cell: ModuleCellRef, turn: u8) -> ModuleCellRef {
    let mut q = cell.q;
    let mut r = cell.r;
    for _ in 0..turn {
        (q, r) = (-r, q.saturating_add(r));
    }
    ModuleCellRef {
        q,
        r,
        level: cell.level,
    }
}

fn rotate_face(face: HexFace, turn: u8) -> HexFace {
    if face.is_vertical() {
        return face;
    }
    HexFace::LATERAL[(face.index() + usize::from(turn)) % 6]
}

fn runtime_port(port: &CompiledPort, turn: u8) -> Result<RoomPrototypePort, CatalogError> {
    let face =
        face_from_compiled_name(&port.face).ok_or_else(|| CatalogError::InvalidRuntimePort {
            module: port.name.clone(),
        })?;
    let class =
        class_from_compiled_name(&port.class).ok_or_else(|| CatalogError::InvalidRuntimePort {
            module: port.name.clone(),
        })?;
    Ok(RoomPrototypePort {
        cell: rotate_cell(port.cell, turn),
        face: rotate_face(face, turn),
        class,
        name: port.name.clone(),
    })
}

pub(crate) fn face_from_compiled_name(name: &str) -> Option<HexFace> {
    HexFace::ALL
        .into_iter()
        .find(|&face| face_name(face) == name)
}

pub(crate) fn class_from_compiled_name(name: &str) -> Option<PortClass> {
    [
        PortClass::Sealed,
        PortClass::Door,
        PortClass::RampOpen,
        PortClass::ShaftOpen,
    ]
    .into_iter()
    .find(|&class| class_name(class) == name)
}

fn validate_compiled_contract(
    module: &CompiledModule,
    contract: &ModuleContract,
) -> Result<(), CatalogError> {
    let compiled_footprint = module
        .footprint
        .iter()
        .flat_map(|cell| {
            (0..cell.levels).map(move |level| {
                (
                    ModuleCellRef {
                        q: cell.q,
                        r: cell.r,
                        level: cell.level.saturating_add_unsigned(level),
                    },
                    cell.floor,
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let contract_footprint = contract
        .spatial
        .logical_footprint
        .cells
        .iter()
        .map(|cell| (cell.cell, cell.floor))
        .collect::<BTreeMap<_, _>>();
    if compiled_footprint != contract_footprint {
        return Err(CatalogError::InvalidContract {
            module: module.id.clone(),
            diagnostic: ContractDiagnostic::whole(
                "compiled_logical_footprint_mismatch",
                "compiled and contract logical footprints differ",
            ),
        });
    }
    let interfaces = contract
        .interfaces
        .iter()
        .map(|interface| (interface.port.as_str(), interface))
        .collect::<BTreeMap<_, _>>();
    let mut compiled_names = BTreeSet::new();
    for port in &module.ports {
        if port.name.trim().is_empty() || !compiled_names.insert(port.name.as_str()) {
            return Err(CatalogError::InvalidContract {
                module: module.id.clone(),
                diagnostic: ContractDiagnostic::whole(
                    "invalid_compiled_port_name",
                    format!(
                        "compiled port names must be unique and nonempty: {:?}",
                        port.name
                    ),
                ),
            });
        }
        let Some(interface) = interfaces.get(port.name.as_str()) else {
            return Err(CatalogError::InvalidContract {
                module: module.id.clone(),
                diagnostic: ContractDiagnostic::whole(
                    "compiled_port_unbound",
                    format!("compiled port {:?} has no interface contract", port.name),
                ),
            });
        };
        let face = face_from_compiled_name(&port.face);
        let class = class_from_compiled_name(&port.class);
        if interface.cell != port.cell
            || face != Some(interface.profile.face)
            || class != Some(interface.profile.class)
        {
            return Err(CatalogError::InvalidContract {
                module: module.id.clone(),
                diagnostic: ContractDiagnostic::whole(
                    "compiled_port_interface_mismatch",
                    format!(
                        "compiled port {:?} does not match its cell/face/class interface",
                        port.name
                    ),
                ),
            });
        }
    }
    if compiled_names != interfaces.keys().copied().collect::<BTreeSet<_>>() {
        return Err(CatalogError::InvalidContract {
            module: module.id.clone(),
            diagnostic: ContractDiagnostic::whole(
                "interface_without_compiled_port",
                "interface and compiled-port name sets differ",
            ),
        });
    }
    Ok(())
}

fn rotate_hulls(hulls: &[Vec<[f32; 3]>], turn: u8) -> Vec<Vec<Vec3>> {
    let rotation = Quat::from_rotation_y(-f32::from(turn) * std::f32::consts::TAU / 6.0);
    hulls
        .iter()
        .map(|hull| {
            hull.iter()
                .map(|point| rotation * Vec3::from_array(*point))
                .collect()
        })
        .collect()
}

fn rotate_lights(lights: &[CompiledLight], turn: u8) -> Vec<TileLight> {
    let rotation = Quat::from_rotation_y(-f32::from(turn) * std::f32::consts::TAU / 6.0);
    lights
        .iter()
        .map(|light| TileLight {
            kind: light.kind,
            position: rotation * Vec3::from_array(light.position),
        })
        .collect()
}

fn expanded_rotated_footprint(footprint: &[ModuleCell], turn: u8) -> Vec<ModuleCellRef> {
    let mut cells = footprint
        .iter()
        .flat_map(|cell| {
            (0..cell.levels).map(move |level| ModuleCellRef {
                q: cell.q,
                r: cell.r,
                level: cell.level.saturating_add_unsigned(level),
            })
        })
        .map(|cell| rotate_cell(cell, turn))
        .collect::<Vec<_>>();
    cells.sort();
    cells.dedup();
    cells
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CatalogError {}

#[derive(Clone, Debug)]
pub struct CatalogBuild {
    pub catalog: CompiledTileCatalog,
    pub compatibility_manifest: Manifest,
    pub audit: CatalogAudit,
}

/// Shared with [`crate::composition`] so both content artifacts are digested by
/// exactly one implementation.
pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn hash_hulls(module: &AuthoredModule) -> String {
    let mut hasher = Sha256::new();
    for hull in &module.prototype.hulls {
        hasher.update((hull.len() as u64).to_le_bytes());
        for point in hull {
            hasher.update(point.x.to_bits().to_le_bytes());
            hasher.update(point.y.to_bits().to_le_bytes());
            hasher.update(point.z.to_bits().to_le_bytes());
        }
    }
    format!("{:x}", hasher.finalize())
}

fn face_name(face: HexFace) -> &'static str {
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

fn class_name(class: PortClass) -> &'static str {
    match class {
        PortClass::Sealed => "sealed",
        PortClass::Door => "door",
        PortClass::RampOpen => "ramp_open",
        PortClass::ShaftOpen => "shaft_open",
    }
}

fn normalized_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn ignored_paths(root: &Path) -> Result<BTreeSet<String>, CatalogError> {
    let path = root.join(".tileignore");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(BTreeSet::new());
    };
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.replace('\\', "/"))
        .collect())
}

fn walk_maps(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), CatalogError> {
    let entries = std::fs::read_dir(dir)
        .map_err(|error| CatalogError::Io(format!("{}: {error}", dir.display())))?;
    for entry in entries {
        let entry = entry.map_err(|error| CatalogError::Io(error.to_string()))?;
        let path = entry.path();
        if path.is_dir() {
            walk_maps(&path, paths)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("map") {
            paths.push(path);
        }
    }
    Ok(())
}

pub fn discover_sources(root: &Path) -> Result<Vec<PathBuf>, CatalogError> {
    let ignored = ignored_paths(root)?;
    let mut paths = Vec::new();
    walk_maps(root, &mut paths)?;
    paths.retain(|path| !ignored.contains(&normalized_relative(root, path)));
    paths.sort_by_key(|path| normalized_relative(root, path));
    Ok(paths)
}

fn compatibility_entry(module: &AuthoredModule, source_path: String) -> Option<ManifestEntry> {
    // Version-2 sources always expand from the compiled catalog, including
    // an explicit one-register scope such as `liminal_grid`. Mirroring those
    // strict cells into the legacy manifest would load the unrotated source a
    // second time and bypass its compiled weight/rotation contract.
    if module.authoring_version >= 2 {
        return None;
    }
    let scope = (module.register_scope.len() == 1).then(|| &module.register_scope[0])?;
    if scope == "all" || scope != &module.prototype.key.register {
        return None;
    }
    let ports = HexFace::ALL
        .into_iter()
        .filter_map(|face| {
            let class = module.prototype.signature.port(face);
            (class != PortClass::Sealed).then(|| PortDecl {
                face: face_name(face).to_string(),
                class: class_name(class).to_string(),
            })
        })
        .collect();
    Some(ManifestEntry {
        key: module.prototype.key.clone(),
        map_path: source_path,
        levels: module.prototype.levels,
        ports,
    })
}

fn compile_module(
    module: &AuthoredModule,
    source_path: String,
    source_text: &str,
    structural_hash: String,
) -> CompiledModule {
    let ports = module
        .ports
        .iter()
        .map(|port| CompiledPort {
            cell: port.cell,
            face: face_name(port.face).to_string(),
            class: class_name(port.class).to_string(),
            name: port.name.clone(),
        })
        .collect();
    let lights = module
        .prototype
        .lights
        .iter()
        .map(|light| CompiledLight {
            kind: light.kind,
            position: light.position.to_array(),
        })
        .collect();
    let sockets = module
        .sockets
        .iter()
        .map(|socket| CompiledSocket {
            id: socket.id.clone(),
            kind: socket.kind,
            cell: socket.cell,
            position: socket.position.to_array(),
            yaw_degrees: socket.yaw_degrees,
        })
        .collect();
    let stair_spine = module
        .prototype
        .spine
        .nodes
        .iter()
        .map(Vec3::to_array)
        .collect();
    let deck_path = module
        .prototype
        .deck
        .nodes
        .iter()
        .map(Vec3::to_array)
        .collect();
    CompiledModule {
        id: module.id.clone(),
        source_path,
        source_sha256: sha256(source_text.as_bytes()),
        kind: module.kind,
        archetype: module.archetype.clone(),
        variant: module.prototype.key.variant,
        levels: module.prototype.levels,
        room_role: module.room_role.clone(),
        register_scope: module.register_scope.clone(),
        rotations: match module.rotation {
            RotationPolicy::None => vec![0],
            RotationPolicy::SixFold => (0..6).collect(),
        },
        weight: module.weight,
        footprint: module.footprint.clone(),
        ports,
        lights,
        sockets,
        stair_spine,
        deck_path,
        structural_hash,
        legacy_key: (module.authoring_version < 2).then(|| module.prototype.key.clone()),
        contract: module.contract.clone(),
    }
}

/// Validate and compile every non-ignored `.map` below `root`. Source paths,
/// IDs, hull sets, and output records are sorted before hashing.
pub fn build_catalog(root: &Path) -> Result<CatalogBuild, CatalogError> {
    let paths = discover_sources(root)?;
    let mut modules = Vec::with_capacity(paths.len());
    let mut id_paths: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    let mut hull_sets: BTreeMap<String, CompiledHullSet> = BTreeMap::new();
    let mut manifest_entries = Vec::new();
    let mut strict_sources = 0;
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .map_err(|error| CatalogError::Io(format!("{}: {error}", path.display())))?;
        let module = parse_authored_module(&text).map_err(|error| CatalogError::Source {
            path: path.clone(),
            error,
        })?;
        strict_sources += usize::from(module.authoring_version >= 2);
        id_paths
            .entry(module.id.clone())
            .or_default()
            .push(path.clone());
        let source_path = normalized_relative(root, &path);
        let structural_hash = hash_hulls(&module);
        hull_sets
            .entry(structural_hash.clone())
            .or_insert_with(|| CompiledHullSet {
                structural_hash: structural_hash.clone(),
                hulls: module
                    .prototype
                    .hulls
                    .iter()
                    .map(|hull| hull.iter().map(|point| point.to_array()).collect())
                    .collect(),
            });
        if let Some(entry) = compatibility_entry(&module, source_path.clone()) {
            manifest_entries.push(entry);
        }
        modules.push(compile_module(&module, source_path, &text, structural_hash));
    }
    if let Some((id, paths)) = id_paths.into_iter().find(|(_, paths)| paths.len() > 1) {
        return Err(CatalogError::DuplicateId { id, paths });
    }
    modules.sort_by(|a, b| a.id.cmp(&b.id));
    manifest_entries.sort_by(|a, b| a.key.cmp(&b.key));
    let hull_sets: Vec<_> = hull_sets.into_values().collect();
    let contracted = modules.iter().find(|module| module.contract.is_some());
    let compatibility = modules.iter().find(|module| module.contract.is_none());
    let version = match (contracted, compatibility) {
        (Some(contracted), Some(compatibility)) => {
            return Err(CatalogError::MixedContractCorpus {
                contracted: contracted.id.clone(),
                compatibility: compatibility.id.clone(),
            });
        }
        (Some(_), None) => CONTRACT_CATALOG_VERSION,
        _ => COMPILED_CATALOG_VERSION,
    };
    let mut catalog = CompiledTileCatalog {
        version,
        simulation_content_hash: String::new(),
        hull_sets,
        modules,
    };
    if version == CONTRACT_CATALOG_VERSION {
        catalog.family_index(&catalog.declared_registers())?;
    }
    let canonical = catalog.to_pretty_ron()?;
    catalog.simulation_content_hash = sha256(canonical.as_bytes());
    let audit = CatalogAudit {
        sources: catalog.modules.len(),
        strict_sources,
        legacy_sources: catalog.modules.len() - strict_sources,
        hull_sets: catalog.hull_sets.len(),
        shared_hull_references: catalog
            .modules
            .len()
            .saturating_sub(catalog.hull_sets.len()),
        compatibility_manifest_entries: manifest_entries.len(),
        content_hash: catalog.simulation_content_hash.clone(),
    };
    Ok(CatalogBuild {
        catalog,
        compatibility_manifest: Manifest {
            tiles: manifest_entries,
        },
        audit,
    })
}

pub fn write_catalog_build(
    build: &CatalogBuild,
    catalog_path: &Path,
    manifest_path: &Path,
) -> Result<(), CatalogError> {
    let catalog = build.catalog.to_pretty_ron()?;
    let manifest = ron::ser::to_string_pretty(
        &build.compatibility_manifest,
        ron::ser::PrettyConfig::default(),
    )
    .map_err(|error| CatalogError::Serialize(error.to_string()))?;
    std::fs::write(
        catalog_path,
        format!("// Generated by tilec build. Edit .map sources, not this file.\n{catalog}\n"),
    )
    .map_err(|error| CatalogError::Io(format!("{}: {error}", catalog_path.display())))?;
    let hash_path = catalog_path.with_extension("sha256");
    std::fs::write(
        &hash_path,
        format!("{}\n", build.catalog.simulation_content_hash),
    )
    .map_err(|error| CatalogError::Io(format!("{}: {error}", hash_path.display())))?;
    std::fs::write(
        manifest_path,
        format!("// Generated by tilec build. Edit .map sources, not this file.\n{manifest}\n"),
    )
    .map_err(|error| CatalogError::Io(format!("{}: {error}", manifest_path.display())))?;
    Ok(())
}

#[derive(Clone, Debug, Default)]
pub struct DistrictAuditGroup {
    pub archetype: String,
    pub variant: u16,
    pub district_sources: BTreeMap<String, String>,
    pub is_identical_across_districts: bool,
    pub total_districts: usize,
}

#[derive(Clone, Debug, Default)]
pub struct DistrictAuditResult {
    pub total_sources: usize,
    pub unique_archetype_variants: usize,
    pub identical_archetype_variants: usize,
    pub distinct_archetype_variants: usize,
    pub groups: Vec<DistrictAuditGroup>,
    pub report: String,
}

pub fn audit_district_variations(root: &Path) -> Result<DistrictAuditResult, CatalogError> {
    let paths = discover_sources(root)?;
    let total_sources = paths.len();
    let mut groups: BTreeMap<(String, u16), DistrictAuditGroup> = BTreeMap::new();

    for path in paths {
        let text = std::fs::read_to_string(&path)
            .map_err(|error| CatalogError::Io(format!("{}: {error}", path.display())))?;
        let module = parse_authored_module(&text).map_err(|error| CatalogError::Source {
            path: path.clone(),
            error,
        })?;
        let structural_hash = hash_hulls(&module);
        let archetype = module.archetype.clone();
        let variant = module.prototype.key.variant;
        let register = module.prototype.key.register.clone();

        let group = groups
            .entry((archetype.clone(), variant))
            .or_insert_with(|| DistrictAuditGroup {
                archetype,
                variant,
                district_sources: BTreeMap::new(),
                is_identical_across_districts: true,
                total_districts: 0,
            });
        group.district_sources.insert(register, structural_hash);
    }

    let mut identical_count = 0;
    let mut distinct_count = 0;

    for group in groups.values_mut() {
        group.total_districts = group.district_sources.len();
        let first_hash = group
            .district_sources
            .values()
            .next()
            .cloned()
            .unwrap_or_default();
        let all_same = group.district_sources.values().all(|h| h == &first_hash);
        group.is_identical_across_districts = all_same;
        if all_same && group.total_districts > 1 {
            identical_count += 1;
        } else {
            distinct_count += 1;
        }
    }

    let mut report = String::new();
    report.push_str(&format!(
        "DISTRICT VARIATION AUDIT\nScanned {} source maps across {} unique (archetype, variant) groups.\n",
        total_sources,
        groups.len()
    ));
    report.push_str(&format!(
        "Identical across districts: {}/{} groups ({:.1}%)\nDistinct physical variations: {}/{} groups ({:.1}%)\n\n",
        identical_count,
        groups.len(),
        if groups.is_empty() { 0.0 } else { identical_count as f32 / groups.len() as f32 * 100.0 },
        distinct_count,
        groups.len(),
        if groups.is_empty() { 0.0 } else { distinct_count as f32 / groups.len() as f32 * 100.0 }
    ));

    report.push_str("Summary of Groups:\n");
    for group in groups.values() {
        let status = if group.is_identical_across_districts && group.total_districts > 1 {
            "IDENTICAL GEOMETRY"
        } else if group.total_districts == 1 {
            "SINGLE DISTRICT ONLY"
        } else {
            "HAS PHYSICAL VARIATIONS"
        };
        report.push_str(&format!(
            "  - {}-v{} ({} districts): {}\n",
            group.archetype, group.variant, group.total_districts, status
        ));
    }

    let group_list: Vec<_> = groups.into_values().collect();
    Ok(DistrictAuditResult {
        total_sources,
        unique_archetype_variants: group_list.len(),
        identical_archetype_variants: identical_count,
        distinct_archetype_variants: distinct_count,
        groups: group_list,
        report,
    })
}

/// Load every runtime cell for a tile directory: legacy compatibility
/// manifest entries plus the strict compiled catalog expanded for
/// `registers`. This is the one canonical corpus loader — since the corpus
/// went all-strict (`register_scope: all`), the manifest alone is empty and
/// anything loading only `manifest.ron` sees zero tiles.
pub fn load_runtime_cells(
    root: &Path,
    registers: &[&str],
) -> Result<Vec<crate::TilePrototype>, String> {
    let mut cells = crate::Manifest::load(&root.join("manifest.ron"))
        .map_err(|error| format!("manifest: {error:?}"))?
        .load_tiles(root)
        .map_err(|error| format!("manifest tiles: {error:?}"))?;
    let path = root.join("compiled_catalog.ron");
    if path.exists() {
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let compiled = CompiledTileCatalog::from_ron(&text)
            .map_err(|error| format!("compiled catalog: {error}"))?;
        let strict = compiled
            .runtime_catalog(registers)
            .map_err(|error| format!("runtime catalog: {error}"))?;
        cells.extend(strict.cells);
    }
    Ok(cells)
}

/// Generate a minimal strict source. Geometry is a safe center floor brush;
/// authors add contract-shell walls and ports in TrenchBroom before building.
pub fn new_module_template(id: &str, kind: ModuleKind) -> String {
    let room = if kind == ModuleKind::Room {
        "\"kind\" \"room\"\n\"room_role\" \"decision\"\n"
    } else {
        "\"kind\" \"cell\"\n"
    };
    format!(
        "// Observed 2 strict authored module. Validate with: tilec validate <file>\n{{\n\"classname\" \"worldspawn\"\n{}\n}}\n{{\n\"classname\" \"tile_meta\"\n\"authoring_version\" \"2\"\n\"id\" \"{id}\"\n{room}\"archetype\" \"hall_cap\"\n\"register\" \"generic\"\n\"register_scope\" \"all\"\n\"variant\" \"0\"\n\"levels\" \"1\"\n\"rotation_policy\" \"sixfold\"\n\"weight\" \"1\"\n}}\n{{\n\"classname\" \"tile_cell\"\n\"q\" \"0\"\n\"r\" \"0\"\n\"level\" \"0\"\n\"levels\" \"1\"\n\"floor\" \"solid\"\n}}\n{{\n\"classname\" \"tile_port\"\n\"q\" \"0\"\n\"r\" \"0\"\n\"level\" \"0\"\n\"face\" \"east\"\n\"class\" \"door\"\n\"name\" \"east_threshold\"\n\"origin\" \"112 0 48\"\n}}\n{{\n\"classname\" \"tile_light\"\n\"kind\" \"practical\"\n\"origin\" \"48 0 96\"\n}}\n",
        crate::tile_source::hex_slab_brush(0.0, 8.0)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::FloorPolicy;

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "observed2_authoring_{name}_{}_{}",
            std::process::id(),
            nonce
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("temp dir");
        path
    }

    #[test]
    fn strict_catalog_is_deterministic_and_hash_verifies() {
        let root = temp_dir("catalog");
        std::fs::write(
            root.join("module.map"),
            new_module_template("test/catalog", ModuleKind::Cell),
        )
        .expect("write map");
        let first = build_catalog(&root).expect("build");
        let second = build_catalog(&root).expect("rebuild");
        assert_eq!(first.catalog, second.catalog);
        first.catalog.verify_hash().expect("content hash");
        assert!(
            first
                .catalog
                .modules
                .iter()
                .all(|module| module.contract.is_none())
        );
        assert!(
            !first.catalog.to_pretty_ron().unwrap().contains("contract:"),
            "v2 sources must preserve byte-compatible catalog-v3 serialization"
        );
        assert_eq!(first.audit.strict_sources, 1);
        assert_eq!(first.audit.compatibility_manifest_entries, 0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn catalog_reader_fails_closed_on_unknown_or_incomplete_contract_versions() {
        let unsupported = CompiledTileCatalog {
            version: 99,
            ..Default::default()
        }
        .to_pretty_ron()
        .unwrap();
        assert!(matches!(
            CompiledTileCatalog::from_ron(&unsupported),
            Err(CatalogError::UnsupportedVersion(99))
        ));
        assert!(matches!(
            CompiledTileCatalog {
                version: 99,
                ..Default::default()
            }
            .runtime_catalog(&[]),
            Err(CatalogError::UnsupportedVersion(99))
        ));

        let root = temp_dir("incomplete_v4");
        std::fs::write(
            root.join("module.map"),
            new_module_template("test/incomplete-v4", ModuleKind::Cell),
        )
        .expect("write map");
        let mut catalog = build_catalog(&root).expect("build").catalog;
        let module_id = catalog.modules[0].id.clone();
        catalog.modules[0].contract = Some(ModuleContract {
            spatial: crate::ModuleSpatialContract {
                logical_footprint: crate::LogicalFootprint { cells: Vec::new() },
                geometry_envelope: crate::GeometryEnvelope {
                    bounds: crate::QuantizedBox {
                        min: crate::QuantizedPoint { x: 0, y: 0, z: 0 },
                        max: crate::QuantizedPoint { x: 1, y: 1, z: 1 },
                    },
                },
                clearance_volumes: Vec::new(),
            },
            assembly: crate::AssemblyContract {
                family: crate::ModuleFamilyId("hostile/downgrade".to_string()),
                scope: crate::AssemblyScope::Cell,
                family_weight: 1,
            },
            traversal: crate::ModuleTraversalContract {
                nodes: Vec::new(),
                edges: Vec::new(),
                port_bindings: BTreeMap::new(),
            },
            interfaces: Vec::new(),
        });
        assert!(matches!(
            catalog.runtime_catalog(&[]),
            Err(CatalogError::UnexpectedContractInV3(id)) if id == module_id
        ));
        catalog.modules[0].contract = None;
        catalog.version = CONTRACT_CATALOG_VERSION;
        assert!(matches!(
            CompiledTileCatalog::from_ron(&catalog.to_pretty_ron().unwrap()),
            Err(CatalogError::IncompleteContractCatalog)
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    /// Version 3 is compiled now, but only as a *complete* contract: a source
    /// that claims the version without an assembly identity is still a fault,
    /// and version 4 remains unopened.
    #[test]
    fn authoring_versions_past_the_contract_compiler_still_fail_closed() {
        for version in [0, 4, 5] {
            let text = new_module_template("test/future", ModuleKind::Cell).replace(
                "\"authoring_version\" \"2\"",
                &format!("\"authoring_version\" \"{version}\""),
            );
            assert!(
                matches!(
                    parse_authored_module(&text),
                    Err(SourceError::InvalidProperty {
                        entity: "tile_meta",
                        ..
                    })
                ),
                "authoring_version {version} must not import"
            );
        }
        let anonymous = new_module_template("test/anonymous", ModuleKind::Cell)
            .replace("\"authoring_version\" \"2\"", "\"authoring_version\" \"3\"");
        assert!(matches!(
            parse_authored_module(&anonymous),
            Err(SourceError::InvalidProperty {
                entity: "tile_meta",
                ..
            })
        ));
    }

    /// The compatibility promise. A version-1/2 corpus compiles to exactly the
    /// catalog-v3 bytes it did before the contract compiler existed, so the
    /// committed simulation content hash cannot move underneath LAN clients.
    #[test]
    fn compatibility_sources_keep_their_contract_free_catalog_v3_serialization() {
        let root = temp_dir("v1_v2_unchanged");
        std::fs::write(
            root.join("module.map"),
            new_module_template("test/compat", ModuleKind::Cell),
        )
        .expect("write map");
        let built = build_catalog(&root).expect("build");
        assert_eq!(built.catalog.version, COMPILED_CATALOG_VERSION);
        assert!(
            built
                .catalog
                .modules
                .iter()
                .all(|module| module.contract.is_none())
        );
        let text = built.catalog.to_pretty_ron().expect("serialize");
        assert!(!text.contains("contract:"), "{text}");
        assert!(!text.contains("family"), "{text}");
        built.catalog.verify_hash().expect("content hash");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn canonical_catalog_ron_always_uses_lf_line_endings() {
        let ron = CompiledTileCatalog::default()
            .to_pretty_ron()
            .expect("serialize catalog");

        assert!(ron.contains('\n'));
        assert!(!ron.contains('\r'));
    }

    #[test]
    fn catalog_build_writes_the_network_hash_sidecar() {
        let root = temp_dir("hash_sidecar");
        std::fs::write(
            root.join("module.map"),
            new_module_template("test/hash-sidecar", ModuleKind::Cell),
        )
        .expect("write map");
        let built = build_catalog(&root).expect("build");
        let catalog = root.join("compiled_catalog.ron");
        let manifest = root.join("manifest.ron");
        write_catalog_build(&built, &catalog, &manifest).expect("write build");
        assert_eq!(
            std::fs::read_to_string(root.join("compiled_catalog.sha256"))
                .expect("hash sidecar")
                .trim(),
            built.audit.content_hash
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sixfold_rotation_expands_without_duplicating_hulls() {
        let root = temp_dir("rotation");
        std::fs::write(
            root.join("module.map"),
            new_module_template("test/rotation", ModuleKind::Cell),
        )
        .expect("write map");
        let built = build_catalog(&root).expect("build");
        assert_eq!(built.catalog.modules[0].rotations, [0, 1, 2, 3, 4, 5]);
        assert_eq!(built.catalog.hull_sets.len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn strict_cells_expand_into_runtime_rotations_and_registers() {
        let root = temp_dir("runtime_cells");
        std::fs::write(
            root.join("module.map"),
            new_module_template("test/runtime-cell", ModuleKind::Cell),
        )
        .expect("write map");
        let built = build_catalog(&root).expect("build");
        let runtime = built
            .catalog
            .runtime_catalog(&["institutional", "monolith"])
            .expect("runtime expansion");
        // `register_scope: all` expands into every requested register, each
        // with all six rotations.
        assert_eq!(runtime.cells.len(), 12);
        assert!(runtime.rooms.is_empty());
        for register in ["institutional", "monolith"] {
            for (turn, face) in HexFace::LATERAL.into_iter().enumerate() {
                let tile = runtime
                    .cells
                    .iter()
                    .find(|tile| {
                        tile.key.register == register
                            && tile.key.variant == u16::try_from(turn).expect("turn")
                    })
                    .expect("rotated tile");
                assert_eq!(tile.signature.port(face), PortClass::Door);
                assert_eq!(tile.weight, 1);
                assert_eq!(tile.lights.len(), 1);
                assert!(
                    (tile.lights[0].position.y - 6.0).abs() < 1e-5,
                    "unexpected rotated light {:?}",
                    tile.lights[0].position
                );
                if turn == 0 {
                    assert_eq!(tile.lights[0].position, Vec3::new(3.0, 6.0, 0.0));
                }
                if turn == 3 {
                    assert!((tile.lights[0].position - Vec3::new(-3.0, 6.0, 0.0)).length() < 1e-5);
                }
            }
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn explicit_multi_register_scope_expands_every_named_register() {
        let root = temp_dir("multi_register_scope");
        let text = new_module_template("test/multi-register", ModuleKind::Cell).replace(
            "\"register_scope\" \"all\"",
            "\"register_scope\" \"institutional,monolith\"",
        );
        std::fs::write(root.join("module.map"), text).expect("write map");
        let built = build_catalog(&root).expect("build");
        let runtime = built
            .catalog
            .runtime_catalog(&["institutional", "monolith", "liminal_grid"])
            .expect("runtime expansion");
        assert_eq!(runtime.cells.len(), 12);
        assert!(
            runtime
                .cells
                .iter()
                .all(|tile| { matches!(tile.key.register.as_str(), "institutional" | "monolith") })
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn strict_single_register_scope_stays_out_of_legacy_manifest() {
        let root = temp_dir("strict_single_scope");
        let text = new_module_template("test/single-register", ModuleKind::Cell)
            .replace(
                "\"register_scope\" \"all\"",
                "\"register_scope\" \"liminal_grid\"",
            )
            .replace("\"register\" \"generic\"", "\"register\" \"liminal_grid\"");
        std::fs::write(root.join("module.map"), text).expect("write map");
        let built = build_catalog(&root).expect("build");
        assert_eq!(built.audit.compatibility_manifest_entries, 0);
        assert!(built.compatibility_manifest.tiles.is_empty());
        let runtime = built
            .catalog
            .runtime_catalog(&["institutional", "liminal_grid"])
            .expect("runtime expansion");
        assert_eq!(runtime.cells.len(), 6);
        assert!(
            runtime
                .cells
                .iter()
                .all(|tile| tile.key.register == "liminal_grid")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn strict_rooms_expand_as_whole_room_runtime_modules() {
        let root = temp_dir("runtime_rooms");
        std::fs::write(
            root.join("module.map"),
            new_module_template("test/runtime-room", ModuleKind::Room),
        )
        .expect("write map");
        let built = build_catalog(&root).expect("build");
        let runtime = built
            .catalog
            .runtime_catalog(&["institutional"])
            .expect("runtime expansion");
        assert!(runtime.cells.is_empty());
        assert_eq!(runtime.rooms.len(), 6);
        assert!(
            runtime
                .rooms
                .iter()
                .all(|room| room.room_role == "decision")
        );
        assert!(
            runtime
                .rooms
                .iter()
                .all(|room| room.footprint.len() == 1 && !room.hulls.is_empty())
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn ignore_file_excludes_teaching_templates() {
        let root = temp_dir("ignore");
        let text = new_module_template("test/one", ModuleKind::Cell);
        std::fs::write(root.join("one.map"), &text).expect("write one");
        std::fs::write(root.join("copy.map"), text).expect("write copy");
        std::fs::write(root.join(".tileignore"), "copy.map\n").expect("write ignore");
        assert_eq!(discover_sources(&root).expect("discover").len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn floor_policy_stays_in_the_compiled_contract() {
        let cell = ModuleCell {
            q: 0,
            r: 0,
            level: 0,
            levels: 1,
            floor: FloorPolicy::Open,
        };
        assert_eq!(cell.floor, FloorPolicy::Open);
    }
}
