//! TrenchBroom-authored convex geometry traversed by Rapier's kinematic capsule.
//!
//! This lab is deliberately independent from the earlier importer and determinism
//! spikes. TrenchBroom data projects into stable domain IDs and convex hulls; raw
//! Rapier owns collision queries and fixed-step character movement; Bevy only
//! samples [`player_input::PlayerIntent`] and presents the resulting state.

mod app;
pub mod import;
pub mod map_source;
pub mod physics;

pub use app::run;
