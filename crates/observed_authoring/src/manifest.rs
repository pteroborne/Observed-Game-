//! The tile manifest: the authored catalog the hex solver draws from.
//!
//! `assets/tiles/manifest.ron` maps `TileKey { archetype, register, variant }`
//! to a `.map` path and the tile's declared ports. The catalog is
//! manifest-driven, not mask-enumerated: authored tiles ARE the alphabet, and
//! the coverage validator proves every port signature a consumer demands has
//! at least one tile — a missing variant would otherwise mean unsolvable seeds.

use std::path::{Path, PathBuf};

use observed_hex::{HexFace, PortClass, PortSignature};
use serde::{Deserialize, Serialize};

use crate::tile::{TileError, TilePrototype, load_tile};

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TileKey {
    pub archetype: String,
    pub register: String,
    pub variant: u16,
}

/// One declared port in the manifest (readable names; converted to the typed
/// vocabulary on load).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PortDecl {
    pub face: String,
    pub class: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ManifestEntry {
    pub key: TileKey,
    /// Path relative to the manifest file's directory.
    pub map_path: String,
    pub levels: u8,
    pub ports: Vec<PortDecl>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Manifest {
    pub tiles: Vec<ManifestEntry>,
}

#[derive(Debug)]
pub enum ManifestError {
    Io(String),
    Ron(String),
    DuplicateKey(TileKey),
    Tile(TileKey, TileError),
    /// Loaded tile disagrees with its manifest declaration.
    Mismatch(TileKey, String),
}

impl ManifestEntry {
    pub fn declared_signature(&self) -> Result<PortSignature, ManifestError> {
        let mut ports = [PortClass::Sealed; 8];
        for decl in &self.ports {
            let face = face_from_manifest(&decl.face)
                .ok_or_else(|| mismatch(&self.key, format!("unknown face '{}'", decl.face)))?;
            let class = class_from_manifest(&decl.class)
                .ok_or_else(|| mismatch(&self.key, format!("unknown class '{}'", decl.class)))?;
            ports[face.index()] = class;
        }
        PortSignature::try_from_ports(ports).map_err(|invalid| {
            mismatch(
                &self.key,
                format!("invalid port {:?} on {:?}", invalid.class, invalid.face),
            )
        })
    }
}

fn mismatch(key: &TileKey, message: String) -> ManifestError {
    ManifestError::Mismatch(key.clone(), message)
}

fn face_from_manifest(name: &str) -> Option<HexFace> {
    HexFace::ALL
        .into_iter()
        .find(|&face| crate::tile::face_name(face) == name)
}

fn class_from_manifest(name: &str) -> Option<PortClass> {
    PortClass::ALL
        .into_iter()
        .find(|&class| crate::tile::class_name(class) == name)
}

impl Manifest {
    pub fn from_ron(text: &str) -> Result<Manifest, ManifestError> {
        ron::from_str(text).map_err(|error| ManifestError::Ron(error.to_string()))
    }

    pub fn load(path: &Path) -> Result<Manifest, ManifestError> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| ManifestError::Io(format!("{}: {error}", path.display())))?;
        Self::from_ron(&text)
    }

    /// Load and cross-validate every declared tile: the `.map` parses, its
    /// footprint snaps, and its authored key/levels/ports agree with the
    /// manifest declaration.
    pub fn load_tiles(&self, base_dir: &Path) -> Result<Vec<TilePrototype>, ManifestError> {
        let mut seen = std::collections::BTreeSet::new();
        let mut tiles = Vec::with_capacity(self.tiles.len());
        for entry in &self.tiles {
            if !seen.insert(entry.key.clone()) {
                return Err(ManifestError::DuplicateKey(entry.key.clone()));
            }
            let path: PathBuf = base_dir.join(&entry.map_path);
            let tile =
                load_tile(&path).map_err(|error| ManifestError::Tile(entry.key.clone(), error))?;
            if tile.key != entry.key {
                return Err(mismatch(
                    &entry.key,
                    format!("map declares key {:?}", tile.key),
                ));
            }
            if tile.levels != entry.levels {
                return Err(mismatch(
                    &entry.key,
                    format!(
                        "map declares levels {}, manifest {}",
                        tile.levels, entry.levels
                    ),
                ));
            }
            let declared = entry.declared_signature()?;
            if tile.signature != declared {
                return Err(mismatch(
                    &entry.key,
                    format!(
                        "map ports {:?} disagree with manifest {:?}",
                        tile.signature, declared
                    ),
                ));
            }
            tiles.push(tile);
        }
        Ok(tiles)
    }

    /// The coverage validator: which demanded signatures have no tile?
    /// Consumers (the solver's variant alphabet) pass every signature they can
    /// require; a non-empty return is a content gap that must fail the build.
    pub fn uncovered(&self, demands: &[PortSignature]) -> Vec<PortSignature> {
        let available: std::collections::BTreeSet<PortSignature> = self
            .tiles
            .iter()
            .filter_map(|entry| entry.declared_signature().ok())
            .collect();
        demands
            .iter()
            .copied()
            .filter(|signature| !available.contains(signature))
            .collect()
    }
}
