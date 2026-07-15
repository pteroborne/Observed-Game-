//! Deterministic, multiplayer-shaped gameplay over the continuous full-WFC facility.
//!
//! The assembled game is an input and presentation adapter. This module owns every
//! rule that must later agree between a dedicated host, clients, bots, and replay.

mod bot;
mod equipment;
mod geometry;
mod guardian;
mod knowledge;
mod model;

pub use equipment::{Deployable, DeployableKind, EquipmentState, TeamLoadout};
pub use geometry::{
    CELL_SIZE, FLOOR_THICKNESS, FullWfcGeometrySnapshot, LEVEL_HEIGHT, SHAFT_OPENING,
    StructurePiece, StructureRole, THRESHOLD_CLEAR_HEIGHT, THRESHOLD_WIDTH, WALL_HEIGHT,
    WALL_THICKNESS, cell_origin,
};
pub use guardian::{GuardianState, GuardianStatus};
pub use knowledge::{MapCellKnowledge, MapDiscovery, TeamMapKnowledge};
pub use model::{
    ActionButtons, FULL_WFC_INPUT_VERSION, FullWfcMatch, FullWfcMatchConfig, GameplayEvent,
    GameplayEventKind, InputFrame, MatchError, MatchSnapshot, MatchStatus, PlayerCommand,
    PlayerSnapshot, PlayerState, TeamState,
};
