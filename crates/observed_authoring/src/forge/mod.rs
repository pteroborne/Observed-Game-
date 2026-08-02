//! The tile forge: parametric authoring of `.map` sources, in Rust.
//!
//! A byte-exact port of `tools/tileforge.py`, which the repository has been
//! carrying alongside `tile_source/geometry.rs` as a second implementation of
//! the same brush math. See [`geometry`] for why that mattered and how the port
//! is gated.

pub mod entities;
pub mod geometry;
pub mod halls;
pub mod probe;
