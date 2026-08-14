//! Mesh construction for hex schematic views.
//!
//! A schematic is drawn as batched line lists and low solid bands, not as one
//! entity per edge. At production scale a single layer resolves to roughly
//! twelve thousand cells; as entities that is hopeless, and as three or four
//! merged meshes it is nothing. Everything here accumulates into per-colour
//! buffers that the caller turns into one mesh each.
//!
//! This crate has no opinion about colour. Callers ask `observed_style` what a
//! semantic state looks like and hand the answer to their own materials; the
//! geometry here is colour-blind on purpose, so two views drawing the same
//! lattice cannot disagree about its shape while agreeing about its palette.

mod batch;
mod glyph;
mod prism;

pub use batch::{LineBatch, SurfaceBatch};
pub use glyph::{floor_ring, level_arrow_glyph, ramp_glyph, stair_glyph, wall_bands};
pub use prism::hex_prism;
