//! Guardian Form Lab — what a major Guardian could look like.
//!
//! Three candidate forms for the major Guardian, each acting out the four things a
//! major does in a match: hunting, frozen because someone sees it, frozen by an anchor
//! lantern, and the catch. They stand beside the existing minor Guardian, a 1.8 m
//! person and the facility's real doorway, so size and silhouette can be judged
//! together. The forms that are not chosen are candidates for new kinds of minor.
//!
//! The forms live in `observed_guardian`, shared with the game, which carries their
//! tests; `view` draws them here, and `capture` writes the evidence.
pub mod capture;
pub mod sound;
pub mod view;

pub use observed_guardian::{form, mesh, roll};
