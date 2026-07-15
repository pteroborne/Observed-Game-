//! **Facility topology** as a production model: authored modular rooms with typed,
//! validated ports; the world that spawns/attaches/rotates/replaces them with exact
//! port alignment; and the graph constraint (a protected route spine) that keeps the
//! observe/decohere structure traversable.
//!
//! Promoted out of `room_lab` (rooms/ports/registry/world) and `constraint_lab` (the
//! spine constraint) in refactor R6. It depends only on `glam` for vector math,
//! `observed_core` for stable IDs, and `observed_observation` for the reused graph.
//! The optional, default-on `bevy` feature is the adapter: it derives `Resource` on
//! the registry/world/constraint resources and adds the `color()` presentation
//! helpers. `room_lab` and `constraint_lab` re-export this crate and are its debug
//! projections.
//!
//! - [`room_def`] — room templates, typed ports, surfaces, the authored registry, and
//!   the transform/world-port/collision helpers.
//! - [`room_world`] — the validated room world: spawn, attach (auto-aligned), rotate,
//!   replace, despawn, and collision generation.
//! - [`constraints`] — the persistent route spine over the observe/decohere graph.

pub mod constraints;
#[cfg(feature = "wfc")]
pub mod full_wfc;
pub mod junction;
pub mod map_spec;
pub mod room_def;
pub mod room_world;
#[cfg(feature = "wfc")]
pub mod wfc;

pub use junction::*;
pub use map_spec::*;
pub use room_def::*;
pub use room_world::*;
