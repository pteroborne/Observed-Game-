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
pub mod catalog;
pub mod manifest;
pub mod source;
#[cfg(test)]
mod tests;
pub mod tile;
pub mod tile_source;

/// TrenchBroom units per world meter. Integer so every canonical hex corner
/// lands on an integer editor coordinate (7 m -> 112 units, 8 m -> 128).
pub const UNITS_PER_METER: f64 = 16.0;

pub use catalog::{
    CatalogAudit, CatalogBuild, CatalogError, CompiledModule, CompiledTileCatalog, RoomPrototype,
    RoomPrototypePort, RuntimeAuthoringCatalog, build_catalog, discover_sources,
    new_module_template, write_catalog_build,
};
pub use manifest::{Manifest, ManifestEntry, ManifestError, TileKey};
pub use source::{
    AuthoredModule, FloorPolicy, ModuleCell, ModuleCellRef, ModuleKind, ModulePort, ModuleSummary,
    RotationPolicy, SourceError, parse_authored_module, validate_module,
};
pub use tile::{TileError, TilePrototype, load_tile, parse_tile};
