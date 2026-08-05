//! Match-layer physical projection for the Arc L hex facility.
//!
//! The geometry snapshot is pure data shared by rendering and the deterministic
//! Rapier controller. The square [`crate::full_wfc`] projection remains a
//! separate regression fixture until the Arc L integration cutover.

mod content;
mod geometry;
mod model;
pub mod trim;

pub use content::HexMatchContent;
pub use geometry::{
    HexGeometryDelta, HexGeometryError, HexLightSource, HexRoomSocket, HexStructurePiece,
    HexStructureRole, HexWfcGeometrySnapshot,
};
pub use model::{
    DUAL_STATION_HOLD_TICKS, HEX_INPUT_VERSION, HexActionButtons, HexDeployedLantern, HexDoorState,
    HexGuardianState, HexGuardianStatus, HexInputFrame, HexLanternCache, HexLanternState,
    HexMapCellKnowledge, HexMapCellSnapshot, HexMapDiscovery, HexMatchConfig, HexMatchError,
    HexMatchEvent, HexMatchEventKind, HexMatchSnapshot, HexMatchStatus, HexPlayerCommand,
    HexPlayerMapKnowledge, HexPlayerSnapshot, HexPlayerState, HexTeamObjectiveState,
    HexTeamSnapshot, HexTeamState, HexWfcMatch, KEYSTONES_REQUIRED, MAX_ROSTER,
};
pub use trim::{HexTrimKind, HexTrimPiece, derive_trim, derive_trim_for};

/// Test corpus that preserves the legacy hall fixtures while supplying the
/// authored towers that atomically replaced their generated counterparts.
///
/// The production loader already combines both catalogs. Most match tests keep
/// the compatibility halls to avoid reshuffling unrelated geometry assertions,
/// but those cells no longer include any `stair_tower`, so the strict tower
/// family must come from the committed catalog.
#[cfg(test)]
fn test_catalog() -> &'static observed_authoring::RuntimeHexCatalog {
    static CATALOG: std::sync::OnceLock<observed_authoring::RuntimeHexCatalog> =
        std::sync::OnceLock::new();
    CATALOG.get_or_init(|| {
        let cwd_relative = std::path::PathBuf::from("assets/tiles");
        let base = if cwd_relative.exists() {
            cwd_relative
        } else {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles")
        };
        let registers = observed_content::ArchitectureRegister::ALL
            .map(observed_content::ArchitectureRegister::slug);
        observed_authoring::RuntimeHexCatalog::load(&base, &registers)
            .expect("committed runtime hex catalog loads")
    })
}

#[cfg(test)]
fn compatibility_test_content() -> &'static std::sync::Arc<HexMatchContent> {
    static CONTENT: std::sync::OnceLock<std::sync::Arc<HexMatchContent>> =
        std::sync::OnceLock::new();
    CONTENT.get_or_init(|| {
        let mut cells = observed_authoring::tile_source::compatibility_cells()
            .expect("compatibility tiles generate");
        cells.extend(
            test_catalog()
                .cells
                .iter()
                .filter(|tile| tile.key.archetype == "stair_tower")
                .cloned(),
        );
        std::sync::Arc::new(HexMatchContent::from_runtime_catalog(
            observed_authoring::RuntimeHexCatalog {
                cells,
                rooms: test_catalog().rooms.clone(),
                composition: observed_facility::hex_wfc::HexCompositionProfile::baseline(),
                simulation_content_hash: [0; 32],
            },
        ))
    })
}

#[cfg(test)]
fn test_tiles() -> Vec<observed_authoring::TilePrototype> {
    compatibility_test_content().cells().to_vec()
}
