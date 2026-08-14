//! Immutable authored content used by one canonical hex match.
//!
//! Filesystem discovery remains a host concern. Once loaded, the selected cell
//! and room prototypes, authored composition profile, network identity, and
//! production traversal configuration travel together so a match cannot pair
//! independently assembled pieces of simulation content.

use std::path::Path;

use observed_authoring::{RoomPrototype, RuntimeHexCatalog, TilePrototype};
use observed_facility::hex_wfc::profile::HexCompositionProfile;
use observed_traversal::{FpsConfig, TraversalRuntimeProfile};

#[derive(Clone, Debug)]
pub struct HexMatchContent {
    catalog: RuntimeHexCatalog,
    traversal_profile: TraversalRuntimeProfile,
}

impl HexMatchContent {
    /// Load one validated runtime catalog and attach the exact controller
    /// configuration used by canonical hex matches.
    pub fn load(base: &Path, register_slugs: &[&str]) -> Result<Self, String> {
        RuntimeHexCatalog::load(base, register_slugs).map(Self::from_runtime_catalog)
    }

    /// Parse the canonical authored artifacts compiled into a browser client.
    pub fn from_embedded(
        compiled_catalog: &str,
        compiled_catalog_hash: &str,
        composition_profile: &str,
        composition_profile_hash: &str,
        register_slugs: &[&str],
    ) -> Result<Self, String> {
        RuntimeHexCatalog::from_embedded(
            compiled_catalog,
            compiled_catalog_hash,
            composition_profile,
            composition_profile_hash,
            register_slugs,
        )
        .map(Self::from_runtime_catalog)
    }

    /// Consume an already-loaded catalog without cloning its authored corpus.
    #[must_use]
    pub fn from_runtime_catalog(catalog: RuntimeHexCatalog) -> Self {
        Self {
            catalog,
            traversal_profile: TraversalRuntimeProfile::canonical_hex(),
        }
    }

    /// Compatibility value for the legacy slice-based test constructors.
    pub(super) fn compatibility(
        cells: &[TilePrototype],
        rooms: &[RoomPrototype],
        composition: &HexCompositionProfile,
    ) -> Self {
        Self::from_runtime_catalog(RuntimeHexCatalog {
            cells: cells.to_vec(),
            rooms: rooms.to_vec(),
            composition: composition.clone(),
            simulation_content_hash: [0; 32],
        })
    }

    #[must_use]
    pub fn cells(&self) -> &[TilePrototype] {
        &self.catalog.cells
    }

    #[must_use]
    pub fn rooms(&self) -> &[RoomPrototype] {
        &self.catalog.rooms
    }

    #[must_use]
    pub fn composition(&self) -> &HexCompositionProfile {
        &self.catalog.composition
    }

    #[must_use]
    pub fn traversal_config(&self) -> FpsConfig {
        self.traversal_profile.controller()
    }

    #[must_use]
    pub fn traversal_profile(&self) -> &TraversalRuntimeProfile {
        &self.traversal_profile
    }

    #[must_use]
    pub fn simulation_content_hash(&self) -> [u8; 32] {
        self.catalog.simulation_content_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_content_preserves_the_shipped_hex_controller_profile() {
        let content = HexMatchContent::compatibility(&[], &[], &HexCompositionProfile::baseline());
        let mut expected = FpsConfig::deliberate_rapier();
        expected.look_step = 1.0;

        assert_eq!(content.traversal_config(), expected);
        assert_eq!(
            content.traversal_profile(),
            &TraversalRuntimeProfile::canonical_hex()
        );
        assert_eq!(content.simulation_content_hash(), [0; 32]);
    }
}
