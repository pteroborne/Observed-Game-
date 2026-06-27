//! The observe/decohere model was promoted into the production crate
//! [`observed_observation`] (refactor R5). It is re-exported here so this lab — and the
//! other labs that reuse it via `observation_lab::model::*` — keep the same paths while
//! the rules and their tests live in the crate. This lab is the debug projection.

pub use observed_observation::*;
