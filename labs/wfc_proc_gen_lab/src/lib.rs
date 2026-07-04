//! WFC feasibility lab — library surface.
//!
//! The runnable showcase is the binary (`src/main.rs`). This lib target archives
//! WFC code that must keep compiling but has no live consumer:
//! [`hallway_wfc`] is the game's former hallway-interior WFC generator, parked here
//! (with `ghx_proc_gen`) in case WFC is adopted for map generation later.
//!
//! [`map_topology`] is Phase 45's live consumer: it visualizes
//! `observed_facility::wfc::generate_liminal_map` output in the showcase binary. The
//! `corpus` test module below is this phase's success criteria — a 50-seed corpus
//! proving generation determinism, room-count/role coverage, `MapSpec::validate()`,
//! and objective-sequence coherence against `observed_match::CompetitiveFacility`.

pub mod hallway_wfc;
pub mod map_topology;

#[cfg(test)]
mod corpus;
