//! WFC feasibility lab — library surface.
//!
//! The runnable showcase is the binary (`src/main.rs`). This lib target archives
//! WFC code that must keep compiling but has no live consumer:
//! [`hallway_wfc`] is the game's former hallway-interior WFC generator, parked here
//! (with `ghx_proc_gen`) in case WFC is adopted for map generation later.

pub mod hallway_wfc;
