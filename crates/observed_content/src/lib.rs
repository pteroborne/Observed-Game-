//! Immutable, render-free content schema for architecture and district composition.

use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Version of the code-owned procedural architecture catalogue.
///
/// These values participate in generated-map identity independently of the authored
/// content-manifest schema. Increment this version when a register's simulation
/// contract changes incompatibly; presentation-only tuning does not require a bump.
pub const ARCHITECTURE_CATALOG_VERSION: u32 = 2;

/// Stable, production-safe identities for the procedural architecture registers.
///
/// The explicit discriminants are persisted in generated-map/replay metadata. Do not
/// reorder or reuse them. Media inspirations deliberately do not appear in this API.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
#[serde(rename_all = "snake_case")]
pub enum ArchitectureRegister {
    ShadowScreen = 0,
    Monolith = 1,
    OverlitGrid = 2,
    Institutional = 3,
    FacetMonument = 4,
    Megastructure = 5,
    Wellshaft = 6,
    InfiniteGallery = 7,
    Thinning = 8,
}

impl ArchitectureRegister {
    pub const ALL: [Self; 9] = [
        Self::ShadowScreen,
        Self::Monolith,
        Self::OverlitGrid,
        Self::Institutional,
        Self::FacetMonument,
        Self::Megastructure,
        Self::Wellshaft,
        Self::InfiniteGallery,
        Self::Thinning,
    ];

    pub const fn stable_id(self) -> u8 {
        self as u8
    }

    pub const fn slug(self) -> &'static str {
        match self {
            Self::ShadowScreen => "shadow_screen",
            Self::Monolith => "monolith",
            Self::OverlitGrid => "overlit_grid",
            Self::Institutional => "institutional",
            Self::FacetMonument => "facet_monument",
            Self::Megastructure => "megastructure",
            Self::Wellshaft => "wellshaft",
            Self::InfiniteGallery => "infinite_gallery",
            Self::Thinning => "thinning",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::ShadowScreen => "Shadow Screen",
            Self::Monolith => "Monolith",
            Self::OverlitGrid => "Overlit Grid",
            Self::Institutional => "Institutional",
            Self::FacetMonument => "Facet Monument",
            Self::Megastructure => "Megastructure",
            Self::Wellshaft => "Wellshaft",
            Self::InfiniteGallery => "Infinite Gallery",
            Self::Thinning => "Thinning",
        }
    }
}

impl TryFrom<u8> for ArchitectureRegister {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|register| register.stable_id() == value)
            .ok_or(value)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentManifest {
    pub schema_version: u32,
    pub content_version: String,
    pub traversal: TraversalProfile,
    pub districts: Vec<DistrictDefinition>,
    pub modules: Vec<ArchitectureModule>,
    pub assets: Vec<AssetDefinition>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TraversalProfile {
    pub walk_speed: f32,
    pub run_speed: f32,
    pub ground_accel: f32,
    pub ground_decel: f32,
    pub air_accel: f32,
    pub gravity: f32,
    pub max_fall: f32,
    pub jump_speed: f32,
    pub jump_cooldown: f32,
    pub radius: f32,
    pub half_height: f32,
    pub eye_height: f32,
    pub look_step: f32,
    pub pitch_limit: f32,
    pub step_height: f32,
    pub controller_offset: f32,
    pub minimum_step_width: f32,
    pub maximum_slope_degrees: f32,
    pub ground_snap: f32,
}

impl TraversalProfile {
    pub fn deliberate_rapier() -> Self {
        Self {
            walk_speed: 4.6,
            run_speed: 7.0,
            ground_accel: 60.0,
            ground_decel: 90.0,
            air_accel: 18.0,
            gravity: 19.0,
            max_fall: 45.0,
            jump_speed: 6.2,
            jump_cooldown: 0.25,
            radius: 0.38,
            half_height: 0.90,
            eye_height: 1.6,
            look_step: 0.035,
            pitch_limit: 1.4,
            step_height: 0.42,
            controller_offset: 0.025,
            minimum_step_width: 0.25,
            maximum_slope_degrees: 36.0,
            ground_snap: 0.24,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DistrictDefinition {
    pub id: String,
    pub label: String,
    /// Must name one of the six code-owned `observed_style::District` roles.
    pub style_district: String,
    pub fog_end_scale: f32,
    pub practical_intensity_scale: f32,
    pub allowed_architecture_tags: Vec<String>,
    pub dressing_slots: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceKind {
    Room,
    Corridor,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureModule {
    pub id: String,
    pub space_kind: SpaceKind,
    pub architecture_kind: String,
    pub role_tags: Vec<String>,
    pub district_tags: Vec<String>,
    pub source_map: String,
    pub source_sha256: String,
    pub baked_path: String,
    pub baked_sha256: String,
    pub allowed_rotations_degrees: Vec<u16>,
    pub wfc_weight: u32,
    pub bounds: Bounds3,
    pub ports: Vec<ModulePort>,
    pub sockets: Vec<ModuleSocket>,
    pub navigation: NavigationSpec,
    pub visual_scene: Option<String>,
    pub dressing_slots: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds3 {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PortKind {
    Passage,
    Ladder,
    RaisedPassage,
    Vertical,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModulePort {
    pub id: String,
    pub kind: PortKind,
    pub translation: [f32; 3],
    pub yaw_degrees: i16,
    pub clearance: [f32; 3],
    pub floor_height: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SocketKind {
    Equipment,
    Machinery,
    Ladder,
    Grapple,
    Observation,
    Dressing,
    Hazard,
    Navigation,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleSocket {
    pub id: String,
    pub kind: SocketKind,
    pub translation: [f32; 3],
    pub yaw_degrees: i16,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationSpec {
    pub surfaces: Vec<NavigationSurface>,
    pub links: Vec<NavigationLink>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationSurface {
    pub id: String,
    pub floor_height: f32,
    pub polygon: Vec<[f32; 2]>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationLinkKind {
    Walk,
    Step,
    Jump,
    Ladder,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationLink {
    pub id: String,
    pub kind: NavigationLinkKind,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub bidirectional: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetDefinition {
    pub id: String,
    pub path: String,
    pub sha256: String,
    pub author: String,
    pub source_url: String,
    pub license: String,
    pub scale: f32,
    pub fallback: String,
    pub allowed_districts: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BakedModule {
    pub schema_version: u32,
    pub module_id: String,
    pub hulls: Vec<ConvexHull>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConvexHull {
    pub id: u32,
    pub points: Vec<[f32; 3]>,
}

/// Frozen authored composition consumed by preview, collision, rendering, and bots.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlaceLayoutSnapshot {
    pub generation: u32,
    pub placements: Vec<ModulePlacement>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModulePlacement {
    pub module_id: String,
    pub translation: [f32; 3],
    pub yaw_degrees: i16,
    pub scale: [f32; 3],
    pub entry_port: String,
    pub exit_port: String,
}

impl PlaceLayoutSnapshot {
    pub fn validate(&self, manifest: &ContentManifest) -> Result<(), ManifestError> {
        if self.placements.is_empty() {
            return Err(ManifestError(
                "place layout has no module placements".into(),
            ));
        }
        for placement in &self.placements {
            let module = manifest
                .modules
                .iter()
                .find(|module| module.id == placement.module_id)
                .ok_or_else(|| {
                    ManifestError(format!(
                        "place layout references unknown module `{}`",
                        placement.module_id
                    ))
                })?;
            if !placement.translation.iter().all(|value| value.is_finite())
                || placement
                    .scale
                    .iter()
                    .any(|value| !value.is_finite() || *value <= 0.0)
                || !module
                    .allowed_rotations_degrees
                    .contains(&(placement.yaw_degrees.rem_euclid(360) as u16))
                || !module
                    .ports
                    .iter()
                    .any(|port| port.id == placement.entry_port)
                || !module
                    .ports
                    .iter()
                    .any(|port| port.id == placement.exit_port)
            {
                return Err(ManifestError(format!(
                    "place layout has invalid placement for `{}`",
                    placement.module_id
                )));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct SimulationContentHash(pub [u8; 32]);

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct PresentationContentHash(pub [u8; 32]);

impl fmt::Debug for SimulationContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex(&self.0))
    }
}

impl fmt::Debug for PresentationContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex(&self.0))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestError(pub String);

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ManifestError {}

impl ContentManifest {
    pub fn parse(source: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(source).map_err(|error| ManifestError(error.to_string()))
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(ManifestError(format!(
                "unsupported content schema {}; expected {SUPPORTED_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        if self.content_version.trim().is_empty() {
            return Err(ManifestError("content_version may not be empty".into()));
        }
        validate_traversal(self.traversal)?;
        let district_ids = unique_ids("district", self.districts.iter().map(|item| &item.id))?;
        let asset_ids = unique_ids("asset", self.assets.iter().map(|item| &item.id))?;
        unique_ids("module", self.modules.iter().map(|item| &item.id))?;

        const STYLE_DISTRICTS: [&str; 6] = [
            "archive", "reactor", "atrium", "foundry", "hollow", "spillway",
        ];
        for district in &self.districts {
            if !STYLE_DISTRICTS.contains(&district.style_district.as_str())
                || !(0.5..=1.5).contains(&district.fog_end_scale)
                || !(0.5..=1.5).contains(&district.practical_intensity_scale)
            {
                return Err(ManifestError(format!(
                    "district `{}` escapes the style/legibility contract",
                    district.id
                )));
            }
            for slot in &district.dressing_slots {
                if !asset_ids.contains(slot) {
                    return Err(ManifestError(format!(
                        "district `{}` references unknown dressing `{slot}`",
                        district.id
                    )));
                }
            }
        }
        for asset in &self.assets {
            if asset.license != "CC0-1.0"
                || !asset.source_url.starts_with("https://")
                || !is_sha256(&asset.sha256)
                || asset.scale <= 0.0
                || asset.fallback.trim().is_empty()
            {
                return Err(ManifestError(format!(
                    "asset `{}` fails provenance/fallback validation",
                    asset.id
                )));
            }
            for district in &asset.allowed_districts {
                if !district_ids.contains(district) {
                    return Err(ManifestError(format!(
                        "asset `{}` allows unknown district `{district}`",
                        asset.id
                    )));
                }
            }
        }
        for module in &self.modules {
            validate_module(module, &asset_ids)?;
        }
        Ok(())
    }

    pub fn canonicalized(&self) -> Self {
        let mut manifest = self.clone();
        manifest.districts.sort_by(|a, b| a.id.cmp(&b.id));
        manifest.modules.sort_by(|a, b| a.id.cmp(&b.id));
        manifest.assets.sort_by(|a, b| a.id.cmp(&b.id));
        for district in &mut manifest.districts {
            district.allowed_architecture_tags.sort();
            district.dressing_slots.sort();
        }
        for module in &mut manifest.modules {
            module.role_tags.sort();
            module.district_tags.sort();
            module.allowed_rotations_degrees.sort();
            module.ports.sort_by(|a, b| a.id.cmp(&b.id));
            module.sockets.sort_by(|a, b| a.id.cmp(&b.id));
            module.navigation.surfaces.sort_by(|a, b| a.id.cmp(&b.id));
            module.navigation.links.sort_by(|a, b| a.id.cmp(&b.id));
            module.dressing_slots.sort();
        }
        for asset in &mut manifest.assets {
            asset.allowed_districts.sort();
        }
        manifest
    }

    pub fn simulation_hash(&self) -> SimulationContentHash {
        #[derive(Serialize)]
        struct SimulationView<'a> {
            schema_version: u32,
            content_version: &'a str,
            traversal: TraversalProfile,
            modules: Vec<SimulationModule<'a>>,
        }
        #[derive(Serialize)]
        struct SimulationModule<'a> {
            id: &'a str,
            space_kind: SpaceKind,
            architecture_kind: &'a str,
            role_tags: &'a [String],
            district_tags: &'a [String],
            source_sha256: &'a str,
            baked_sha256: &'a str,
            allowed_rotations_degrees: &'a [u16],
            wfc_weight: u32,
            bounds: Bounds3,
            ports: &'a [ModulePort],
            sockets: Vec<&'a ModuleSocket>,
            navigation: &'a NavigationSpec,
        }
        let canonical = self.canonicalized();
        let modules = canonical
            .modules
            .iter()
            .map(|module| SimulationModule {
                id: &module.id,
                space_kind: module.space_kind,
                architecture_kind: &module.architecture_kind,
                role_tags: &module.role_tags,
                district_tags: &module.district_tags,
                source_sha256: &module.source_sha256,
                baked_sha256: &module.baked_sha256,
                allowed_rotations_degrees: &module.allowed_rotations_degrees,
                wfc_weight: module.wfc_weight,
                bounds: module.bounds,
                ports: &module.ports,
                sockets: module
                    .sockets
                    .iter()
                    .filter(|socket| socket.kind != SocketKind::Dressing)
                    .collect(),
                navigation: &module.navigation,
            })
            .collect();
        SimulationContentHash(hash_json(&SimulationView {
            schema_version: canonical.schema_version,
            content_version: &canonical.content_version,
            traversal: canonical.traversal,
            modules,
        }))
    }

    pub fn presentation_hash(&self) -> PresentationContentHash {
        PresentationContentHash(hash_json(&self.canonicalized()))
    }
}

impl BakedModule {
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION || self.module_id.trim().is_empty() {
            return Err(ManifestError("invalid baked module header".into()));
        }
        let mut ids = BTreeSet::new();
        for hull in &self.hulls {
            if !ids.insert(hull.id)
                || hull.points.len() < 4
                || hull.points.iter().flatten().any(|value| !value.is_finite())
            {
                return Err(ManifestError(format!(
                    "module `{}` has invalid convex hull {}",
                    self.module_id, hull.id
                )));
            }
        }
        Ok(())
    }

    pub fn canonical_json(&self) -> Vec<u8> {
        let mut module = self.clone();
        module.hulls.sort_by_key(|hull| hull.id);
        for hull in &mut module.hulls {
            hull.points
                .sort_by_key(|point| (point[0].to_bits(), point[1].to_bits(), point[2].to_bits()));
        }
        serde_json::to_vec_pretty(&module).expect("baked module serializes")
    }
}

fn validate_traversal(profile: TraversalProfile) -> Result<(), ManifestError> {
    let values = [
        profile.walk_speed,
        profile.run_speed,
        profile.ground_accel,
        profile.ground_decel,
        profile.air_accel,
        profile.gravity,
        profile.max_fall,
        profile.jump_speed,
        profile.jump_cooldown,
        profile.radius,
        profile.half_height,
        profile.eye_height,
        profile.look_step,
        profile.pitch_limit,
        profile.step_height,
        profile.controller_offset,
        profile.minimum_step_width,
        profile.maximum_slope_degrees,
        profile.ground_snap,
    ];
    if values
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
        || profile.run_speed < profile.walk_speed
        || profile.half_height <= profile.radius
        || !(1.0..89.0).contains(&profile.maximum_slope_degrees)
    {
        return Err(ManifestError("invalid traversal profile".into()));
    }
    Ok(())
}

fn validate_module(
    module: &ArchitectureModule,
    asset_ids: &BTreeSet<String>,
) -> Result<(), ManifestError> {
    if module.architecture_kind.trim().is_empty()
        || module.source_map.trim().is_empty()
        || module.baked_path.trim().is_empty()
        || !is_sha256(&module.source_sha256)
        || !is_sha256(&module.baked_sha256)
        || module.wfc_weight == 0
        || module.ports.is_empty()
        || module.allowed_rotations_degrees.is_empty()
        || module
            .allowed_rotations_degrees
            .iter()
            .any(|rotation| ![0, 90, 180, 270].contains(rotation))
        || module
            .bounds
            .min
            .iter()
            .zip(module.bounds.max)
            .any(|(min, max)| !min.is_finite() || !max.is_finite() || *min >= max)
    {
        return Err(ManifestError(format!(
            "module `{}` fails structural validation",
            module.id
        )));
    }
    unique_ids("module port", module.ports.iter().map(|port| &port.id))?;
    unique_ids(
        "module socket",
        module.sockets.iter().map(|socket| &socket.id),
    )?;
    unique_ids(
        "navigation surface",
        module.navigation.surfaces.iter().map(|surface| &surface.id),
    )?;
    unique_ids(
        "navigation link",
        module.navigation.links.iter().map(|link| &link.id),
    )?;
    for port in &module.ports {
        if port
            .clearance
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
            || !port.translation.iter().all(|value| value.is_finite())
            || !port.floor_height.is_finite()
        {
            return Err(ManifestError(format!(
                "module `{}` has invalid port `{}`",
                module.id, port.id
            )));
        }
    }
    for surface in &module.navigation.surfaces {
        if surface.polygon.len() < 3
            || !surface.floor_height.is_finite()
            || surface
                .polygon
                .iter()
                .flatten()
                .any(|value| !value.is_finite())
        {
            return Err(ManifestError(format!(
                "module `{}` has invalid navigation surface `{}`",
                module.id, surface.id
            )));
        }
    }
    for slot in &module.dressing_slots {
        if !asset_ids.contains(slot) {
            return Err(ManifestError(format!(
                "module `{}` references unknown dressing `{slot}`",
                module.id
            )));
        }
    }
    Ok(())
}

fn unique_ids<'a>(
    kind: &str,
    ids: impl Iterator<Item = &'a String>,
) -> Result<BTreeSet<String>, ManifestError> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if id.trim().is_empty() || !seen.insert(id.clone()) {
            return Err(ManifestError(format!(
                "invalid or duplicate {kind} id `{id}`"
            )));
        }
    }
    Ok(seen)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn hash_json(value: &impl Serialize) -> [u8; 32] {
    Sha256::digest(serde_json::to_vec(value).expect("canonical content serializes")).into()
}

pub fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing into String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn architecture_register_ids_and_catalogue_order_are_pinned() {
        let ids: Vec<_> = ArchitectureRegister::ALL
            .into_iter()
            .map(ArchitectureRegister::stable_id)
            .collect();
        assert_eq!(ids, (0..9).collect::<Vec<_>>());
        assert_eq!(ARCHITECTURE_CATALOG_VERSION, 2);
        for register in ArchitectureRegister::ALL {
            assert_eq!(
                ArchitectureRegister::try_from(register.stable_id()),
                Ok(register)
            );
            assert!(!register.slug().is_empty());
            assert!(!register.label().is_empty());
        }
        assert_eq!(ArchitectureRegister::try_from(9), Err(9));
    }

    fn fixture() -> ContentManifest {
        ContentManifest {
            schema_version: 1,
            content_version: "test-v1".into(),
            traversal: TraversalProfile::deliberate_rapier(),
            districts: vec![DistrictDefinition {
                id: "archive".into(),
                label: "Archive".into(),
                style_district: "archive".into(),
                fog_end_scale: 1.0,
                practical_intensity_scale: 1.0,
                allowed_architecture_tags: vec!["megastructure".into()],
                dressing_slots: vec!["cables".into()],
            }],
            modules: vec![ArchitectureModule {
                id: "corridor.gantry.01".into(),
                space_kind: SpaceKind::Corridor,
                architecture_kind: "gantry".into(),
                role_tags: vec!["vertical".into()],
                district_tags: vec!["archive".into()],
                source_map: "maps/modules.map".into(),
                source_sha256: "1".repeat(64),
                baked_path: "content/modules/gantry.json".into(),
                baked_sha256: "2".repeat(64),
                allowed_rotations_degrees: vec![180, 0],
                wfc_weight: 4,
                bounds: Bounds3 {
                    min: [-4.0, 0.0, -10.0],
                    max: [4.0, 5.0, 10.0],
                },
                ports: vec![ModulePort {
                    id: "ground_in".into(),
                    kind: PortKind::Passage,
                    translation: [0.0, 0.0, 10.0],
                    yaw_degrees: 0,
                    clearance: [4.5, 3.0, 1.0],
                    floor_height: 0.0,
                }],
                sockets: vec![
                    ModuleSocket {
                        id: "control".into(),
                        kind: SocketKind::Equipment,
                        translation: [0.0, 1.0, 0.0],
                        yaw_degrees: 0,
                    },
                    ModuleSocket {
                        id: "decor".into(),
                        kind: SocketKind::Dressing,
                        translation: [1.0, 0.0, 0.0],
                        yaw_degrees: 0,
                    },
                ],
                navigation: NavigationSpec {
                    surfaces: vec![NavigationSurface {
                        id: "floor".into(),
                        floor_height: 0.0,
                        polygon: vec![[-3.0, -9.0], [3.0, -9.0], [3.0, 9.0], [-3.0, 9.0]],
                    }],
                    links: Vec::new(),
                },
                visual_scene: Some("visuals/gantry.glb".into()),
                dressing_slots: vec!["cables".into()],
            }],
            assets: vec![AssetDefinition {
                id: "cables".into(),
                path: "models/cables.glb".into(),
                sha256: "3".repeat(64),
                author: "Kenney".into(),
                source_url: "https://opengameart.org/content/modular-space-kit".into(),
                license: "CC0-1.0".into(),
                scale: 1.0,
                fallback: "cable_bundle".into(),
                allowed_districts: vec!["archive".into()],
            }],
        }
    }

    #[test]
    fn valid_fixture_has_stable_separate_hashes() {
        let fixture = fixture();
        fixture.validate().unwrap();
        assert_eq!(fixture.simulation_hash(), fixture.simulation_hash());
        assert_eq!(fixture.presentation_hash(), fixture.presentation_hash());
        assert_ne!(fixture.simulation_hash().0, fixture.presentation_hash().0);
    }

    #[test]
    fn record_order_does_not_change_hashes() {
        let original = fixture();
        let mut reordered = original.clone();
        reordered.modules[0].allowed_rotations_degrees.reverse();
        reordered.modules[0].sockets.reverse();
        assert_eq!(original.simulation_hash(), reordered.simulation_hash());
        assert_eq!(original.presentation_hash(), reordered.presentation_hash());
    }

    #[test]
    fn presentation_asset_change_does_not_change_simulation_hash() {
        let original = fixture();
        let mut changed = original.clone();
        changed.assets[0].sha256 = "f".repeat(64);
        assert_eq!(original.simulation_hash(), changed.simulation_hash());
        assert_ne!(original.presentation_hash(), changed.presentation_hash());
    }

    #[test]
    fn collision_or_movement_change_updates_simulation_hash() {
        let original = fixture();
        let mut changed = original.clone();
        changed.modules[0].baked_sha256 = "e".repeat(64);
        assert_ne!(original.simulation_hash(), changed.simulation_hash());
        let mut changed = original.clone();
        changed.traversal.run_speed += 0.1;
        assert_ne!(original.simulation_hash(), changed.simulation_hash());
    }

    #[test]
    fn place_layout_requires_known_modules_ports_and_allowed_rotation() {
        let manifest = fixture();
        let mut layout = PlaceLayoutSnapshot {
            generation: 4,
            placements: vec![ModulePlacement {
                module_id: "corridor.gantry.01".into(),
                translation: [0.0, 0.0, 0.0],
                yaw_degrees: 0,
                scale: [1.0, 1.0, 1.0],
                entry_port: "ground_in".into(),
                exit_port: "ground_in".into(),
            }],
        };
        layout.validate(&manifest).unwrap();
        layout.placements[0].yaw_degrees = 90;
        assert!(layout.validate(&manifest).is_err());
        layout.placements[0].yaw_degrees = 0;
        layout.placements[0].exit_port = "missing".into();
        assert!(layout.validate(&manifest).is_err());
    }

    #[test]
    fn schema_and_references_fail_closed() {
        let mut invalid = fixture();
        invalid.schema_version = 99;
        assert!(invalid.validate().is_err());
        let mut invalid = fixture();
        invalid.modules[0].dressing_slots = vec!["missing".into()];
        assert!(invalid.validate().is_err());
        let source = serde_json::to_string(&fixture()).unwrap();
        let source = source.replacen("{", "{\"unknown\":true,", 1);
        assert!(ContentManifest::parse(&source).is_err());
    }

    #[test]
    fn baked_module_output_is_canonical() {
        let mut module = BakedModule {
            schema_version: 1,
            module_id: "gantry".into(),
            hulls: vec![
                ConvexHull {
                    id: 2,
                    points: vec![
                        [1.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [0.0, 0.0, 1.0],
                        [0.0, 0.0, 0.0],
                    ],
                },
                ConvexHull {
                    id: 1,
                    points: vec![
                        [2.0, 0.0, 0.0],
                        [0.0, 2.0, 0.0],
                        [0.0, 0.0, 2.0],
                        [0.0, 0.0, 0.0],
                    ],
                },
            ],
        };
        module.validate().unwrap();
        let a = module.canonical_json();
        module.hulls.reverse();
        for hull in &mut module.hulls {
            hull.points.reverse();
        }
        assert_eq!(a, module.canonical_json());
    }

    #[test]
    fn committed_production_manifest_and_bakes_match_their_hashes() {
        let manifest = ContentManifest::parse(include_str!(
            "../../../assets/content/content_manifest.json"
        ))
        .unwrap();
        manifest.validate().unwrap();
        assert_eq!(manifest.districts.len(), 6);
        assert_eq!(manifest.modules.len(), 2);

        let fixtures = [
            (
                "corridor.gantry.01",
                include_bytes!("../../../assets/content/modules/gantry.json").as_slice(),
            ),
            (
                "corridor.colonnade.01",
                include_bytes!("../../../assets/content/modules/colonnade.json").as_slice(),
            ),
        ];
        for (id, bytes) in fixtures {
            let definition = manifest
                .modules
                .iter()
                .find(|module| module.id == id)
                .unwrap();
            assert_eq!(definition.baked_sha256, sha256(bytes));
            let baked: BakedModule = serde_json::from_slice(bytes).unwrap();
            baked.validate().unwrap();
        }
    }
}
