//! Which floors the viewport draws.
//!
//! The type itself lives in `observed_style::iso`, shared with the game's
//! spectator overview: "which floor am I reading, and what context comes with
//! it" is visual language, and two copies of it would drift.

pub use observed_style::iso::Layer;
