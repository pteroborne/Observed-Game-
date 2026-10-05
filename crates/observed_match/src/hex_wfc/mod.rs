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
pub use geometry::open_edge::{OpenEdges, RAILED_BELOW_LEVEL, open_edges};
pub use geometry::{
    HexGeometryDelta, HexGeometryError, HexLightSource, HexModuleInstanceId, HexModuleRevision,
    HexPiecePart, HexRoomSocket, HexStructurePiece, HexStructureRole, HexTileCatalogue,
    HexTileSupply, HexTraversalCursor, HexTraversalLease, HexWfcGeometrySnapshot, ProjectedPort,
    ProjectedTraversalGraph, ProjectedTraversalGuide, project_hypothetical_cell,
    project_hypothetical_cells,
};
#[cfg(test)]
pub(crate) use model::MAX_MUTATION_TICKS;
pub use model::prison::{
    HexPrison, HexPrisonMaze, LOBBY_HOLD_TICKS, MAZE_COLS, MAZE_REGISTER, MAZE_ROWS,
};
pub use model::{
    DOOR_HALF_WIDTH, DOOR_HEIGHT, DOOR_REACH, DUAL_STATION_HOLD_TICKS, HEX_INPUT_VERSION,
    HexActionButtons, HexAnchorSite, HexBodyPlace, HexBotDriver, HexDeployedLantern,
    HexDeployedPad, HexDirectedError, HexDoor, HexDoorState, HexGuardianState, HexGuardianStatus,
    HexInputFrame, HexInteraction, HexInteractionAction, HexKillingPush, HexKineticTarget,
    HexKineticVerb, HexLanternCache, HexLanternState, HexMapCellKnowledge, HexMapCellSnapshot,
    HexMapDiscovery, HexMatchConfig, HexMatchError, HexMatchEvent, HexMatchEventKind,
    HexMatchSnapshot, HexMatchStatus, HexMinorState, HexPadState, HexPlayerCommand,
    HexPlayerMapKnowledge, HexPlayerSnapshot, HexPlayerState, HexPlumbAim, HexReleasedGuardian,
    HexReleasedKind, HexSight, HexTeamObjectiveState, HexTeamSnapshot, HexTeamState, HexWfcMatch,
    KEYSTONES_REQUIRED, KINETIC_COOLDOWN_TICKS, KINETIC_PULL_SPEED, KINETIC_PUSH_SPEED,
    KINETIC_REACH, KINETIC_STAGGER_FRICTION, KINETIC_STAGGER_TICKS, MAX_ROSTER,
    MINOR_BREAKING_DROP, MINOR_SIGHT_STEPS, PAD_CONTACT_RADIUS, PAD_REARM_TICKS, PADS_PER_PLAYER,
    PLUMB_COOLDOWN_TICKS, PLUMB_TICKS, SENSOR_HANG, SENSOR_REACH, SIGHT_REACH, SIGHT_REFRESH_TICKS,
    door_pose,
};
pub use trim::{HexTrimKind, HexTrimPiece, derive_thresholds, derive_trim, derive_trim_for};

/// Test corpus that preserves the legacy hall fixtures while supplying the
/// authored climb compositions, which the generated library has no counterpart for.
///
/// The production loader already combines both catalogs. Most match tests keep
/// the compatibility halls to avoid reshuffling unrelated geometry assertions, so
/// the climbs must come from the committed catalog.
#[cfg(test)]
pub(crate) fn test_catalog() -> &'static observed_authoring::RuntimeHexCatalog {
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
pub(crate) fn compatibility_test_content() -> &'static std::sync::Arc<HexMatchContent> {
    static CONTENT: std::sync::OnceLock<std::sync::Arc<HexMatchContent>> =
        std::sync::OnceLock::new();
    CONTENT.get_or_init(|| {
        let mut cells = observed_authoring::tile_source::compatibility_cells()
            .expect("compatibility tiles generate");
        cells.extend(
            test_catalog()
                .cells
                .iter()
                // The authored climbs: the generated library has none.
                .filter(|tile| {
                    tile.key.archetype.starts_with("climb_")
                        || tile.key.archetype == "cistern"
                        || tile.key.archetype == "archive_well"
                        || tile.key.archetype == "rain_court"
                        || tile.key.archetype == "jade_nave"
                        || tile.key.archetype == "switching_concourse"
                        || tile.key.archetype.starts_with("chargeworks_")
                })
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
