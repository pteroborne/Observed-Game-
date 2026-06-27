//! **Interaction & equipment** as production models.
//!
//! - [`interaction`] — the logical interaction state machine: activate/operate, exclusive
//!   and shared (co-op) holds with quorum and interruption, carry/drop/socket/recover,
//!   and climb, resolved by a pure deterministic tick.
//! - [`equipment`] — persistent equipment whose state (carried / deployed / socketed /
//!   on the ground / powered) is independent of any render entity, so it survives a
//!   carrier leaving, a room being replaced, and its visual being despawned.
//!
//! Promoted out of `interaction_lab` and `equipment_lab` in refactor R8; those labs are
//! the debug projections. Depends only on `glam` for vector math and `observed_core`
//! for stable IDs. The optional, default-on `bevy` feature is the adapter: it derives
//! `Resource` on the two world types so the labs can insert them directly.

pub mod equipment;
pub mod interaction;
