//! TrenchBroom tile authoring pipeline for the hex facility (Arc L).
//!
//! `.map` text → [`brush`] convex vertex math → [`tile`] schema projection and
//! exact-snap footprint validation against the [`observed_hex`] quantized
//! hexagon → [`manifest`] catalog keyed `TileKey { archetype, register,
//! variant }`. [`tile_source`] is the typed generator for the locked authoring
//! template and the seed tiles; committed `.map` files are pinned to it by
//! test so the editable assets never drift from source.
//!
//! Space conventions: TrenchBroom is Z-up in integer units at
//! [`UNITS_PER_METER`]; world is Y-up meters. `world = (x/S, z/S, -y/S)`.
//! A tile's local origin is its cell center; level 0's floor is world `y = 0`.

pub mod brush;
pub mod manifest;
#[cfg(test)]
mod tests;
pub mod tile;
pub mod tile_source;

/// TrenchBroom units per world meter. Integer so every canonical hex corner
/// lands on an integer editor coordinate (7 m -> 112 units, 8 m -> 128).
pub const UNITS_PER_METER: f64 = 16.0;

pub use manifest::{Manifest, ManifestEntry, ManifestError, TileKey};
pub use tile::{TileError, TilePrototype, load_tile, parse_tile};
