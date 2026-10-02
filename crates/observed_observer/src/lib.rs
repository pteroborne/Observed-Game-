//! The Observer's form: a floating eye, as parts, poses and meshes, with no rendering
//! and no simulation.
//!
//! The form is a list of parts (a shape and a look), and a pose places every part for
//! where the body is looking and a clock. The game draws every Observer but the one
//! you are from it; `observer_form_lab` stages it for stills. Colours are
//! `observed_style::observer`'s.
pub mod form;
pub mod mesh;
