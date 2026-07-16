//! The hex-prism grid vocabulary for the Arc L tile facility.
//!
//! One crate owns every number and relation the hex world is built from: axial
//! coordinates on a rhombic-bounded lattice ([`coords`]), the eight-face
//! adjacency model ([`faces`]), the port-class vocabulary that replaces the
//! square lattice's boolean opening mask ([`ports`]), and the quantized-hexagon
//! physical metrics that let TrenchBroom brushes snap exactly ([`metrics`]).
//!
//! The solver (`observed_facility::hex_wfc`), the geometry projector
//! (`observed_match::hex_wfc`), and the tile importer (`observed_authoring`)
//! all consume these definitions rather than carrying local copies — grid and
//! authored assets cannot disagree because they share one source of truth.

pub mod coords;
pub mod faces;
pub mod metrics;
pub mod ports;

pub use coords::{HexCoord, HexGridSize, lateral_distance, travel_distance};
pub use faces::HexFace;
pub use metrics::{
    ACROSS_CORNERS, ACROSS_FLATS, CORNERS, TILE_LEVEL_HEIGHT, face_edge, hex_origin,
    hex_origin_plan,
};
pub use ports::{PortClass, PortSignature, ports_compatible};
