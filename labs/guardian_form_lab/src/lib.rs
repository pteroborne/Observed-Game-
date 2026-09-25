//! Guardian Form Lab — what a major Guardian could look like.
//!
//! Three candidate forms for the major Guardian, each acting out the four things a
//! major does in a match: hunting, frozen because someone sees it, frozen by an anchor
//! lantern, and the catch. They stand beside the existing minor Guardian, a 1.8 m
//! person and the facility's real doorway, so size and silhouette can be judged
//! together. The forms that are not chosen are candidates for new kinds of minor.
//!
//! The pure core (`form`, `roll`) has no rendering in it and is what the tests
//! exercise; `view` draws what it says, and `capture` writes the evidence.
pub mod capture;
pub mod form;
pub mod mesh;
pub mod roll;
pub mod view;
