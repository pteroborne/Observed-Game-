//! WFC catalogue lab library surface.
//!
//! [`map_topology`] renders `generate_liminal_map_v2` as the default proof, including
//! architecture-register regions and traversal archetypes. The v1 topology and the
//! old abstract tile-WFC view remain explicit regression paths. [`hallway_wfc`] is a
//! thin compatibility wrapper around the production corridor-interior generator.

pub mod hallway_wfc;
pub mod map_topology;

#[cfg(test)]
mod corpus;
