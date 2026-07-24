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
pub const UNITS_PER_METER: f64 = 16.0;

pub use cad_renderer::{DynamicHull, render_cad_blueprint, render_dynamic_cad_blueprint};
pub use catalog::{
    CatalogAudit, CatalogBuild, CatalogError, CompiledLight, CompiledModule, CompiledTileCatalog,
    DistrictAuditGroup, DistrictAuditResult, RoomPrototype, RoomPrototypePort,
    RuntimeAuthoringCatalog, audit_district_variations, build_catalog, discover_sources,
    load_runtime_cells, new_module_template, write_catalog_build,
};
pub use manifest::{Manifest, ManifestEntry, ManifestError, TileKey};
pub use source::{
    AuthoredModule, FloorPolicy, ModuleCell, ModuleCellRef, ModuleKind, ModulePort, ModuleSummary,
    RotationPolicy, SourceError, parse_authored_module, validate_module,
};
pub use tile::{TileError, TileLight, TileLightKind, TilePrototype, load_tile, parse_tile};

/// The exact authored corpus consumed by both interactive and headless hex matches.
/// Filesystem discovery stays outside the deterministic simulation, while this loader
/// prevents clients and dedicated servers from assembling subtly different catalogs.
#[derive(Clone, Debug)]
pub struct RuntimeHexCatalog {
    pub cells: Vec<TilePrototype>,
    pub rooms: Vec<RoomPrototype>,
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
        Ok(Self {
            cells,
            rooms: strict.rooms,
            simulation_content_hash: decode_sha256(sidecar.trim())?,
        })
    }
}

fn decode_sha256(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64 {
        return Err("compiled catalog hash is not 64 hexadecimal characters".to_string());
    }
    let mut hash = [0_u8; 32];
    for (index, byte) in hash.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| "compiled catalog hash contains non-hexadecimal text".to_string())?;
    }
    Ok(hash)
}
