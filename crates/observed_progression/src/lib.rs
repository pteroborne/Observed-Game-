//! **Progression & session** as production models, both orthogonal to the simulation.
//!
//! - [`progression`] — a `Profile` that accrues XP from match placements, levels up,
//!   unlocks cosmetics by level/win thresholds, equips them per slot, and serializes
//!   to a compact round-tripping string. By construction it never feeds the match.
//! - [`session`] — deterministic matchmaking and session formation: a compatible
//!   four-player roster from a queue, stable `PlayerId`/balanced `TeamId` assignment,
//!   the lobby state machine (readiness, countdown, host migration, reconnect, launch
//!   manifest, rematch, closure).
//!
//! Promoted out of `progression_lab` and `session_lab` in refactor R9; those labs are
//! the debug projections. Pure data (`observed_core` IDs only); the optional, default-on
//! `bevy` feature derives `Resource` on the world/profile types for the labs.

pub mod progression;
pub mod session;
