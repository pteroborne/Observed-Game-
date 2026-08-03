//! `module_studio`: watch authored `.map` sources and show what is wrong with
//! them, on the geometry that is wrong.
//!
//! The sibling tool in this crate authors *composition* - what the solver tends
//! to build out of the modules. This one authors the modules themselves, and
//! the two share chrome, camera framing, and the style crate while keeping
//! separate state. They are a second binary rather than a second crate for
//! exactly that reason: everything below the panel is common, and nothing above
//! it is.

pub mod app;
pub mod diagnose;
pub mod panel;
pub mod probe;
pub mod render;
pub mod route;
pub mod walk;
pub mod watch;
