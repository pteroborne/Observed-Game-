//! TrenchBroom tile authoring pipeline for the hex facility (Arc L).
//!
//! `.map` text → [`brush`] convex vertex math → [`tile`] schema projection and
//! exact-snap footprint validation against the [`observed_hex`] quantized
//! hexagon → [`manifest`] catalog keyed `TileKey { archetype, register,
//! variant }` compatibility catalog. Version-2 [`source`] metadata compiles
//! through [`catalog`], with `.map` files as canonical sources. [`tile_source`]
//! remains only as the Arc-L regression-fixture generator.
//!
//! Space conventions: TrenchBroom is Z-up in integer units at
//! [`UNITS_PER_METER`]; world is Y-up meters. `world = (x/S, z/S, -y/S)`.
//! A tile's local origin is its cell center; level 0's floor is world `y = 0`.

pub mod brush;
pub mod cad_renderer;
pub mod catalog;
pub mod certification;
pub mod compiler;
pub mod composition;
pub mod contract;
pub mod distribution;
pub mod fgd;
pub mod forge;
pub mod generator;
pub mod manifest;
pub mod seam_auditor;
pub mod source;
#[cfg(test)]
mod tests;
pub mod tile;
pub mod tile_source;

/// TrenchBroom units per world meter. Integer so every canonical hex corner
/// lands on an integer editor coordinate (7 m -> 112 units, 8 m -> 128).
pub const QUANTIZED_UNITS_PER_METER: i32 = 16;
pub const UNITS_PER_METER: f64 = QUANTIZED_UNITS_PER_METER as f64;

pub use cad_renderer::{DynamicHull, render_cad_blueprint, render_dynamic_cad_blueprint};
pub use catalog::{
    CONTRACT_CATALOG_VERSION, CatalogAudit, CatalogBuild, CatalogError, CompiledLight,
    CompiledModule, CompiledSocket, CompiledTileCatalog, DistrictAuditGroup, DistrictAuditResult,
    RoomPrototype, RoomPrototypePort, RoomPrototypeSocket, RuntimeAuthoringCatalog,
    audit_district_variations, build_catalog, discover_sources, load_runtime_cells,
    new_module_template, write_catalog_build,
};
pub use certification::{
    CertificationAuthority, CertificationFailure, CertificationFailureKind, CertificationReport,
    CertifiedInterface, CertifiedPortPair, CertifiedVerticalCase, CorpusCertificationReport,
    FailureTracePoint, TRAVERSAL_CERTIFICATE_VERSION, TraversalCertificate,
    TraversalCertificateHash, certify_catalog, certify_catalog_selection, certify_compiled_module,
    certify_runtime_prototype,
};
pub use compiler::{
    AssemblyFamilyIndex, FamilyDiagnostic, FamilyEntry, FamilyMember, compile_module_contract,
    compile_spatial_contract,
};
pub use composition::{
    COMPOSITION_PROFILE_FILE, COMPOSITION_PROFILE_SHA_FILE, CompositionBuild, CompositionError,
    fold_simulation_content_hash, load_profile, parse_profile, write_profile_build,
};
pub use contract::{
    AssemblyContract, AssemblyScope, AssemblyVariantId, ClearanceVolume, ContractDiagnostic,
    DiagnosticLocation, FACE_LOCAL_SCALE, FaceLocalBox, FaceLocalDirection, FaceLocalPoint,
    GeometryEnvelope, InterfaceFingerprint, InterfaceProfile, LateralFaceFrame, LogicalCell,
    LogicalFootprint, ModuleContract, ModuleFamilyId, ModuleInterface, ModuleSpatialContract,
    ModuleTraversalContract, QuantizedAperture, QuantizedBox, QuantizedDirection, QuantizedPoint,
    QuantizedPose, RuntimeAssembly, RuntimeModuleContract, VerticalFaceFrame,
};
pub use manifest::{Manifest, ManifestEntry, ManifestError, TileKey};
pub use source::{
    AuthoredModule, CONTRACT_AUTHORING_VERSION, FloorPolicy, ModuleCell, ModuleCellRef, ModuleKind,
    ModulePort, ModuleSocket, ModuleSummary, RoomSocketKind, RotationPolicy, SourceError,
    editor_origin_to_world, parse_authored_module, validate_module,
};
pub use tile::{
    DeckPath, STAIR_SPINE_MIN_SEPARATION, StairSpine, TileError, TileLight, TileLightKind,
    TilePrototype, load_tile, parse_tile,
};

/// The exact authored corpus consumed by both interactive and headless hex matches.
/// Filesystem discovery stays outside the deterministic simulation, while this loader
/// prevents clients and dedicated servers from assembling subtly different catalogs.
///
/// A facility is a pure function of `(seed, config, catalog, composition)`, so
/// [`Self::simulation_content_hash`] folds the compiled catalog's digest
/// together with the authored composition profile's. Everything downstream —
/// `observed_net`'s `Hello`, the dedicated server's join check, replay binding —
/// consumes only the `[u8; 32]`, so adding the profile to the fold needed no
/// protocol change at all.
#[derive(Clone, Debug)]
pub struct RuntimeHexCatalog {
    pub cells: Vec<TilePrototype>,
    pub rooms: Vec<RoomPrototype>,
    /// The authored composition the solver must run under. Loaded here rather
    /// than separately so a caller cannot pair a catalog with a profile that
    /// was not hashed with it.
    pub composition: observed_facility::hex_wfc::profile::HexCompositionProfile,
    pub simulation_content_hash: [u8; 32],
}

impl RuntimeHexCatalog {
    pub fn load(base: &std::path::Path, register_slugs: &[&str]) -> Result<Self, String> {
        let mut cells = tile_source::compatibility_cells()
            .map_err(|error| format!("compatibility cells: {error:?}"))?;
        cells.extend(
            Manifest::load(&base.join("manifest.ron"))
                .map_err(|error| format!("manifest: {error:?}"))?
                .load_tiles(base)
                .map_err(|error| format!("manifest tiles: {error:?}"))?,
        );
        let text = std::fs::read_to_string(base.join("compiled_catalog.ron"))
            .map_err(|error| format!("compiled catalog: {error}"))?;
        let compiled = CompiledTileCatalog::from_ron(&text)
            .map_err(|error| format!("compiled catalog schema: {error:?}"))?;
        let sidecar = std::fs::read_to_string(base.join("compiled_catalog.sha256"))
            .map_err(|error| format!("compiled catalog hash: {error}"))?;
        if sidecar.trim() != compiled.simulation_content_hash {
            return Err("compiled catalog and hash sidecar disagree".to_string());
        }
        let strict = compiled
            .runtime_catalog(register_slugs)
            .map_err(|error| format!("runtime catalog: {error:?}"))?;
        cells.extend(strict.cells);
        let composition =
            composition::load_profile(base).map_err(|error| format!("composition: {error}"))?;
        Ok(Self {
            cells,
            rooms: strict.rooms,
            simulation_content_hash: composition::fold_simulation_content_hash(
                sidecar.trim(),
                &composition.content_hash,
            ),
            composition: composition.profile,
        })
    }
}
