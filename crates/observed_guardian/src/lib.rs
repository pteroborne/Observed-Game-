//! The Guardians' forms: parts, poses and meshes, with no rendering and no simulation.
//!
//! A form is a list of parts (a shape and a look), and a pose places every part for a
//! state, the seconds spent in it, and a clock. The game draws the major Guardian from
//! it, driven by the simulation's Guardian status; `guardian_form_lab` draws every
//! form side by side. See `docs/guardian_forms.md`.
//!
//! The Tumbler is the major Guardian. The Plumb and the Roller are kept here for the
//! minor roster and other specials.
pub mod form;
pub mod mesh;
pub mod roll;
