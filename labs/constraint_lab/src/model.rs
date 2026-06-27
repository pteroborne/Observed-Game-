//! The mutable-graph-constraint model was promoted into the production crate
//! [`observed_facility`] (refactor R6). It is re-exported here so this lab — and the
//! labs that reuse it via `constraint_lab::model::ConstraintWorld` — keep the same
//! paths while the rules and their tests live in the crate. This lab is the projection.

pub use observed_facility::constraints::*;
