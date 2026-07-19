//! Deterministic source discovery and compiled authoring catalog.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use glam::{Quat, Vec3};
use observed_hex::{HexFace, PortClass, PortSignature};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::manifest::{Manifest, ManifestEntry, PortDecl, TileKey};
use crate::source::{
    AuthoredModule, ModuleCell, ModuleCellRef, ModuleKind, RotationPolicy, SourceError,
    parse_authored_module,
};
use crate::tile::TilePrototype;

pub const COMPILED_CATALOG_VERSION: u16 = 2;

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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
    pub structural_hash: String,
    /// Present while the old runtime manifest remains the compatibility seam.
    pub legacy_key: Option<TileKey>,
}

/// One exact external connection on a compiled whole-room module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoomPrototypePort {
    pub cell: ModuleCellRef,
    pub face: HexFace,
    pub class: PortClass,
    pub name: String,
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
    pub hulls: Vec<Vec<Vec3>>,
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
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|error| CatalogError::Serialize(error.to_string()))
    }

    pub fn from_ron(text: &str) -> Result<Self, CatalogError> {
        ron::from_str(text).map_err(|error| CatalogError::Serialize(error.to_string()))
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
                let rotated_ports = module
                    .ports
                    .iter()
                    .map(|port| runtime_port(port, turn))
                    .collect::<Result<Vec<_>, _>>()?;
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
                            hulls: rotated_hulls.clone(),
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
    let mut registers = if scope.iter().any(|register| register == "all") {
        architecture_registers
            .iter()
            .map(|register| (*register).to_string())
            .collect::<Vec<_>>()
    } else {
        scope.to_vec()
    };
    registers.sort();
    registers.dedup();
    registers
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

fn face_from_compiled_name(name: &str) -> Option<HexFace> {
    HexFace::ALL
        .into_iter()
        .find(|&face| face_name(face) == name)
}

fn class_from_compiled_name(name: &str) -> Option<PortClass> {
    [
        PortClass::Sealed,
        PortClass::Door,
        PortClass::RampOpen,
        PortClass::ShaftOpen,
    ]
    .into_iter()
    .find(|&class| class_name(class) == name)
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

fn sha256(bytes: &[u8]) -> String {
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
        structural_hash,
        legacy_key: (module.authoring_version < 2).then(|| module.prototype.key.clone()),
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
    let mut catalog = CompiledTileCatalog {
        version: COMPILED_CATALOG_VERSION,
        simulation_content_hash: String::new(),
        hull_sets,
        modules,
    };
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

/// Generate a minimal strict source. Geometry is a safe center floor brush;
/// authors add contract-shell walls and ports in TrenchBroom before building.
pub fn new_module_template(id: &str, kind: ModuleKind) -> String {
    let room = if kind == ModuleKind::Room {
        "\"kind\" \"room\"\n\"room_role\" \"decision\"\n"
    } else {
        "\"kind\" \"cell\"\n"
    };
    format!(
        "// Observed 2 strict authored module. Validate with: tilec validate <file>\n{{\n\"classname\" \"worldspawn\"\n{}\n}}\n{{\n\"classname\" \"tile_meta\"\n\"authoring_version\" \"2\"\n\"id\" \"{id}\"\n{room}\"archetype\" \"hall_cap\"\n\"register\" \"generic\"\n\"register_scope\" \"all\"\n\"variant\" \"0\"\n\"levels\" \"1\"\n\"rotation_policy\" \"sixfold\"\n\"weight\" \"1\"\n}}\n{{\n\"classname\" \"tile_cell\"\n\"q\" \"0\"\n\"r\" \"0\"\n\"level\" \"0\"\n\"levels\" \"1\"\n\"floor\" \"solid\"\n}}\n{{\n\"classname\" \"tile_port\"\n\"q\" \"0\"\n\"r\" \"0\"\n\"level\" \"0\"\n\"face\" \"east\"\n\"class\" \"door\"\n\"name\" \"east_threshold\"\n\"origin\" \"112 0 48\"\n}}\n",
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
        assert_eq!(first.audit.strict_sources, 1);
        assert_eq!(first.audit.compatibility_manifest_entries, 0);
        let _ = std::fs::remove_dir_all(root);
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
        assert_eq!(runtime.cells.len(), 12);
        assert!(runtime.rooms.is_empty());
        for (turn, face) in HexFace::LATERAL.into_iter().enumerate() {
            let tile = runtime
                .cells
                .iter()
                .find(|tile| {
                    tile.key.register == "institutional"
                        && tile.key.variant == u16::try_from(turn).expect("turn")
                })
                .expect("rotated tile");
            assert_eq!(tile.signature.port(face), PortClass::Door);
            assert_eq!(tile.weight, 1);
        }
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
