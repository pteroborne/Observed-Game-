//! The deterministic first-person controller was promoted into the production crate
//! [`observed_traversal`] (refactor R6). It is re-exported here so this lab — and the
//! many labs/game that reuse it via `fps_controller_lab::controller::*` — keep the same
//! paths while the kernel and its tests live in the crate. This lab is the projection.

pub use observed_traversal::*;
