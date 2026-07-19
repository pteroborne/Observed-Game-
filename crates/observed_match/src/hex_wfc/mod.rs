//! Match-layer physical projection for the Arc L hex facility.
//!
//! The geometry snapshot is pure data shared by rendering and the deterministic
//! Rapier controller. The square [`crate::full_wfc`] projection remains a
//! separate regression fixture until the Arc L integration cutover.

mod geometry;
mod model;

pub use geometry::{
    HexGeometryDelta, HexGeometryError, HexStructurePiece, HexStructureRole, HexWfcGeometrySnapshot,
};
pub use model::{
    HEX_INPUT_VERSION, HexActionButtons, HexDeployedLantern, HexDoorState, HexGuardianState,
    HexGuardianStatus, HexInputFrame, HexLanternCache, HexLanternState, HexMapCellKnowledge,
    HexMapDiscovery, HexMatchConfig, HexMatchError, HexMatchEvent, HexMatchEventKind,
    HexMatchSnapshot, HexMatchStatus, HexPlayerCommand, HexPlayerMapKnowledge, HexPlayerSnapshot,
    HexPlayerState, HexWfcMatch,
};
