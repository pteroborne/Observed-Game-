//! Always-open remote threshold preview over isolated Rapier Places.
//!
//! The lab ratchets the corrected observation rule: local player observation and
//! an anchor both freeze BLAME, but only the anchor changes the frame light.

mod app;
pub mod authoring;
pub mod model;
pub mod physics;

pub use app::run;
