//! The **competitive match brain** as a production model: the full team race over the
//! observe/decohere facility, the director (AI adversary), and the spatial hybrid
//! match — all dimension-agnostic pure logic.
//!
//! - [`competition`] — multiple teams racing to capacity-limited exits with indirect
//!   interference via a shared control (the team race).
//! - [`director`] — the collapse that absorbs fall-behind teams into the facility AI;
//!   absorbed teams escalate it (director pressure).
//! - [`mutable`] — the spine objective over the observe/decohere + constraint graph.
//! - [`maze`] — the deterministic seeded maze that embeds the room graph as real
//!   walkable corridors.
//! - [`facility`] — the competitive facility: progress is spine position, a full match
//!   resolves deterministically (fastest escapes, the rest are absorbed).
//! - [`hybrid`] — the complete first-person hybrid match: fixed-step traversal in the
//!   rerouting maze, round boundaries, safe/risky routes, and exact replay.
//! - [`teamplay`] — deterministic two-member bot teams solving procedural co-op
//!   room beats for the spectated elimination-series slice.
//!
//! Promoted out of `competition_lab`, `director_lab`, `mutable_facility`,
//! `competitive_facility`, `fps_maze_lab`, and `fps_hybrid_match_lab` in refactor R9;
//! those labs are the debug projections. Built on the promoted `observed_observation`,
//! `observed_facility`, and `observed_traversal` crates. The optional, default-on `bevy`
//! feature derives `Resource` on the world types for the labs/game.

pub mod competition;
pub mod director;
pub mod elimination;
pub mod facility;
pub mod full_wfc;
pub mod hex_wfc;
pub mod hybrid;
pub mod maze;
pub mod mutable;
pub mod teamplay;
