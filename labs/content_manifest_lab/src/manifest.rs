//! Typed parsing, validation, and deterministic commitment of content configuration.

use std::{collections::BTreeSet, fmt, fs, path::Path};

use observed_style::{
    DISTRICT_MIN_FOG_START, District, MarkerRole, SIGNAL_MIN_LUMINANCE, district, luminance, marker,
};
use rapier_portal_lab::authoring::ArchitectureKind;
use serde::{Deserialize, Serialize};

pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentManifest {
    pub schema_version: u32,
    pub seed: u64,
    pub districts: Vec<DistrictDefinition>,
    pub architecture_modules: Vec<ArchitectureModuleDefinition>,
    pub asset_packs: Vec<AssetPackDefinition>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistrictDefinition {
    pub id: String,
    pub label: String,
    pub style_district: String,
    pub architecture_module: String,
    pub fog_end_scale: f32,
    pub practical_intensity_scale: f32,
    pub asset_slots: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureModuleDefinition {
    pub id: String,
    pub architecture_kind: String,
    pub source_path: String,
    pub ports: Vec<String>,
    pub allowed_rotations_degrees: Vec<i16>,
    pub wfc_weight: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetPackDefinition {
    pub id: String,
    pub path: String,
    pub author: String,
    pub source_url: String,
    pub license: String,
    pub content_fingerprint: String,
    pub scale: f32,
    pub fallback: String,
    pub allowed_districts: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestError(pub String);

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ManifestError {}

pub fn parse(source: &str) -> Result<ContentManifest, ManifestError> {
    serde_json::from_str(source).map_err(|error| ManifestError(error.to_string()))
}

pub fn style_district(id: &str) -> Option<District> {
    District::ALL
        .into_iter()
        .find(|district| district.label() == id)
}

pub fn architecture_kind(id: &str) -> Option<ArchitectureKind> {
    ArchitectureKind::ALL
        .into_iter()
        .find(|kind| kind.label() == id)
}

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub fn file_fingerprint(path: &Path) -> Result<String, ManifestError> {
    let bytes = fs::read(path)
        .map_err(|error| ManifestError(format!("cannot read {}: {error}", path.display())))?;
    Ok(format!("{:016x}", fnv1a64(&bytes)))
}

fn insert_unique(ids: &mut BTreeSet<String>, kind: &str, id: &str) -> Result<(), ManifestError> {
    if id.trim().is_empty() {
        return Err(ManifestError(format!("{kind} has an empty stable id")));
    }
    if !ids.insert(id.to_owned()) {
        return Err(ManifestError(format!("duplicate {kind} id `{id}`")));
    }
    Ok(())
}

impl ContentManifest {
    pub fn from_path(path: &Path) -> Result<Self, ManifestError> {
        let source = fs::read_to_string(path)
            .map_err(|error| ManifestError(format!("cannot read {}: {error}", path.display())))?;
        parse(&source)
    }

    pub fn validate(&self, workspace_root: &Path) -> Result<(), ManifestError> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(ManifestError(format!(
                "unsupported schema version {}; expected {SUPPORTED_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        if self.districts.len() < 2 {
            return Err(ManifestError(
                "the proof requires at least two district definitions".into(),
            ));
        }

        let mut module_ids = BTreeSet::new();
        for module in &self.architecture_modules {
            insert_unique(&mut module_ids, "architecture module", &module.id)?;
            if architecture_kind(&module.architecture_kind).is_none() {
                return Err(ManifestError(format!(
                    "module `{}` has unknown architecture kind `{}`",
                    module.id, module.architecture_kind
                )));
            }
            if module.wfc_weight == 0 || module.ports.is_empty() {
                return Err(ManifestError(format!(
                    "module `{}` needs ports and a non-zero WFC weight",
                    module.id
                )));
            }
            if module.allowed_rotations_degrees.is_empty()
                || module
                    .allowed_rotations_degrees
                    .iter()
                    .any(|rotation| ![0, 90, 180, 270].contains(rotation))
            {
                return Err(ManifestError(format!(
                    "module `{}` has an unsupported authored rotation",
                    module.id
                )));
            }
            if !workspace_root.join(&module.source_path).is_file() {
                return Err(ManifestError(format!(
                    "module `{}` source does not exist: {}",
                    module.id, module.source_path
                )));
            }
        }

        let mut asset_ids = BTreeSet::new();
        for asset in &self.asset_packs {
            insert_unique(&mut asset_ids, "asset slot", &asset.id)?;
            if asset.license != "CC0-1.0" {
                return Err(ManifestError(format!(
                    "lab intake accepts CC0-1.0 only; `{}` declares `{}`",
                    asset.id, asset.license
                )));
            }
            if !(asset.source_url.starts_with("https://") && asset.scale > 0.0) {
                return Err(ManifestError(format!(
                    "asset `{}` needs an HTTPS source and positive scale",
                    asset.id
                )));
            }
            if !matches!(asset.fallback.as_str(), "cable_bundle" | "portal_frame") {
                return Err(ManifestError(format!(
                    "asset `{}` names unknown fallback `{}`",
                    asset.id, asset.fallback
                )));
            }
            let path = workspace_root.join("assets").join(&asset.path);
            if path.is_file() {
                let actual = file_fingerprint(&path)?;
                if actual != asset.content_fingerprint {
                    return Err(ManifestError(format!(
                        "asset `{}` fingerprint mismatch: expected {}, got {actual}",
                        asset.id, asset.content_fingerprint
                    )));
                }
            } else if asset.fallback.trim().is_empty() {
                return Err(ManifestError(format!(
                    "missing asset `{}` has no procedural fallback",
                    asset.id
                )));
            }
        }

        let mut district_ids = BTreeSet::new();
        for definition in &self.districts {
            insert_unique(&mut district_ids, "district", &definition.id)?;
            let role = style_district(&definition.style_district).ok_or_else(|| {
                ManifestError(format!(
                    "district `{}` names unknown observed_style district `{}`",
                    definition.id, definition.style_district
                ))
            })?;
            let palette = district(role);
            if palette.fog_start < DISTRICT_MIN_FOG_START
                || palette.fog_end * definition.fog_end_scale <= palette.fog_start + 10.0
                || !(0.5..=1.5).contains(&definition.practical_intensity_scale)
            {
                return Err(ManifestError(format!(
                    "district `{}` atmosphere escapes the legibility bounds",
                    definition.id
                )));
            }
            if !module_ids.contains(&definition.architecture_module) {
                return Err(ManifestError(format!(
                    "district `{}` references missing module `{}`",
                    definition.id, definition.architecture_module
                )));
            }
            for slot in &definition.asset_slots {
                if !asset_ids.contains(slot) {
                    return Err(ManifestError(format!(
                        "district `{}` references missing asset `{slot}`",
                        definition.id
                    )));
                }
                let asset = self.asset(slot);
                if !asset.allowed_districts.contains(&definition.id) {
                    return Err(ManifestError(format!(
                        "asset `{slot}` is not allowed in district `{}`",
                        definition.id
                    )));
                }
            }
        }

        for asset in &self.asset_packs {
            for district in &asset.allowed_districts {
                if !district_ids.contains(district) {
                    return Err(ManifestError(format!(
                        "asset `{}` allows unknown district `{district}`",
                        asset.id
                    )));
                }
            }
        }

        let signal = marker(MarkerRole::Control);
        if !signal.signal || luminance(signal.emissive) < SIGNAL_MIN_LUMINANCE {
            return Err(ManifestError(
                "shared Control treatment violates the Legibility Contract".into(),
            ));
        }
        Ok(())
    }

    /// Hashes typed content, not JSON whitespace. Stable IDs are sorted before serialization.
    pub fn deterministic_hash(&self) -> u64 {
        let mut canonical = self.clone();
        canonical.districts.sort_by(|a, b| a.id.cmp(&b.id));
        canonical
            .architecture_modules
            .sort_by(|a, b| a.id.cmp(&b.id));
        canonical.asset_packs.sort_by(|a, b| a.id.cmp(&b.id));
        for district in &mut canonical.districts {
            district.asset_slots.sort();
        }
        for module in &mut canonical.architecture_modules {
            module.ports.sort();
            module.allowed_rotations_degrees.sort();
        }
        for asset in &mut canonical.asset_packs {
            asset.allowed_districts.sort();
        }
        let bytes = serde_json::to_vec(&canonical).expect("typed manifest serializes");
        fnv1a64(&bytes)
    }

    pub fn district(&self, index: usize) -> &DistrictDefinition {
        &self.districts[index % self.districts.len()]
    }

    pub fn module(&self, id: &str) -> &ArchitectureModuleDefinition {
        self.architecture_modules
            .iter()
            .find(|module| module.id == id)
            .expect("validated district module reference")
    }

    pub fn asset(&self, id: &str) -> &AssetPackDefinition {
        self.asset_packs
            .iter()
            .find(|asset| asset.id == id)
            .expect("validated district asset reference")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> ContentManifest {
        parse(include_str!("../assets/content_manifest.json")).expect("fixture parses")
    }

    fn workspace_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
    }

    #[test]
    fn committed_manifest_is_valid_and_resolves_real_content() {
        fixture().validate(workspace_root()).unwrap();
    }

    #[test]
    fn canonical_hash_ignores_record_order_and_json_whitespace() {
        let original = fixture();
        let mut reordered = original.clone();
        reordered.districts.reverse();
        reordered.asset_packs.reverse();
        reordered.architecture_modules.reverse();
        reordered.districts[0].asset_slots.reverse();
        assert_eq!(
            original.deterministic_hash(),
            reordered.deterministic_hash()
        );
        let pretty = serde_json::to_string_pretty(&original).unwrap();
        assert_eq!(
            original.deterministic_hash(),
            parse(&pretty).unwrap().deterministic_hash()
        );
    }

    #[test]
    fn unknown_fields_and_versions_fail_closed() {
        let source = include_str!("../assets/content_manifest.json");
        let with_unknown = source.replacen("{", "{\"surprise\":true,", 1);
        assert!(parse(&with_unknown).is_err());
        let mut unsupported = fixture();
        unsupported.schema_version += 1;
        assert!(unsupported.validate(workspace_root()).is_err());
    }

    #[test]
    fn missing_asset_is_allowed_only_with_an_explicit_fallback() {
        let mut manifest = fixture();
        manifest.asset_packs[0].path = "not-present.glb".into();
        assert!(manifest.validate(workspace_root()).is_ok());
        manifest.asset_packs[0].fallback.clear();
        assert!(manifest.validate(workspace_root()).is_err());
    }

    #[test]
    fn config_cannot_name_ad_hoc_style_or_unlicensed_content() {
        let mut manifest = fixture();
        manifest.districts[0].style_district = "custom-neon-purple".into();
        assert!(manifest.validate(workspace_root()).is_err());
        let mut manifest = fixture();
        manifest.asset_packs[0].license = "unknown".into();
        assert!(manifest.validate(workspace_root()).is_err());
        let mut manifest = fixture();
        manifest.asset_packs[0].allowed_districts.clear();
        assert!(manifest.validate(workspace_root()).is_err());
    }
}
